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
    CurseforgeDetails, DetailSources, GalleryItem, ModrinthDetails, VersionDependencies,
    VersionNotes,
};
use warden_project::add::plan::PackState;
use warden_project::search::{ActiveSource, ProjectKind, SourceId};

use crate::commands::add::{add_sources, search_sources};
use crate::commands::add_curseforge::ready_client;
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::state::AppState;

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// Fontes de detalhe ligadas no app: o Modrinth e, com chave válida, a CurseForge (galeria pelo
/// protocolo `warden-img://`, notas só em memória).
fn detail_sources(state: &AppState) -> DetailSources {
    let sources =
        DetailSources::new().with(Arc::new(ModrinthDetails::new(state.modrinth.clone())));
    match ready_client(state) {
        Some(client) => sources.with(Arc::new(CurseforgeDetails::new(client))),
        None => sources,
    }
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
/// Nunca falha: o mapeamento é embutido no app.
#[tauri::command]
#[specta::specta]
pub(crate) fn discover_categories(
    state: State<'_, AppState>,
    kind: ProjectKind,
) -> Vec<DiscoverCategory> {
    let curseforge_on = search_sources(&state).iter().any(
        |source| matches!(source, ActiveSource::Ready(ready) if ready.id() == SourceId::Curseforge),
    );
    warden_discovery::categories::categories(kind, curseforge_on)
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
