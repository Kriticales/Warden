//! Contrato IPC para Meus packs, criação e importação.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::path::{Path, PathBuf};

use tauri::State;
use warden_catalog::{MinecraftVersionKind, Refresh};
use warden_core::{CancellationToken, PackId};
use warden_packwiz_cli::Packwiz;
use warden_project::ProjectErrorCode;
use warden_project::create::{CreatePack, CreatedPack};
use warden_project::hygiene::HygieneFinding;
use warden_project::open::{ImportPreview, ImportedPack};
use warden_project::registry::PackRow;

use crate::error::AppError;
use crate::state::AppState;

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

fn packwiz(state: &AppState) -> Result<Packwiz, AppError> {
    let binary = Packwiz::locate_binary().map_err(|e| AppError::from_domain(&e))?;
    Ok(Packwiz::new(
        binary,
        state.paths.packwiz_cache_dir(),
        state.paths.packwiz_config_file(),
    ))
}

fn registered_path(state: &AppState, id: PackId) -> Result<PathBuf, AppError> {
    state
        .packs
        .get(id)
        .map(|record| record.path)
        .map_err(domain)
}

/// Lista de packs registrados, inclusive pastas perdidas e manifestos inválidos.
#[tauri::command]
#[specta::specta]
pub(crate) fn packs_list(state: State<'_, AppState>) -> Vec<PackRow> {
    state.packs.list()
}

/// Dados de um pack da lista.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_get(state: State<'_, AppState>, pack_id: PackId) -> Result<PackRow, AppError> {
    state
        .packs
        .list()
        .into_iter()
        .find(|row| row.id == pack_id)
        .ok_or_else(|| AppError::new(ProjectErrorCode::PackNotFound))
}

/// Cria um pack vazio com versões confirmadas pelo catálogo e ponto inicial no histórico.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_create(
    state: State<'_, AppState>,
    mut request: CreatePack,
) -> Result<CreatedPack, AppError> {
    let settings = state.settings.get();
    if request.author.trim().is_empty() {
        request.author = settings.player_name.clone();
    }
    let default_dir = settings
        .packs_dir
        .as_deref()
        .unwrap_or(state.paths.default_packs_dir());
    warden_project::create::validate(&request, default_dir).map_err(domain)?;
    let minecraft = state
        .catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .map_err(|e| AppError::from_domain(&e))?;
    if !minecraft
        .versions
        .iter()
        .any(|v| v.id == request.minecraft && v.kind == MinecraftVersionKind::Release)
    {
        return Err(AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "minecraft"));
    }
    if let (Some(loader), Some(version)) = (request.loader, request.loader_version.as_deref()) {
        let listed = state
            .catalog
            .loader_versions(loader, &request.minecraft, Refresh::IfStale, None)
            .await
            .map_err(|e| AppError::from_domain(&e))?;
        if listed.get(version).is_none() {
            return Err(AppError::new(ProjectErrorCode::InvalidLoaderVersion)
                .with_param("version", version));
        }
    }
    let cli = packwiz(&state)?;
    warden_project::create::create(
        &request,
        default_dir,
        &state.packs,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// Verifica uma pasta packwiz sem escrever nela.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_import_preview(path: PathBuf) -> Result<ImportPreview, AppError> {
    ensure_directory(&path)?;
    warden_project::open::preview(&path).map_err(domain)
}

/// Abre e registra uma pasta packwiz. `add_controls` aceita os arquivos de controle padrão.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_import(
    state: State<'_, AppState>,
    path: PathBuf,
    add_controls: bool,
) -> Result<ImportedPack, AppError> {
    ensure_directory(&path)?;
    warden_project::open::import(&path, add_controls, &state.packs)
        .await
        .map_err(domain)
}

/// Localiza a nova pasta de um pack perdido sem trocar seu histórico local.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_relocate(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: PathBuf,
) -> Result<(), AppError> {
    ensure_directory(&path)?;
    warden_project::open::preview(&path).map_err(domain)?;
    state.packs.get(pack_id).map_err(domain)?;
    if warden_project::open::project_id(&path).map_err(domain)? != Some(pack_id) {
        return Err(AppError::new(ProjectErrorCode::InvalidPack)
            .with_detail("identificador da pasta não corresponde ao registro"));
    }
    state.packs.relocate(pack_id, path).map_err(domain)
}

/// Retira apenas do registro local.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_forget(state: State<'_, AppState>, pack_id: PackId) -> Result<(), AppError> {
    state.packs.forget(pack_id).map_err(domain)
}

/// Itens que não devem entrar no pack distribuído.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_hygiene_scan(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Vec<HygieneFinding>, AppError> {
    warden_project::hygiene::scan(&registered_path(&state, pack_id)?).map_err(domain)
}

/// Remove os itens escolhidos após criar ponto de segurança.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_hygiene_fix(
    state: State<'_, AppState>,
    pack_id: PackId,
    paths: Vec<String>,
) -> Result<Vec<String>, AppError> {
    let _lock = state.locks.write(pack_id).await;
    let root = registered_path(&state, pack_id)?;
    let author = warden_packwiz::read_pack(&root)
        .map_err(|e| AppError::from_domain(&e))?
        .pack
        .value
        .author;
    warden_project::hygiene::fix(
        &root,
        &paths,
        &author,
        &packwiz(&state)?,
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// Move um pack para a Lixeira depois de digitar o nome exato.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_trash(
    state: State<'_, AppState>,
    pack_id: PackId,
    confirmation: String,
) -> Result<(), AppError> {
    let _lock = state.locks.write(pack_id).await;
    warden_project::trash::trash_pack(&state.packs, pack_id, &confirmation).map_err(domain)
}

fn ensure_directory(path: &Path) -> Result<(), AppError> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "path"));
    }
    Ok(())
}
