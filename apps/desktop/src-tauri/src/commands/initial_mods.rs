//! Comandos dos mods iniciais e dos kits de desempenho (SPEC T03 e T08; ADR-0033; P1-18).
//!
//! - `initial_mods_offer`: a etapa "Mods iniciais" do Criar pack para um Minecraft e um loader
//!   (antes de o pack existir).
//! - `initial_mods_apply`: o ponto único, depois do "Pack criado": grava os itens marcados pelo
//!   mesmo caminho de "Adicionar" (`add_plan` + `add_apply`), a config inicial do Crash
//!   Assistant e as ferramentas do jogador.
//! - `kits_list`: os kits de desempenho que servem ao loader e à versão (dados embutidos, sem
//!   rede). Aplicar um kit num pack existente passa pelo `add_plan`/`add_apply` e pelo diálogo
//!   de dependências da P1-09.
//!
//! Finos: validam a entrada, pegam a trava do pack e chamam a `warden-project`.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use tauri::{AppHandle, State};
use warden_catalog::Loader;
use warden_core::{CancellationToken, PackId};
use warden_project::initial_mods::{InitialModsRequest, InitialModsResult, InitialOffer};
use warden_project::kits::Kit;

use crate::commands::add::{add_sources, channel_policy};
use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Gravar os mods iniciais" do pack recém-criado.
pub(crate) const APPLY: OperationKind = OperationKind::new("initial-mods.apply");

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// Chave do loader, como nos dados.
fn loader_key(loader: Loader) -> &'static str {
    match loader {
        Loader::Forge => "forge",
        Loader::NeoForge => "neoforge",
        Loader::Fabric => "fabric",
    }
}

/// O que a etapa "Mods iniciais" mostra: spark e Crash Assistant (e a Fabric API no Fabric), com
/// a versão de cada um. Vanilla não oferece nada. Se o Modrinth não responder, o erro é
/// `SEARCH_SOURCE_UNAVAILABLE` e a etapa deixa seguir sem os mods.
#[tauri::command]
#[specta::specta]
pub(crate) async fn initial_mods_offer(
    state: State<'_, AppState>,
    minecraft: String,
    loader: Option<Loader>,
) -> Result<InitialOffer, AppError> {
    warden_project::initial_mods::offer(
        &add_sources(&state),
        channel_policy(&state),
        minecraft.trim(),
        loader.map(loader_key),
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// Grava os mods iniciais marcados (e o kit escolhido) no pack, numa só transação, e avisa a
/// interface. Itens sem versão para o pack ou de uma fonte desligada ficam de fora e voltam em
/// `leftOut`; o pack continua válido.
#[tauri::command]
#[specta::specta]
pub(crate) async fn initial_mods_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    request: InitialModsRequest,
) -> Result<InitialModsResult, AppError> {
    let cli = packwiz(&state)?;
    let handle = state.operations.start(APPLY, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(&state, pack_id)?;
        warden_project::initial_mods::apply(
            &root,
            &add_sources(&state),
            channel_policy(&state),
            &request,
            &cli,
            handle.token(),
        )
        .await
        .map_err(domain)
    }
    .await;
    let result = handle.finish(result)?;
    if !result.added.is_empty() {
        notify_changed(&app, pack_id, vec![PackArea::Inventory, PackArea::Configs]);
    }
    Ok(result)
}

/// Os kits de desempenho do loader e da versão do Minecraft (nenhum para vanilla ou para uma
/// faixa sem kit). Dados embutidos: não usa a rede.
#[tauri::command]
#[specta::specta]
pub(crate) fn kits_list(minecraft: String, loader: Option<Loader>) -> Vec<Kit> {
    warden_project::kits::kits_for(minecraft.trim(), loader.map(loader_key))
}
