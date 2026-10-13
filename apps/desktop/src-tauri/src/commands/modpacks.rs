//! Comando de "Navegar por modpacks" (SPEC T08; ARCHITECTURE §4.1 e §17.1; P1-17).
//!
//! - `modpack_contents`: a lista **Mods deste modpack**, com o estado de cada mod para o pack
//!   aberto. A busca de modpacks é a busca combinada com `kind = modpack` (`search_projects`),
//!   e "Adicionar N selecionados" é o `add_plan`/`add_apply` de vários itens.
//!
//! Fino: lê o pack (com a trava de leitura) e chama a `warden-discovery`. Não grava nada: nem no
//! pack, nem na pasta de dados (o arquivo do modpack é lido por partes, só em memória).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::sync::OnceLock;

use tauri::State;
use warden_core::{CancellationToken, PackId};
use warden_discovery::modpacks::{
    ModpackContents, ModpackFailure, ModpackSources, PackView, modpack_contents as contents,
};
use warden_http::{HttpClient, HttpConfig};
use warden_project::add::plan::PackState;
use warden_project::search::SourceId;

use crate::commands::add_curseforge::ready_client;
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::state::AppState;

/// O cliente HTTP da leitura por partes, um só para o app (divide conexões e limites).
fn http() -> Result<HttpClient, AppError> {
    static CLIENT: OnceLock<HttpClient> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    let client = HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(CLIENT.get_or_init(|| client).clone())
}

fn failure(error: ModpackFailure) -> AppError {
    match error {
        ModpackFailure::Source(error) => AppError::from_domain(&error),
        ModpackFailure::Modpack(error) => AppError::from_domain(&error),
    }
}

/// Os mods de um modpack, com "Já no seu pack", "Compatível", "Sem versão para o seu pack" e
/// "Arquivo fora das lojas".
#[tauri::command]
#[specta::specta]
pub(crate) async fn modpack_contents(
    state: State<'_, AppState>,
    pack_id: PackId,
    source: SourceId,
    project_id: String,
) -> Result<ModpackContents, AppError> {
    let project_id = project_id.trim().to_owned();
    if project_id.is_empty() {
        return Err(
            AppError::new(warden_project::ProjectErrorCode::InvalidInput)
                .with_param("field", "projectId"),
        );
    }
    let pack = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        PackState::read(&root).map_err(|error| AppError::from_domain(&error))?
    };
    let sources = ModpackSources {
        modrinth: state.modrinth.clone(),
        curseforge: ready_client(&state),
        http: http()?,
    };
    let view = PackView {
        target: &pack.target,
        items: &pack.items,
    };
    contents(
        &sources,
        &view,
        source,
        &project_id,
        &CancellationToken::new(),
    )
    .await
    .map_err(failure)
}
