//! Contrato IPC dos formatos de outros launchers (E-02; SPEC T19): `.mrpack` e zip da
//! CurseForge. Dois comandos: `export_format_analyze` (o que o formato perde, o que pede
//! decisão e o que impede) e `export_format_run` (gera, valida e grava; o destino só vem do
//! diálogo Rust). "Abrir pasta" do resultado reaproveita `export_reveal`.
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;

use secrecy::SecretString;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use warden_core::{CancellationToken, PackId};
use warden_curseforge::CurseforgeClient;
use warden_export::{
    ExportSource, FormatAnalysis, FormatChoices, FormatExportResult, FormatRequest, LauncherFormat,
    Lookups,
};
use warden_http::{HttpClient, HttpConfig};
use warden_secrets::SecretKind;

#[cfg(debug_assertions)]
use super::export::E2E_PICK_FOLDER_ENV;
use super::export::{pack_path, packwiz, remember_export};
use crate::error::AppError;
use crate::operations::OperationKind;
use crate::state::AppState;

const EXPORT_FORMAT_RUN: OperationKind = OperationKind::new("export.formatRun");

fn domain(error: &warden_export::Error) -> AppError {
    AppError::from_domain(error)
}

/// A chave da CurseForge e o cliente dela; `None` sem chave (a conferência da distribuição
/// fica de fora e a tela avisa).
fn curseforge(
    state: &AppState,
) -> Result<(Option<SecretString>, Option<CurseforgeClient>), AppError> {
    let key = state
        .secrets
        .get(SecretKind::Curseforge)
        .map_err(|error| AppError::from_domain(&error))?;
    let Some(secret) = key else {
        return Ok((None, None));
    };
    let http = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
        .map_err(|error| AppError::from_domain(&error))?;
    let client = CurseforgeClient::new(http, Some(secret.clone()))
        .map_err(|error| AppError::from_domain(&error))?;
    Ok((Some(secret), Some(client)))
}

/// Lê o pack para o formato: o que se perde, o que pede decisão e o que impede de gerar.
/// Não escreve nada.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_format_analyze(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: ExportSource,
    format: LauncherFormat,
) -> Result<FormatAnalysis, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_path(&state, pack_id)?;
    let (_, client) = curseforge(&state)?;
    let lookups = Lookups {
        modrinth: &state.modrinth,
        curseforge: client.as_ref(),
    };
    warden_export::analyze_format(&root, &source, format, &lookups, &CancellationToken::new())
        .await
        .map_err(|error| domain(&error))
}

/// Abre o diálogo nativo, gera o arquivo, confere e grava. `None` = a pessoa desistiu do
/// diálogo.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_format_run(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    pack_id: PackId,
    source: ExportSource,
    format: LauncherFormat,
    choices: FormatChoices,
) -> Result<Option<FormatExportResult>, AppError> {
    let Some(destination) = pick_destination(&app, &window, format).await? else {
        return Ok(None);
    };
    if !destination.is_absolute() {
        return Err(AppError::internal("o diálogo devolveu um caminho relativo"));
    }
    let operation = state
        .operations
        .start(EXPORT_FORMAT_RUN, Some(pack_id), true);
    let token = operation.token().clone();
    let _lock = state.locks.read_for(pack_id, &operation).await;
    let root = pack_path(&state, pack_id)?;
    let packwiz = packwiz(&state)?;
    let (key, client) = curseforge(&state)?;
    let staging = state.paths.cache_dir().join("staging");
    let result = warden_export::export_format(
        &FormatRequest {
            root: &root,
            source: &source,
            format,
            choices: &choices,
            destination: &destination,
            staging_parent: &staging,
            packwiz: &packwiz,
            key: key.as_ref(),
            lookups: Lookups {
                modrinth: &state.modrinth,
                curseforge: client.as_ref(),
            },
        },
        &token,
    )
    .await
    .map(|result| {
        remember_export(&result.path);
        Some(result)
    })
    .map_err(|error| {
        if token.is_cancelled() {
            AppError::cancelled()
        } else {
            domain(&error)
        }
    });
    operation.finish(result)
}

/// Diálogo nativo "Salvar como". `None` = a pessoa desistiu.
async fn pick_destination(
    app: &AppHandle,
    window: &WebviewWindow,
    format: LauncherFormat,
) -> Result<Option<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_PICK_FOLDER_ENV).filter(|value| !value.is_empty()) {
        let picked = std::fs::read_to_string(&file).unwrap_or_default();
        let picked = picked.trim();
        return Ok((!picked.is_empty()).then(|| PathBuf::from(picked)));
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let dialog = app.dialog().file().set_parent(window);
    let (title, label, name) = match format {
        LauncherFormat::Mrpack => (
            "Salvar arquivo .mrpack do pack",
            "Modpack do Modrinth",
            "pack.mrpack",
        ),
        LauncherFormat::Curseforge => (
            "Salvar arquivo .zip da CurseForge",
            "Arquivo ZIP",
            "pack.zip",
        ),
    };
    dialog
        .set_title(title)
        .add_filter(label, &[format.extension()])
        .set_file_name(name)
        .save_file(move |picked| {
            let _ = sender.send(picked.and_then(|path| path.as_path().map(PathBuf::from)));
        });
    receiver
        .await
        .map_err(|error| AppError::internal(error.to_string()))
}
