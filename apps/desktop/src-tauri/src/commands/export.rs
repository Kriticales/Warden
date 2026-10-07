//! Contrato IPC da exportação nativa. O destino só vem do diálogo Rust.
#![allow(clippy::needless_pass_by_value)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;
use warden_core::{CancellationToken, PackId};
use warden_export::{ExportFormat, ExportPreview, ExportResult, ExportSource};
use warden_packwiz_cli::Packwiz;

use crate::error::AppError;
use crate::operations::OperationKind;
use crate::state::AppState;

const EXPORT_RUN: OperationKind = OperationKind::new("export.run");

/// Em build de debug, o E2E troca o diálogo nativo (que o `WebDriver` não alcança) pelo caminho
/// escrito no arquivo apontado por esta variável, como em `pack_choose_folder` (arquivo vazio =
/// o usuário desistiu).
#[cfg(debug_assertions)]
const E2E_PICK_FOLDER_ENV: &str = "WARDEN_E2E_PICK_FOLDER";

/// Saídas geradas nesta sessão: "Abrir pasta" só mostra um destes caminhos, nunca um caminho
/// qualquer vindo da interface (ARCHITECTURE §20).
static EXPORTED: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Mutex::default);

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
    let picked = pick_destination(&app, &window, format).await?;
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
    .map(|result| {
        EXPORTED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(result.path.clone());
        Some(result)
    })
    .map_err(|error| {
        if token.is_cancelled() {
            AppError::cancelled()
        } else {
            AppError::from_domain(&error)
        }
    });
    operation.finish(result)
}

/// "Abrir pasta" do resultado: o Explorador com a saída selecionada. Só aceita caminhos que
/// `export_run` gerou nesta sessão.
#[tauri::command]
#[specta::specta]
pub(crate) fn export_reveal(app: AppHandle, path: PathBuf) -> Result<(), AppError> {
    let known = EXPORTED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&path);
    if !known {
        return Err(AppError::internal(
            "caminho que não é uma exportação desta sessão",
        ));
    }
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|error| AppError::internal(format!("mostrar a exportação: {error}")))
}

/// Diálogo nativo de destino. `None` = o usuário desistiu.
async fn pick_destination(
    app: &AppHandle,
    window: &WebviewWindow,
    format: ExportFormat,
) -> Result<Option<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_PICK_FOLDER_ENV).filter(|value| !value.is_empty()) {
        let picked = std::fs::read_to_string(&file).unwrap_or_default();
        let picked = picked.trim();
        return Ok((!picked.is_empty()).then(|| PathBuf::from(picked)));
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let dialog = app.dialog().file().set_parent(window);
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
    receiver
        .await
        .map_err(|error| AppError::internal(error.to_string()))
}
