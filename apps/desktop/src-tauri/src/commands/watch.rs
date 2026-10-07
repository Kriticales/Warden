//! Vigia de mudanças externas do pack aberto (A-05; ARCHITECTURE §15).

#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use tauri::{AppHandle, State};
use tauri_specta::Event;
use warden_core::PackId;
use warden_project::watch::WatchConfig;

use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::events::{PackArea, PackChanged};
use crate::state::AppState;

/// Começa a vigiar a pasta do pack (a tela do pack chama ao abrir). Mudanças feitas por outros
/// programas chegam como `pack-changed` com `external: true`. Chamadas repetidas contam
/// aberturas: o vigia só para quando todas pedirem `pack_watch_stop`.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_watch_start(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<(), AppError> {
    let root = pack_root(&state, pack_id)?;
    state.watchers.start(
        pack_id,
        &root,
        state.locks.ledger(),
        WatchConfig::default(),
        Box::new(move |pack, areas| notify_external(&app, pack, areas)),
    )
}

/// Para de vigiar a pasta do pack (a tela do pack chama ao sair).
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_watch_stop(state: State<'_, AppState>, pack_id: PackId) {
    state.watchers.stop(pack_id);
}

/// Avisa a interface de que o pack mudou por fora do Warden.
fn notify_external(app: &AppHandle, pack_id: PackId, areas: Vec<PackArea>) {
    let event = PackChanged {
        pack_id,
        areas,
        external: true,
    };
    if let Err(error) = event.emit(app) {
        tracing::warn!(%error, "não foi possível avisar a interface da mudança externa");
    }
}
