//! Mods opcionais e fixar versão (SPEC T11; P1-14).
//!
//! - `item_option_get` / `item_set_optional`: a tabela `[option]` do `.pw.toml` de um item.
//! - `items_set_pinned`: `pin = true` (itens fixados ficam fora de "Atualizar todos").
//! - `instance_optional_choices_get` / `instance_set_optional_choices`: quais opcionais ficam
//!   ligados na instância de teste. Dado deste computador (`state/optional-choices.json`, pela
//!   `warden-instance`), nunca do pack; mudar não conta como alteração não salva.
//!
//! Finos: validam a entrada, pegam a trava do pack e chamam a `warden-project`. Toda escrita no
//! pack passa pela trava de escrita (a segunda espera na fila e aparece em Tarefas) e avisa a
//! interface com `pack-changed`.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::BTreeMap;

use serde::Serialize;
use tauri::{AppHandle, State};
use warden_core::PackId;
use warden_instance::{InstanceDirs, OptionalChoices};
use warden_packwiz::ModOption;
use warden_packwiz_cli::Packwiz;
use warden_project::ProjectErrorCode;
use warden_project::optional::{OptionSettings, OptionalItem};

use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Marcar como opcional" / "Deixar de ser opcional".
pub(crate) const SET_OPTIONAL: OperationKind = OperationKind::new("inventory.setOptional");
/// "Fixar versão" / "Soltar versão".
pub(crate) const SET_PINNED: OperationKind = OperationKind::new("inventory.setPinned");

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

fn invalid(field: &str) -> AppError {
    AppError::new(ProjectErrorCode::InvalidInput).with_param("field", field)
}

/// A configuração de opcional de um item (`null`: o item não é opcional).
#[tauri::command]
#[specta::specta]
pub(crate) async fn item_option_get(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<Option<OptionSettings>, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    warden_project::optional::option_of(&root, &path).map_err(domain)
}

/// Marca (`settings`) ou desmarca (`null`) um item como opcional. Devolve os arquivos
/// alterados; vazio quando já estava assim.
#[tauri::command]
#[specta::specta]
pub(crate) async fn item_set_optional(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
    settings: Option<OptionSettings>,
) -> Result<Vec<String>, AppError> {
    let cli = packwiz(&state)?;
    let changed = set_optional_impl(&state, &cli, pack_id, &path, settings.as_ref()).await?;
    if !changed.is_empty() {
        notify_changed(&app, pack_id, vec![PackArea::Inventory]);
    }
    Ok(changed)
}

pub(crate) async fn set_optional_impl(
    state: &AppState,
    cli: &Packwiz,
    pack_id: PackId,
    path: &str,
    settings: Option<&OptionSettings>,
) -> Result<Vec<String>, AppError> {
    if path.is_empty() {
        return Err(invalid("path"));
    }
    let handle = state.operations.start(SET_OPTIONAL, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(state, pack_id)?;
        warden_project::optional::set_optional(&root, path, settings, cli, handle.token())
            .await
            .map_err(domain)
    }
    .await;
    handle.finish(result)
}

/// Fixa (`pinned = true`) ou solta a versão de um ou vários itens: uma escrita só.
#[tauri::command]
#[specta::specta]
pub(crate) async fn items_set_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    paths: Vec<String>,
    pinned: bool,
) -> Result<Vec<String>, AppError> {
    let cli = packwiz(&state)?;
    let changed = set_pinned_impl(&state, &cli, pack_id, &paths, pinned).await?;
    if !changed.is_empty() {
        notify_changed(&app, pack_id, vec![PackArea::Inventory]);
    }
    Ok(changed)
}

pub(crate) async fn set_pinned_impl(
    state: &AppState,
    cli: &Packwiz,
    pack_id: PackId,
    paths: &[String],
    pinned: bool,
) -> Result<Vec<String>, AppError> {
    if paths.is_empty() {
        return Err(invalid("paths"));
    }
    let handle = state.operations.start(SET_PINNED, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(state, pack_id)?;
        warden_project::pin::set_pins(&root, paths, pinned, cli, handle.token())
            .await
            .map_err(domain)
    }
    .await;
    handle.finish(result)
}

/// Um opcional do pack com a escolha desta instância.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OptionalChoice {
    /// Caminho do `.pw.toml`.
    pub(crate) path: String,
    /// Nome do item.
    pub(crate) name: String,
    /// Texto que o jogador veria.
    pub(crate) description: String,
    /// Se o pack o liga por padrão.
    pub(crate) default: bool,
    /// Se está ligado na instância (a escolha gravada ou, sem ela, o padrão).
    pub(crate) enabled: bool,
}

/// Os opcionais do pack e quais estão ligados na instância de teste.
#[tauri::command]
#[specta::specta]
pub(crate) async fn instance_optional_choices_get(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Vec<OptionalChoice>, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let items = warden_project::optional::optional_items(&root).map_err(domain)?;
    let dirs = InstanceDirs::for_pack(&state.paths, pack_id);
    let saved = OptionalChoices::load(&dirs.state_dir).map_err(instance)?;
    Ok(choices_view(items, &saved))
}

fn choices_view(items: Vec<OptionalItem>, saved: &OptionalChoices) -> Vec<OptionalChoice> {
    items
        .into_iter()
        .map(|item| {
            let option = ModOption {
                optional: true,
                description: item.description.clone(),
                default: item.default,
            };
            OptionalChoice {
                enabled: saved.is_enabled(&item.path, Some(&option)),
                path: item.path,
                name: item.name,
                description: item.description,
                default: item.default,
            }
        })
        .collect()
}

/// Grava quais opcionais ficam ligados na instância de teste (caminho do `.pw.toml` → ligado).
/// Só vale para itens opcionais do pack; qualquer outro caminho é recusado. As escolhas
/// antigas de itens que deixaram de ser opcionais saem. Não toca no pack.
#[tauri::command]
#[specta::specta]
pub(crate) async fn instance_set_optional_choices(
    state: State<'_, AppState>,
    pack_id: PackId,
    choices: BTreeMap<String, bool>,
) -> Result<(), AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let items = warden_project::optional::optional_items(&root).map_err(domain)?;
    let dirs = InstanceDirs::for_pack(&state.paths, pack_id);
    let merged = merge_choices(&items, &choices)?;
    tauri::async_runtime::spawn_blocking(move || merged.save(&dirs.state_dir))
        .await
        .map_err(|error| AppError::internal(format!("gravar as escolhas dos opcionais: {error}")))?
        .map_err(instance)
}

fn merge_choices(
    items: &[OptionalItem],
    choices: &BTreeMap<String, bool>,
) -> Result<OptionalChoices, AppError> {
    let mut merged = OptionalChoices::default();
    for (path, enabled) in choices {
        if !items.iter().any(|item| &item.path == path) {
            return Err(invalid("choices").with_param("path", path));
        }
        merged.set(path, *enabled);
    }
    Ok(merged)
}

fn instance(error: warden_instance::Error) -> AppError {
    AppError::from_domain(&error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, default: bool) -> OptionalItem {
        OptionalItem {
            path: path.into(),
            name: path.into(),
            description: String::new(),
            default,
        }
    }

    #[test]
    fn sem_escolha_gravada_vale_o_padrao_do_item() {
        let items = vec![item("mods/a.pw.toml", true), item("mods/b.pw.toml", false)];
        let view = choices_view(items.clone(), &OptionalChoices::default());
        assert_eq!(
            view.iter().map(|c| c.enabled).collect::<Vec<_>>(),
            [true, false]
        );
        let mut saved = OptionalChoices::default();
        saved.set("mods/a.pw.toml", false);
        saved.set("mods/b.pw.toml", true);
        let view = choices_view(items, &saved);
        assert_eq!(
            view.iter().map(|c| c.enabled).collect::<Vec<_>>(),
            [false, true]
        );
        assert_eq!(view[0].default, true);
    }

    #[test]
    fn so_aceita_caminhos_de_itens_opcionais() {
        let items = vec![item("mods/a.pw.toml", true)];
        let mut choices = BTreeMap::new();
        choices.insert("mods/a.pw.toml".to_owned(), false);
        let merged = merge_choices(&items, &choices).unwrap();
        assert_eq!(merged.choices["mods/a.pw.toml"], false);
        choices.insert("mods/outro.pw.toml".to_owned(), true);
        let error = merge_choices(&items, &choices).unwrap_err();
        assert_eq!(error.params["field"], "choices");
        assert_eq!(error.params["path"], "mods/outro.pw.toml");
    }
}
