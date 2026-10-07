//! Comandos do domínio `test` (ARCHITECTURE §4.1; SPEC T13; ROADMAP L-04).
//!
//! - `test_start`: testa o pack (operação `test.start`); o canal recebe etapas, progresso,
//!   avisos e as linhas do console; volta quando o jogo fecha, com a sessão gravada.
//! - `test_stop`: "Parar jogo" (ou cancelar a preparação).
//! - `test_game_state` e `test_live_console`: o jogo aberto agora e o console dele (para a
//!   interface se reencontrar com o teste depois de recarregar).
//! - `test_sessions_list`, `test_session_get`, `test_session_save_log`, `test_artifact_open`:
//!   sessões anteriores, o console gravado, "Salvar em arquivo…" e "Abrir crash report".
//! - `instance_reveal_folder`, `instance_worlds_list`, `instance_delete_worlds`: o grupo
//!   "Instância de teste" do menu ▾ (Recriar instância é o `instance_recreate` da P1-08).
//! - `test_quit_app`: "Fechar o Warden" confirmado com o jogo aberto (CA-T13-07).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager as _, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;
use tauri_specta::Event as _;
use warden_core::PackId;
use warden_instance::InstanceDirs;

use crate::error::{AppError, AppErrorCode};
use crate::events::{GameQuitRequested, GameState, OperationEvent};
use crate::state::AppState;
use crate::test_session::{
    ConsoleLine, QUIT_WAIT, TestRequest, TestSessionSummary, TestSink, read_output_log, run_test,
    session_dir, sessions_of,
};

/// Em build de debug, o E2E troca o diálogo de salvar (que o `WebDriver` não alcança) pelo
/// caminho escrito neste arquivo.
const E2E_SAVE_FILE_ENV: &str = "WARDEN_E2E_SAVE_FILE";

/// Testa o pack. Volta quando o jogo fecha, com a sessão gravada.
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_start(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    request: TestRequest,
    on_event: Channel<OperationEvent>,
) -> Result<TestSessionSummary, AppError> {
    let emitter = app.clone();
    let sink = TestSink {
        events: Arc::new(move |event| {
            // A interface pode ter recarregado; o teste continua e o console fica no backend.
            let _ = on_event.send(event);
        }),
        game_state: Arc::new(move |game: &GameState| {
            if let Err(error) = game.clone().emit(&emitter) {
                tracing::warn!(%error, "evento game-state não enviado");
            }
        }),
    };
    run_test(&state, pack_id, request, sink).await
}

/// "Parar jogo" (ou cancelar a preparação). Volta quando o jogo saiu (até 5 s, CA-T13-02).
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_stop(state: State<'_, AppState>, pack_id: PackId) -> Result<(), AppError> {
    state.tests.stop(pack_id).await
}

/// O jogo aberto (ou em preparação) no Warden, se houver.
#[tauri::command]
#[specta::specta]
pub(crate) fn test_game_state(state: State<'_, AppState>) -> Option<GameState> {
    state.tests.current()
}

/// As linhas do console do teste aberto do pack (vazio sem teste aberto).
#[tauri::command]
#[specta::specta]
pub(crate) fn test_live_console(state: State<'_, AppState>, pack_id: PackId) -> Vec<ConsoleLine> {
    state.tests.live_console(pack_id)
}

/// As sessões gravadas do pack, da mais nova para a mais antiga.
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_sessions_list(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Vec<TestSessionSummary>, AppError> {
    state
        .packs
        .get(pack_id)
        .map_err(|error| AppError::from_domain(&error))?;
    let paths = state.paths.clone();
    blocking(move || sessions_of(&paths, pack_id)).await
}

/// Uma sessão gravada com o console dela.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestSessionView {
    /// O resumo.
    pub(crate) summary: TestSessionSummary,
    /// As últimas linhas do `output.log` (até 50 000).
    pub(crate) lines: Vec<ConsoleLine>,
    /// Se o começo do log ficou de fora.
    pub(crate) truncated: bool,
}

/// Uma sessão gravada com o console. Erros: `app.TEST_SESSION_NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_session_get(
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
) -> Result<TestSessionView, AppError> {
    let paths = state.paths.clone();
    blocking(move || {
        let dir = session_dir(&paths, pack_id, &session_id)?;
        let record = dir.read_record().map_err(|_| {
            AppError::new(AppErrorCode::TestSessionNotFound).with_param("sessionId", &session_id)
        })?;
        let (lines, truncated) = read_output_log(&dir.output_log())
            .map_err(|error| AppError::internal(format!("ler o output.log: {error}")))?;
        Ok(TestSessionView {
            summary: TestSessionSummary::from_record(&session_id, &record),
            lines,
            truncated,
        })
    })
    .await
}

/// "Salvar em arquivo…": copia o log completo da sessão para onde o usuário escolher.
/// Devolve o caminho gravado (`None` = desistiu).
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_session_save_log(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
) -> Result<Option<String>, AppError> {
    let dir = session_dir(&state.paths, pack_id, &session_id)?;
    let source = dir.output_log();
    let Some(destination) = pick_save_file(&app, &window, &session_id).await? else {
        return Ok(None);
    };
    blocking(move || {
        std::fs::copy(&source, &destination)
            .map_err(|error| AppError::internal(format!("salvar o log do teste: {error}")))?;
        Ok(Some(destination.display().to_string()))
    })
    .await
}

async fn pick_save_file(
    app: &AppHandle,
    window: &WebviewWindow,
    session_id: &str,
) -> Result<Option<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_SAVE_FILE_ENV).filter(|value| !value.is_empty()) {
        let picked = std::fs::read_to_string(&file).unwrap_or_default();
        let picked = picked.trim();
        return Ok((!picked.is_empty()).then(|| PathBuf::from(picked)));
    }
    #[cfg(not(debug_assertions))]
    let _ = E2E_SAVE_FILE_ENV;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(window)
        .set_title("Salvar o console do teste")
        .add_filter("Log", &["log", "txt"])
        .set_file_name(format!("teste-{session_id}.log"))
        .save_file(move |picked| {
            let _ = sender.send(picked.and_then(|path| path.as_path().map(Path::to_path_buf)));
        });
    receiver
        .await
        .map_err(|error| AppError::internal(error.to_string()))
}

/// Abre um crash report (ou `hs_err_pid`) da sessão no programa padrão. `path` é relativo à
/// pasta do jogo, como no resumo. Erros: `app.TEST_ARTIFACT_INVALID`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_artifact_open(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
    path: String,
) -> Result<(), AppError> {
    let paths = state.paths.clone();
    let file = blocking(move || {
        let dir = session_dir(&paths, pack_id, &session_id)?;
        let record = dir.read_record().map_err(|_| {
            AppError::new(AppErrorCode::TestSessionNotFound).with_param("sessionId", &session_id)
        })?;
        let summary = TestSessionSummary::from_record(&session_id, &record);
        let known = summary
            .crash_reports
            .iter()
            .chain(&summary.hs_err_files)
            .any(|artifact| *artifact == path);
        let invalid = || AppError::new(AppErrorCode::TestArtifactInvalid).with_param("path", &path);
        if !known {
            return Err(invalid());
        }
        let game_dir = InstanceDirs::for_pack(&paths, pack_id).game_dir;
        let file = warden_core::resolve_inside(&game_dir, &path).map_err(|_| invalid())?;
        if !file.is_file() {
            return Err(invalid());
        }
        Ok(file)
    })
    .await?;
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|error| AppError::internal(format!("abrir o crash report: {error}")))
}

/// "Abrir pasta da instância de teste". Erros: `app.TEST_INSTANCE_MISSING`.
#[tauri::command]
#[specta::specta]
pub(crate) fn instance_reveal_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<(), AppError> {
    state
        .packs
        .get(pack_id)
        .map_err(|error| AppError::from_domain(&error))?;
    let game_dir = InstanceDirs::for_pack(&state.paths, pack_id).game_dir;
    if !game_dir.is_dir() {
        return Err(AppError::new(AppErrorCode::TestInstanceMissing));
    }
    app.opener()
        .open_path(game_dir.to_string_lossy(), None::<&str>)
        .map_err(|error| AppError::internal(format!("abrir a pasta da instância: {error}")))
}

/// Os mundos de teste da instância (pastas de `saves/`), em ordem alfabética.
#[tauri::command]
#[specta::specta]
pub(crate) async fn instance_worlds_list(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Vec<String>, AppError> {
    let saves = InstanceDirs::for_pack(&state.paths, pack_id)
        .game_dir
        .join("saves");
    blocking(move || Ok(worlds(&saves))).await
}

/// "Apagar mundos de teste…" (decisão D6): apaga as pastas de `saves/`. Recusado com o jogo
/// do pack aberto (`app.GAME_ALREADY_RUNNING`). Devolve quantos mundos apagou.
#[tauri::command]
#[specta::specta]
pub(crate) async fn instance_delete_worlds(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<u32, AppError> {
    if let Some(game) = state.tests.current().filter(|game| game.pack_id == pack_id) {
        return Err(AppError::new(AppErrorCode::GameAlreadyRunning)
            .with_param("packId", game.pack_id.to_string())
            .with_param("packName", game.pack_name));
    }
    let saves = InstanceDirs::for_pack(&state.paths, pack_id)
        .game_dir
        .join("saves");
    blocking(move || delete_worlds(&saves)).await
}

fn worlds(saves: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(saves) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn delete_worlds(saves: &Path) -> Result<u32, AppError> {
    let mut removed = 0u32;
    for name in worlds(saves) {
        let path = saves.join(&name);
        std::fs::remove_dir_all(&path)
            .map_err(|error| AppError::internal(format!("apagar o mundo {name}: {error}")))?;
        removed += 1;
    }
    tracing::info!(removed, "mundos de teste apagados");
    Ok(removed)
}

/// "Fechar o Warden" confirmado com o jogo aberto: para o jogo, espera a sessão ser gravada
/// (até 10 s; o Job Object encerra o que sobrar quando o Warden sair) e fecha o app.
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_quit_app(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let finished = state.tests.stop_all(QUIT_WAIT).await;
    tracing::info!(finished, "fechando o Warden depois de parar o jogo");
    app.exit(0);
    Ok(())
}

/// Fechar a janela com um jogo aberto não fecha: a interface pergunta antes (evento
/// `game-quit-requested`).
pub(crate) fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    let tauri::WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    let Some(state) = window.try_state::<AppState>() else {
        return;
    };
    let Some(game) = state.tests.current() else {
        return;
    };
    api.prevent_close();
    if let Err(error) = (GameQuitRequested { game }).emit(window.app_handle()) {
        tracing::warn!(%error, "evento game-quit-requested não enviado");
    }
}

/// Roda trabalho de disco fora da thread do IPC.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("tarefa do teste interrompida: {error}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mundos_de_teste_listados_e_apagados() {
        let dir = tempfile::tempdir().unwrap();
        let saves = dir.path().join("saves");
        assert!(worlds(&saves).is_empty());
        std::fs::create_dir_all(saves.join("Mundo B").join("region")).unwrap();
        std::fs::create_dir_all(saves.join("Mundo A")).unwrap();
        std::fs::write(saves.join("solto.txt"), "x").unwrap();
        assert_eq!(worlds(&saves), vec!["Mundo A", "Mundo B"]);
        assert_eq!(delete_worlds(&saves).unwrap(), 2);
        assert!(worlds(&saves).is_empty());
        assert!(saves.join("solto.txt").exists(), "só pastas de mundo");
    }
}
