//! Comandos do "Testar como o jogador recebe" (SPEC T13; ROADMAP L-07). O teste em si é o
//! `test_start` com o modo `asPlayer`; aqui fica a leitura do resultado gravado na sessão.
//!
//! - `test_player_report`: o que o jogador recebeu × o pack, de uma sessão gravada.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use tauri::State;
use warden_core::PackId;
use warden_instance::player_install::PlayerReport;

use crate::error::{AppError, AppErrorCode};
use crate::state::AppState;
use crate::test_session::{player_report_of, session_dir};

/// O resultado do "como o jogador recebe" de uma sessão (`None`: sessão de outro modo).
/// Erros: `app.TEST_SESSION_NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_player_report(
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
) -> Result<Option<PlayerReport>, AppError> {
    let paths = state.paths.clone();
    tokio::task::spawn_blocking(move || report(&paths, pack_id, &session_id))
        .await
        .map_err(|error| AppError::internal(format!("leitura do resultado: {error}")))?
}

fn report(
    paths: &warden_core::AppPaths,
    pack_id: PackId,
    session_id: &str,
) -> Result<Option<PlayerReport>, AppError> {
    let dir = session_dir(paths, pack_id, session_id)?;
    let record = dir.read_record().map_err(|_| {
        AppError::new(AppErrorCode::TestSessionNotFound).with_param("sessionId", session_id)
    })?;
    Ok(player_report_of(&record))
}
