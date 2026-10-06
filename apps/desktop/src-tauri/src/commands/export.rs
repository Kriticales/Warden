//! Contrato IPC da exportação nativa. O destino só vem do diálogo Rust.
#![allow(clippy::needless_pass_by_value)]

use std::path::{Path, PathBuf};

use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use warden_core::{CancellationToken, PackId};
use warden_export::{ExportFormat, ExportPreview, ExportResult, ExportSource};
use warden_packwiz_cli::Packwiz;

use crate::error::AppError;
use crate::operations::OperationKind;
use crate::state::AppState;

const EXPORT_RUN: OperationKind = OperationKind::new("export.run");

fn pack_path(state: &AppState, id: PackId) -> Result<PathBuf, AppError> {
    state
        .packs
        .get(id)
        .map(|record| record.path)
        .map_err(|error| AppError::from_domain(&error))
}

fn packwiz(state: &AppState) -> Result<Packwiz, AppError> {
    let binary = Packwiz::locate_binary().map_err(|error| AppError::from_domain(&error))?;
    Ok(Packwiz::new(
        binary,
        state.paths.packwiz_cache_dir(),
        state.paths.packwiz_config_file(),
    ))
}

/// Lista exata da saída e avisos antes de exportar.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_preview(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: ExportSource,
) -> Result<ExportPreview, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_path(&state, pack_id)?;
    tauri::async_runtime::spawn_blocking(move || warden_export::preview(&root, &source))
        .await
        .map_err(|error| AppError::internal(error.to_string()))?
        .map_err(|error| AppError::from_domain(&error))
}

/// Adiciona uma regra ao `.packwizignore` e atualiza o índice.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_exclude(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<(), AppError> {
    let _lock = state.locks.write(pack_id).await;
    warden_export::exclude_from_pack(
        &pack_path(&state, pack_id)?,
        &path,
        &packwiz(&state)?,
        &CancellationToken::new(),
    )
    .await
    .map_err(|error| AppError::from_domain(&error))
}

/// Abre o diálogo nativo e gera a saída. `None` significa cancelamento do diálogo.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_run(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    pack_id: PackId,
    source: ExportSource,
    format: ExportFormat,
) -> Result<Option<ExportResult>, AppError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let dialog = app.dialog().file().set_parent(&window);
    match format {
        ExportFormat::Folder => dialog
            .set_title("Escolha uma pasta vazia para o pack")
            .pick_folder(move |picked| {
                let _ = sender.send(picked.and_then(|path| path.as_path().map(Path::to_path_buf)));
            }),
        ExportFormat::Zip => dialog
            .set_title("Salvar arquivo .zip do pack")
            .add_filter("Arquivo ZIP", &["zip"])
            .set_file_name("pack.zip")
            .save_file(move |picked| {
                let _ = sender.send(picked.and_then(|path| path.as_path().map(Path::to_path_buf)));
            }),
    }
    let picked = receiver
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
    let Some(destination) = picked else {
        return Ok(None);
    };
    if !destination.is_absolute() {
        return Err(AppError::internal("o diálogo devolveu um caminho relativo"));
    }
    let operation = state.operations.start(EXPORT_RUN, Some(pack_id), true);
    let token = operation.token().clone();
    let _lock = state.locks.read_for(pack_id, &operation).await;
    let result = warden_export::export(
        &pack_path(&state, pack_id)?,
        &source,
        format,
        &destination,
        &state.paths.cache_dir().join("staging"),
        &packwiz(&state)?,
        &token,
    )
    .await
    .map(Some)
    .map_err(|error| {
        if token.is_cancelled() {
            AppError::cancelled()
        } else {
            AppError::from_domain(&error)
        }
    });
    operation.finish(result)
}
