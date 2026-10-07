//! Comandos do domínio `graph` (ARCHITECTURE §9.6; SPEC T06 e T07; D-07).
//!
//! - `graph_dependents`: o que para de funcionar se os itens saírem (fecho transitivo) mais as
//!   relações diretas "Depende de" e "Usado por" (bloco dos detalhes do item e, com a P1-15, o
//!   diálogo de remover).
//! - `graph_why_in_pack`: "Por que está no pack" (CA-T07-03).
//! - `graph_orphans`: bibliotecas que nenhum item mantido usa mais.
//!
//! Finos: montam o grafo da `warden-diagnostics` com o que o app tem (o inventário, os jars da
//! instância de teste e, para os itens sem jar, as dependências obrigatórias do Modrinth) e
//! chamam as consultas. O grafo fica em memória por pack e é refeito quando o inventário, algum
//! jar ou as arestas inferidas mudam (ou passados cinco minutos, para a rede ser reconsultada).
//! Só leitura: nada aqui altera o pack.
//!
//! Hoje o app ainda não guarda o histórico de adições (V-02 e P1-09 vão guardar), então a regra
//! é a da R5A sem histórico: o que nenhum item exige foi escolha do usuário, exceto as
//! bibliotecas conhecidas.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use tauri::State;
use warden_core::PackId;
use warden_diagnostics::graph::{
    DependentsReport, Graph, GraphInput, GraphItem, InferredStore, Orphan, UnknownItem, WhyReport,
};
use warden_instance::InstanceDirs;
use warden_jarmeta::{JarMetadata, Limits, Loader};
use warden_project::ProjectErrorCode;
use warden_project::inventory::{Inventory, InventoryItem, ItemKind, ItemSource, ItemState};

use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::state::AppState;

/// Quanto tempo um grafo montado vale sem mudança nos arquivos.
const CACHE_TTL: Duration = Duration::from_secs(300);

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// O que acontece se estes itens saírem do pack, e as relações diretas deles.
///
/// `paths`: caminhos dos itens na lista (um ou vários). Erro `ITEM_NOT_FOUND` se algum não
/// está no inventário.
#[tauri::command]
#[specta::specta]
pub(crate) async fn graph_dependents(
    state: State<'_, AppState>,
    pack_id: PackId,
    paths: Vec<String>,
) -> Result<DependentsReport, AppError> {
    dependents_impl(&state, pack_id, &paths).await
}

/// Por que o item está no pack: "Você adicionou" ou a cadeia até um item do usuário.
#[tauri::command]
#[specta::specta]
pub(crate) async fn graph_why_in_pack(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<WhyReport, AppError> {
    why_impl(&state, pack_id, &path).await
}

/// Bibliotecas (e itens) que nenhum item mantido usa mais.
#[tauri::command]
#[specta::specta]
pub(crate) async fn graph_orphans(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Vec<Orphan>, AppError> {
    orphans_impl(&state, pack_id).await
}

async fn dependents_impl(
    state: &AppState,
    pack_id: PackId,
    paths: &[String],
) -> Result<DependentsReport, AppError> {
    if paths.is_empty() {
        return Err(AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "paths"));
    }
    let graph = pack_graph(state, pack_id).await?;
    graph.dependents(paths).map_err(unknown)
}

async fn why_impl(state: &AppState, pack_id: PackId, path: &str) -> Result<WhyReport, AppError> {
    let graph = pack_graph(state, pack_id).await?;
    graph.why_in_pack(path).map_err(unknown)
}

async fn orphans_impl(state: &AppState, pack_id: PackId) -> Result<Vec<Orphan>, AppError> {
    Ok(pack_graph(state, pack_id).await?.orphans())
}

fn unknown(error: UnknownItem) -> AppError {
    AppError::new(ProjectErrorCode::ItemNotFound).with_param("path", error.0)
}

/// Um item do inventário que entra no grafo.
struct Candidate {
    path: String,
    key: String,
    name: String,
    /// Onde procurar o jar, em ordem: instância de teste e, para arquivo do próprio pack, a
    /// pasta do pack.
    jars: Vec<PathBuf>,
    /// Chave do item no Modrinth (versão instalada) para o cache de metadados.
    source_version: Option<String>,
}

/// Mods do inventário (resource packs, shaders e arquivos soltos não têm dependências de mod).
fn candidates(inventory: &Inventory, root: &Path, game_dir: &Path) -> Vec<Candidate> {
    inventory
        .items
        .iter()
        .filter(|item| item.kind == ItemKind::Mod && item.state == ItemState::Ok)
        .map(|item| Candidate {
            path: item.path.clone(),
            key: item.key.clone(),
            name: item.name.clone(),
            jars: jar_locations(item, root, game_dir),
            source_version: item.source_version_id.clone(),
        })
        .collect()
}

/// Onde está o jar do item: `<instância>/minecraft/<pasta do metafile>/<arquivo>`; o arquivo
/// guardado no próprio pack também vale direto da pasta do pack.
fn jar_locations(item: &InventoryItem, root: &Path, game_dir: &Path) -> Vec<PathBuf> {
    let mut places = Vec::new();
    let Some(file_name) = item.file_name.as_deref() else {
        return places;
    };
    if !is_jar(file_name) {
        return places;
    }
    let folder = item.path.rsplit_once('/').map_or("", |(folder, _)| folder);
    let relative = if folder.is_empty() {
        file_name.to_owned()
    } else {
        format!("{folder}/{file_name}")
    };
    if let Ok(path) = warden_core::resolve_inside(game_dir, &relative) {
        places.push(path);
    }
    if item.source == ItemSource::Local
        && let Ok(path) = warden_core::resolve_inside(root, &item.path)
    {
        places.push(path);
    }
    places
}

fn is_jar(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
}

fn loader_of(key: Option<&str>) -> Loader {
    match key {
        Some("fabric") => Loader::Fabric,
        Some("quilt") => Loader::Quilt,
        Some("neoforge") => Loader::NeoForge,
        // Forge, LiteLoader ou pack sem loader (sem mods): o Forge lê `mods.toml`, `mcmod.info`
        // e, com o Connector, `fabric.mod.json`.
        _ => Loader::Forge,
    }
}

/// Primeiro local que existe como arquivo, com tamanho e data (para a impressão digital).
fn existing(places: &[PathBuf]) -> Option<(&PathBuf, u64, u128)> {
    places.iter().find_map(|path| {
        let meta = fs::metadata(path).ok().filter(fs::Metadata::is_file)?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_nanos());
        Some((path, meta.len(), modified))
    })
}

/// Lê os jars em paralelo (vários mods por thread), tolerando os ilegíveis.
fn read_jars(paths: Vec<(usize, PathBuf)>) -> Vec<(usize, JarMetadata)> {
    if paths.is_empty() {
        return Vec::new();
    }
    let threads = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .min(8);
    let chunk = paths.len().div_ceil(threads).max(1);
    let limits = Limits::default();
    std::thread::scope(|scope| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|part| {
                let limits = &limits;
                scope.spawn(move || {
                    part.iter()
                        .filter_map(|(index, path)| {
                            match warden_jarmeta::read_jar_file(path, limits) {
                                Ok(meta) => Some((*index, meta)),
                                Err(error) => {
                                    tracing::warn!(%error, ?path, "jar ilegível; fica fora do grafo de dependências");
                                    None
                                }
                            }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    })
}

/// Arquivo das arestas inferidas do pack (`<dados>/graph/<pack>.json`).
fn inferred_file(state: &AppState, pack_id: PackId) -> PathBuf {
    state
        .paths
        .data_dir()
        .join("graph")
        .join(format!("{pack_id}.json"))
}

struct Cached {
    fingerprint: String,
    built: Instant,
    graph: Arc<Graph>,
}

fn cache() -> &'static Mutex<HashMap<PackId, Cached>> {
    static CACHE: OnceLock<Mutex<HashMap<PackId, Cached>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// Impressão digital do que alimenta o grafo: itens, jars (tamanho e data) e o arquivo das
/// arestas inferidas. Mudou algo, o grafo é refeito.
fn fingerprint(loader: Option<&str>, candidates: &[Candidate], inferred_path: &Path) -> String {
    let mut text = format!("{loader:?}\n");
    for candidate in candidates {
        let _ = write!(
            text,
            "{}|{}|{:?}|",
            candidate.path, candidate.key, candidate.source_version
        );
        match existing(&candidate.jars) {
            Some((_, len, modified)) => {
                let _ = writeln!(text, "{len}:{modified}");
            }
            None => text.push_str("-\n"),
        }
    }
    match fs::metadata(inferred_path) {
        Ok(meta) => {
            let _ = write!(text, "inferred:{}:{:?}", meta.len(), meta.modified().ok());
        }
        Err(_) => text.push_str("inferred:-"),
    }
    text
}

/// Lê os jars e junta, para os itens sem jar, as dependências obrigatórias da API.
async fn load_items(
    state: &AppState,
    inventory: &Inventory,
    candidates: &[Candidate],
) -> Result<Vec<GraphItem>, AppError> {
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

    // Itens sem jar usam as dependências obrigatórias do Modrinth (cache primeiro).
    let key_to_path: HashMap<&str, &str> = candidates
        .iter()
        .map(|candidate| (candidate.key.as_str(), candidate.path.as_str()))
        .collect();
    let api: HashMap<String, Vec<String>> =
        if (0..candidates.len()).any(|index| !jar_of.contains_key(&index)) {
            warden_project::remove::direct_dependencies(inventory, Some(&state.modrinth))
                .await
                .into_iter()
                .filter_map(|(key, needs)| {
                    let path = (*key_to_path.get(key.as_str())?).to_owned();
                    let needs = needs
                        .iter()
                        .filter_map(|need| key_to_path.get(need.as_str()).map(|p| (*p).to_owned()))
                        .collect();
                    Some((path, needs))
                })
                .collect()
        } else {
            HashMap::new()
        };

    let items: Vec<GraphItem> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let jar = jar_of.remove(&index);
            // As relações da API só valem para o que não tem jar: o jar é mais exato.
            let api_required = if jar.is_some() {
                Vec::new()
            } else {
                api.get(&candidate.path).cloned().unwrap_or_default()
            };
            GraphItem {
                path: candidate.path.clone(),
                name: candidate.name.clone(),
                jar,
                api_required,
            }
        })
        .collect();
    Ok(items)
}

/// O grafo do pack: do cache, se nada mudou, ou montado agora.
async fn pack_graph(state: &AppState, pack_id: PackId) -> Result<Arc<Graph>, AppError> {
    let (root, inventory, loader) = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(state, pack_id)?;
        let inventory = warden_project::inventory::inventory(&root, Some(&state.modrinth))
            .await
            .map_err(domain)?;
        let loader = warden_project::meta::read_meta(&root)
            .map_err(domain)?
            .loader;
        (root, inventory, loader)
    };
    let game_dir = InstanceDirs::for_pack(&state.paths, pack_id).game_dir;
    let candidates = candidates(&inventory, &root, &game_dir);
    let inferred_path = inferred_file(state, pack_id);

    let fingerprint = fingerprint(loader.as_deref(), &candidates, &inferred_path);
    {
        let guard = cache().lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(hit) = guard.get(&pack_id)
            && hit.fingerprint == fingerprint
            && hit.built.elapsed() < CACHE_TTL
        {
            return Ok(Arc::clone(&hit.graph));
        }
    }

    let items = load_items(state, &inventory, &candidates).await?;

    let inferred = match InferredStore::load(&inferred_path) {
        Ok(store) => store.edges,
        Err(error) => {
            tracing::warn!(%error, "arestas inferidas ilegíveis; seguindo sem elas");
            Vec::new()
        }
    };
    let graph = Arc::new(Graph::build(&GraphInput {
        loader: loader_of(loader.as_deref()),
        items,
        additions: None,
        inferred,
    }));
    cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            pack_id,
            Cached {
                fingerprint,
                built: Instant::now(),
                graph: Arc::clone(&graph),
            },
        );
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use warden_diagnostics::graph::{RelationKind, Why};
    use warden_packwiz_cli::Packwiz;

    use super::*;
    use crate::commands::inventory::tests::{packwiz_binary, registered_pack, test_packwiz};
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;

    /// Um jar do Forge com um `mods.toml` que declara `mod_id` e as dependências obrigatórias.
    fn forge_jar(path: &Path, mod_id: &str, requires: &[&str]) {
        let mut toml = format!(
            "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"MIT\"\n[[mods]]\nmodId=\"{mod_id}\"\nversion=\"1.0\"\n"
        );
        for required in requires {
            let _ = write!(
                toml,
                "[[dependencies.{mod_id}]]\nmodId=\"{required}\"\nmandatory=true\nversionRange=\"[1,)\"\nordering=\"NONE\"\nside=\"BOTH\"\n"
            );
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

    async fn setup() -> Option<(tempfile::TempDir, AppState, PackId, Packwiz)> {
        let binary = packwiz_binary()?;
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cli = test_packwiz(&state, binary);
        let (id, _root) = registered_pack(&state, dir.path(), &cli, 3).await;
        // mod-0 exige mod-1, que exige mod-2 (ids dos mods = nomes dos arquivos sem versão).
        let mods = InstanceDirs::for_pack(&state.paths, id)
            .game_dir
            .join("mods");
        forge_jar(&mods.join("mod-0-1.0.0.jar"), "mod0", &["mod1"]);
        forge_jar(&mods.join("mod-1-1.0.0.jar"), "mod1", &["mod2"]);
        forge_jar(&mods.join("mod-2-1.0.0.jar"), "mod2", &[]);
        Some((dir, state, id, cli))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn le_os_jars_da_instancia_e_responde_as_tres_consultas() {
        let Some((_dir, state, id, _cli)) = setup().await else {
            return;
        };
        let report = dependents_impl(&state, id, &["mods/mod-2.pw.toml".to_owned()])
            .await
            .unwrap();
        let affected: Vec<(&str, u32)> = report
            .affected
            .iter()
            .map(|a| (a.item.name.as_str(), a.depth))
            .collect();
        assert_eq!(affected, vec![("Mod 1", 1), ("Mod 0", 2)]);
        assert_eq!(report.used_by[0].kind, RelationKind::Required);

        let why = why_impl(&state, id, "mods/mod-2.pw.toml").await.unwrap();
        let Why::RequiredBy { chains } = why.why else {
            panic!("esperava cadeia");
        };
        let chain: Vec<&str> = chains[0].iter().map(|l| l.item.name.as_str()).collect();
        assert_eq!(chain, vec!["Mod 2", "Mod 1", "Mod 0"]);
        let direct = why_impl(&state, id, "mods/mod-0.pw.toml").await.unwrap();
        assert_eq!(direct.why, Why::NoDependents);

        assert!(orphans_impl(&state, id).await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn arestas_inferidas_entram_e_o_cache_acompanha_a_mudanca() {
        let Some((_dir, state, id, _cli)) = setup().await else {
            return;
        };
        let target = ["mods/mod-0.pw.toml".to_owned()];
        let before = dependents_impl(&state, id, &target).await.unwrap();
        assert!(before.used_by.is_empty());

        let mut store = InferredStore::default();
        store.add(warden_diagnostics::graph::InferredEdge {
            from: "mods/mod-2.pw.toml".into(),
            to: "mods/mod-0.pw.toml".into(),
            source: warden_diagnostics::graph::InferredSource::Bisect,
            note: "a busca do culpado".into(),
        });
        store.save(&inferred_file(&state, id)).unwrap();
        let after = dependents_impl(&state, id, &target).await.unwrap();
        assert_eq!(after.used_by.len(), 1);
        assert_eq!(after.used_by[0].kind, RelationKind::Inferred);
        // O ciclo mod-0 → mod-1 → mod-2 → mod-0 virou um só nó.
        let why = why_impl(&state, id, "mods/mod-0.pw.toml").await.unwrap();
        assert_eq!(why.cycle.len(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn caminho_desconhecido_e_lista_vazia_explicam_o_problema() {
        let Some((_dir, state, id, _cli)) = setup().await else {
            return;
        };
        let missing = dependents_impl(&state, id, &["mods/nao-existe.pw.toml".to_owned()])
            .await
            .unwrap_err();
        assert_eq!(
            missing.code,
            ErrorCode::Project(ProjectErrorCode::ItemNotFound)
        );
        assert_eq!(missing.params["path"], "mods/nao-existe.pw.toml");
        let empty = dependents_impl(&state, id, &[]).await.unwrap_err();
        assert_eq!(empty.params["field"], "paths");
    }

    #[test]
    fn loader_e_locais_do_jar() {
        assert_eq!(loader_of(Some("fabric")), Loader::Fabric);
        assert_eq!(loader_of(Some("neoforge")), Loader::NeoForge);
        assert_eq!(loader_of(Some("quilt")), Loader::Quilt);
        assert_eq!(loader_of(None), Loader::Forge);
        assert!(is_jar("a.JAR"));
        assert!(!is_jar("a.zip"));
    }
}
