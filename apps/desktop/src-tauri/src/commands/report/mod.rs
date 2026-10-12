//! Comandos do domínio `report`: "Relatar problema" (UI-02; categoria, descrição única e
//! print da janela na UI-03).
//!
//! O relato vira uma issue do GitHub já preenchida, que a pessoa confere e envia no navegador:
//! nada sai do computador sem o clique em "Abrir no GitHub", e quem envia é a própria pessoa,
//! na página do GitHub.
//!
//! - `problem_report_preview`: junta o que a pessoa escreveu com o que o Warden coleta sozinho
//!   (versão e commit, sistema, tela, pack aberto, último erro mostrado e as últimas linhas
//!   dos registros), passa **tudo** pela redação (`warden_diagnostics::Redactor`, com os
//!   valores reais das chaves do cofre) e devolve o corpo exato, o endereço e o que precisou
//!   ser cortado para o endereço caber (`body.rs`).
//! - `problem_report_open`: abre no navegador padrão o endereço que a pré-visualização
//!   mostrou. Só aceita a página de issue nova do repositório do Warden.
//! - `problem_report_capture`: copia para a área de transferência um print só da janela do
//!   Warden (a interface esconde o diálogo antes; `capture.rs`).

mod body;
mod capture;

use std::fmt::Write as _;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::sync::Arc;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager as _, State};
use tauri_plugin_opener::OpenerExt as _;
use warden_core::PackId;
use warden_diagnostics::{RedactionProfile, Redactor};
use warden_secrets::Secrets;

use crate::commands::app::{AppInfo, Platform, app_info};
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::state::AppState;

pub(crate) use body::ReportCategory;
use body::{Cut, MAX_URL_CHARS, ReportParts, fit, new_issue_url};
use capture::{CapturedImage, NativeCapture, TauriClipboard, capture_to_clipboard};

/// Linhas dos registros lidas (as mais novas).
const LOG_LINES: usize = 200;

/// Bytes lidos do fim de cada arquivo de registro (bastam para [`LOG_LINES`] linhas).
const LOG_TAIL_BYTES: u64 = 256 * 1024;

/// Em build de debug, o E2E troca o navegador pelo arquivo apontado por esta variável: o
/// endereço é gravado nele em vez de abrir.
#[cfg(debug_assertions)]
const E2E_OPEN_URL_ENV: &str = "WARDEN_E2E_OPEN_URL";

/// Em build de debug, o E2E confere o print pelo arquivo apontado por esta variável: a imagem
/// copiada também é gravada nele (PAM).
#[cfg(debug_assertions)]
const E2E_CAPTURE_FILE_ENV: &str = "WARDEN_E2E_CAPTURE_FILE";

/// Último erro que a interface mostrou.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShownError {
    /// `domínio.CÓDIGO`, quando o erro mostrado veio de um `AppError`.
    pub(crate) code: Option<String>,
    /// A frase mostrada.
    pub(crate) message: String,
}

/// O que a pessoa escreveu e o que a interface sabe (tela e pack abertos, último erro).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProblemReportInput {
    /// Categoria (abre o corpo e vira o rótulo da issue).
    pub(crate) category: ReportCategory,
    /// Título (obrigatório para abrir; a pré-visualização aceita vazio).
    pub(crate) title: String,
    /// Descrição (opcional).
    pub(crate) description: String,
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
    /// Corpo que vai no endereço, redigido.
    pub(crate) body: String,
    /// Endereço da issue nova (`problem_report_open` abre este).
    pub(crate) url: String,
    /// Rótulos da issue, como vão no endereço (`relato-do-dono`, `categoria: erro`).
    pub(crate) labels: Vec<String>,
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
    let player = state.settings.get().player_name;
    let secrets = Arc::clone(&state.secrets);
    let logs_dir = state.paths.logs_dir();
    let info = app_info()?;
    // O cofre do sistema e o disco ficam fora da thread do IPC.
    tauri::async_runtime::spawn_blocking(move || {
        let redactor = redactor(&player, &secrets);
        let log = input
            .include_log
            .then(|| read_log_tail(&logs_dir, LOG_LINES));
        let collected = Collected {
            system: system_text(info.platform),
            info,
            pack,
            log,
        };
        Ok(preview(&input, collected, &redactor))
    })
    .await
    .map_err(|error| AppError::internal(format!("relato interrompido: {error}")))?
}

/// Abre o endereço da pré-visualização no navegador padrão.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn problem_report_open(app: AppHandle, url: String) -> Result<(), AppError> {
    check_url(&url)?;
    tracing::info!(chars = url.len(), "relato de problema aberto no navegador");
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_OPEN_URL_ENV).filter(|value| !value.is_empty()) {
        return std::fs::write(&file, &url)
            .map_err(|error| AppError::internal(format!("E2E: gravar o endereço: {error}")));
    }
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|error| AppError::internal(format!("abrir o relato no navegador: {error}")))
}

/// Copia para a área de transferência um print só da janela do Warden (UI-03). A interface
/// esconde o diálogo antes de chamar e o mostra de novo depois. Qualquer falha volta como
/// erro, e a interface abre a issue mesmo assim, dizendo que o print não foi copiado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn problem_report_capture(app: AppHandle) -> Result<CapturedImage, AppError> {
    let window = app
        .get_webview_window(crate::MAIN_WINDOW)
        .ok_or_else(|| AppError::internal("relato: janela principal não encontrada"))?;
    tauri::async_runtime::spawn_blocking(move || {
        let captured = capture_to_clipboard(&NativeCapture::new(window), &TauriClipboard::new(&app));
        match captured {
            Ok((size, shot)) => {
                tracing::info!(width = size.width, height = size.height, "relato: print copiado");
                #[cfg(debug_assertions)]
                if let Some(file) =
                    std::env::var_os(E2E_CAPTURE_FILE_ENV).filter(|value| !value.is_empty())
                {
                    std::fs::write(&file, shot.to_pam()).map_err(|error| {
                        AppError::internal(format!("E2E: gravar o print: {error}"))
                    })?;
                }
                #[cfg(not(debug_assertions))]
                drop(shot);
                Ok(size)
            }
            Err(error) => {
                tracing::warn!(%error, "relato: print não copiado");
                Err(AppError::internal(format!("relato: print não copiado: {error}")))
            }
        }
    })
    .await
    .map_err(|error| AppError::internal(format!("relato: print interrompido: {error}")))?
}

/// Só a página de issue nova do Warden, dentro do limite e sem espaços.
fn check_url(url: &str) -> Result<(), AppError> {
    let prefix = format!("{}?", new_issue_url());
    if url.starts_with(&prefix)
        && url.len() <= MAX_URL_CHARS
        && url.bytes().all(|byte| byte.is_ascii_graphic())
    {
        Ok(())
    } else {
        Err(AppError::internal(
            "relato: endereço fora da página de issue nova do Warden",
        ))
    }
}

/// Redator com o ambiente (usuário, computador), o nome do jogador e os valores reais das
/// chaves guardadas.
pub(crate) fn redactor(player: &str, secrets: &Secrets) -> Redactor {
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
}

/// Redige cada pedaço (com os nomes que o texto inteiro revela) e monta o relato.
fn preview(input: &ProblemReportInput, collected: Collected, base: &Redactor) -> ProblemReportPreview {
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
    .collect::<Vec<_>>()
    .join("\n");
    let redactor = base.learning_from(&everything);
    let clean = |text: &str| redactor.redact(text).text;

    for (_, value) in &mut environment {
        *value = clean(value);
    }
    let parts = ReportParts {
        category: input.category,
        title: clean(&input.title),
        description: clean(&input.description),
        environment: environment.clone(),
        log: log_text.map(|text| clean(&text).lines().map(str::to_owned).collect()),
    };
    let fitted = fit(&parts);
    ProblemReportPreview {
        title: fitted.title,
        body: fitted.body,
        url: fitted.url,
        labels: fitted.labels,
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
        assert!(report.body.starts_with("## Erro\n\nCliquei em Testar e o <computador>"));
        assert_eq!(report.labels, ["relato-do-dono", "categoria: erro"]);
        // O endereço leva exatamente o título e o corpo da pré-visualização.
        assert!(url.contains(&format!("title={}&", report.title)));
        assert!(url.ends_with(&format!("body={}", report.body)));
    }

    #[test]
    fn ambiente_coletado_aparece_no_corpo() {
        let collected = Collected {
            info: info(),
            system: "Ubuntu 24.04 LTS (Linux 6.8.0)".into(),
            pack: None,
            log: None,
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
        check_url(&report.url).unwrap();
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
        check_url("https://github.com/Kriticales/Warden/issues/new?title=a&body=b").unwrap();
        for url in [
            "https://github.com/outro/repo/issues/new?title=a",
            "https://github.com/Kriticales/Warden/issues/new",
            "https://github.com/Kriticales/Warden/issues/new?title=a b",
            "http://github.com/Kriticales/Warden/issues/new?title=a",
            "https://github.com/Kriticales/Warden/issues/new@evil.com?x",
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
            "category": "slowness",
            "title": "t",
            "description": "a",
            "includeLog": false,
            "screen": "Meus packs",
            "packId": null,
            "lastError": { "code": "app.INTERNAL", "message": "m" }
        }))
        .unwrap();
        assert_eq!(input.screen, "Meus packs");
        assert_eq!(input.category, ReportCategory::Slowness);
        assert_eq!(serde_json::to_value(ReportCut::Log).unwrap(), "log");
        for (text, category) in [
            ("error", ReportCategory::Error),
            ("visual", ReportCategory::Visual),
            ("suggestion", ReportCategory::Suggestion),
            ("other", ReportCategory::Other),
        ] {
            assert_eq!(serde_json::from_value::<ReportCategory>(text.into()).unwrap(), category);
        }
    }
}
