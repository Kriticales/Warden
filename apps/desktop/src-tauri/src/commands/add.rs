//! Comandos de busca e adição (ARCHITECTURE §4.1 "search/add"; SPEC T08 e T09; P1-09).
//!
//! - `search_projects`: a busca combinada (as fontes são internas; a CurseForge entra na
//!   P1-10), já filtrada pelo pack e com "Já no pack".
//! - `project_details` e `project_versions`: a pré-visualização e o seletor de versão.
//! - `add_plan`: o plano de um ou vários itens (dependências, conflitos, duplicados), sem
//!   gravar nada.
//! - `add_apply`: grava os itens confirmados numa única transação (um `packwiz refresh`),
//!   na fila de escrita do pack, e avisa a interface com `pack-changed`.
//!
//! Finos: validam a entrada, pegam a trava do pack e chamam a `warden-project`.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::sync::Arc;

use tauri::{AppHandle, State};
use warden_core::{CancellationToken, PackId};
use warden_project::add::modrinth::ModrinthAdd;
use warden_project::add::plan::{AddPlan, AddPlanRequest, PackState};
use warden_project::add::{AddApplyRequest, AddResult, AddSources, ChannelPolicy};
use warden_project::search::modrinth::ModrinthSearch;
use warden_project::search::{
    ActiveSource, ProjectPreview, ProjectVersions, SearchPage, SearchRequest, SourceId,
};

use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Adicionar ao pack" (um ou vários itens).
pub(crate) const APPLY: OperationKind = OperationKind::new("add.apply");

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// Fontes da busca ligadas no app. Registro acréscimo-apenas: a P1-10 acrescenta a CurseForge
/// (ou o aviso de chave ausente ou recusada).
fn search_sources(state: &AppState) -> Vec<ActiveSource> {
    vec![ActiveSource::Ready(Arc::new(ModrinthSearch::new(
        state.modrinth.clone(),
    )))]
}

/// Fontes do plano e da gravação. A P1-10 acrescenta a CurseForge.
pub(crate) fn add_sources(state: &AppState) -> AddSources {
    AddSources::new().with(Arc::new(ModrinthAdd::new(state.modrinth.clone())))
}

/// Canais aceitos por padrão (Configurações: versões beta e alpha).
pub(crate) fn channel_policy(state: &AppState) -> ChannelPolicy {
    ChannelPolicy {
        allow_prerelease: state.settings.get().show_prerelease_versions,
    }
}

/// Uma página da busca combinada, filtrada pelo pack (versão do Minecraft, loader e tipo).
#[tauri::command]
#[specta::specta]
pub(crate) async fn search_projects(
    state: State<'_, AppState>,
    pack_id: PackId,
    request: SearchRequest,
) -> Result<SearchPage, AppError> {
    let pack = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        PackState::read(&root).map_err(domain)?
    };
    warden_project::search::search(
        &search_sources(&state),
        &pack.target,
        &pack.installed_keys(),
        &request,
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// A pré-visualização de um projeto (cabeçalho, descrição e links).
#[tauri::command]
#[specta::specta]
pub(crate) async fn project_details(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: SourceId,
    project_id: String,
) -> Result<ProjectPreview, AppError> {
    pack_root(&state, pack_id)?;
    warden_project::search::project_preview(
        &search_sources(&state),
        source,
        project_id.trim(),
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// As versões de um projeto que servem para o pack, com a padrão do canal configurado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn project_versions(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: SourceId,
    project_id: String,
) -> Result<ProjectVersions, AppError> {
    let pack = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        PackState::read(&root).map_err(domain)?
    };
    warden_project::search::project_versions(
        &add_sources(&state),
        &pack.target,
        channel_policy(&state),
        source,
        project_id.trim(),
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// O plano de "Adicionar" para um ou vários itens: o que vai entrar, as dependências, o que já
/// está no pack, conflitos e duplicados entre fontes. Nada é gravado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_plan(
    state: State<'_, AppState>,
    pack_id: PackId,
    request: AddPlanRequest,
) -> Result<AddPlan, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    warden_project::add::plan::add_plan(
        &root,
        &add_sources(&state),
        channel_policy(&state),
        &request,
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// Grava os itens confirmados (ou todos, ou nenhum) e avisa a interface.
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    request: AddApplyRequest,
) -> Result<AddResult, AppError> {
    let cli = packwiz(&state)?;
    let handle = state.operations.start(APPLY, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(&state, pack_id)?;
        warden_project::add::add_apply(&root, &add_sources(&state), &request, &cli, handle.token())
            .await
            .map_err(domain)
    }
    .await;
    let result = handle.finish(result)?;
    if !result.added.is_empty() || !result.removed.is_empty() {
        notify_changed(&app, pack_id, vec![PackArea::Inventory]);
    }
    Ok(result)
}
