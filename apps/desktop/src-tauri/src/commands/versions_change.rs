//! Comandos de "Trocar de versão" e da versão do loader (SPEC T07, T11 e T06; P1-15).
//!
//! - `item_version_options`, `item_version_plan`, `item_version_change`: "Trocar de versão" de
//!   um item do Modrinth ou da CurseForge (qualquer versão compatível, inclusive mais antiga).
//! - `loader_version_check`: antes de confirmar, o que fica incompatível com outra versão do
//!   loader (o mesmo loader do pack).
//! - `loader_version_set`: grava a versão nova (só a linha do loader em `[versions]`) e roda de
//!   novo a verificação rápida dos itens contra ela.
//! - [`removal_queries`]: o que deixa de funcionar (a cadeia inteira) e as dependências que
//!   ficam sem uso, pelas consultas do grafo da D-07, para o `items_remove_plan`.
//!
//! A verificação do loader é a regra `E_LOADERVERSION` do diagnóstico (D-01) sobre os jars que
//! já estão na instância de teste (ou guardados no próprio pack): a faixa de versões do loader
//! que cada mod declara. Mod sem jar fica como "não verificado", nunca como compatível.
//! Finos: validam, pegam a trava do pack e chamam a `warden-project`.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};
use warden_core::{CancellationToken, PackId};
use warden_diagnostics::graph::{Graph, GraphInput};
use warden_diagnostics::model::ItemRef;
use warden_diagnostics::pretest::{self, PackContext, PackItem, Pass};
use warden_instance::InstanceDirs;
use warden_jarmeta::{JarMetadata, VersionRange};
use warden_project::change_version::{
    ChangeVersionOptions, ChangeVersionPlan, VersionChanged,
};
use warden_project::inventory::Inventory;
use warden_project::loader_version::{LoaderCheck, LoaderIssue, LoaderVersionChanged};
use warden_project::orphans::{GraphOrphan, OrphanOffer, orphans_after_removal};
use warden_project::remove::AffectedItem;

use crate::commands::add::add_sources;
use crate::commands::graph::{candidates, existing, loader_of, pack_graph_parts, read_jars};
use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Trocar de versão" de um item.
pub(crate) const CHANGE: OperationKind = OperationKind::new("versions.change");
/// "Trocar a versão do loader".
pub(crate) const LOADER: OperationKind = OperationKind::new("versions.loader");

/// Regra do diagnóstico: versão do loader fora da faixa que o mod declara.
const LOADER_RULE: &str = "E_LOADERVERSION";

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// As versões compatíveis do item, com a instalada e as mais antigas marcadas.
#[tauri::command]
#[specta::specta]
pub(crate) async fn item_version_options(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<ChangeVersionOptions, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    warden_project::change_version::options(
        &root,
        &add_sources(&state),
        &path,
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// O que muda trocando o item para a versão escolhida (rebaixamento, dependências que faltam,
/// incompatibilidades). Nada é gravado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn item_version_plan(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
    version_id: String,
) -> Result<ChangeVersionPlan, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    warden_project::change_version::plan(
        &root,
        &add_sources(&state),
        &path,
        version_id.trim(),
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// Troca o item para a versão escolhida (uma transação, um `packwiz refresh`).
/// `expected_current_id`: a versão instalada que o diálogo mostrou.
#[tauri::command]
#[specta::specta]
pub(crate) async fn item_version_change(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
    expected_current_id: Option<String>,
    version_id: String,
) -> Result<VersionChanged, AppError> {
    let cli = packwiz(&state)?;
    let handle = state.operations.start(CHANGE, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(&state, pack_id)?;
        warden_project::change_version::apply(
            &root,
            &add_sources(&state),
            &path,
            expected_current_id.as_deref(),
            version_id.trim(),
            &cli,
            handle.token(),
        )
        .await
        .map_err(domain)
    }
    .await;
    let changed = handle.finish(result)?;
    notify_changed(&app, pack_id, vec![PackArea::Inventory]);
    Ok(changed)
}

/// O que fica incompatível se o loader do pack passar para `version`. Nada é gravado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn loader_version_check(
    state: State<'_, AppState>,
    pack_id: PackId,
    version: String,
) -> Result<LoaderCheck, AppError> {
    let to = warden_project::loader_version::validate_version(&version)
        .map_err(domain)?
        .to_owned();
    let (root, inventory) = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        let inventory = warden_project::inventory::inventory(&root, None)
            .await
            .map_err(domain)?;
        (root, inventory)
    };
    check_loader(&state, pack_id, &root, &inventory, &to).await
}

/// A versão gravada e a verificação rápida dos itens contra ela.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoaderVersionSet {
    /// O que foi gravado.
    pub changed: LoaderVersionChanged,
    /// A verificação dos itens com a versão gravada (comparada com a de antes).
    pub check: LoaderCheck,
}

/// Grava a versão nova do loader (só a linha dele em `[versions]`) e verifica os itens contra
/// ela. `expected_from`: a versão que o diálogo mostrou.
#[tauri::command]
#[specta::specta]
pub(crate) async fn loader_version_set(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    expected_from: String,
    version: String,
) -> Result<LoaderVersionSet, AppError> {
    let handle = state.operations.start(LOADER, Some(pack_id), false);
    let result = async {
        let (root, changed, inventory) = {
            let _lock = state.locks.write_for(pack_id, &handle).await;
            let root = pack_root(&state, pack_id)?;
            let changed = warden_project::loader_version::set_loader_version(
                &root,
                &expected_from,
                &version,
            )
            .await
            .map_err(domain)?;
            let inventory = warden_project::inventory::inventory(&root, None)
                .await
                .map_err(domain)?;
            (root, changed, inventory)
        };
        // O diagnóstico rápido do loader, com a versão gravada contra a de antes.
        let check =
            check_loader_between(&state, pack_id, &root, &inventory, &changed.from, &changed.to)
                .await?;
        Ok(LoaderVersionSet { changed, check })
    }
    .await;
    let set = handle.finish(result)?;
    if set.changed.changed {
        notify_changed(&app, pack_id, vec![PackArea::Meta, PackArea::Inventory]);
    }
    Ok(set)
}

/// A verificação contra `to`, comparada com a versão atual do pack.
async fn check_loader(
    state: &AppState,
    pack_id: PackId,
    root: &Path,
    inventory: &Inventory,
    to: &str,
) -> Result<LoaderCheck, AppError> {
    let (_, from) = warden_project::loader_version::current_loader(root).map_err(domain)?;
    check_loader_between(state, pack_id, root, inventory, &from, to).await
}

async fn check_loader_between(
    state: &AppState,
    pack_id: PackId,
    root: &Path,
    inventory: &Inventory,
    from: &str,
    to: &str,
) -> Result<LoaderCheck, AppError> {
    let meta = warden_project::meta::read_meta(root).map_err(domain)?;
    let loader_key = meta.loader.clone().unwrap_or_default();
    let minecraft = meta.minecraft.clone().unwrap_or_default();
    let game_dir = InstanceDirs::for_pack(&state.paths, pack_id).game_dir;
    let candidates = candidates(inventory, root, &game_dir);
    let jar_paths: Vec<(usize, PathBuf)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            existing(&candidate.jars).map(|(path, _, _)| (index, path.clone()))
        })
        .collect();
    let jars = tokio::task::spawn_blocking(move || read_jars(jar_paths))
        .await
        .map_err(|error| AppError::internal(format!("ler os jars do pack: {error}")))?;
    let mut jar_of: HashMap<usize, JarMetadata> = jars.into_iter().collect();
    let loader = loader_of(meta.loader.as_deref());
    let mut items = Vec::new();
    let mut unchecked = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        match jar_of.remove(&index) {
            Some(jar) => items.push(PackItem {
                path: candidate.path.clone(),
                filename: candidate.name.clone(),
                side: warden_packwiz::Side::Both,
                file_hash: None,
                api: None,
                jar: Some(jar),
                accepted_minecraft_versions: Vec::new(),
                resource_pack_format: None,
            }),
            None => unchecked.push(candidate.name.clone()),
        }
    }
    let names: HashMap<&str, &str> = candidates
        .iter()
        .map(|candidate| (candidate.path.as_str(), candidate.name.as_str()))
        .collect();
    let issues = |version: &str| -> Result<Vec<LoaderIssue>, AppError> {
        loader_issues(&items, loader, &minecraft, version, &names)
    };
    let with_current = issues(from)?;
    let with_new = issues(to)?;
    Ok(LoaderCheck::classify(
        &loader_key,
        from,
        to,
        with_current,
        with_new,
        u32::try_from(items.len()).unwrap_or(u32::MAX),
        unchecked,
    ))
}

/// Os itens que a regra `E_LOADERVERSION` acusa com a versão `version` do loader.
fn loader_issues(
    items: &[PackItem],
    loader: warden_jarmeta::Loader,
    minecraft: &str,
    version: &str,
    names: &HashMap<&str, &str>,
) -> Result<Vec<LoaderIssue>, AppError> {
    let input = pretest::Input {
        pack: PackContext {
            minecraft: minecraft.to_owned(),
            minecraft_release_target: None,
            loader,
            loader_version: version.to_owned(),
            language_loader_versions: HashMap::new(),
            // Só a regra do loader importa aqui; o Java do teste não entra na conta.
            java_major: u16::MAX,
            minimum_java_major: None,
            resource_pack_format: None,
            acceptable_game_versions: Vec::new(),
        },
        items: items.to_vec(),
        pass: Pass::Complete,
    };
    let findings = pretest::analyze(&input)
        .map_err(|error| AppError::internal(format!("diagnóstico do loader: {error}")))?;
    let mut issues: Vec<LoaderIssue> = Vec::new();
    for finding in findings
        .iter()
        .filter(|finding| finding.rule().as_str() == LOADER_RULE)
    {
        for item in finding.items() {
            let ItemRef::Metafile { path } = item else {
                continue;
            };
            if issues.iter().any(|issue| &issue.path == path) {
                continue;
            }
            let requires = items
                .iter()
                .find(|candidate| &candidate.path == path)
                .and_then(|candidate| candidate.jar.as_ref())
                .and_then(|jar| required_range(jar, loader));
            issues.push(LoaderIssue {
                path: path.clone(),
                name: names.get(path.as_str()).map_or_else(|| path.clone(), |n| (*n).to_owned()),
                requires,
            });
        }
    }
    Ok(issues)
}

/// A faixa de versões do loader que o mod declara, como está no arquivo.
fn required_range(jar: &JarMetadata, loader: warden_jarmeta::Loader) -> Option<String> {
    jar.effective_mods_for_loader(loader)
        .into_iter()
        .find_map(|meta| meta.loader_version.as_ref().and_then(range_text))
}

fn range_text(range: &VersionRange) -> Option<String> {
    match range {
        VersionRange::Any => None,
        VersionRange::Fabric { predicates } => Some(predicates.join(" || ")),
        VersionRange::Maven { spec } => Some(spec.clone()),
        VersionRange::Exact { version } => Some(version.clone()),
    }
}

/// O que deixa de funcionar se `paths` saírem (a cadeia inteira, com `provides` e jar-in-jar)
/// e as dependências que ficam sem uso, pelo grafo da D-07. Itens fora do grafo (resource
/// packs, shaders, arquivos inválidos) não têm dependentes.
pub(crate) async fn removal_queries(
    state: &AppState,
    pack_id: PackId,
    inventory: &Inventory,
    paths: &[String],
) -> Result<(Vec<AffectedItem>, Vec<OrphanOffer>), AppError> {
    let (graph, input) = pack_graph_parts(state, pack_id).await?;
    let targets: Vec<String> = paths
        .iter()
        .map(|path| warden_packwiz::clean_path(&path.replace('\\', "/")))
        .filter(|path| input.items.iter().any(|item| &item.path == path))
        .collect();
    if targets.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let report = graph
        .dependents(&targets)
        .map_err(|error| AppError::internal(error.to_string()))?;
    let affected = report
        .affected
        .into_iter()
        .map(|affected| AffectedItem {
            path: affected.item.path,
            depth: affected.depth,
            needs: affected.needs.into_iter().map(|node| node.name).collect(),
        })
        .collect();
    let without = GraphInput {
        items: input
            .items
            .iter()
            .filter(|item| !targets.contains(&item.path))
            .cloned()
            .collect(),
        ..(*input).clone()
    };
    let orphans = orphans_after_removal(
        inventory,
        &targets,
        &graph_orphans(&graph),
        graph_orphans(&Graph::build(&without)),
    );
    Ok((affected, orphans))
}

fn graph_orphans(graph: &Graph) -> Vec<GraphOrphan> {
    graph
        .orphans()
        .into_iter()
        .map(|orphan| GraphOrphan {
            path: orphan.item.path,
            optional_users: orphan
                .optional_users
                .into_iter()
                .map(|node| node.name)
                .collect(),
        })
        .collect()
}
