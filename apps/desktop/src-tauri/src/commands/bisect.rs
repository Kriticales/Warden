//! Comandos do domínio `bisect` (ARCHITECTURE §4.1 e §9.7; SPEC T25; ROADMAP D-12).
//!
//! - `bisect_plan`: o que o diálogo "Encontrar o mod culpado" mostra (quantos mods entram,
//!   sempre ligados, desligados, a estimativa, se a versão entra direto no mundo e o mundo de
//!   teste);
//! - `bisect_start`, `bisect_resume`, `bisect_continue_without`: rodam a busca (operação
//!   `bisect.run`) até terminar ou pausar; o canal recebe as rodadas (`OperationEvent::Round`);
//! - `bisect_pause` ("Pausar depois desta rodada"), `bisect_cancel`, `bisect_answer` (modo
//!   assistido);
//! - `bisect_get`: a busca guardada do pack (rodando, pausada ou a última terminada);
//! - `bisect_round_log`: o log gravado de uma rodada ("Ver log").
//!
//! Finos: a orquestração é a de `bisect_session.rs`.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_specta::Event as _;
use warden_core::PackId;

use crate::bisect_session::{
    self, AssistedAnswer, BisectPlan, BisectRequest, BisectSink, BisectView, RunMode,
};
use crate::error::AppError;
use crate::events::{GameState, OperationEvent};
use crate::state::AppState;
use crate::test_session::ConsoleLine;

fn sink(app: AppHandle, on_event: Channel<OperationEvent>) -> BisectSink {
    BisectSink {
        events: Arc::new(move |event| {
            // A interface pode ter recarregado; a busca continua e o estado fica no disco.
            let _ = on_event.send(event);
        }),
        game_state: Arc::new(move |game: &GameState| {
            if let Err(error) = game.clone().emit(&app) {
                tracing::warn!(%error, "evento game-state não enviado");
            }
        }),
    }
}

/// O que o diálogo de configuração mostra.
#[tauri::command]
#[specta::specta]
pub(crate) async fn bisect_plan(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<BisectPlan, AppError> {
    bisect_session::plan(&state, pack_id).await
}

/// Começa uma busca nova. Volta quando ela termina ou pausa. Erros: `app.GAME_ALREADY_RUNNING`,
/// `bisect.NOTHING_TO_SEARCH`, `bisect.EULA_NEEDED`, `bisect.NEEDS_SERVER`,
/// `bisect.TEST_WORLD_FAILED` (a busca fica pausada antes da primeira rodada).
#[tauri::command]
#[specta::specta]
pub(crate) async fn bisect_start(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    request: BisectRequest,
    on_event: Channel<OperationEvent>,
) -> Result<BisectView, AppError> {
    bisect_session::run(&state, pack_id, RunMode::Start(request), sink(app, on_event)).await
}

/// Continua a busca pausada (até em outro dia). Erros: `bisect.NOT_FOUND`,
/// `bisect.NOT_RESUMABLE`, `bisect.TEST_WORLD_FAILED`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn bisect_resume(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    on_event: Channel<OperationEvent>,
) -> Result<BisectView, AppError> {
    bisect_session::run(&state, pack_id, RunMode::Resume, sink(app, on_event)).await
}

/// "Continuar a busca sem" o conjunto achado, quando há outro problema. Erros:
/// `bisect.NOT_FOUND`, `bisect.NOT_RESUMABLE`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn bisect_continue_without(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    on_event: Channel<OperationEvent>,
) -> Result<BisectView, AppError> {
    bisect_session::run(&state, pack_id, RunMode::ContinueWithout, sink(app, on_event)).await
}

/// "Pausar depois desta rodada". Erros: `bisect.NOT_RESUMABLE` (nenhuma busca rodando).
#[tauri::command]
#[specta::specta]
pub(crate) fn bisect_pause(pack_id: PackId) -> Result<(), AppError> {
    bisect_session::pause(pack_id)
}

/// Cancela a busca (rodando ou pausada), guardando o resultado parcial.
#[tauri::command]
#[specta::specta]
pub(crate) async fn bisect_cancel(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<(), AppError> {
    bisect_session::cancel(&state.paths, pack_id).await
}

/// Resposta do modo assistido. Erros: `bisect.NO_QUESTION`.
#[tauri::command]
#[specta::specta]
pub(crate) fn bisect_answer(pack_id: PackId, answer: AssistedAnswer) -> Result<(), AppError> {
    bisect_session::answer(pack_id, answer)
}

/// A busca guardada do pack, se houver.
#[tauri::command]
#[specta::specta]
pub(crate) fn bisect_get(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Option<BisectView>, AppError> {
    bisect_session::get(&state.paths, pack_id)
}

/// O log gravado de uma rodada. Erros: `bisect.NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) fn bisect_round_log(
    state: State<'_, AppState>,
    pack_id: PackId,
    round: u32,
) -> Result<Vec<ConsoleLine>, AppError> {
    bisect_session::round_log(&state.paths, pack_id, round)
}
