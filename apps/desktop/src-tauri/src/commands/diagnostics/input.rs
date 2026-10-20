//! A entrada do pré-teste (`warden_diagnostics::pretest::Input`) montada com o que o app tem
//! (ARCHITECTURE §9.2): o `pack.toml`, o inventário, os metafiles, o Java do teste, os
//! metadados das APIs (Modrinth pelo cache; CurseForge na hora, quando há chave) e, na passagem
//! completa, os jars da instância de teste (e os arquivos guardados no próprio pack).
//!
//! Tudo aqui é tolerante: API fora do ar, metafile ilegível ou jar que não abre deixam o item
//! sem aquela parte, e as regras que dependem dela simplesmente não valem para ele. Nada aqui
//! altera o pack.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use warden_core::PackId;
use warden_diagnostics::Source;
use warden_diagnostics::pretest::{
    ApiMetadata, ApiRelation, ApiRelationKind, Input, PackContext, PackItem, Pass, ReleaseChannel,
};
use warden_instance::InstanceDirs;
use warden_jarmeta::{JarMetadata, Loader};
use warden_java::{JavaChoiceRequest, LoaderKind, MinecraftVersion};
use warden_packwiz::Side;
use warden_project::dedup::{CrossIdentity, same_across};
use warden_project::inventory::{InventoryItem, ItemKind, ItemSource, ItemState};
use warden_project::search::SourceId;

use crate::commands::graph::{existing, jar_locations, read_jars};
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::operations::OperationHandle;
use crate::state::AppState;

/// O pack lido para uma passagem.
pub(crate) struct PackSnapshot {
    /// A pasta do pack.
    pub(crate) root: PathBuf,
    /// Resumo do `pack.toml` e do índice, para saber se o relatório ficou velho.
    pub(crate) pack_hash: Option<String>,
    /// Versão do Minecraft.
    pub(crate) minecraft: Option<String>,
    /// Loader do pack (`None`: vanilla ou um que o diagnóstico não conhece).
    pub(crate) loader: Option<Loader>,
    /// Versão do loader.
    pub(crate) loader_version: Option<String>,
    /// Itens do inventário (todos, inclusive os inválidos, para os nomes).
    pub(crate) items: Vec<InventoryItem>,
    /// Lado e hash do arquivo de cada metafile válido.
    metafiles: HashMap<String, (Side, Option<String>)>,
}

/// Lê o pack (com a trava de leitura, solta antes de qualquer rede).
pub(crate) async fn read_snapshot(
    state: &AppState,
    pack_id: PackId,
    operation: Option<&OperationHandle>,
) -> Result<PackSnapshot, AppError> {
    let _lock = match operation {
        Some(handle) => state.locks.read_for(pack_id, handle).await,
        None => state.locks.read(pack_id).await,
    };
    let root = pack_root(state, pack_id)?;
    // Sem o cliente do Modrinth: só o disco (nomes, versões e lados vêm dos metafiles).
    let inventory = warden_project::inventory::inventory(&root, None)
        .await
        .map_err(|error| AppError::from_domain(&error))?;
    let read_root = root.clone();
    tokio::task::spawn_blocking(move || snapshot_of(read_root, inventory.items))
        .await
        .map_err(|error| AppError::internal(format!("ler o pack para o diagnóstico: {error}")))?
}

fn snapshot_of(root: PathBuf, items: Vec<InventoryItem>) -> Result<PackSnapshot, AppError> {
    let domain = |error: warden_project::Error| AppError::from_domain(&error);
    let meta = warden_project::meta::read_meta(&root).map_err(domain)?;
    let read = warden_packwiz::read_pack(&root).map_err(|error| AppError::from_domain(&error))?;
    let index_file = read.pack.value.index.file.clone();
    let index_dir = Path::new(&index_file)
        .parent()
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .filter(|parent| !parent.is_empty());
    let metafiles = read
        .metafiles
        .iter()
        .filter_map(|entry| {
            let parsed = entry.result.as_ref().ok()?;
            let path = match &index_dir {
                Some(dir) => format!("{dir}/{}", entry.path),
                None => entry.path.clone(),
            };
            let download = &parsed.value.download;
            let hash = (!download.hash.is_empty())
                .then(|| format!("{}:{}", download.hash_format, download.hash.to_lowercase()));
            Some((path, (parsed.value.side.clone(), hash)))
        })
        .collect();
    let pack_hash = crate::test_session::pack_tree_hash(&root, &index_file)
        .inspect_err(|error| tracing::warn!(%error, "resumo do pack não calculado"))
        .ok();
    Ok(PackSnapshot {
        root,
        pack_hash,
        minecraft: meta.minecraft,
        loader: meta.loader.as_deref().and_then(diagnostics_loader),
        loader_version: meta.loader_version,
        items,
        metafiles,
    })
}

/// O loader do diagnóstico pela chave do `pack.toml`.
pub(crate) fn diagnostics_loader(key: &str) -> Option<Loader> {
    match key {
        "fabric" => Some(Loader::Fabric),
        "quilt" => Some(Loader::Quilt),
        "forge" => Some(Loader::Forge),
        "neoforge" => Some(Loader::NeoForge),
        _ => None,
    }
}

fn java_loader(loader: Loader) -> LoaderKind {
    match loader {
        Loader::Fabric => LoaderKind::Fabric,
        Loader::Quilt => LoaderKind::Quilt,
        Loader::Forge => LoaderKind::Forge,
        Loader::NeoForge => LoaderKind::NeoForge,
    }
}

/// Itens que entram no diagnóstico: mods, resource packs e shaders com o arquivo legível.
fn checked(item: &InventoryItem) -> bool {
    item.state == ItemState::Ok
        && matches!(
            item.kind,
            ItemKind::Mod | ItemKind::ResourcePack | ItemKind::Shader
        )
}

/// A entrada montada e o que ela ajuda a mostrar.
pub(crate) struct Built {
    /// A entrada das regras.
    pub(crate) input: Input,
    /// Mod id (dos jars lidos) → caminho do item.
    pub(crate) mod_paths: HashMap<String, String>,
    /// Mod id → nome declarado no jar.
    pub(crate) mod_names: HashMap<String, String>,
}

impl Built {
    /// A passagem que valeu (a completa vira rápida quando falta o jar de algum mod).
    pub(crate) fn pass(&self) -> Pass {
        self.input.pass
    }
}

/// Monta a entrada de uma passagem. `None` quando o pack não tem loader conhecido ou versão
/// do Minecraft (vanilla: não há regras de mods para conferir).
pub(crate) async fn build_input(
    state: &AppState,
    pack_id: PackId,
    snapshot: &PackSnapshot,
    pass: Pass,
) -> Result<Option<Built>, AppError> {
    let (Some(loader), Some(minecraft)) = (snapshot.loader, snapshot.minecraft.clone()) else {
        return Ok(None);
    };
    let loader_version = snapshot.loader_version.clone().unwrap_or_default();
    let (java_major, minimum_java_major) =
        java_for(state, pack_id, &minecraft, loader, &loader_version)?;

    let candidates: Vec<&InventoryItem> =
        snapshot.items.iter().filter(|item| checked(item)).collect();
    let api = api_metadata(state, &candidates, &snapshot.metafiles).await;

    let mut jars = if pass == Pass::Complete {
        read_pack_jars(state, pack_id, snapshot, &candidates).await?
    } else {
        HashMap::new()
    };

    // Sem o jar de algum mod (pack nunca testado, mod novo que ainda não foi baixado), a
    // passagem completa daria aquele mod como ausente e acusaria dependências que estão no
    // pack: vale a rápida, e a completa fica para a "Verificação final" do Testar.
    let mut pass = pass;
    if pass == Pass::Complete {
        let mods = candidates
            .iter()
            .filter(|item| item.kind == ItemKind::Mod)
            .count();
        if jars.len() < mods {
            tracing::info!(
                mods,
                jars = jars.len(),
                "faltam jars de mods; verificação rápida no lugar da completa"
            );
            pass = Pass::Quick;
        }
    }

    let (mod_paths, mod_names) = mod_maps(loader, &candidates, &jars);

    let items = candidates
        .iter()
        .map(|item| {
            let (side, file_hash) = snapshot
                .metafiles
                .get(&item.path)
                .cloned()
                .unwrap_or((Side::Unset, None));
            PackItem {
                path: item.path.clone(),
                filename: item
                    .file_name
                    .clone()
                    .unwrap_or_else(|| file_name_of(&item.path)),
                side,
                file_hash,
                api: api.get(&item.path).cloned(),
                jar: jars.remove(&item.path),
                accepted_minecraft_versions: Vec::new(),
                resource_pack_format: None,
            }
        })
        .collect();

    Ok(Some(Built {
        input: Input {
            pack: PackContext {
                minecraft,
                minecraft_release_target: None,
                loader,
                loader_version,
                language_loader_versions: HashMap::new(),
                java_major,
                minimum_java_major,
                resource_pack_format: None,
                acceptable_game_versions: Vec::new(),
            },
            items,
            pass,
        },
        mod_paths,
        mod_names,
    }))
}

/// Os jars dos mods: o arquivo guardado no pack ou, para os baixados, a cópia da instância de
/// teste com o nome do metafile (um mod atualizado tem outro nome e fica sem jar).
async fn read_pack_jars(
    state: &AppState,
    pack_id: PackId,
    snapshot: &PackSnapshot,
    candidates: &[&InventoryItem],
) -> Result<HashMap<String, JarMetadata>, AppError> {
    let game_dir = InstanceDirs::for_pack(&state.paths, pack_id).game_dir;
    let paths: Vec<(usize, PathBuf)> = candidates
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind == ItemKind::Mod)
        .filter_map(|(index, item)| {
            let mut places = jar_locations(item, &snapshot.root, &game_dir);
            // O arquivo guardado no pack é a verdade; a cópia da instância pode ser velha.
            places.reverse();
            existing(&places).map(|(path, _, _)| (index, path.clone()))
        })
        .collect();
    let read = tokio::task::spawn_blocking(move || read_jars(paths))
        .await
        .map_err(|error| AppError::internal(format!("ler os jars do pack: {error}")))?;
    Ok(read
        .into_iter()
        .filter_map(|(index, meta)| Some((candidates.get(index)?.path.clone(), meta)))
        .collect())
}

/// Mod id → caminho do item e mod id → nome declarado, pelos jars lidos.
fn mod_maps(
    loader: Loader,
    candidates: &[&InventoryItem],
    jars: &HashMap<String, JarMetadata>,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut mod_paths = HashMap::new();
    let mut mod_names = HashMap::new();
    for item in candidates {
        let Some(jar) = jars.get(&item.path) else {
            continue;
        };
        for meta in jar.effective_mods_for_loader(loader) {
            mod_paths
                .entry(meta.id.clone())
                .or_insert_with(|| item.path.clone());
            if let Some(name) = meta.name.as_ref().filter(|name| !name.trim().is_empty()) {
                mod_names
                    .entry(meta.id.clone())
                    .or_insert_with(|| name.clone());
            }
        }
    }
    (mod_paths, mod_names)
}

fn file_name_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// O Java do teste (a mesma escolha do Testar) e o mais antigo que a faixa do Minecraft aceita.
///
/// Sem decisão possível (versão que a tabela não conhece e sem o JSON da versão), as regras de
/// Java ficam de fora: `u16::MAX` não acusa nenhuma classe e não há mínimo.
fn java_for(
    state: &AppState,
    pack_id: PackId,
    minecraft: &str,
    loader: Loader,
    loader_version: &str,
) -> Result<(u16, Option<u16>), AppError> {
    let settings = crate::commands::pack_meta::read_test_settings(state, pack_id)?;
    let kind = java_loader(loader);
    let request = JavaChoiceRequest {
        user_choice: settings.java.clone(),
        ..JavaChoiceRequest::automatic(
            minecraft,
            kind,
            Some(loader_version).filter(|version| !version.is_empty()),
        )
    };
    let chosen = match state.java.choose(&request) {
        Ok(choice) => u16::try_from(choice.major).unwrap_or(u16::MAX),
        Err(error) => {
            tracing::info!(%error, "Java do teste indefinido; regras de Java ficam de fora");
            return Ok((u16::MAX, None));
        }
    };
    let minimum = MinecraftVersion::parse(minecraft)
        .and_then(|version| {
            state
                .java
                .table()
                .range_for(&version, kind)
                .and_then(|range| range.java.first().copied())
        })
        .and_then(|major| u16::try_from(major).ok());
    Ok((chosen, minimum))
}

/// Metadados das APIs por caminho do item, com o mesmo projeto noutra fonte. Falha de rede: o
/// item fica sem eles.
async fn api_metadata(
    state: &AppState,
    items: &[&InventoryItem],
    metafiles: &HashMap<String, (Side, Option<String>)>,
) -> HashMap<String, ApiMetadata> {
    let mut found = HashMap::new();
    found.extend(modrinth_metadata(state, items).await);
    found.extend(curseforge_metadata(state, items).await);
    link_equivalents(state, items, metafiles, &mut found).await;
    found
}

/// Liga cada item ao mesmo projeto noutra fonte (o `MezzConfig` da CurseForge atende a quem pede
/// o do Modrinth), com a peça de identidade entre fontes (`warden_project::dedup`):
///
/// - pelo SHA-1 que o `.pw.toml` da CurseForge guarda, conferido no Modrinth (uma requisição);
/// - para as dependências que nenhum item atende pela mesma fonte, pelo slug ou nome
///   normalizado do projeto pedido (uma requisição por fonte, com o cache de cada uma).
///
/// A impressão digital da CurseForge (murmur2) pediria os bytes dos jars, que a passagem
/// rápida não tem; o nome cobre o caminho do Modrinth para a CurseForge. Falha de rede: o
/// item fica sem equivalentes.
async fn link_equivalents(
    state: &AppState,
    items: &[&InventoryItem],
    metafiles: &HashMap<String, (Side, Option<String>)>,
    found: &mut HashMap<String, ApiMetadata>,
) {
    let by_path: Vec<(String, String)> = items
        .iter()
        .filter(|item| item.source == ItemSource::Curseforge && found.contains_key(&item.path))
        .filter_map(|item| {
            let (_, hash) = metafiles.get(&item.path)?;
            let sha1 = hash.as_deref()?.strip_prefix("sha1:")?;
            Some((item.path.clone(), sha1.to_ascii_lowercase()))
        })
        .collect();
    if !by_path.is_empty() {
        let hashes: Vec<String> = by_path.iter().map(|(_, sha1)| sha1.clone()).collect();
        match state
            .modrinth
            .version_files(&hashes, warden_modrinth::HashAlgorithm::Sha1, None)
            .await
        {
            Ok(versions) => {
                for (path, sha1) in &by_path {
                    if let Some(version) = versions.get(sha1) {
                        add_equivalent(found, path, Source::Modrinth, &version.project_id);
                    }
                }
            }
            Err(error) => {
                tracing::info!(%error, "hashes da CurseForge não conferidos no Modrinth");
            }
        }
    }
    for target in unmet_targets(state, found).await {
        let wanted = target.identity();
        for item in items {
            let Some(api) = found.get(&item.path) else {
                continue;
            };
            let identity = CrossIdentity {
                source: Some(source_id(api.source)),
                project_id: Some(&api.project_id),
                sha1: None,
                equivalents: &[],
                slug: api
                    .project_slug
                    .as_deref()
                    .or_else(|| warden_packwiz::metafile_stem(&item.path))
                    .unwrap_or_default(),
                title: &item.name,
            };
            if same_across(&wanted, &identity).is_some() {
                add_equivalent(found, &item.path, target.source, &target.id);
            }
        }
    }
}

/// Um projeto pedido por algum item (obrigatório ou incompatível) que nenhum item atende pela
/// mesma fonte, com o slug e o nome que a fonte informa.
struct Target {
    source: Source,
    id: String,
    slug: String,
    title: String,
}

impl Target {
    fn identity(&self) -> CrossIdentity<'_> {
        CrossIdentity {
            source: Some(source_id(self.source)),
            project_id: Some(&self.id),
            sha1: None,
            equivalents: &[],
            slug: &self.slug,
            title: &self.title,
        }
    }
}

async fn unmet_targets(state: &AppState, found: &HashMap<String, ApiMetadata>) -> Vec<Target> {
    let met = |source: Source, id: &str| {
        found.values().any(|api| {
            (api.source == source && api.project_id == id)
                || api
                    .equivalents
                    .iter()
                    .any(|(known, known_id)| *known == source && known_id == id)
        })
    };
    let mut modrinth = Vec::new();
    let mut curseforge = Vec::new();
    for api in found.values() {
        for relation in &api.relations {
            if met(api.source, &relation.project_id) {
                continue;
            }
            match api.source {
                Source::Modrinth => modrinth.push(relation.project_id.clone()),
                Source::CurseForge => curseforge.extend(relation.project_id.parse::<u64>().ok()),
            }
        }
    }
    modrinth.sort();
    modrinth.dedup();
    curseforge.sort_unstable();
    curseforge.dedup();
    let mut targets = Vec::new();
    if !modrinth.is_empty() {
        match state.modrinth.projects(&modrinth, None).await {
            Ok(projects) => targets.extend(projects.into_iter().map(|project| Target {
                source: Source::Modrinth,
                id: project.id,
                slug: project.slug,
                title: project.title,
            })),
            Err(error) => tracing::info!(%error, "dependências do Modrinth sem nome para conferir"),
        }
    }
    if !curseforge.is_empty()
        && let Ok(client) = crate::commands::inventory::curseforge(state)
    {
        match client.projects(&curseforge, None).await {
            Ok(projects) => targets.extend(projects.into_iter().map(|project| Target {
                source: Source::CurseForge,
                id: project.id.to_string(),
                slug: project.slug,
                title: project.name,
            })),
            Err(error) => {
                tracing::info!(%error, "dependências da CurseForge sem nome para conferir");
            }
        }
    }
    targets
}

fn add_equivalent(found: &mut HashMap<String, ApiMetadata>, path: &str, source: Source, id: &str) {
    if let Some(api) = found.get_mut(path)
        && api.source != source
        && !api
            .equivalents
            .iter()
            .any(|(known, known_id)| *known == source && known_id == id)
    {
        api.equivalents.push((source, id.to_owned()));
    }
}

fn source_id(source: Source) -> SourceId {
    match source {
        Source::Modrinth => SourceId::Modrinth,
        Source::CurseForge => SourceId::Curseforge,
    }
}

async fn modrinth_metadata(
    state: &AppState,
    items: &[&InventoryItem],
) -> HashMap<String, ApiMetadata> {
    let wanted: Vec<(&InventoryItem, String)> = items
        .iter()
        .filter(|item| item.source == ItemSource::Modrinth)
        .filter_map(|item| Some((*item, item.source_version_id.clone()?)))
        .collect();
    if wanted.is_empty() {
        return HashMap::new();
    }
    let ids: Vec<String> = wanted.iter().map(|(_, id)| id.clone()).collect();
    let versions = match state.modrinth.versions(&ids, None).await {
        Ok(versions) => versions,
        Err(error) => {
            tracing::warn!(%error, "versões do Modrinth indisponíveis para o diagnóstico");
            return HashMap::new();
        }
    };
    let project_ids: Vec<String> = versions
        .iter()
        .map(|version| version.project_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let projects: HashMap<String, warden_modrinth::Project> =
        match state.modrinth.projects(&project_ids, None).await {
            Ok(projects) => projects
                .into_iter()
                .map(|project| (project.id.clone(), project))
                .collect(),
            Err(error) => {
                tracing::warn!(%error, "projetos do Modrinth indisponíveis para o diagnóstico");
                HashMap::new()
            }
        };
    let by_id: HashMap<&str, &warden_modrinth::Version> = versions
        .iter()
        .map(|version| (version.id.as_str(), version))
        .collect();
    wanted
        .into_iter()
        .filter_map(|(item, id)| {
            let version = by_id.get(id.as_str())?;
            let project = projects.get(&version.project_id);
            Some((item.path.clone(), modrinth_api(version, project)))
        })
        .collect()
}

fn side_supported(support: warden_modrinth::SideSupport) -> Option<bool> {
    match support {
        warden_modrinth::SideSupport::Required | warden_modrinth::SideSupport::Optional => {
            Some(true)
        }
        warden_modrinth::SideSupport::Unsupported => Some(false),
        warden_modrinth::SideSupport::Unknown => None,
    }
}

fn modrinth_api(
    version: &warden_modrinth::Version,
    project: Option<&warden_modrinth::Project>,
) -> ApiMetadata {
    use warden_modrinth::{DependencyType, ProjectStatus, VersionType};
    let relations = version
        .dependencies
        .iter()
        .filter_map(|dependency| {
            let kind = match dependency.dependency_type {
                DependencyType::Required => ApiRelationKind::Required,
                DependencyType::Incompatible => ApiRelationKind::Incompatible,
                _ => return None,
            };
            Some(ApiRelation {
                project_id: dependency.project_id.clone()?,
                kind,
                field: "dependencies".into(),
            })
        })
        .collect();
    ApiMetadata {
        source: Source::Modrinth,
        url: format!("https://api.modrinth.com/v2/version/{}", version.id),
        project_id: version.project_id.clone(),
        project_slug: project.map(|project| project.slug.clone()),
        game_versions: version.game_versions.clone(),
        loaders: version
            .loaders
            .iter()
            .filter_map(|loader| diagnostics_loader(loader))
            .collect(),
        release: match version.version_type {
            VersionType::Beta => ReleaseChannel::Beta,
            VersionType::Alpha => ReleaseChannel::Alpha,
            _ => ReleaseChannel::Release,
        },
        relations,
        archived: project.is_some_and(|project| project.status == ProjectStatus::Archived),
        download_blocked: false,
        client_side: project.and_then(|project| side_supported(project.client_side)),
        server_side: project.and_then(|project| side_supported(project.server_side)),
        equivalents: Vec::new(),
    }
}

async fn curseforge_metadata(
    state: &AppState,
    items: &[&InventoryItem],
) -> HashMap<String, ApiMetadata> {
    let wanted: Vec<(&InventoryItem, u64)> = items
        .iter()
        .filter(|item| item.source == ItemSource::Curseforge)
        .filter_map(|item| Some((*item, item.source_version_id.as_deref()?.parse().ok()?)))
        .collect();
    if wanted.is_empty() {
        return HashMap::new();
    }
    let client = match crate::commands::inventory::curseforge(state) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(code = %error.code, "CurseForge indisponível para o diagnóstico");
            return HashMap::new();
        }
    };
    let ids: Vec<u64> = wanted.iter().map(|(_, id)| *id).collect();
    let files = match client.files_by_id(&ids, None).await {
        Ok(files) => files,
        Err(error) => {
            // Sem chave da CurseForge, também cai aqui: o item fica sem os metadados.
            tracing::info!(%error, "arquivos da CurseForge indisponíveis para o diagnóstico");
            return HashMap::new();
        }
    };
    // O slug do projeto, para achar o mesmo mod vindo do Modrinth.
    let mut mod_ids: Vec<u64> = files.iter().map(|file| file.mod_id).collect();
    mod_ids.sort_unstable();
    mod_ids.dedup();
    let projects: HashMap<u64, warden_curseforge::Mod> = match client.projects(&mod_ids, None).await
    {
        Ok(projects) => projects
            .into_iter()
            .map(|project| (project.id, project))
            .collect(),
        Err(error) => {
            tracing::info!(%error, "projetos da CurseForge indisponíveis para o diagnóstico");
            HashMap::new()
        }
    };
    let by_id: HashMap<u64, &warden_curseforge::File> =
        files.iter().map(|file| (file.id, file)).collect();
    wanted
        .into_iter()
        .filter_map(|(item, id)| {
            let file = by_id.get(&id)?;
            Some((
                item.path.clone(),
                curseforge_api(file, projects.get(&file.mod_id)),
            ))
        })
        .collect()
}

fn curseforge_api(
    file: &warden_curseforge::File,
    project: Option<&warden_curseforge::Mod>,
) -> ApiMetadata {
    use warden_curseforge::{RelationType, ReleaseType};
    let mut loaders = Vec::new();
    let mut game_versions = Vec::new();
    for value in &file.game_versions {
        match value.to_ascii_lowercase().as_str() {
            "forge" => loaders.push(Loader::Forge),
            "fabric" => loaders.push(Loader::Fabric),
            "neoforge" => loaders.push(Loader::NeoForge),
            "quilt" => loaders.push(Loader::Quilt),
            text if text.starts_with(|c: char| c.is_ascii_digit()) => {
                game_versions.push(value.clone());
            }
            _ => {}
        }
    }
    let relations = file
        .dependencies
        .iter()
        .filter_map(|dependency| {
            let kind = match dependency.relation_type {
                RelationType::RequiredDependency => ApiRelationKind::Required,
                RelationType::Incompatible => ApiRelationKind::Incompatible,
                _ => return None,
            };
            Some(ApiRelation {
                project_id: dependency.mod_id.to_string(),
                kind,
                field: "dependencies".into(),
            })
        })
        .collect();
    ApiMetadata {
        source: Source::CurseForge,
        url: format!("https://api.curseforge.com/v1/mods/{}/files/{}", file.mod_id, file.id),
        project_id: file.mod_id.to_string(),
        project_slug: project
            .map(|project| project.slug.clone())
            .filter(|slug| !slug.is_empty()),
        game_versions,
        loaders,
        release: match file.release_type {
            Some(ReleaseType::Beta) => ReleaseChannel::Beta,
            Some(ReleaseType::Alpha) => ReleaseChannel::Alpha,
            _ => ReleaseChannel::Release,
        },
        relations,
        archived: false,
        download_blocked: file.download_url.is_none(),
        client_side: None,
        server_side: None,
        equivalents: Vec::new(),
    }
}

/// Entrada mínima para montar o índice de pacotes e configs de mixin (D-05) da instância: só
/// os caminhos e os nomes dos arquivos dos mods; o índice lê os jars (com cache por hash).
pub(crate) fn index_input(snapshot: &PackSnapshot) -> Option<Input> {
    let loader = snapshot.loader?;
    let items = snapshot
        .items
        .iter()
        .filter(|item| checked(item) && item.kind == ItemKind::Mod)
        .map(|item| PackItem {
            path: item.path.clone(),
            filename: item
                .file_name
                .clone()
                .unwrap_or_else(|| file_name_of(&item.path)),
            side: Side::Unset,
            file_hash: None,
            api: None,
            jar: None,
            accepted_minecraft_versions: Vec::new(),
            resource_pack_format: None,
        })
        .collect();
    Some(Input {
        pack: PackContext {
            minecraft: snapshot.minecraft.clone().unwrap_or_default(),
            minecraft_release_target: None,
            loader,
            loader_version: snapshot.loader_version.clone().unwrap_or_default(),
            language_loader_versions: HashMap::new(),
            java_major: u16::MAX,
            minimum_java_major: None,
            resource_pack_format: None,
            acceptable_game_versions: Vec::new(),
        },
        items,
        pass: Pass::Complete,
    })
}
