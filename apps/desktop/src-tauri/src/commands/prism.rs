//! Contrato IPC da instância pronta para o Prism (E-04; ARCHITECTURE §12.3). O destino só vem
//! do diálogo Rust; a chave da CurseForge nunca passa pela interface.
#![allow(clippy::needless_pass_by_value)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use secrecy::SecretString;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;
use warden_core::{CancellationToken, PackId};
use warden_curseforge::CurseforgeClient;
use warden_export::{ExportSource, PrismAnalysis, PrismOptions, PrismResult, PrismServices};
use warden_http::{HttpClient, HttpConfig};
use warden_instance::DownloadCache;
use warden_packwiz::PackManifest;
use warden_packwiz_cli::Packwiz;
use warden_secrets::SecretKind;

use crate::error::AppError;
use crate::operations::OperationKind;
use crate::state::AppState;

const PRISM_RUN: OperationKind = OperationKind::new("export.prism");

/// Em build de debug, o E2E troca o diálogo nativo pelo caminho escrito neste arquivo (o mesmo
/// mecanismo de `export_run`).
#[cfg(debug_assertions)]
const E2E_PICK_FOLDER_ENV: &str = "WARDEN_E2E_PICK_FOLDER";

/// Arquivos gerados nesta sessão: "Mostrar arquivo" só abre um destes (ARCHITECTURE §20).
static GENERATED: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Mutex::default);

fn pack_root(state: &AppState, id: PackId) -> Result<PathBuf, AppError> {
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

fn services(state: &AppState) -> Result<PrismServices, AppError> {
    let http = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
        .map_err(|error| AppError::from_domain(&error))?;
    let key: Option<SecretString> = state
        .secrets
        .get(SecretKind::Curseforge)
        .map_err(|error| AppError::from_domain(&error))?;
    let curseforge =
        CurseforgeClient::new(http.clone(), key).map_err(|error| AppError::from_domain(&error))?;
    let cache = DownloadCache::open(&state.paths.downloads_cache_dir())
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(PrismServices::new(
        http,
        Some(curseforge),
        state.modrinth.clone(),
        Arc::new(cache),
    ))
}

/// O que a geração vai fazer: contagens, mods bloqueados com a troca possível e arquivos de
/// terceiros que pedem confirmação. Só consulta as APIs; não baixa nem grava.
#[tauri::command]
#[specta::specta]
pub(crate) async fn prism_analyze(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: ExportSource,
) -> Result<PrismAnalysis, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    warden_export::analyze_prism(
        &root,
        &source,
        &services(&state)?,
        &CancellationToken::new(),
    )
    .await
    .map_err(|error| AppError::from_domain(&error))
}

/// Abre o diálogo de destino e gera o `.mrpack`. `None` = a pessoa desistiu do diálogo.
#[tauri::command]
#[specta::specta]
pub(crate) async fn prism_run(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    pack_id: PackId,
    source: ExportSource,
    options: PrismOptions,
) -> Result<Option<PrismResult>, AppError> {
    let root = pack_root(&state, pack_id)?;
    let Some(destination) = pick_destination(&app, &window, &default_name(&root)).await? else {
        return Ok(None);
    };
    if !destination.is_absolute() {
        return Err(AppError::internal("o diálogo devolveu um caminho relativo"));
    }
    let services = services(&state)?;
    let operation = state.operations.start(PRISM_RUN, Some(pack_id), true);
    let token = operation.token().clone();
    let _lock = state.locks.read_for(pack_id, &operation).await;
    let result = warden_export::export_prism(
        &root,
        &source,
        &destination,
        &state.paths.cache_dir().join("staging"),
        &packwiz(&state)?,
        &services,
        &options,
        &operation,
        &token,
    )
    .await
    .map(|result| {
        GENERATED
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

/// "Mostrar arquivo": só aceita arquivos que `prism_run` gerou nesta sessão.
#[tauri::command]
#[specta::specta]
pub(crate) fn prism_reveal(app: AppHandle, path: PathBuf) -> Result<(), AppError> {
    let known = GENERATED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&path);
    if !known {
        return Err(AppError::internal(
            "caminho que não é uma instância gerada nesta sessão",
        ));
    }
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|error| AppError::internal(format!("mostrar o arquivo: {error}")))
}

/// `<pack>-<versão>-prism.mrpack`, só com caracteres seguros para nome de arquivo.
fn default_name(root: &Path) -> String {
    let manifest = std::fs::read_to_string(root.join("pack.toml"))
        .ok()
        .and_then(|text| PackManifest::parse(&text).ok())
        .map(|parsed| parsed.value);
    let clean = |text: &str| -> String {
        text.chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, '.' | '_' | '-') {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_owned()
    };
    let (name, version) = manifest.map_or_else(
        || (String::new(), String::new()),
        |m| (clean(&m.name), clean(&m.version)),
    );
    let name = if name.is_empty() {
        "pack".to_owned()
    } else {
        name
    };
    if version.is_empty() {
        format!("{name}-prism.mrpack")
    } else {
        format!("{name}-{version}-prism.mrpack")
    }
}

async fn pick_destination(
    app: &AppHandle,
    window: &WebviewWindow,
    file_name: &str,
) -> Result<Option<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_PICK_FOLDER_ENV).filter(|value| !value.is_empty()) {
        let picked = std::fs::read_to_string(&file).unwrap_or_default();
        let picked = picked.trim();
        return Ok((!picked.is_empty()).then(|| PathBuf::from(picked)));
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(window)
        .set_title("Salvar a instância pronta para o Prism")
        .add_filter("Instância do Prism (.mrpack)", &["mrpack"])
        .set_file_name(file_name)
        .save_file(move |picked| {
            let _ = sender.send(picked.and_then(|path| path.as_path().map(Path::to_path_buf)));
        });
    receiver
        .await
        .map_err(|error| AppError::internal(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_padrao_usa_pack_e_versao_sem_caracteres_perigosos() {
        let temp = tempfile::tempdir().unwrap();
        let mut manifest = PackManifest::new("Meu Pack: o/melhor", "1.21.1");
        manifest.version = "1.2.0".into();
        std::fs::write(temp.path().join("pack.toml"), manifest.to_toml_string()).unwrap();
        assert_eq!(
            default_name(temp.path()),
            "Meu-Pack--o-melhor-1.2.0-prism.mrpack"
        );
        assert_eq!(default_name(&temp.path().join("nada")), "pack-prism.mrpack");
    }
}
