//! Comandos da página de descoberta (SPEC T08; ARCHITECTURE §4.1 e §17.1; P1-16).
//!
//! - `discover_home`: o início com o campo vazio (populares e atualizados para o pack).
//! - `discover_categories`: a lista curada de categorias do tipo escolhido.
//! - `project_gallery`, `version_notes` e `version_dependencies`: as partes da pré-visualização
//!   completa (as notas só ao abrir a linha da versão).
//!
//! Finos: validam a entrada e chamam a `warden-discovery`. Nenhum grava no pack.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::sync::Arc;

use tauri::State;
use warden_core::{CancellationToken, PackId};
use warden_discovery::categories::DiscoverCategory;
use warden_discovery::home::DiscoverHome;
use warden_discovery::preview::{
    DetailSources, GalleryItem, ModrinthDetails, VersionDependencies, VersionNotes,
};
use warden_project::add::plan::PackState;
use warden_project::search::{ProjectKind, SourceId};

use crate::commands::add::{add_sources, search_sources};
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::state::AppState;

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// Fontes de detalhe ligadas no app. Registro acréscimo-apenas: a P1-10 acrescenta a
/// CurseForge (galeria e notas pelo protocolo `warden-img://`).
fn detail_sources(state: &AppState) -> DetailSources {
    DetailSources::new().with(Arc::new(ModrinthDetails::new(state.modrinth.clone())))
}

/// O início da descoberta: "Populares para <loader> <versão>" e "Atualizados recentemente".
#[tauri::command]
#[specta::specta]
pub(crate) async fn discover_home(
    state: State<'_, AppState>,
    pack_id: PackId,
    kind: ProjectKind,
) -> Result<DiscoverHome, AppError> {
    let pack = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        PackState::read(&root).map_err(domain)?
    };
    warden_discovery::home::home(
        &search_sources(&state),
        &pack.target,
        &pack.installed_keys(),
        kind,
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// As categorias curadas do tipo de projeto. Sem a CurseForge ligada, só as que o Modrinth tem.
#[tauri::command]
#[specta::specta]
pub(crate) fn discover_categories(
    state: State<'_, AppState>,
    kind: ProjectKind,
) -> Result<Vec<DiscoverCategory>, AppError> {
    let curseforge_on = search_sources(&state).iter().any(|source| {
        matches!(source, warden_project::search::ActiveSource::Ready(ready) if ready.id() == SourceId::Curseforge)
    });
    Ok(warden_discovery::categories::categories(
        kind,
        curseforge_on,
    ))
}

/// A galeria do projeto (miniaturas e imagens inteiras).
#[tauri::command]
#[specta::specta]
pub(crate) async fn project_gallery(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: SourceId,
    project_id: String,
) -> Result<Vec<GalleryItem>, AppError> {
    pack_root(&state, pack_id)?;
    warden_discovery::preview::gallery(
        &detail_sources(&state),
        source,
        project_id.trim(),
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// As notas (changelog) de uma versão; pedidas só ao abrir a linha dela.
#[tauri::command]
#[specta::specta]
pub(crate) async fn version_notes(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: SourceId,
    project_id: String,
    version_id: String,
) -> Result<Option<VersionNotes>, AppError> {
    pack_root(&state, pack_id)?;
    warden_discovery::preview::version_notes(
        &detail_sources(&state),
        source,
        project_id.trim(),
        version_id.trim(),
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// As dependências da versão, cada uma com "Já no pack" ou "Será adicionada".
#[tauri::command]
#[specta::specta]
pub(crate) async fn version_dependencies(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: SourceId,
    version_id: String,
) -> Result<VersionDependencies, AppError> {
    let installed = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        PackState::read(&root).map_err(domain)?.installed_keys()
    };
    warden_discovery::preview::version_dependencies(
        &add_sources(&state),
        &installed,
        source,
        version_id.trim(),
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}
