//! Comandos do domínio `report`: "Relatar problema" (UI-02 e UI-03).
//!
//! O relato vira uma issue do GitHub no repositório do Warden. Nada sai do computador sem um
//! clique: com o login do GitHub, **Enviar** cria a issue pela API, com os prints (UI-03);
//! sem login, **Abrir no GitHub** abre a issue nova já preenchida, e quem envia é a própria
//! pessoa, na página do GitHub (UI-02).
//!
//! - `problem_report_preview`: junta o que a pessoa escreveu (categoria, título, descrição e
//!   o nome dos prints) com o que o Warden coleta sozinho (versão e commit, sistema, tela, pack
//!   aberto, último erro mostrado e as últimas linhas dos registros), passa **tudo** pela
//!   redação (`warden_diagnostics::Redactor`, com os valores reais das chaves do cofre, os
//!   tokens do login inclusive) e devolve os dois corpos exatos (o do envio direto e o do
//!   endereço), o endereço e o que precisou ser cortado (`body.rs`).
//! - `problem_report_send`: monta de novo o relato (nunca usa texto vindo da interface sem
//!   redação) e envia pela API (`send.rs`), com o progresso no canal da operação.
//! - `problem_report_open`: abre no navegador padrão a issue nova que a pré-visualização
//!   mostrou ou a issue criada. Só aceita essas páginas do repositório do Warden.
//! - Prints (`attachments.rs`): `problem_report_draft` abre e fecha o rascunho,
//!   `problem_report_attach` (colar), `problem_report_choose_images` (diálogo),
//!   `problem_report_detach` (Remover) e o evento `report-images-dropped` (arrastar).

mod attachments;
mod body;
mod send;

use std::fmt::Write as _;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::sync::Arc;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager as _, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;
use tauri_specta::Event as _;
use warden_core::PackId;
use warden_diagnostics::{RedactionProfile, Redactor};
use warden_github::{Endpoints, Label};
use warden_secrets::Secrets;

use crate::commands::app::{AppInfo, Platform, app_info};
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::events::OperationEvent;
use crate::operations::OperationKind;
use crate::state::AppState;

pub(crate) use attachments::{ReportDraft, ReportImage, ReportImagesAdded};
use body::{Cut, MAX_URL_CHARS, PrintRef, REPORT_LABEL, ReportParts, fit, fit_direct, new_issue_url};
pub(crate) use send::ProblemReportSent;

/// Operação do envio direto (painel Tarefas).
const SEND: OperationKind = OperationKind::new("report.send");

/// Em build de debug, o E2E troca o diálogo de imagens pelos caminhos deste arquivo (um por
/// linha), como em "Adicionar arquivo local".
#[cfg(debug_assertions)]
const E2E_PICK_FILES_ENV: &str = "WARDEN_E2E_PICK_FILES";

/// Linhas dos registros lidas (as mais novas).
const LOG_LINES: usize = 200;

/// Bytes lidos do fim de cada arquivo de registro (bastam para [`LOG_LINES`] linhas).
const LOG_TAIL_BYTES: u64 = 256 * 1024;

/// Em build de debug, o E2E troca o navegador pelo arquivo apontado por esta variável: o
/// endereço é gravado nele em vez de abrir.
#[cfg(debug_assertions)]
const E2E_OPEN_URL_ENV: &str = "WARDEN_E2E_OPEN_URL";

/// Último erro que a interface mostrou.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShownError {
    /// `domínio.CÓDIGO`, quando o erro mostrado veio de um `AppError`.
    pub(crate) code: Option<String>,
    /// A frase mostrada.
    pub(crate) message: String,
}

/// Categoria do relato (UI-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ReportCategory {
    /// Erro.
    Error,
    /// Sugestão.
    Suggestion,
    /// Visual ou texto.
    VisualOrText,
    /// Lentidão.
    Slowness,
    /// Outro.
    Other,
}

impl ReportCategory {
    /// Nome como a interface escreve.
    const fn name(self) -> &'static str {
        match self {
            Self::Error => "Erro",
            Self::Suggestion => "Sugestão",
            Self::VisualOrText => "Visual ou texto",
            Self::Slowness => "Lentidão",
            Self::Other => "Outro",
        }
    }

    /// Cor da label no GitHub.
    const fn color(self) -> &'static str {
        match self {
            Self::Error => "d73a4a",
            Self::Suggestion => "a2eeef",
            Self::VisualOrText => "fbca04",
            Self::Slowness => "f9d0c4",
            Self::Other => "c5def5",
        }
    }

    /// `categoria: erro`.
    fn label_name(self) -> String {
        format!("categoria: {}", self.name().to_lowercase())
    }

    fn label(self) -> Label {
        Label {
            name: self.label_name(),
            color: self.color().to_owned(),
            description: format!("Relato do Warden na categoria \"{}\"", self.name()),
        }
    }
}

/// A label de todos os relatos.
fn report_label() -> Label {
    Label {
        name: REPORT_LABEL.to_owned(),
        color: "1d76db".to_owned(),
        description: "Relato enviado pelo botão \"Relatar problema\" do Warden".to_owned(),
    }
}

/// O que a pessoa escreveu e o que a interface sabe (tela e pack abertos, último erro).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProblemReportInput {
    /// Categoria.
    pub(crate) category: ReportCategory,
    /// Título (obrigatório para enviar; a pré-visualização aceita vazio).
    pub(crate) title: String,
    /// "Descrição".
    pub(crate) description: String,
    /// Prints do rascunho (tokens de `problem_report_attach`), na ordem; só no envio direto.
    pub(crate) attachments: Vec<String>,
    /// "Incluir os registros do Warden".
    pub(crate) include_log: bool,
    /// Tela atual, nos termos da interface ("Pack › Mods").
    pub(crate) screen: String,
    /// Pack aberto, se houver.
    pub(crate) pack_id: Option<PackId>,
    /// Último erro mostrado, se houver.
    pub(crate) last_error: Option<ShownError>,
}

/// O que precisou ser cortado para o endereço caber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ReportCut {
    /// Nada.
    None,
    /// As linhas mais antigas dos registros.
    Log,
    /// Os registros e o fim do texto escrito.
    Text,
}

/// Um dado coletado sozinho, já redigido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportField {
    /// Rótulo ("Sistema").
    pub(crate) label: String,
    /// Valor ("Windows 11 (10.0.26100)").
    pub(crate) value: String,
}

/// A pré-visualização: exatamente o que vai para o GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProblemReportPreview {
    /// Título da issue, redigido.
    pub(crate) title: String,
    /// Labels da issue.
    pub(crate) labels: Vec<String>,
    /// Corpo do envio direto (UI-03), redigido, com os prints antes do envio.
    pub(crate) direct_body: String,
    /// Linhas dos registros no corpo do envio direto.
    pub(crate) direct_log_lines: u32,
    /// Corpo que vai no endereço de "Abrir no GitHub", redigido (sem os prints).
    pub(crate) body: String,
    /// Endereço da issue nova (`problem_report_open` abre este).
    pub(crate) url: String,
    /// Corpo completo, com todas as linhas lidas dos registros (para a área de transferência
    /// quando algo foi cortado).
    pub(crate) full_body: String,
    /// O que foi cortado.
    pub(crate) cut: ReportCut,
    /// Dados coletados sozinhos, redigidos.
    pub(crate) environment: Vec<ReportField>,
    /// Linhas dos registros no corpo.
    pub(crate) log_lines: u32,
}

/// Monta a pré-visualização do relato.
#[tauri::command]
#[specta::specta]
pub(crate) async fn problem_report_preview(
    state: State<'_, AppState>,
    input: ProblemReportInput,
) -> Result<ProblemReportPreview, AppError> {
    let pack = match input.pack_id {
        Some(pack_id) => {
            let _lock = state.locks.read(pack_id).await;
            Some(pack_text(&state, pack_id))
        }
        None => None,
    };
    let prints = state.report_draft.names(&input.attachments);
    let player = state.settings.get().player_name;
    let secrets = Arc::clone(&state.secrets);
    let logs_dir = state.paths.logs_dir();
    let info = app_info()?;
    // O cofre do sistema e o disco ficam fora da thread do IPC.
    tauri::async_runtime::spawn_blocking(move || {
        let redactor = redactor(&player, &secrets);
        let collected = collect(&input, info, pack, &logs_dir, prints);
        Ok(preview(&input, collected, &redactor))
    })
    .await
    .map_err(|error| AppError::internal(format!("relato interrompido: {error}")))?
}

/// Envia o relato direto pela API do GitHub (UI-03). O texto é montado e redigido de novo
/// aqui; o progresso vai no canal da operação ("Enviando print 1 de 2…").
#[tauri::command]
#[specta::specta]
pub(crate) async fn problem_report_send(
    state: State<'_, AppState>,
    input: ProblemReportInput,
    on_event: Channel<OperationEvent>,
) -> Result<ProblemReportSent, AppError> {
    let operation = state.operations.start(SEND, None, false).with_events(move |event| {
        // O diálogo pode ter fechado; o envio continua até o fim.
        let _ = on_event.send(event);
    });
    let result = async {
        if input.title.trim().is_empty() {
            return Err(AppError::internal("relato: título vazio no envio"));
        }
        let prepared = prepare(&state, &input).await?;
        let sent = send::send(&state.github, &prepared, &operation).await?;
        // Enviado: o rascunho de prints acabou.
        for image in &prepared.images {
            state.report_draft.remove(&image.id);
        }
        tracing::info!(numero = sent.number, prints = prepared.images.len(), "relato enviado");
        Ok(sent)
    }
    .await;
    operation.finish(result)
}

/// Monta o relato do envio direto, redigido, com os prints do rascunho.
async fn prepare(state: &AppState, input: &ProblemReportInput) -> Result<send::Prepared, AppError> {
    let pack = match input.pack_id {
        Some(pack_id) => {
            let _lock = state.locks.read(pack_id).await;
            Some(pack_text(state, pack_id))
        }
        None => None,
    };
    let images = state.report_draft.images(&input.attachments)?;
    let names = images.iter().map(|image| image.name.clone()).collect();
    let player = state.settings.get().player_name;
    let secrets = Arc::clone(&state.secrets);
    let logs_dir = state.paths.logs_dir();
    let info = app_info()?;
    let category = input.category;
    let input = input.clone();
    let parts = tauri::async_runtime::spawn_blocking(move || {
        let redactor = redactor(&player, &secrets);
        let collected = collect(&input, info, pack, &logs_dir, names);
        redacted_parts(&input, collected, &redactor).0
    })
    .await
    .map_err(|error| AppError::internal(format!("relato interrompido: {error}")))?;
    Ok(send::Prepared {
        labels: vec![report_label(), category.label()],
        parts,
        images,
    })
}

/// Abre ou fecha o rascunho de prints do diálogo (os dois esvaziam). Com ele aberto, arquivos
/// soltos sobre a janela vão para os prints.
#[tauri::command]
#[specta::specta]
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn problem_report_draft(state: State<'_, AppState>, open: bool) -> Result<(), AppError> {
    state.report_draft.reset(open);
    Ok(())
}

/// Acrescenta um print colado (Ctrl+V): o conteúdo vem em base64.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn problem_report_attach(
    state: State<'_, AppState>,
    name: String,
    data: String,
) -> Result<ReportImage, AppError> {
    // Recusa o grande demais antes de decodificar (base64: 4 caracteres a cada 3 bytes).
    if data.len() > attachments::MAX_IMAGE_BYTES / 3 * 4 + 8 {
        return Err(AppError::new(crate::error::AppErrorCode::ReportImageTooLarge)
            .with_param("name", attachments::clean_name(&name))
            .with_param(
                "maxMb",
                (attachments::MAX_IMAGE_BYTES / 1024 / 1024).to_string(),
            ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| {
            AppError::new(crate::error::AppErrorCode::ReportImageInvalid)
                .with_param("name", attachments::clean_name(&name))
        })?;
    state.report_draft.add(&name, bytes)
}

/// "Escolher imagens…": o diálogo nativo (PNG e JPEG).
#[tauri::command]
#[specta::specta]
pub(crate) async fn problem_report_choose_images(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<ReportImagesAdded, AppError> {
    let paths = pick_images(&app, &window).await?;
    let draft_paths = paths.clone();
    let added = state.report_draft.add_files(&draft_paths);
    tracing::info!(
        aceitos = added.added.len(),
        recusados = added.refused.len(),
        "imagens escolhidas para o relato"
    );
    Ok(added)
}

/// "Remover" um print.
#[tauri::command]
#[specta::specta]
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn problem_report_detach(state: State<'_, AppState>, token: String) -> Result<(), AppError> {
    state.report_draft.remove(&token);
    Ok(())
}

/// `report-images-dropped`: imagens soltas sobre a janela com o diálogo do relato aberto.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
#[tauri_specta(event_name = "report-images-dropped")]
pub(crate) struct ReportImagesDropped(pub(crate) ReportImagesAdded);

/// Arquivos soltos sobre a janela com o diálogo do relato aberto viram prints (e não vão para
/// "Adicionar arquivo local"). Devolve se tratou o evento.
pub(crate) fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) -> bool {
    let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event else {
        return false;
    };
    let Some(state) = window.app_handle().try_state::<AppState>() else {
        return false;
    };
    if !state.report_draft.is_open() || paths.is_empty() {
        return false;
    }
    let added = state.report_draft.add_files(paths);
    tracing::info!(
        aceitos = added.added.len(),
        recusados = added.refused.len(),
        "imagens soltas no relato"
    );
    if let Err(error) = ReportImagesDropped(added).emit(window.app_handle()) {
        tracing::warn!(%error, "evento report-images-dropped não enviado");
    }
    true
}

async fn pick_images(app: &AppHandle, window: &WebviewWindow) -> Result<Vec<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_PICK_FILES_ENV).filter(|value| !value.is_empty()) {
        let picked = std::fs::read_to_string(&file).unwrap_or_default();
        return Ok(picked
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect());
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Escolha os prints para o relato")
        .add_filter("Imagens PNG e JPEG", &["png", "jpg", "jpeg"])
        .set_parent(window)
        .pick_files(move |picked| {
            // Quem esperava pode ter ido embora (janela fechada); não há o que fazer.
            let _ = sender.send(
                picked
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|path| path.into_path().ok())
                    .collect::<Vec<_>>(),
            );
        });
    receiver
        .await
        .map_err(|error| AppError::internal(format!("diálogo de imagens interrompido: {error}")))
}

/// O que o Warden coleta sozinho (sem redação).
fn collect(
    input: &ProblemReportInput,
    info: AppInfo,
    pack: Option<String>,
    logs_dir: &Path,
    prints: Vec<String>,
) -> Collected {
    Collected {
        system: system_text(info.platform),
        info,
        pack,
        log: input
            .include_log
            .then(|| read_log_tail(logs_dir, LOG_LINES)),
        prints,
    }
}

/// Abre no navegador padrão a issue nova da pré-visualização ou a issue criada.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn problem_report_open(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<(), AppError> {
    check_url(&url, state.github.endpoints())?;
    tracing::info!(chars = url.len(), "relato de problema aberto no navegador");
    open_in_browser(&app, &url)
}

/// Abre um endereço no navegador padrão. Em build de debug, com `WARDEN_E2E_OPEN_URL`, grava
/// o endereço nesse arquivo em vez de abrir (E2E).
pub(crate) fn open_in_browser(app: &AppHandle, url: &str) -> Result<(), AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_OPEN_URL_ENV).filter(|value| !value.is_empty()) {
        return std::fs::write(&file, url)
            .map_err(|error| AppError::internal(format!("E2E: gravar o endereço: {error}")));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| AppError::internal(format!("abrir no navegador: {error}")))
}

/// Só a página de issue nova do Warden (dentro do limite e sem espaços) ou a página de uma
/// issue do Warden.
fn check_url(url: &str, endpoints: &Endpoints) -> Result<(), AppError> {
    let prefix = format!("{}?", new_issue_url());
    let new_issue = url.starts_with(&prefix)
        && url.len() <= MAX_URL_CHARS
        && url.bytes().all(|byte| byte.is_ascii_graphic());
    if new_issue || endpoints.is_issue_page(url) {
        Ok(())
    } else {
        Err(AppError::internal(
            "relato: endereço fora das páginas de issue do Warden",
        ))
    }
}

/// Redator com o ambiente (usuário, computador), o nome do jogador e os valores reais das
/// chaves guardadas.
fn redactor(player: &str, secrets: &Secrets) -> Redactor {
    let mut profile = RedactionProfile::from_environment();
    if !player.trim().is_empty() {
        profile = profile.with_player(player);
    }
    match secrets.current_values() {
        Ok(secrets) => {
            for secret in &secrets {
                profile = profile.with_secret(secret);
            }
        }
        // Os formatos conhecidos de chave continuam redigidos pelas regras fixas.
        Err(error) => tracing::warn!(%error, "relato: chaves do cofre não lidas para a redação"),
    }
    Redactor::new(profile)
}

/// O que o Warden coletou sozinho, ainda sem redação.
struct Collected {
    info: AppInfo,
    system: String,
    pack: Option<String>,
    log: Option<Vec<String>>,
    /// Nomes dos prints, na ordem.
    prints: Vec<String>,
}

/// Monta o relato redigido e os dois corpos.
fn preview(input: &ProblemReportInput, collected: Collected, base: &Redactor) -> ProblemReportPreview {
    let (parts, environment) = redacted_parts(input, collected, base);
    let direct = fit_direct(&parts);
    // "Abrir no GitHub" não leva prints: o GitHub não aceita imagens pelo endereço.
    let fitted = fit(&ReportParts {
        prints: Vec::new(),
        ..parts.clone()
    });
    ProblemReportPreview {
        title: fitted.title,
        labels: parts.labels,
        direct_body: direct.body,
        direct_log_lines: u32::try_from(direct.log_lines).unwrap_or(u32::MAX),
        body: fitted.body,
        url: fitted.url,
        full_body: fitted.full_body,
        cut: match fitted.cut {
            Cut::None => ReportCut::None,
            Cut::Log => ReportCut::Log,
            Cut::Text => ReportCut::Text,
        },
        environment: environment
            .into_iter()
            .map(|(label, value)| ReportField { label, value })
            .collect(),
        log_lines: u32::try_from(fitted.log_lines).unwrap_or(u32::MAX),
    }
}

/// Redige cada pedaço (com os nomes que o texto inteiro revela) e devolve as partes do relato
/// e o coletado, redigido.
fn redacted_parts(
    input: &ProblemReportInput,
    collected: Collected,
    base: &Redactor,
) -> (ReportParts, Vec<(String, String)>) {
    let error_text = input
        .last_error
        .as_ref()
        .map(|error| match error.code.as_deref().map(str::trim) {
            Some(code) if !code.is_empty() => format!("`{code}`: {}", error.message.trim()),
            _ => error.message.trim().to_owned(),
        });
    let mut environment = vec![
        ("Warden".to_owned(), version_text(&collected.info)),
        ("Sistema".to_owned(), collected.system),
        ("Tela".to_owned(), non_empty(&input.screen, "não informada")),
        (
            "Pack aberto".to_owned(),
            collected.pack.unwrap_or_else(|| "nenhum".to_owned()),
        ),
        (
            "Último erro mostrado".to_owned(),
            error_text.unwrap_or_else(|| "nenhum".to_owned()),
        ),
    ];
    let log_text = collected.log.as_ref().map(|lines| lines.join("\n"));

    // Um nome que só aparece num pedaço (uma linha antiga do registro) some de todos.
    let everything = [
        input.title.as_str(),
        &input.description,
        log_text.as_deref().unwrap_or_default(),
    ]
    .into_iter()
    .chain(environment.iter().map(|(_, value)| value.as_str()))
    .chain(collected.prints.iter().map(String::as_str))
    .collect::<Vec<_>>()
    .join("\n");
    let redactor = base.learning_from(&everything);
    let clean = |text: &str| redactor.redact(text).text;

    for (_, value) in &mut environment {
        *value = clean(value);
    }
    let parts = ReportParts {
        title: clean(&input.title),
        category: input.category.name().to_owned(),
        labels: vec![REPORT_LABEL.to_owned(), input.category.label_name()],
        description: clean(&input.description),
        prints: collected
            .prints
            .iter()
            .map(|name| PrintRef {
                name: attachments::clean_name(&clean(name)),
                url: None,
            })
            .collect(),
        environment: environment.clone(),
        log: log_text.map(|text| clean(&text).lines().map(str::to_owned).collect()),
    };
    (parts, environment)
}

fn non_empty(text: &str, fallback: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        fallback.to_owned()
    } else {
        text.to_owned()
    }
}

/// "0.1.0 (commit 1a2b3c4d5e6f)", com "desenvolvimento" no build de debug.
fn version_text(info: &AppInfo) -> String {
    let mut details = Vec::new();
    if let Some(commit) = &info.commit {
        details.push(format!("commit {commit}"));
    }
    if info.debug_build {
        details.push("desenvolvimento".to_owned());
    }
    if details.is_empty() {
        info.version.clone()
    } else {
        format!("{} ({})", info.version, details.join(", "))
    }
}

/// "Nome · Minecraft 1.20.1 · Fabric 0.16.5" ou o motivo de não ter lido.
fn pack_text(state: &AppState, pack_id: PackId) -> String {
    let meta = pack_root(state, pack_id)
        .and_then(|root| warden_project::meta::read_meta(&root).map_err(|e| AppError::from_domain(&e)));
    match meta {
        Ok(meta) => {
            let mut text = meta.name;
            if let Some(minecraft) = meta.minecraft {
                let _ = write!(text, " · Minecraft {minecraft}");
            }
            if let Some(loader) = meta.loader {
                let _ = write!(text, " · {}", loader_name(&loader));
                if let Some(version) = meta.loader_version {
                    let _ = write!(text, " {version}");
                }
            }
            text
        }
        Err(error) => format!("não foi possível ler o pack ({})", error.code),
    }
}

/// Nome do loader como a interface escreve.
fn loader_name(key: &str) -> String {
    match key {
        "fabric" => "Fabric".to_owned(),
        "forge" => "Forge".to_owned(),
        "neoforge" => "NeoForge".to_owned(),
        "quilt" => "Quilt".to_owned(),
        "liteloader" => "LiteLoader".to_owned(),
        other => other.to_owned(),
    }
}

/// Sistema e versão: "Windows 11 (10.0.26100.4061)" ou "Ubuntu 24.04 LTS (Linux 6.8.0)".
fn system_text(platform: Platform) -> String {
    match platform {
        Platform::Windows => windows_version().map_or_else(
            || "Windows (versão não identificada)".to_owned(),
            |version| windows_text(&version),
        ),
        Platform::Linux => {
            let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
            let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
            linux_text(&release, kernel.trim())
        }
        Platform::Other => std::env::consts::OS.to_owned(),
    }
}

/// "10.0.22631.4317" → "Windows 11 (10.0.22631.4317)" (o Windows 11 é o build 22000 ou maior).
fn windows_text(version: &str) -> String {
    let build: u32 = version
        .split('.')
        .nth(2)
        .and_then(|build| build.parse().ok())
        .unwrap_or(0);
    let name = if version.starts_with("10.") && build >= 22_000 {
        "Windows 11"
    } else if version.starts_with("10.") {
        "Windows 10"
    } else {
        "Windows"
    };
    format!("{name} ({version})")
}

/// A versão do Windows pelo `ver` do `cmd` (sem janela). A frase muda com o idioma do
/// sistema, mas o número (`10.0.26100.4061`) não.
#[cfg(windows)]
fn windows_version() -> Option<String> {
    use std::os::windows::process::CommandExt as _;
    /// `CREATE_NO_WINDOW`: o `cmd` não pisca uma janela.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    version_in(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(windows))]
fn windows_version() -> Option<String> {
    None
}

/// O primeiro número com pontos do texto ("Microsoft Windows [versão 10.0.26100.4061]").
#[cfg_attr(not(windows), allow(dead_code))] // fora do Windows, só nos testes
fn version_in(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|part| part.trim_matches('.'))
        .find(|part| part.contains('.') && part.split('.').all(|n| !n.is_empty()))
        .map(str::to_owned)
}

/// `PRETTY_NAME` do `/etc/os-release` e a versão do kernel.
fn linux_text(os_release: &str, kernel: &str) -> String {
    let name = os_release
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Linux".to_owned());
    if kernel.is_empty() {
        name
    } else {
        format!("{name} (Linux {kernel})")
    }
}

/// As últimas `max` linhas dos registros (`warden.AAAA-MM-DD.log`), da mais antiga para a mais
/// nova, completando com o arquivo do dia anterior se o de hoje for curto.
fn read_log_tail(logs_dir: &Path, max: usize) -> Vec<String> {
    let mut files: Vec<_> = std::fs::read_dir(logs_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("warden"))
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("log"))
        })
        .collect();
    // O nome tem a data: em ordem alfabética, o mais novo fica por último.
    files.sort();
    let mut lines: Vec<String> = Vec::new();
    for file in files.iter().rev() {
        let mut tail = tail_lines(file);
        let room = max - lines.len();
        if tail.len() > room {
            tail.drain(..tail.len() - room);
        }
        tail.append(&mut lines);
        lines = tail;
        if lines.len() >= max {
            break;
        }
    }
    lines
}

/// Linhas não vazias do fim do arquivo (até [`LOG_TAIL_BYTES`]); arquivo ilegível = nenhuma.
fn tail_lines(file: &Path) -> Vec<String> {
    let read = || -> std::io::Result<String> {
        let mut handle = std::fs::File::open(file)?;
        let length = handle.metadata()?.len();
        let start = length.saturating_sub(LOG_TAIL_BYTES);
        handle.seek(SeekFrom::Start(start))?;
        let mut bytes = Vec::new();
        handle.read_to_end(&mut bytes)?;
        let mut text = String::from_utf8_lossy(&bytes).into_owned();
        if start > 0 {
            // A primeira linha começou antes do trecho lido.
            text = text.split_once('\n').map(|(_, rest)| rest.to_owned()).unwrap_or_default();
        }
        Ok(text)
    };
    match read() {
        Ok(text) => text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect(),
        Err(error) => {
            tracing::warn!(%error, "relato: arquivo de registro não lido");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;

    use super::*;

    /// Segredos montados em pedaços: nenhum aparece inteiro no código.
    fn fake_curseforge_key() -> String {
        format!("$2a$10${}", "Ab1Cd2Ef3Gh4Ij5Kl6Mn7Op8Qr9St0Uv1Wx2Yz3Ab4Cd5Ef6Gh7")
    }
    fn fake_gemini_key() -> String {
        format!("AIza{}", "SyA1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q")
    }
    fn fake_github_token() -> String {
        format!("ghp_{}", "aB3dE5gH7jK9mN1pQ3sT5vW7yZ9bC1dE3fG5")
    }
    const VAULT_VALUE: &str = "valor-real-do-cofre-123";
    const USER: &str = "Joaozinho";
    const COMPUTER: &str = "PC-DA-SALA";

    fn input() -> ProblemReportInput {
        ProblemReportInput {
            category: ReportCategory::Error,
            title: format!("Travou na pasta do {USER}"),
            description: format!("Cliquei em Testar e o {COMPUTER} mostrou um erro"),
            attachments: Vec::new(),
            include_log: true,
            screen: "Pack › Mods".into(),
            pack_id: None,
            last_error: Some(ShownError {
                code: Some("app.INTERNAL".into()),
                message: format!(r"Não deu para ler C:\Users\{USER}\Documents\pack.toml"),
            }),
        }
    }

    fn info() -> AppInfo {
        AppInfo {
            version: "0.1.0".into(),
            commit: Some("1a2b3c4d5e6f".into()),
            platform: Platform::Linux,
            debug_build: false,
        }
    }

    fn log_with_private_data() -> Vec<String> {
        vec![
            format!(r"2026-10-12T10:00:00Z INFO warden: lendo C:\Users\{USER}\AppData\Roaming"),
            format!("2026-10-12T10:00:01Z INFO warden: home /home/{}/packs", USER.to_lowercase()),
            format!("2026-10-12T10:00:02Z DEBUG warden: chave {}", fake_curseforge_key()),
            format!("2026-10-12T10:00:03Z DEBUG warden: gemini key={}", fake_gemini_key()),
            format!("2026-10-12T10:00:04Z DEBUG warden: token {}", fake_github_token()),
            format!("2026-10-12T10:00:05Z INFO warden: computador {COMPUTER}"),
            format!("2026-10-12T10:00:06Z INFO warden: cofre devolveu {VAULT_VALUE}"),
        ]
    }

    fn test_redactor() -> Redactor {
        let profile = RedactionProfile::from_env_vars(|name| match name {
            "USERNAME" => Some(USER.to_owned()),
            "COMPUTERNAME" => Some(COMPUTER.to_owned()),
            _ => None,
        })
        .with_secret(&SecretString::from(VAULT_VALUE.to_owned()));
        Redactor::new(profile)
    }

    fn forbidden() -> Vec<String> {
        vec![
            USER.to_owned(),
            USER.to_lowercase(),
            COMPUTER.to_owned(),
            fake_curseforge_key(),
            fake_gemini_key(),
            fake_github_token(),
            VAULT_VALUE.to_owned(),
            "AIza".to_owned(),
            "ghp_".to_owned(),
            "$2a$".to_owned(),
        ]
    }

    fn decoded_url(url: &str) -> String {
        let mut bytes = Vec::new();
        let raw = url.as_bytes();
        let mut index = 0;
        while index < raw.len() {
            if raw[index] == b'%' {
                let hex = std::str::from_utf8(&raw[index + 1..index + 3]).unwrap();
                bytes.push(u8::from_str_radix(hex, 16).unwrap());
                index += 3;
            } else {
                bytes.push(raw[index]);
                index += 1;
            }
        }
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn nada_pessoal_sai_no_corpo_nem_no_endereco() {
        let collected = Collected {
            info: info(),
            system: "Ubuntu 24.04 LTS (Linux 6.8.0)".into(),
            pack: Some(format!("Pack do {USER} · Minecraft 1.20.1 · Fabric 0.16.5")),
            log: Some(log_with_private_data()),
            prints: Vec::new(),
        };
        let report = preview(&input(), collected, &test_redactor());
        let url = decoded_url(&report.url);
        for text in [&report.title, &report.body, &report.full_body, &report.url, &url] {
            for secret in forbidden() {
                assert!(!text.contains(&secret), "{secret:?} vazou em:\n{text}");
            }
        }
        for field in &report.environment {
            for secret in forbidden() {
                assert!(!field.value.contains(&secret), "{secret:?} vazou em {field:?}");
            }
        }
        assert!(report.body.contains("<usuário>"));
        assert!(report.body.contains("<segredo>"));
        assert!(report.body.contains("<computador>"));
        assert_eq!(report.cut, ReportCut::None);
        assert_eq!(report.log_lines, 7);
        // O endereço leva exatamente o título e o corpo da pré-visualização.
        assert!(url.contains(&format!("title={}&", report.title)));
        assert!(url.ends_with(&format!("body={}", report.body)));
    }

    /// UI-03: categoria e descrição nos dois corpos; os prints só no corpo do envio direto;
    /// as labels no endereço.
    #[test]
    fn categoria_prints_e_labels() {
        let collected = Collected {
            info: info(),
            system: "Linux".into(),
            pack: None,
            log: None,
            prints: vec![format!("tela do {USER}.png"), "erro.jpg".into()],
        };
        let mut sugestion = input();
        sugestion.category = ReportCategory::Suggestion;
        sugestion.include_log = false;
        let report = preview(&sugestion, collected, &test_redactor());
        assert_eq!(report.labels, ["relato-do-dono", "categoria: sugestão"]);
        assert!(report.direct_body.starts_with("**Categoria:** Sugestão\n\n### Descrição\n\n"));
        assert!(report.body.starts_with("**Categoria:** Sugestão\n\n### Descrição\n\n"));
        assert!(report.direct_body.contains("![Print 1: tela do usuário.png]("), "{}", report.direct_body);
        assert!(report.direct_body.contains("![Print 2: erro.jpg]("));
        assert!(!report.body.contains("### Prints"));
        assert!(!report.direct_body.contains(USER));
        let url = decoded_url(&report.url);
        assert!(url.contains("&labels=relato-do-dono,categoria: sugestão&"), "{url}");
        let names: Vec<_> = [
            ReportCategory::Error,
            ReportCategory::Suggestion,
            ReportCategory::VisualOrText,
            ReportCategory::Slowness,
            ReportCategory::Other,
        ]
        .iter()
        .map(|category| category.label().name)
        .collect();
        assert_eq!(
            names,
            [
                "categoria: erro",
                "categoria: sugestão",
                "categoria: visual ou texto",
                "categoria: lentidão",
                "categoria: outro"
            ]
        );
        assert_eq!(report_label().name, "relato-do-dono");
    }

    /// CA-UI03-09: os tokens do login (no cofre) somem do corpo, mesmo colados no texto.
    #[tokio::test]
    async fn ca_ui03_09_tokens_do_login_sao_redigidos() {
        use crate::commands::github_login::tests::{CLIENT_ID, access, refresh, session, signed_in};
        let dir = tempfile::tempdir().unwrap();
        let server = wiremock::MockServer::start().await;
        let (_, secrets) = session(dir.path(), &server, CLIENT_ID);
        signed_in(dir.path(), &secrets, 3_600);
        let mut leaking = input();
        leaking.description = format!("token {} e renovação {}", access(1), refresh(1));
        let collected = Collected {
            info: info(),
            system: "Linux".into(),
            pack: None,
            log: Some(vec![format!("Authorization: Bearer {}", access(1))]),
            prints: Vec::new(),
        };
        let report = preview(&leaking, collected, &redactor("Steve", &secrets));
        for text in [&report.direct_body, &report.body, &report.full_body, &report.url] {
            assert!(!text.contains(&access(1)) && !text.contains(&refresh(1)), "{text}");
            assert!(!text.contains("ghu_") && !text.contains("ghr_"), "{text}");
        }
        assert!(report.direct_body.contains("<segredo>"));
    }

    #[test]
    fn ambiente_coletado_aparece_no_corpo() {
        let collected = Collected {
            info: info(),
            system: "Ubuntu 24.04 LTS (Linux 6.8.0)".into(),
            pack: None,
            log: None,
            prints: Vec::new(),
        };
        let mut without_log = input();
        without_log.include_log = false;
        without_log.last_error = None;
        let report = preview(&without_log, collected, &test_redactor());
        assert!(report.body.contains("- **Warden:** 0.1.0 (commit 1a2b3c4d5e6f)"));
        assert!(report.body.contains("- **Sistema:** Ubuntu 24.04 LTS (Linux 6.8.0)"));
        assert!(report.body.contains("- **Tela:** Pack › Mods"));
        assert!(report.body.contains("- **Pack aberto:** nenhum"));
        assert!(report.body.contains("- **Último erro mostrado:** nenhum"));
        assert!(!report.body.contains("Registros do Warden"));
        assert_eq!(report.log_lines, 0);
        let labels: Vec<_> = report.environment.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Warden", "Sistema", "Tela", "Pack aberto", "Último erro mostrado"]
        );
    }

    #[test]
    fn ultimo_erro_com_e_sem_codigo() {
        let collected = || Collected {
            info: info(),
            system: "Linux".into(),
            pack: None,
            log: None,
            prints: Vec::new(),
        };
        let mut with_code = input();
        with_code.last_error = Some(ShownError {
            code: Some("core.TIMEOUT".into()),
            message: "A operação demorou demais.".into(),
        });
        let report = preview(&with_code, collected(), &test_redactor());
        assert!(report
            .body
            .contains("- **Último erro mostrado:** `core.TIMEOUT`: A operação demorou demais."));

        // Erro da própria interface (sem `AppError`): só a frase.
        let mut without_code = input();
        without_code.last_error = Some(ShownError {
            code: None,
            message: "Não deu para copiar o console.".into(),
        });
        let report = preview(&without_code, collected(), &test_redactor());
        assert!(report
            .body
            .contains("- **Último erro mostrado:** Não deu para copiar o console."));
    }

    #[test]
    fn registro_longo_e_cortado_e_completo_fica_sem_dados_pessoais() {
        let mut log: Vec<String> = (0..300)
            .map(|n| format!("2026-10-12T10:00:00Z INFO warden: passo {n:03} em /home/{USER}/x"))
            .collect();
        log.extend(log_with_private_data());
        let collected = Collected {
            info: info(),
            system: "Linux".into(),
            pack: None,
            log: Some(log),
            prints: Vec::new(),
        };
        let report = preview(&input(), collected, &test_redactor());
        assert_eq!(report.cut, ReportCut::Log);
        assert!(report.url.len() <= MAX_URL_CHARS);
        assert!(report.body.contains(body::LOG_CUT_MARK));
        assert!(report.full_body.contains("passo 000"));
        for text in [&report.body, &report.full_body, &report.url] {
            for secret in forbidden() {
                assert!(!text.contains(&secret), "{secret:?} vazou");
            }
        }
        check_url(&report.url, &Endpoints::official().unwrap()).unwrap();
    }

    #[test]
    fn le_as_ultimas_linhas_dos_registros() {
        let dir = tempfile::tempdir().unwrap();
        let old = "ontem 0\nontem 1\nontem 2\nontem 3\nontem 4\n";
        std::fs::write(dir.path().join("warden.2026-10-11.log"), old).unwrap();
        std::fs::write(dir.path().join("warden.2026-10-12.log"), "hoje 0\n\nhoje 1\n").unwrap();
        std::fs::write(dir.path().join("outro.txt"), "não é registro\n").unwrap();
        assert_eq!(read_log_tail(dir.path(), 4), ["ontem 3", "ontem 4", "hoje 0", "hoje 1"]);
        assert_eq!(read_log_tail(dir.path(), 1), ["hoje 1"]);
        assert!(read_log_tail(&dir.path().join("nao-existe"), 10).is_empty());
    }

    #[test]
    fn arquivo_grande_le_so_o_fim_sem_linha_pela_metade() {
        let dir = tempfile::tempdir().unwrap();
        let line = format!("{}\n", "a".repeat(99));
        let mut text = line.repeat(5_000);
        text.push_str("última\n");
        std::fs::write(dir.path().join("warden.2026-10-12.log"), text).unwrap();
        let lines = read_log_tail(dir.path(), 10_000);
        assert_eq!(lines.last().map(String::as_str), Some("última"));
        assert!(lines.iter().all(|line| line == "última" || line.len() == 99));
        assert!(lines.len() < 5_000);
    }

    #[test]
    fn registro_dos_dados_de_teste_e_lido_com_privacidade() {
        // Os registros de verdade, com o arquivo de log escrito pelo app de teste.
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("warden.2026-10-12.log"), log_with_private_data().join("\n"))
            .unwrap();
        let collected = Collected {
            info: info(),
            system: "Linux".into(),
            pack: None,
            log: Some(read_log_tail(&logs, LOG_LINES)),
            prints: Vec::new(),
        };
        let report = preview(&input(), collected, &test_redactor());
        assert_eq!(report.log_lines, 7);
        for secret in forbidden() {
            assert!(!report.url.contains(&secret));
            assert!(!decoded_url(&report.url).contains(&secret));
        }
    }

    #[test]
    fn redator_do_app_usa_o_cofre_e_o_jogador() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::state::tests::test_state(dir.path());
        state
            .secrets
            .set(
                warden_secrets::SecretKind::Gemini,
                &SecretString::from(VAULT_VALUE.to_owned()),
            )
            .unwrap();
        let text = redactor("Steve_42", &state.secrets)
            .redact(&format!("{VAULT_VALUE} e Steve_42 entraram"))
            .text;
        assert_eq!(text, "<segredo> e <jogador> entraram");
    }

    #[test]
    fn so_abre_a_pagina_de_issue_nova_do_warden() {
        let endpoints = Endpoints::official().unwrap();
        let check_url = |url: &str| super::check_url(url, &endpoints);
        check_url("https://github.com/Kriticales/Warden/issues/new?title=a&body=b").unwrap();
        // UI-03: a issue criada também abre ("Ver no GitHub").
        check_url("https://github.com/Kriticales/Warden/issues/42").unwrap();
        for url in [
            "https://github.com/outro/repo/issues/new?title=a",
            "https://github.com/Kriticales/Warden/issues/new",
            "https://github.com/Kriticales/Warden/issues/new?title=a b",
            "http://github.com/Kriticales/Warden/issues/new?title=a",
            "https://github.com/Kriticales/Warden/issues/new@evil.com?x",
            "https://github.com/Kriticales/Warden/issues/42/../../outro",
            "https://github.com/Kriticales/Outro/issues/42",
        ] {
            assert!(check_url(url).is_err(), "{url}");
        }
        let long = format!(
            "https://github.com/Kriticales/Warden/issues/new?body={}",
            "a".repeat(MAX_URL_CHARS)
        );
        assert!(check_url(&long).is_err());
    }

    #[test]
    fn versoes_do_sistema() {
        assert_eq!(windows_text("10.0.26100.4061"), "Windows 11 (10.0.26100.4061)");
        assert_eq!(windows_text("10.0.19045.5011"), "Windows 10 (10.0.19045.5011)");
        assert_eq!(
            version_in("\r\nMicrosoft Windows [versão 10.0.26100.4061]\r\n").as_deref(),
            Some("10.0.26100.4061")
        );
        assert_eq!(version_in("nada aqui"), None);
        assert_eq!(
            linux_text("NAME=\"Ubuntu\"\nPRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\n", "6.8.0-45"),
            "Ubuntu 24.04.1 LTS (Linux 6.8.0-45)"
        );
        assert_eq!(linux_text("", ""), "Linux");
        assert!(!system_text(app_info().unwrap().platform).is_empty());
    }

    #[test]
    fn versao_e_loader_como_a_interface_escreve() {
        assert_eq!(version_text(&info()), "0.1.0 (commit 1a2b3c4d5e6f)");
        let debug = AppInfo {
            commit: None,
            debug_build: true,
            ..info()
        };
        assert_eq!(version_text(&debug), "0.1.0 (desenvolvimento)");
        assert_eq!(loader_name("neoforge"), "NeoForge");
        assert_eq!(loader_name("outro"), "outro");
    }

    #[test]
    fn pack_aberto_com_nome_minecraft_e_loader() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::state::tests::test_state(dir.path());
        let root = dir.path().join("pack");
        std::fs::create_dir_all(&root).unwrap();
        let mut manifest = warden_packwiz::PackManifest::new("Vale Sereno", "1.20.1");
        manifest
            .versions
            .get_or_insert_with(Default::default)
            .insert("neoforge".into(), "21.1.77".into());
        std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
        let record = warden_project::registry::PackRecord {
            id: PackId::new(),
            name: "Vale Sereno".into(),
            path: root,
            last_test: None,
            extra: std::collections::BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        assert_eq!(
            pack_text(&state, id),
            "Vale Sereno · Minecraft 1.20.1 · NeoForge 21.1.77"
        );
        let missing = pack_text(&state, PackId::new());
        assert!(missing.starts_with("não foi possível ler o pack ("), "{missing}");
    }

    #[test]
    fn entrada_em_camel_case() {
        let input: ProblemReportInput = serde_json::from_value(serde_json::json!({
            "category": "visualOrText",
            "title": "t",
            "description": "d",
            "attachments": ["abc"],
            "includeLog": false,
            "screen": "Meus packs",
            "packId": null,
            "lastError": { "code": "app.INTERNAL", "message": "m" }
        }))
        .unwrap();
        assert_eq!(input.screen, "Meus packs");
        assert_eq!(input.category, ReportCategory::VisualOrText);
        assert_eq!(input.attachments, ["abc"]);
        assert_eq!(serde_json::to_value(ReportCut::Log).unwrap(), "log");
    }
}
