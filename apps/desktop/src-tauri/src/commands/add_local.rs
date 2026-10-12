//! Arquivo do computador em "Adicionar" (SPEC T08 "Arquivo"; P1-11; CA-T08-04 e CA-T08-05).
//!
//! **A interface nunca vê caminhos** (ARCHITECTURE §20; nada de caminho pessoal em mensagem que
//! possa ir para a IA): o arquivo escolhido no diálogo nativo ou arrastado para a janela fica
//! guardado aqui, e a tela recebe só um `token` e o nome do arquivo ([`LocalFileRef`]).
//!
//! - `add_local_choose_files`: o diálogo nativo "Escolher arquivo do computador…" (vários
//!   arquivos `.jar`/`.zip`), pela API Rust do plugin `dialog`.
//! - Arrastar arquivos para a janela: o Tauri entrega os caminhos ao Rust ([`on_window_event`]);
//!   os `.jar`/`.zip` viram tokens e a interface recebe o evento `local-files-dropped`.
//! - `add_local_inspect`: metadados, compatibilidade e a procura pelo hash no Modrinth e, com a
//!   chave, na CurseForge (sem chave, nenhuma requisição vai para a CurseForge).
//! - `add_local_apply`: copia os arquivos (ou grava a referência do link direto) numa transação
//!   com um `packwiz refresh`, na fila de escrita do pack, e avisa a interface.
//! - `add_local_discard`: esquece os tokens (e apaga o download do link direto).
//!
//! O arquivo identificado vira referência pelo caminho normal (`add_plan`/`add_apply`, sem
//! copiar o jar): a tela abre o diálogo de dependências com a versão achada.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Manager as _, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_specta::Event as _;
use warden_core::{OperationId, PackId};
use warden_project::add::AddResult;
use warden_project::add::local::{
    FileFacts, FileReport, HashLookup, LocalAdd, LocalApplyItem, add_local_apply as apply,
    identify, inspect, read_file,
};
use warden_project::search::PackTarget;

use crate::commands::add_curseforge;
use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::{LocalFilesDropped, PackArea};
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Ler arquivos do computador" (metadados e procura pelo hash).
const INSPECT: OperationKind = OperationKind::new("add.localInspect");
/// "Adicionar arquivos ao pack".
const APPLY: OperationKind = OperationKind::new("add.localApply");

/// Os E2E não alcançam o diálogo nativo: em build de debug, com esta variável, o app lê os
/// arquivos escolhidos (um caminho por linha) do arquivo apontado por ela, como em
/// `pack_choose_folder` (arquivo vazio = o usuário desistiu).
#[cfg(debug_assertions)]
const E2E_PICK_FILES_ENV: &str = "WARDEN_E2E_PICK_FILES";

/// Quanto tempo um token vale sem uso.
const TOKEN_TTL: Duration = Duration::from_hours(1);

/// Um arquivo que a tela pode adicionar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileRef {
    /// Identificador opaco do arquivo nesta sessão.
    pub token: String,
    /// Nome do arquivo (sem a pasta).
    pub file_name: String,
}

/// O resultado da leitura de um arquivo.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalFileResult {
    /// O token pedido.
    pub(crate) token: String,
    /// O relatório, se o arquivo pôde ser lido.
    pub(crate) report: Option<FileReport>,
    /// Por que não pôde.
    pub(crate) error: Option<AppError>,
}

/// Um arquivo guardado para a tela.
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    /// Onde estão os bytes.
    pub(crate) path: PathBuf,
    /// Link direto de onde o arquivo veio.
    pub(crate) link: Option<String>,
    /// Pasta temporária do download (apagada ao esquecer o token).
    pub(crate) staging: Option<PathBuf>,
    /// Último uso.
    used: Instant,
}

static PENDING: LazyLock<Mutex<HashMap<String, Pending>>> = LazyLock::new(Mutex::default);

fn pending() -> std::sync::MutexGuard<'static, HashMap<String, Pending>> {
    PENDING.lock().unwrap_or_else(PoisonError::into_inner)
}

fn remove_staging(staging: Option<&Path>) {
    if let Some(dir) = staging
        && let Err(error) = std::fs::remove_dir_all(dir)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(error = %error.kind(), "pasta temporária do link não apagada");
    }
}

/// Guarda um arquivo e devolve o token. Esquece os tokens vencidos.
pub(crate) fn remember(
    path: PathBuf,
    link: Option<String>,
    staging: Option<PathBuf>,
) -> Option<LocalFileRef> {
    let file_name = path.file_name()?.to_str()?.to_owned();
    let token = OperationId::new().to_string();
    let mut map = pending();
    let expired: Vec<String> = map
        .iter()
        .filter(|(_, entry)| entry.used.elapsed() > TOKEN_TTL)
        .map(|(token, _)| token.clone())
        .collect();
    for old in expired {
        if let Some(entry) = map.remove(&old) {
            remove_staging(entry.staging.as_deref());
        }
    }
    map.insert(
        token.clone(),
        Pending {
            path,
            link,
            staging,
            used: Instant::now(),
        },
    );
    Some(LocalFileRef { token, file_name })
}

fn lookup(token: &str) -> Result<Pending, AppError> {
    let mut map = pending();
    let entry = map.get_mut(token).ok_or_else(|| {
        AppError::new(warden_project::ProjectErrorCode::InvalidInput)
            .with_param("field", "token")
            .with_param("reason", "expired")
    })?;
    entry.used = Instant::now();
    Ok(entry.clone())
}

fn forget(tokens: &[String]) {
    let mut map = pending();
    for token in tokens {
        if let Some(entry) = map.remove(token) {
            remove_staging(entry.staging.as_deref());
        }
    }
}

/// Só `.jar` e `.zip` entram por aqui (SPEC T06: "Arrastar arquivos `.jar`/`.zip`").
fn accepted(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("jar") || ext.eq_ignore_ascii_case("zip"))
}

/// Guarda os arquivos aceitos; devolve os tokens e quantos ficaram de fora.
fn remember_all(paths: Vec<PathBuf>) -> (Vec<LocalFileRef>, u32) {
    let mut files = Vec::new();
    let mut ignored = 0_u32;
    for path in paths {
        match accepted(&path).then(|| remember(path, None, None)).flatten() {
            Some(file) => files.push(file),
            None => ignored = ignored.saturating_add(1),
        }
    }
    (files, ignored)
}

/// Arquivos soltos sobre a janela: os `.jar`/`.zip` viram tokens e a interface recebe
/// `local-files-dropped` (quem decide o que fazer é a tela aberta: Mods ou Adicionar).
pub(crate) fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event else {
        return;
    };
    if paths.is_empty() {
        return;
    }
    let (files, ignored) = remember_all(paths.clone());
    tracing::info!(files = files.len(), ignored, "arquivos soltos na janela");
    if let Err(error) = (LocalFilesDropped { files, ignored }).emit(window.app_handle()) {
        tracing::warn!(%error, "evento local-files-dropped não enviado");
    }
}

async fn pick_files(app: &AppHandle, window: &WebviewWindow) -> Result<Vec<PathBuf>, AppError> {
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
        .set_title("Escolha os arquivos para adicionar ao pack")
        .add_filter("Mods, resource packs e shaders", &["jar", "zip"])
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
        .map_err(|error| AppError::internal(format!("diálogo de arquivos interrompido: {error}")))
}

/// "Escolher arquivo do computador…": o diálogo nativo (vários `.jar`/`.zip`). Vazio: o
/// usuário desistiu.
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_local_choose_files(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Vec<LocalFileRef>, AppError> {
    pack_root(&state, pack_id)?;
    let (files, ignored) = remember_all(pick_files(&app, &window).await?);
    tracing::info!(files = files.len(), ignored, "arquivos escolhidos no diálogo");
    Ok(files)
}

/// O cliente da CurseForge para a procura pela impressão digital (só com chave válida).
fn curseforge_lookup(state: &AppState) -> Option<warden_curseforge::CurseforgeClient> {
    add_curseforge::lookup_client(state)
}

/// Lê cada arquivo (metadados, compatibilidade com o pack, se o pack já o tem) e procura pelo
/// hash no Modrinth e, com a chave, na CurseForge. Erros de um arquivo ficam no resultado dele.
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_local_inspect(
    state: State<'_, AppState>,
    pack_id: PackId,
    tokens: Vec<String>,
) -> Result<Vec<LocalFileResult>, AppError> {
    let operation = state.operations.start(INSPECT, Some(pack_id), true);
    let result = async {
        let entries: Vec<_> = tokens
            .iter()
            .map(|token| (token.clone(), lookup(token).map(|entry| entry.path)))
            .collect();
        let (target, read) = {
            let _lock = state.locks.read_for(pack_id, &operation).await;
            let root = pack_root(&state, pack_id)?;
            let target = PackTarget::read(&root).map_err(|e| AppError::from_domain(&e))?;
            let read = tauri::async_runtime::spawn_blocking(move || read_all(&root, entries))
                .await
                .map_err(|e| AppError::internal(format!("leitura interrompida: {e}")))?;
            (target, read)
        };
        operation.ensure_not_cancelled()?;
        let facts: Vec<FileFacts> = read
            .iter()
            .filter_map(|(_, result)| result.as_ref().ok())
            .map(|(_, facts)| facts.clone())
            .collect();
        let curseforge = curseforge_lookup(&state);
        let identified = identify(
            HashLookup {
                modrinth: Some(&state.modrinth),
                curseforge: curseforge.as_ref(),
            },
            &facts,
            &target,
            operation.token(),
        )
        .await;
        operation.ensure_not_cancelled()?;
        let mut matches = identified.matches.into_iter();
        Ok(read
            .into_iter()
            .map(|(token, result)| match result {
                Ok((mut report, _)) => {
                    report.matches = matches.next().unwrap_or_default();
                    report.lookup_failed.clone_from(&identified.failed);
                    LocalFileResult {
                        token,
                        report: Some(report),
                        error: None,
                    }
                }
                Err(error) => LocalFileResult {
                    token,
                    report: None,
                    error: Some(error),
                },
            })
            .collect())
    }
    .await;
    let result = if operation.is_cancelled() {
        Err(AppError::cancelled())
    } else {
        result
    };
    operation.finish(result)
}

type ReadResult = Result<(FileReport, FileFacts), AppError>;

fn read_all(
    root: &Path,
    entries: Vec<(String, Result<PathBuf, AppError>)>,
) -> Vec<(String, ReadResult)> {
    entries
        .into_iter()
        .map(|(token, path)| {
            let result = path.and_then(|path| {
                let (bytes, facts) = read_file(&path).map_err(|e| AppError::from_domain(&e))?;
                let report = inspect(root, &bytes, &facts).map_err(|e| AppError::from_domain(&e))?;
                Ok((report, facts))
            });
            (token, result)
        })
        .collect()
}

/// Grava os arquivos confirmados: copiados para a pasta do tipo (ou, do link direto, a
/// referência com o link e o `sha256`), numa transação com um `packwiz refresh`. Ou todos
/// entram, ou nenhum. Os tokens gravados são esquecidos.
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_local_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    items: Vec<LocalApplyItem>,
) -> Result<AddResult, AppError> {
    let cli = packwiz(&state)?;
    let mut adds = Vec::new();
    for item in &items {
        let entry = lookup(&item.token)?;
        adds.push(LocalAdd {
            path: entry.path,
            sha256: item.sha256.clone(),
            kind: item.kind,
            link: entry.link,
            name: String::new(),
            // A tela manda o lado sugerido pelo relatório (ou o escolhido).
            side: item.side.unwrap_or(warden_project::side::SideChoice::Both),
            accept_incompatible: item.accept_incompatible,
        });
    }
    let handle = state.operations.start(APPLY, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(&state, pack_id)?;
        apply(&root, &adds, &cli, handle.token())
            .await
            .map_err(|e| AppError::from_domain(&e))
    }
    .await;
    let result = handle.finish(result)?;
    let tokens: Vec<String> = items.into_iter().map(|item| item.token).collect();
    forget(&tokens);
    if !result.added.is_empty() {
        notify_changed(&app, pack_id, vec![PackArea::Inventory]);
    }
    Ok(result)
}

/// Esquece arquivos que a tela não vai adicionar (e apaga o download do link direto).
#[tauri::command]
#[specta::specta]
pub(crate) fn add_local_discard(tokens: Vec<String>) {
    forget(&tokens);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_guardam_o_caminho_e_a_tela_so_ve_o_nome() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Pasta com espaço").join("Meu Mod ç.jar");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"x").unwrap();
        let other = dir.path().join("leia.txt");
        std::fs::write(&other, b"x").unwrap();
        let (files, ignored) = remember_all(vec![file.clone(), other, dir.path().join("sumiu.jar")]);
        assert_eq!(ignored, 2);
        let [only] = files.as_slice() else {
            panic!("{files:?}");
        };
        assert_eq!(only.file_name, "Meu Mod ç.jar");
        let json = serde_json::to_string(only).unwrap();
        assert!(!json.contains("Pasta com"), "{json}");
        assert_eq!(lookup(&only.token).unwrap().path, file);
        forget(std::slice::from_ref(&only.token));
        assert!(lookup(&only.token).is_err());
    }

    #[test]
    fn esquecer_apaga_o_download_do_link() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let file = staging.join("mod.jar");
        std::fs::write(&file, b"x").unwrap();
        let reference = remember(file, Some("https://example.com/mod.jar".into()), Some(staging.clone()))
            .unwrap();
        assert_eq!(
            lookup(&reference.token).unwrap().link.as_deref(),
            Some("https://example.com/mod.jar")
        );
        add_local_discard(vec![reference.token]);
        assert!(!staging.exists());
    }
}
