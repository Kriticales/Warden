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
    let set = set_loader_impl(&state, pack_id, &expected_from, &version).await?;
    if set.changed.changed {
        notify_changed(&app, pack_id, vec![PackArea::Meta, PackArea::Inventory]);
    }
    Ok(set)
}

async fn set_loader_impl(
    state: &AppState,
    pack_id: PackId,
    expected_from: &str,
    version: &str,
) -> Result<LoaderVersionSet, AppError> {
    let handle = state.operations.start(LOADER, Some(pack_id), false);
    let result = async {
        let (root, changed, inventory) = {
            let _lock = state.locks.write_for(pack_id, &handle).await;
            let root = pack_root(state, pack_id)?;
            let changed = warden_project::loader_version::set_loader_version(
                &root,
                expected_from,
                version,
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
            check_loader_between(state, pack_id, &root, &inventory, &changed.from, &changed.to)
                .await?;
        Ok(LoaderVersionSet { changed, check })
    }
    .await;
    handle.finish(result)
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

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::fs;
    use std::io::Write as _;

    use warden_packwiz_cli::Packwiz;

    use super::*;
    use crate::commands::inventory::tests::{packwiz_binary, registered_pack, test_packwiz};
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;

    /// Um jar do Forge com um `mods.toml` que declara `mod_id`, as dependências obrigatórias e,
    /// opcionalmente, a faixa de versões do Forge.
    fn forge_jar(path: &Path, mod_id: &str, requires: &[&str], forge: Option<&str>) {
        let mut toml = format!(
            "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"MIT\"\n[[mods]]\nmodId=\"{mod_id}\"\nversion=\"1.0\"\n"
        );
        let mut write_dep = |id: &str, range: &str| {
            let _ = write!(
                toml,
                "[[dependencies.{mod_id}]]\nmodId=\"{id}\"\nmandatory=true\nversionRange=\"{range}\"\nordering=\"NONE\"\nside=\"BOTH\"\n"
            );
        };
        if let Some(range) = forge {
            write_dep("forge", range);
        }
        for required in requires {
            write_dep(required, "[1,)");
        }
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        zip.start_file(
            "META-INF/mods.toml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(toml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    /// Pack Forge 1.20.1 com três mods registrado no estado.
    async fn setup() -> Option<(tempfile::TempDir, AppState, PackId, PathBuf, Packwiz)> {
        let binary = packwiz_binary()?;
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cli = test_packwiz(&state, binary);
        let (id, root) = registered_pack(&state, dir.path(), &cli, 3).await;
        let text = fs::read_to_string(root.join("pack.toml")).unwrap();
        let text = warden_packwiz::edit::set_pack_version(&text, "forge", Some("47.2.0")).unwrap();
        fs::write(root.join("pack.toml"), text).unwrap();
        Some((dir, state, id, root, cli))
    }

    fn mods_dir(state: &AppState, id: PackId) -> PathBuf {
        InstanceDirs::for_pack(&state.paths, id).game_dir.join("mods")
    }

    async fn plan(
        state: &AppState,
        id: PackId,
        root: &Path,
        paths: &[&str],
    ) -> warden_project::remove::RemovalPlan {
        let inventory = warden_project::inventory::inventory(root, None).await.unwrap();
        let paths: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        let (affected, orphans) = removal_queries(state, id, &inventory, &paths).await.unwrap();
        warden_project::remove::removal_plan(&inventory, &paths, &affected, orphans).unwrap()
    }

    /// CA-T06-06 pelo app: A → B → C com B e C bibliotecas. Remover C lista B e A (a cadeia);
    /// remover A oferece B e C como sem uso (`provides` e jar-in-jar são cobertos pelos testes
    /// do grafo da D-07, que esta consulta usa).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ca_t06_06_cadeia_inteira_e_bibliotecas_que_ficam_sem_uso() {
        let Some((_dir, state, id, root, _cli)) = setup().await else {
            return;
        };
        let mods = mods_dir(&state, id);
        forge_jar(&mods.join("mod-0-1.0.0.jar"), "meumod", &["kotlinforforge"], None);
        forge_jar(&mods.join("mod-1-1.0.0.jar"), "kotlinforforge", &["architectury"], None);
        forge_jar(&mods.join("mod-2-1.0.0.jar"), "architectury", &[], None);

        let removing_c = plan(&state, id, &root, &["mods/mod-2.pw.toml"]).await;
        let chain: Vec<(&str, u32, &[String])> = removing_c
            .dependents
            .iter()
            .map(|d| (d.name.as_str(), d.depth, d.needs.as_slice()))
            .collect();
        assert_eq!(
            chain,
            [
                ("Mod 1", 1, &["Mod 2".to_owned()][..]),
                ("Mod 0", 2, &["Mod 1".to_owned()][..]),
            ]
        );
        assert!(removing_c.orphans.is_empty());

        let removing_a = plan(&state, id, &root, &["mods/mod-0.pw.toml"]).await;
        assert!(removing_a.dependents.is_empty());
        let orphans: Vec<&str> = removing_a.orphans.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(orphans, ["Mod 1", "Mod 2"]);
        assert_eq!(removing_a.orphans[0].path, "mods/mod-1.pw.toml");

        // Removendo A e a primeira biblioteca juntas, só a segunda fica sem uso.
        let both = plan(&state, id, &root, &["mods/mod-0.pw.toml", "mods/mod-1.pw.toml"]).await;
        let orphans: Vec<&str> = both.orphans.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(orphans, ["Mod 2"]);
    }

    /// Dois itens fornecem o mesmo mod: tirar só um deles não quebra quem precisa dele.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn quem_tem_outro_fornecedor_nao_quebra() {
        let Some((_dir, state, id, root, _cli)) = setup().await else {
            return;
        };
        let mods = mods_dir(&state, id);
        forge_jar(&mods.join("mod-0-1.0.0.jar"), "geckolib", &[], None);
        forge_jar(&mods.join("mod-1-1.0.0.jar"), "usuario", &["geckolib"], None);
        forge_jar(&mods.join("mod-2-1.0.0.jar"), "geckolib", &[], None);
        let one = plan(&state, id, &root, &["mods/mod-2.pw.toml"]).await;
        assert!(one.dependents.is_empty());
        let both = plan(&state, id, &root, &["mods/mod-0.pw.toml", "mods/mod-2.pw.toml"]).await;
        let names: Vec<&str> = both.dependents.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Mod 1"]);
        // Resource pack, shader ou arquivo fora do grafo: sem dependentes, sem erro.
        let none = plan(&state, id, &root, &[]).await;
        assert!(none.dependents.is_empty() && none.orphans.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn loader_mostra_antes_o_que_quebra_e_grava_so_a_linha() {
        let Some((_dir, state, id, root, _cli)) = setup().await else {
            return;
        };
        let mods = mods_dir(&state, id);
        forge_jar(&mods.join("mod-0-1.0.0.jar"), "novo", &[], Some("[47.2,)"));
        forge_jar(&mods.join("mod-1-1.0.0.jar"), "antigo", &[], Some("[47.1,47.2)"));
        // mod-2 sem jar: não verificado.
        let inventory = warden_project::inventory::inventory(&root, None).await.unwrap();
        let check = check_loader(&state, id, &root, &inventory, "47.1.3").await.unwrap();
        assert_eq!(check.loader, "forge");
        assert_eq!((check.from.as_str(), check.to.as_str()), ("47.2.0", "47.1.3"));
        let breaks: Vec<(&str, Option<&str>)> = check
            .breaks
            .iter()
            .map(|i| (i.name.as_str(), i.requires.as_deref()))
            .collect();
        assert_eq!(breaks, [("Mod 0", Some("[47.2,)"))]);
        let fixes: Vec<&str> = check.fixes.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(fixes, ["Mod 1"]);
        assert!(check.still_broken.is_empty());
        assert_eq!(check.checked, 2);
        assert_eq!(check.unchecked, ["Mod 2"]);

        let before = fs::read_to_string(root.join("pack.toml")).unwrap();
        // A versão mostrada no diálogo não é mais a do pack: nada muda.
        let outdated = set_loader_impl(&state, id, "47.0.0", "47.1.3").await.unwrap_err();
        assert_eq!(
            outdated.code,
            ErrorCode::Project(warden_project::ProjectErrorCode::PackChangedExternally)
        );
        assert_eq!(fs::read_to_string(root.join("pack.toml")).unwrap(), before);

        let set = set_loader_impl(&state, id, "47.2.0", "47.1.3").await.unwrap();
        assert!(set.changed.changed);
        // A verificação rápida roda de novo depois de gravar, contra a versão gravada.
        assert_eq!(set.check, check);
        let after = fs::read_to_string(root.join("pack.toml")).unwrap();
        let diff: Vec<(&str, &str)> = before
            .lines()
            .zip(after.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(diff, [("forge = \"47.2.0\"", "forge = \"47.1.3\"")]);
    }
}
