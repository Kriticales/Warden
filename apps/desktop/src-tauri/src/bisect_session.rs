//! A busca do culpado no cliente (SPEC T25; ARCHITECTURE §9.7; ADR-0031; ROADMAP D-12).
//!
//! Liga a `warden-bisect` (o algoritmo e o veredito), a `warden-instance` (a instância de cada
//! rodada) e o jogo do Testar (`test_session`):
//!
//! 1. **Um jogo por vez:** a busca ocupa o lugar do jogo ([`crate::test_session::TestSessions`],
//!    `game-state = bisecting`); o cabeçalho mostra "Buscando o culpado: ver progresso".
//! 2. **Base:** o pack inteiro é materializado uma vez em `instances/<pack>/bisect-base/`
//!    (cache de downloads de sempre); o grafo da busca é lido dos jars da base, com as arestas
//!    inferidas do pack (`<dados>/graph/<pack>.json`) e as ferramentas do jogador de
//!    `.warden/project.toml`.
//! 3. **Mundo de teste** ([`world`]): gerado sem mods pelo servidor vanilla da mesma versão,
//!    com semente fixa, na primeira busca do pack (e quando a versão do Minecraft muda), só com
//!    o aceite da EULA; fica como modelo no estado da instância de teste. O mundo do usuário
//!    nunca é usado: foi salvo com todos os mods, e uma rodada sem parte deles travaria
//!    "diferente".
//! 4. **Rodadas:** cada uma recria `instances/<pack>/bisect/minecraft` só com os itens
//!    ligados (links físicos), copia o mundo de teste modelo para `saves/`,
//!    abre o jogo por Quick Play com `fabric.noGui`/`fml.queryResult`, acompanha a saída com o
//!    [`RoundWatcher`] e **encerra o jogo assim que há veredito**. O log de cada rodada fica em
//!    `state/bisect-logs/<busca>/round-<n>/output.log` ("Ver log").
//! 5. **Veredito:** a assinatura da rodada e a da D-06 (`commands::crashes::signature_of`)
//!    contra a do problema; classe de item desligado vira aresta inferida, guardada no grafo do
//!    pack; Java errado para a busca.
//! 6. **Estado:** `state/bisect.json` depois de cada rodada. **Pausar** espera a rodada atual;
//!    "Parar jogo" ou fechar o app no meio de uma rodada pausa sem contar a rodada; **cancelar**
//!    encerra o jogo, apaga as pastas da busca e guarda o resultado parcial. O resultado
//!    terminado fica em `state/bisect-results/` e é ligado ao travamento de origem
//!    ([`searches_for`], Problemas → Travamentos).
//! 7. **Modo assistido:** ao entrar no mundo, a rodada pergunta ("Aconteceu", "Não aconteceu",
//!    "Não deu para testar"); um travamento detectado sozinho marca a rodada sem perguntar.
//!
//! O pack, a instância de teste e os mundos do usuário nunca mudam. Versões sem Quick Play
//! (antes de 1.20) só entram no mundo pelo servidor local (D-13, [`crate::bisect_server`]): com
//! o aceite no pedido ([`BisectRequest::server`]), o objetivo da busca é [`Goal::Server`] e cada
//! rodada abre também o servidor, com uma cópia do mesmo mundo de teste ([`world`]) e as linhas
//! dele no mesmo [`RoundWatcher`]; sem o aceite, "trava ao entrar no mundo" nessas versões dá
//! `NEEDS_SERVER`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use warden_bisect::graph::{from_graph, mod_ids, normalize_id};
use warden_bisect::judge::{PackLookup, judge_failure};
use warden_bisect::{
    Anchor, BisectError, BisectState, Decision, Estimate, Goal, Role, RoundInfo, RoundKind,
    RoundWatcher, Search, SearchOutcome, Symptom, Target, Universe, Verdict, WatchConfig,
};
use warden_core::{AppPaths, CancellationToken, PackId, Progress, ProgressSink as _};
use warden_diagnostics::graph::{
    Graph, GraphInput, GraphItem, InferredEdge, InferredSource, InferredStore,
};
use warden_diagnostics::postcrash::SessionFiles;
use warden_instance::subset::{
    SubsetOptions, SubsetWorld, bisect_dirs, materialize_subset, remove_dir,
};
use warden_instance::{
    InstallSide, InstanceDirs, MaterializeOptions, OnModified, OptionalChoices, OptionalSelection,
    PlayerToolsMode,
};
use warden_jarmeta::Limits;
use warden_jarmeta::index::{JarIndexCache, PackIndex, PackJar, build_pack_index};
use warden_launcher::process::{GameEvent, GameProcess, SpawnOptions};

use crate::bisect_server::{OpenServer, ServerBase, ServerDirs, ServerSignal};
use crate::error::AppError;
use crate::events::{GamePhase, GameState, OperationEvent};
use crate::operations::{OperationHandle, OperationKind};
use crate::server_session::now_ms;
use crate::state::AppState;
use crate::test_session::{BisectGame, LaunchExtra, Notify, read_output_log};

mod world;

/// Tipo da operação da busca (painel Tarefas).
pub(crate) const BISECT_RUN: OperationKind = OperationKind::new("bisect.run");
/// Etapa "Preparando a busca".
const STAGE_PREPARE: &str = "bisect.prepare";
const STAGE_PREPARE_LABEL: &str = "buscaDoCulpado:etapa.preparar";
/// Etapa "Criando o mundo de teste".
const STAGE_WORLD: &str = "bisect.world";
const STAGE_WORLD_LABEL: &str = "buscaDoCulpado:etapa.mundo";
/// Etapa "Rodada da busca".
const STAGE_ROUND: &str = "bisect.round";
const STAGE_ROUND_LABEL: &str = "buscaDoCulpado:etapa.rodada";
/// Relógio do acompanhamento de cada rodada.
const TICK: Duration = Duration::from_millis(250);
/// Arquivo do `--quickPlayPath`, relativo ao `gameDir` da rodada.
const QUICK_PLAY_FILE: &str = "warden-quickplay.json";
/// Pasta dos logs das rodadas, dentro de `state/`.
const LOGS_DIR: &str = "bisect-logs";
/// Só em build de debug (E2E): a estabilidade de cada rodada, em ms.
const STABILITY_ENV: &str = "WARDEN_E2E_BISECT_STABILITY_MS";
/// A estabilidade nos testes do app (o jogo simulado entra no mundo em menos de 1 s).
#[cfg(test)]
const TEST_STABILITY_MS: u64 = 400;
#[cfg(not(test))]
const TEST_STABILITY_MS: u64 = 0;

/// O pedido de uma busca nova.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BisectRequest {
    /// O que acontece no jogo.
    pub(crate) symptom: Symptom,
    /// Hash da assinatura (D-06) do travamento de onde a busca começou, se veio de um.
    #[serde(default)]
    pub(crate) origin_crash: Option<String>,
    /// Ferramentas do jogador que entram mesmo assim (suspeitas).
    #[serde(default)]
    pub(crate) suspects: Vec<String>,
    /// A pessoa aceitou que cada rodada abra o servidor local (D-13): versões sem Quick Play e
    /// problemas do servidor.
    #[serde(default)]
    pub(crate) server: bool,
}

/// A resposta do modo assistido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AssistedAnswer {
    /// "Aconteceu".
    Happened,
    /// "Não aconteceu".
    NotHappened,
    /// "Não deu para testar".
    CouldNotTest,
}

impl AssistedAnswer {
    fn verdict(self) -> Verdict {
        match self {
            Self::Happened => Verdict::UserYes,
            Self::NotHappened => Verdict::UserNo,
            Self::CouldNotTest => Verdict::UserSkip,
        }
    }
}

/// Um item na resposta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BisectItem {
    /// Caminho no pack.
    pub(crate) path: String,
    /// Nome.
    pub(crate) name: String,
}

/// O que o diálogo de configuração mostra antes de começar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BisectPlan {
    /// Quantos itens entram na busca.
    pub(crate) searched: u32,
    /// Sempre ligados (bibliotecas que todos exigem).
    pub(crate) always_on: Vec<BisectItem>,
    /// Desligados durante a busca (ferramentas do jogador).
    pub(crate) disabled: Vec<BisectItem>,
    /// A estimativa (W do último teste do pack, quando houver).
    pub(crate) estimate: Estimate,
    /// Versão do Minecraft.
    pub(crate) minecraft: String,
    /// Loader (`neoforge`, `fabric`…; `None` = vanilla).
    pub(crate) loader: Option<String>,
    /// A versão entra direto no mundo (Quick Play, 1.20+).
    pub(crate) quick_play: bool,
    /// O mundo de teste gerado sem mods já existe para esta versão (sem ele, a busca o gera
    /// antes da primeira rodada, se a EULA estiver aceita).
    pub(crate) world_ready: bool,
}

/// Situação da busca guardada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BisectStatus {
    /// Rodando agora.
    Running,
    /// Pausada (dá para continuar).
    Paused,
    /// Terminou (achou, parou ou foi cancelada).
    Finished,
}

/// Uma rodada terminada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BisectRoundView {
    /// Número.
    pub(crate) number: u32,
    /// Tipo.
    pub(crate) kind: RoundKind,
    /// Repetição (problema intermitente).
    pub(crate) repeat: u32,
    /// Itens ligados.
    pub(crate) enabled_count: u32,
    /// O veredito.
    pub(crate) verdict: Verdict,
    /// Duração.
    #[specta(type = specta_typescript::Number)]
    pub(crate) duration_ms: u64,
    /// O sinal da falha.
    pub(crate) anchor: Option<Anchor>,
    /// Há log gravado.
    pub(crate) has_log: bool,
    /// Há log do servidor local gravado (D-13).
    pub(crate) has_server_log: bool,
    /// A falha veio do servidor local (D-13).
    pub(crate) server_failed: bool,
}

/// A rodada em andamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BisectCurrent {
    /// Número.
    pub(crate) number: u32,
    /// Tipo.
    pub(crate) kind: RoundKind,
    /// Repetição.
    pub(crate) repeat: u32,
    /// Itens ligados.
    pub(crate) enabled_count: u32,
    /// Nomes acrescentados desde o último prefixo que passou.
    pub(crate) added: Vec<String>,
    /// Nomes do conjunto já achado.
    pub(crate) found: Vec<String>,
    /// Modo assistido: esperando a resposta.
    pub(crate) asking: bool,
    /// Começo da rodada (milissegundos desde 1970).
    #[specta(type = specta_typescript::Number)]
    pub(crate) started_at_ms: u64,
}

/// Uma dependência inferida nesta busca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BisectInferred {
    /// O item que usa.
    pub(crate) from: BisectItem,
    /// O item usado sem declarar.
    pub(crate) to: BisectItem,
}

/// A busca guardada, como a tela do teste a mostra.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BisectView {
    /// Identificador da busca.
    pub(crate) id: String,
    /// Situação.
    pub(crate) status: BisectStatus,
    /// Cada rodada abre o servidor local (D-13).
    pub(crate) server: bool,
    /// O que o usuário descreveu.
    pub(crate) symptom: Symptom,
    /// Travamento de origem (hash da assinatura).
    pub(crate) origin_crash: Option<String>,
    /// Rodadas feitas.
    pub(crate) rounds: Vec<BisectRoundView>,
    /// A rodada em andamento.
    pub(crate) current: Option<BisectCurrent>,
    /// Suspeitos agora.
    pub(crate) suspects: Vec<BisectItem>,
    /// Quantos itens entram na busca.
    pub(crate) searched: u32,
    /// Sempre ligados.
    pub(crate) always_on: Vec<BisectItem>,
    /// Desligados.
    pub(crate) disabled: Vec<BisectItem>,
    /// Rodadas no total (feitas + as que faltam), com os minutos que faltam.
    pub(crate) estimate: Estimate,
    /// Repetições de cada "passou" (problema intermitente).
    pub(crate) repetitions: u32,
    /// Tempo gasto em rodadas.
    #[specta(type = specta_typescript::Number)]
    pub(crate) elapsed_ms: u64,
    /// Começo.
    #[specta(type = specta_typescript::Number)]
    pub(crate) started_at_ms: u64,
    /// Fim.
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) finished_at_ms: Option<u64>,
    /// Como terminou.
    pub(crate) outcome: Option<SearchOutcome>,
    /// Itens do resultado com nome (culpados, ou os que evitam o problema, ou os suspeitos).
    pub(crate) outcome_items: Vec<BisectItem>,
    /// Dependências inferidas ("o mod X usa Y sem declarar a dependência").
    pub(crate) inferred: Vec<BisectInferred>,
    /// Conjuntos achados antes ("continuar sem").
    pub(crate) earlier: Vec<Vec<BisectItem>>,
    /// O Warden está criando o mundo de teste agora (antes da primeira rodada).
    pub(crate) creating_world: bool,
}

/// Para onde vão os eventos da busca.
#[derive(Clone)]
pub(crate) struct BisectSink {
    /// O canal do comando.
    pub(crate) events: Arc<dyn Fn(OperationEvent) + Send + Sync>,
    /// O evento `game-state`.
    pub(crate) game_state: Notify,
}

/// O que fazer ao começar.
#[derive(Debug, Clone)]
pub(crate) enum RunMode {
    /// Busca nova.
    Start(BisectRequest),
    /// Continuar a pausada.
    Resume,
    /// "Continuar a busca sem" o conjunto achado (outro problema).
    ContinueWithout,
}

/// O controle da busca que está rodando (uma por vez, como o jogo).
struct Control {
    pack_id: PackId,
    pause: AtomicBool,
    cancel: AtomicBool,
    token: CancellationToken,
    answer: Mutex<Option<oneshot::Sender<AssistedAnswer>>>,
    current: Mutex<Option<BisectCurrent>>,
    creating_world: AtomicBool,
}

fn running() -> &'static Mutex<Option<Arc<Control>>> {
    static RUNNING: OnceLock<Mutex<Option<Arc<Control>>>> = OnceLock::new();
    RUNNING.get_or_init(Mutex::default)
}

fn control_of(pack_id: PackId) -> Option<Arc<Control>> {
    running()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .filter(|control| control.pack_id == pack_id)
        .cloned()
}

/// Tira o controle quando a busca termina, de qualquer jeito.
struct ControlGuard;

impl Drop for ControlGuard {
    fn drop(&mut self) {
        running()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
}

fn domain(error: &BisectError) -> AppError {
    AppError::from_domain(error)
}

/// Roda trabalho de disco fora da thread do IPC.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("tarefa da busca interrompida: {error}")))?
}

/// Pastas da busca de um pack.
struct Dirs {
    /// `instances/<pack>/state` (da instância de teste: o `bisect.json`, os logs e o mundo de
    /// teste modelo).
    state_dir: PathBuf,
    /// A instância de teste (só lida: os opcionais escolhidos e os jars do diálogo).
    test: InstanceDirs,
    /// O mundo de teste gerado sem mods.
    world: world::WorldDirs,
    /// A base (o pack inteiro).
    base: InstanceDirs,
    /// A instância da rodada.
    round: InstanceDirs,
    /// `instances/<pack>/bisect` (apagada no fim).
    round_root: PathBuf,
    /// O servidor local da busca (D-13), dentro da base e da rodada.
    server: ServerDirs,
}

impl Dirs {
    fn of(paths: &AppPaths, pack_id: PackId) -> Self {
        let test = InstanceDirs::for_pack(paths, pack_id);
        let root = paths.instance_dir(pack_id);
        let (base, round_game_dir) = bisect_dirs(&root);
        let round_root = round_game_dir
            .parent()
            .map_or_else(|| root.join("bisect"), Path::to_path_buf);
        Self {
            state_dir: test.state_dir.clone(),
            world: world::WorldDirs::of(paths, pack_id),
            round: InstanceDirs {
                game_dir: round_game_dir,
                state_dir: round_root.join("state"),
            },
            test,
            base,
            round_root,
            server: ServerDirs::of(&root),
        }
    }

    fn log_dir(&self, search: &str, number: u32) -> PathBuf {
        self.state_dir
            .join(LOGS_DIR)
            .join(search)
            .join(format!("round-{number}"))
    }

    /// Apaga as pastas da busca (fim ou cancelamento). A instância de teste fica.
    fn remove_all(&self) -> Result<(), AppError> {
        remove_dir(&self.round_root).map_err(|error| AppError::from_domain(&error))?;
        let base_root = self
            .base
            .game_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        remove_dir(&base_root).map_err(|error| AppError::from_domain(&error))
    }
}

/// O que a busca lê do pack.
struct PackFacts {
    minecraft: String,
    loader: Option<String>,
}

fn pack_facts(root: &Path) -> Result<PackFacts, AppError> {
    let read = warden_packwiz::read_pack(root).map_err(|error| AppError::from_domain(&error))?;
    let requirement = warden_instance::game_requirement(&read.pack.value)
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(PackFacts {
        minecraft: requirement.minecraft.clone(),
        loader: requirement
            .loader
            .as_ref()
            .map(|loader| loader.loader.clone()),
    })
}

/// A entrada do grafo com os jars de `game_dir` (a base, ou a instância de teste no diálogo)
/// e as arestas inferidas do pack.
async fn graph_input(
    state: &AppState,
    pack_id: PackId,
    root: &Path,
    game_dir: &Path,
) -> Result<GraphInput, AppError> {
    let (inventory, loader) = {
        let _lock = state.locks.read(pack_id).await;
        let inventory = warden_project::inventory::inventory(root, Some(&state.modrinth))
            .await
            .map_err(|error| AppError::from_domain(&error))?;
        let loader = warden_project::meta::read_meta(root)
            .map_err(|error| AppError::from_domain(&error))?
            .loader;
        (inventory, loader)
    };
    let candidates = crate::commands::graph::candidates(&inventory, root, game_dir);
    let jar_paths: Vec<(usize, PathBuf)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            crate::commands::graph::existing(&candidate.jars)
                .map(|(path, _, _)| (index, path.clone()))
        })
        .collect();
    let jars = blocking(move || Ok(crate::commands::graph::read_jars(jar_paths))).await?;
    let mut jar_of: BTreeMap<usize, warden_jarmeta::JarMetadata> = jars.into_iter().collect();
    let items = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| GraphItem {
            path: candidate.path.clone(),
            name: candidate.name.clone(),
            jar: jar_of.remove(&index),
            api_required: Vec::new(),
        })
        .collect();
    // Sem o erro no log: ele traz o caminho inteiro, com a pasta do usuário.
    let inferred = InferredStore::load(&crate::commands::graph::inferred_file(state, pack_id))
        .map_or_else(
            |_| {
                tracing::warn!("arestas inferidas ilegíveis; seguindo sem elas");
                Vec::new()
            },
            |store| store.edges,
        );
    Ok(GraphInput {
        loader: crate::commands::graph::loader_of(loader.as_deref()),
        items,
        additions: None,
        inferred,
    })
}

/// Os itens e a ordem da busca.
fn universe_of(input: &GraphInput, root: &Path, suspects: &[String]) -> Universe {
    let graph = Graph::build(input);
    let tools: Vec<String> = warden_instance::player_tools::excluded_paths(
        root,
        &PlayerToolsMode::Culprit {
            suspects: BTreeSet::new(),
        },
    )
    .into_iter()
    .collect();
    let (items, requirements) = from_graph(&graph, input, &tools);
    Universe::new(items, &requirements, suspects)
}

/// Onde achar os itens pelo id do mod e pela classe (jars da base).
struct AppLookup {
    mods: BTreeMap<String, Vec<String>>,
    index: PackIndex,
}

impl PackLookup for AppLookup {
    fn item_of_mod(&self, mod_id: &str) -> Option<String> {
        self.mods
            .get(&normalize_id(mod_id))
            .and_then(|items| items.first().cloned())
    }

    fn item_of_class(&self, class: &str) -> Option<String> {
        self.index
            .class_owner(class)
            .and_then(|ownership| ownership.owner())
            .map(|owner| owner.item.to_owned())
    }
}

fn lookup_of(input: &GraphInput, jars: &[PackJar], cache_dir: &Path) -> AppLookup {
    let cache = JarIndexCache::in_cache_dir(cache_dir);
    let build = build_pack_index(jars, Some(input.loader), Some(&cache), &Limits::default());
    AppLookup {
        mods: mod_ids(input),
        index: build.index,
    }
}

/// Os jars da base para o índice de classes.
fn base_jars(input: &GraphInput, base: &InstanceDirs) -> Vec<PackJar> {
    let Ok(manifest) = warden_instance::Manifest::load(&base.state_dir) else {
        return Vec::new();
    };
    let items: BTreeSet<&str> = input.items.iter().map(|item| item.path.as_str()).collect();
    manifest
        .files
        .iter()
        .filter(|(file, entry)| {
            Path::new(file)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
                && items.contains(entry.index_path.as_str())
        })
        .map(|(file, entry)| PackJar::new(entry.index_path.clone(), base.game_dir.join(file)))
        .collect()
}

/// O W do último teste do pack (tempo até entrar no mundo, senão até carregar), para a
/// estimativa antes da rodada 0.
fn last_world_ms(state_dir: &Path) -> Option<u64> {
    let sessions = warden_launcher::session::list_sessions(&state_dir.join("sessions")).ok()?;
    sessions.iter().find_map(|session| {
        // Os tempos ficam no nível do campo `perf` (`SessionPerf` com `times` achatado).
        let perf = session.record.as_ref()?.extra.get("perf")?;
        perf.get("worldMs")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| perf.get("loadedMs").and_then(serde_json::Value::as_u64))
    })
}

fn item_of(universe: &Universe, path: &str) -> BisectItem {
    let name = universe.index_of(path).map_or_else(
        || path.to_owned(),
        |index| universe.items()[index].name.clone(),
    );
    BisectItem {
        path: path.to_owned(),
        name,
    }
}

fn items_with_role(universe: &Universe, role: Role) -> Vec<BisectItem> {
    universe
        .with_role(role)
        .into_iter()
        .map(|index| item_of(universe, &universe.items()[index].path))
        .collect()
}

/// O que mostrar no diálogo "Encontrar o mod culpado".
pub(crate) async fn plan(state: &AppState, pack_id: PackId) -> Result<BisectPlan, AppError> {
    state
        .packs
        .get(pack_id)
        .map_err(|error| AppError::from_domain(&error))?;
    let root = crate::commands::inventory::pack_root(state, pack_id)?;
    let dirs = Dirs::of(&state.paths, pack_id);
    let facts = {
        let root = root.clone();
        blocking(move || pack_facts(&root)).await?
    };
    let input = graph_input(state, pack_id, &root, &dirs.test.game_dir).await?;
    let universe = universe_of(&input, &root, &[]);
    let searched = u32::try_from(universe.with_role(Role::Searched).len()).unwrap_or(u32::MAX);
    let quick_play =
        warden_launcher::quick_play::quick_play_support(&facts.minecraft).is_supported();
    let world_ready = {
        let world = dirs.world.clone();
        let minecraft = facts.minecraft.clone();
        blocking(move || Ok(world.ready(&minecraft))).await?
    };
    Ok(BisectPlan {
        searched,
        always_on: items_with_role(&universe, Role::AlwaysOn),
        disabled: items_with_role(&universe, Role::Disabled),
        estimate: warden_bisect::estimate::estimate(searched, last_world_ms(&dirs.state_dir), 1),
        minecraft: facts.minecraft,
        loader: facts.loader,
        quick_play,
        world_ready,
    })
}

/// A busca guardada do pack (a que está rodando, a pausada ou a última terminada).
pub(crate) fn get(paths: &AppPaths, pack_id: PackId) -> Result<Option<BisectView>, AppError> {
    let dirs = Dirs::of(paths, pack_id);
    let Some(saved) = BisectState::load(&dirs.state_dir).map_err(|error| domain(&error))? else {
        return Ok(None);
    };
    Ok(Some(view_of(&saved, control_of(pack_id).as_deref())))
}

fn view_of(saved: &BisectState, control: Option<&Control>) -> BisectView {
    let search = &saved.search;
    let universe = search.universe();
    let status = if search.outcome().is_some() {
        BisectStatus::Finished
    } else if control.is_some() {
        BisectStatus::Running
    } else {
        BisectStatus::Paused
    };
    let info: BTreeMap<u32, &RoundInfo> = saved
        .rounds
        .iter()
        .map(|round| (round.number, round))
        .collect();
    let rounds = search
        .rounds()
        .iter()
        .map(|round| {
            let measured = info.get(&round.number);
            BisectRoundView {
                number: round.number,
                kind: round.kind.clone(),
                repeat: round.repeat,
                enabled_count: round.enabled_count,
                verdict: round.verdict.clone(),
                duration_ms: measured.map_or(0, |info| info.duration_ms),
                anchor: measured.and_then(|info| info.anchor),
                has_log: measured.is_some_and(|info| info.log.is_some()),
                has_server_log: measured.is_some_and(|info| info.server_log.is_some()),
                server_failed: measured.is_some_and(|info| info.server_failed),
            }
        })
        .collect::<Vec<_>>();
    let (left_min, left_max) = search.remaining_rounds();
    let done = u32::try_from(rounds.len()).unwrap_or(u32::MAX);
    let mut estimate = warden_bisect::estimate::from_rounds(
        left_min,
        left_max,
        saved.world_ms,
        search.repetitions(),
    );
    estimate.rounds_min = estimate.rounds_min.saturating_add(done);
    estimate.rounds_max = estimate.rounds_max.saturating_add(done);
    let outcome = search.outcome().cloned();
    let outcome_items = match &outcome {
        Some(SearchOutcome::Found { culprits, .. }) => culprits.clone(),
        Some(SearchOutcome::Contradiction { preventers }) => preventers.clone(),
        Some(SearchOutcome::Cancelled { suspects } | SearchOutcome::Inconclusive { suspects }) => {
            suspects.clone()
        }
        _ => Vec::new(),
    }
    .iter()
    .map(|path| item_of(universe, path))
    .collect();
    BisectView {
        id: saved.id.clone(),
        status,
        server: saved.goal == Goal::Server,
        symptom: saved.symptom.clone(),
        origin_crash: saved.origin_crash.clone(),
        rounds,
        current: control.and_then(|control| {
            control
                .current
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }),
        suspects: search
            .suspects()
            .iter()
            .map(|path| item_of(universe, path))
            .collect(),
        searched: u32::try_from(universe.with_role(Role::Searched).len()).unwrap_or(u32::MAX),
        always_on: items_with_role(universe, Role::AlwaysOn),
        disabled: items_with_role(universe, Role::Disabled),
        estimate,
        repetitions: search.repetitions(),
        elapsed_ms: saved.elapsed_ms,
        started_at_ms: saved.started_at_ms,
        finished_at_ms: saved.finished_at_ms,
        outcome,
        outcome_items,
        inferred: search
            .inferred()
            .iter()
            .map(|(from, to)| BisectInferred {
                from: item_of(universe, from),
                to: item_of(universe, to),
            })
            .collect(),
        earlier: search
            .earlier()
            .iter()
            .map(|set| set.iter().map(|path| item_of(universe, path)).collect())
            .collect(),
        creating_world: control
            .is_some_and(|control| control.creating_world.load(Ordering::SeqCst)),
    }
}

/// "Pausar depois desta rodada". Erros: `bisect.NOT_RESUMABLE` sem busca rodando.
pub(crate) fn pause(pack_id: PackId) -> Result<(), AppError> {
    let control = control_of(pack_id).ok_or_else(|| domain(&BisectError::NotResumable))?;
    control.pause.store(true, Ordering::SeqCst);
    Ok(())
}

/// Resposta do modo assistido. Erros: `bisect.NO_QUESTION`.
pub(crate) fn answer(pack_id: PackId, answer: AssistedAnswer) -> Result<(), AppError> {
    let control = control_of(pack_id).ok_or_else(|| domain(&BisectError::NoQuestion))?;
    let sender = control
        .answer
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .ok_or_else(|| domain(&BisectError::NoQuestion))?;
    sender
        .send(answer)
        .map_err(|_| domain(&BisectError::NoQuestion))
}

/// Cancela: com a busca rodando, encerra o jogo e a busca guarda o parcial ao sair; pausada,
/// guarda aqui. Erros: `bisect.NOT_FOUND`, `bisect.NOT_RESUMABLE` (já terminou).
pub(crate) async fn cancel(paths: &AppPaths, pack_id: PackId) -> Result<(), AppError> {
    if let Some(control) = control_of(pack_id) {
        control.cancel.store(true, Ordering::SeqCst);
        control.token.cancel();
        return Ok(());
    }
    let dirs = Dirs::of(paths, pack_id);
    blocking(move || {
        let mut saved = BisectState::load(&dirs.state_dir)
            .map_err(|error| domain(&error))?
            .ok_or_else(|| domain(&BisectError::NotFound))?;
        if saved.search.outcome().is_some() {
            return Err(domain(&BisectError::NotResumable));
        }
        saved.search.cancel();
        saved.finished_at_ms = Some(now_ms());
        saved.paused = false;
        saved
            .save(&dirs.state_dir)
            .map_err(|error| domain(&error))?;
        dirs.remove_all()
    })
    .await
}

/// O log gravado de uma rodada: o do jogo e, com o servidor local (D-13), o do servidor em
/// seguida, com a origem `server`. Erros: `bisect.NOT_FOUND`.
pub(crate) fn round_log(
    paths: &AppPaths,
    pack_id: PackId,
    number: u32,
) -> Result<Vec<crate::test_session::ConsoleLine>, AppError> {
    let dirs = Dirs::of(paths, pack_id);
    let saved = BisectState::load(&dirs.state_dir)
        .map_err(|error| domain(&error))?
        .ok_or_else(|| domain(&BisectError::NotFound))?;
    let log_dir = dirs.log_dir(&saved.id, number);
    let game = log_dir.join("output.log");
    let server = crate::bisect_server::server_log(&log_dir);
    if !game.is_file() && !server.is_file() {
        return Err(domain(&BisectError::NotFound));
    }
    let read = |path: PathBuf| {
        read_output_log(&path)
            .map(|(lines, _)| lines)
            .map_err(|error| {
                domain(&BisectError::Io {
                    path,
                    source: error,
                })
            })
    };
    let mut lines = if game.is_file() {
        read(game)?
    } else {
        Vec::new()
    };
    if server.is_file() {
        lines.extend(read(server)?.into_iter().map(|mut line| {
            line.origin = crate::test_session::ConsoleOrigin::Server;
            line
        }));
    }
    Ok(lines)
}

/// As buscas ligadas a um travamento (hash da assinatura): Problemas → Travamentos.
pub(crate) fn searches_for(state_dir: &Path, signature: &str) -> Vec<String> {
    let mut ids: Vec<String> = warden_bisect::state::results(state_dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|saved| saved.origin_crash.as_deref() == Some(signature))
        .map(|saved| saved.id)
        .collect();
    if let Ok(Some(current)) = BisectState::load(state_dir)
        && current.origin_crash.as_deref() == Some(signature)
        && !ids.contains(&current.id)
    {
        ids.insert(0, current.id);
    }
    ids
}

/// Roda a busca (nova, continuada ou "continuar sem") até terminar ou pausar. Devolve a busca
/// guardada.
pub(crate) async fn run(
    state: &AppState,
    pack_id: PackId,
    mode: RunMode,
    sink: BisectSink,
) -> Result<BisectView, AppError> {
    let record = state
        .packs
        .get(pack_id)
        .map_err(|error| AppError::from_domain(&error))?;
    let initial = GameState {
        pack_id,
        pack_name: record.name.clone(),
        state: GamePhase::Bisecting,
        session_id: None,
        operation_id: None,
        profile: None,
        started_at_ms: now_ms(),
    };
    let _active = state.tests.begin(initial, Arc::clone(&sink.game_state))?;
    let events = Arc::clone(&sink.events);
    let handle = state
        .operations
        .start(BISECT_RUN, Some(pack_id), true)
        .with_events(move |event| events(event));
    let control = Arc::new(Control {
        pack_id,
        pause: AtomicBool::new(false),
        cancel: AtomicBool::new(false),
        token: handle.token().clone(),
        answer: Mutex::new(None),
        current: Mutex::new(None),
        creating_world: AtomicBool::new(false),
    });
    *running().lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(&control));
    let _guard = ControlGuard;
    state.tests.attach_game(None, Some(handle.token().clone()));
    let result = Runner {
        state,
        pack_id,
        sink,
        handle: &handle,
        control: &control,
        dirs: Dirs::of(&state.paths, pack_id),
    }
    .run(mode)
    .await;
    handle.finish(result)
}

/// O que o acompanhamento de uma rodada viu.
struct Watched {
    watcher: RoundWatcher,
    decision: Option<Decision>,
    user_verdict: Option<Verdict>,
    exit: Option<warden_launcher::process::GameExit>,
    /// O servidor local saiu sozinho durante a rodada (D-13).
    server_exit: Option<warden_launcher::process::GameExit>,
    /// O sinal de falha que o [`RoundWatcher`] está fechando veio do log do servidor (D-13).
    anchor_on_server: bool,
    interrupted: bool,
    /// A origem do relógio da rodada: a abertura do jogo, ou a do servidor local (D-13).
    started_at: SystemTime,
    /// Com o servidor local, quanto depois dele o jogo abriu (as linhas do jogo vêm com o tempo
    /// desde a abertura do jogo).
    game_offset_ms: u64,
}

impl Watched {
    fn new(config: WatchConfig) -> Self {
        Self {
            watcher: RoundWatcher::new(config),
            decision: None,
            user_verdict: None,
            exit: None,
            server_exit: None,
            anchor_on_server: false,
            interrupted: false,
            started_at: SystemTime::now(),
            game_offset_ms: 0,
        }
    }
}

/// Por que o acompanhamento parou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    /// Há veredito, resposta do modo assistido ou interrupção (nos campos do [`Watched`]).
    Settled,
    /// O jogo saiu.
    GameExited,
    /// O servidor local saiu sozinho.
    ServerExited,
    /// O servidor local ficou pronto (só esperando por ele, antes de abrir o jogo).
    ServerReady,
}

/// O próximo evento do jogo; sem jogo (o servidor ainda abrindo), nunca.
async fn next_game(events: Option<&mut warden_launcher::process::GameEvents>) -> Option<GameEvent> {
    match events {
        Some(events) => events.recv().await,
        None => std::future::pending().await,
    }
}

/// O próximo sinal do servidor local; sem servidor, nunca.
async fn next_server(server: Option<&mut OpenServer>) -> ServerSignal {
    match server {
        Some(server) => server.next().await,
        None => std::future::pending().await,
    }
}

/// Milissegundos desde a abertura do processo.
fn elapsed_since(started_at: SystemTime) -> u64 {
    u64::try_from(started_at.elapsed().unwrap_or_default().as_millis()).unwrap_or(u64::MAX)
}

/// Uma rodada acompanhada até o veredito.
struct RoundRun {
    /// `None`: interrompida (pausa por "Parar jogo"/fechar o app, ou cancelamento).
    verdict: Option<Verdict>,
    info: RoundInfo,
    reached_ms: Option<u64>,
    world_interval_ms: Option<u64>,
}

struct Runner<'a> {
    state: &'a AppState,
    pack_id: PackId,
    sink: BisectSink,
    handle: &'a OperationHandle,
    control: &'a Control,
    dirs: Dirs,
}

impl Runner<'_> {
    async fn run(self, mode: RunMode) -> Result<BisectView, AppError> {
        let root = crate::commands::inventory::pack_root(self.state, self.pack_id)?;
        let state_dir = self.dirs.state_dir.clone();
        let saved =
            blocking(move || BisectState::load(&state_dir).map_err(|error| domain(&error))).await?;
        self.handle.stage(STAGE_PREPARE, STAGE_PREPARE_LABEL);
        self.materialize_base(&root, &self.dirs.base, InstallSide::Client)
            .await?;
        let input = graph_input(self.state, self.pack_id, &root, &self.dirs.base.game_dir).await?;
        let mut saved = match mode {
            RunMode::Start(request) => self.new_state(&root, &input, request).await?,
            RunMode::Resume => {
                let saved = saved.ok_or_else(|| domain(&BisectError::NotFound))?;
                if saved.search.outcome().is_some() {
                    return Err(domain(&BisectError::NotResumable));
                }
                saved
            }
            RunMode::ContinueWithout => {
                let mut saved = saved.ok_or_else(|| domain(&BisectError::NotFound))?;
                if !saved.search.continue_without() {
                    return Err(domain(&BisectError::NotResumable));
                }
                saved.finished_at_ms = None;
                saved
            }
        };
        saved.paused = false;
        self.save(&saved).await?;
        let lookup = {
            let jars = base_jars(&input, &self.dirs.base);
            let cache_dir = self.state.paths.cache_dir();
            let loader_input = input.clone();
            blocking(move || Ok(lookup_of(&loader_input, &jars, &cache_dir))).await?
        };
        let mut game = BisectGame::prepare(
            self.state,
            self.pack_id,
            root.clone(),
            self.dirs.round.clone(),
            self.handle,
        )
        .await?;
        // Cancelada (ou o app fechando) no meio da geração do mundo, as rodadas veem o pedido e
        // cancelam ou pausam a busca; outro erro deixa a busca pausada antes da primeira
        // rodada ("Continuar" tenta de novo). Com o servidor local (D-13), o servidor da rodada
        // abre uma cópia do mesmo mundo.
        if matches!(saved.goal, Goal::World | Goal::Server)
            && let Err(error) = self.ensure_world(&saved, &root, &game).await
            && !error.is_cancelled()
        {
            saved.paused = true;
            self.finish(&mut saved).await?;
            return Err(error);
        }
        let server = if saved.goal == Goal::Server {
            Some(self.prepare_server(&root, &saved, &game).await?)
        } else {
            None
        };
        self.rounds(&mut saved, &mut game, &lookup, server.as_ref())
            .await?;
        self.finish(&mut saved).await?;
        Ok(view_of(&saved, None))
    }

    /// O mundo de teste modelo (gerado agora, se ainda não existe para esta versão), com a
    /// tela avisando enquanto o servidor vanilla trabalha.
    async fn ensure_world(
        &self,
        saved: &BisectState,
        root: &Path,
        game: &BisectGame,
    ) -> Result<(), AppError> {
        if self.dirs.world.ready(&saved.minecraft) {
            return Ok(());
        }
        self.handle.stage(STAGE_WORLD, STAGE_WORLD_LABEL);
        self.control.creating_world.store(true, Ordering::SeqCst);
        self.notify_round(saved, 0, None, false);
        let result = world::ensure(
            self.state,
            self.pack_id,
            root,
            &saved.minecraft,
            game.java(),
            self.handle,
        )
        .await;
        self.control.creating_world.store(false, Ordering::SeqCst);
        self.notify_round(saved, 0, None, false);
        result.map(|_| ())
    }

    /// O servidor local da busca (D-13): o pack só com os itens de servidor na base do servidor
    /// e o servidor do loader instalado ("Preparar o servidor").
    async fn prepare_server(
        &self,
        root: &Path,
        saved: &BisectState,
        game: &BisectGame,
    ) -> Result<ServerBase, AppError> {
        self.handle.ensure_not_cancelled()?;
        self.handle.stage(
            crate::server_session::STAGE_SERVER,
            crate::server_session::STAGE_SERVER_LABEL,
        );
        self.materialize_base(root, &self.dirs.server.base, InstallSide::Server)
            .await?;
        crate::bisect_server::prepare_base(
            self.state,
            root,
            game.spec(),
            game.java(),
            &self.dirs.server,
            saved.search.universe().items().len(),
            self.handle,
        )
        .await
    }

    /// Materializa o pack inteiro (só o que mudou) numa base da busca: a do jogo, ou a do
    /// servidor local com só os itens de servidor (D-13), com todas as ferramentas do jogador (a
    /// busca decide quem entra) e os opcionais escolhidos na instância de teste.
    async fn materialize_base(
        &self,
        root: &Path,
        dirs: &InstanceDirs,
        side: InstallSide,
    ) -> Result<(), AppError> {
        let sources = crate::test_session::materialize_sources(self.state)?;
        let choices = OptionalChoices::load(&self.dirs.test.state_dir)
            .map_err(|error| AppError::from_domain(&error))?;
        let options = MaterializeOptions {
            side,
            on_modified: OnModified::Overwrite,
            optionals: OptionalSelection::Explicit(choices),
            player_tools: PlayerToolsMode::AsPlayer,
            ..MaterializeOptions::default()
        };
        loop {
            let report = {
                let _read = self.state.locks.read_for(self.pack_id, self.handle).await;
                let outcome = warden_instance::materialize(
                    &sources,
                    root,
                    dirs,
                    &options,
                    self.handle,
                    self.handle.token(),
                )
                .await
                .map_err(|error| AppError::from_domain(&error))?;
                match outcome {
                    warden_instance::Outcome::Done(report) => report,
                    warden_instance::Outcome::NeedsReview(_) => {
                        return Err(AppError::internal("a base da busca pediu revisão"));
                    }
                }
            };
            if report.blocked.is_empty() {
                if let Some(error) = report.incomplete_error() {
                    return Err(AppError::from_domain(&error));
                }
                return Ok(());
            }
            self.state
                .manual_downloads
                .wait(
                    self.pack_id,
                    report.blocked,
                    Arc::clone(&sources.cache),
                    self.handle,
                )
                .await?;
        }
    }

    async fn new_state(
        &self,
        root: &Path,
        input: &GraphInput,
        request: BisectRequest,
    ) -> Result<BisectState, AppError> {
        let facts = {
            let root = root.to_path_buf();
            blocking(move || pack_facts(&root)).await?
        };
        let universe = universe_of(input, root, &request.suspects);
        if universe.searched_units() == 0 {
            return Err(domain(&BisectError::NothingToSearch));
        }
        let quick_play =
            warden_launcher::quick_play::quick_play_support(&facts.minecraft).is_supported();
        // O mundo de teste existe ou pode ser gerado (o servidor vanilla só abre com a EULA).
        let world = self.dirs.world.ready(&facts.minecraft)
            || crate::server_session::eula_accepted_at(self.state).is_some();
        // Com Quick Play e o mundo de teste, toda rodada entra no mundo. Sem como entrar
        // (versão sem Quick Play, ou sem o mundo e sem a EULA), só "trava ao entrar no mundo"
        // é recusada; os outros sintomas vão até o menu (no modo assistido, a pessoa entra num
        // mundo). Com o servidor local aceito (D-13), toda rodada entra no mundo por ele, numa
        // cópia do mesmo mundo de teste; o servidor só abre com a EULA.
        let goal = match (&request.symptom, quick_play, world) {
            _ if request.server => {
                if crate::server_session::eula_accepted_at(self.state).is_none() {
                    return Err(AppError::from_domain(
                        &warden_server::Error::EulaNotAccepted,
                    ));
                }
                Goal::Server
            }
            (Symptom::CrashOnWorld, false, _) => {
                return Err(domain(&BisectError::NeedsServer {
                    minecraft: facts.minecraft,
                }));
            }
            (Symptom::CrashOnWorld, true, false) => return Err(domain(&BisectError::EulaNeeded)),
            (_, true, true) => Goal::World,
            _ => Goal::Menu,
        };
        let started = now_ms();
        Ok(BisectState {
            schema: warden_bisect::state::SCHEMA,
            id: search_id(started),
            symptom: request.symptom,
            goal,
            target: Target::default(),
            origin_crash: request.origin_crash,
            minecraft: facts.minecraft,
            loader: facts.loader,
            world: matches!(goal, Goal::World | Goal::Server).then(|| world::WORLD_NAME.to_owned()),
            search: Search::new(universe),
            rounds: Vec::new(),
            world_ms: last_world_ms(&self.dirs.state_dir),
            control_interval_ms: None,
            started_at_ms: started,
            elapsed_ms: 0,
            paused: false,
            finished_at_ms: None,
        })
    }

    async fn save(&self, saved: &BisectState) -> Result<(), AppError> {
        let saved = saved.clone();
        let state_dir = self.dirs.state_dir.clone();
        blocking(move || saved.save(&state_dir).map_err(|error| domain(&error))).await
    }

    /// As rodadas até terminar, pausar ou cancelar.
    async fn rounds(
        &self,
        saved: &mut BisectState,
        game: &mut BisectGame,
        lookup: &AppLookup,
        server: Option<&ServerBase>,
    ) -> Result<(), AppError> {
        loop {
            if self.control.cancel.load(Ordering::SeqCst) {
                saved.search.cancel();
                return Ok(());
            }
            if self.control.pause.load(Ordering::SeqCst) || self.handle.is_cancelled() {
                saved.paused = true;
                return Ok(());
            }
            let Some(plan) = saved.search.next_round() else {
                return Ok(());
            };
            let started = Instant::now();
            let run = self.round(saved, game, lookup, &plan, server).await?;
            let Some(verdict) = run.verdict else {
                // Rodada interrompida: cancelar, ou "Parar jogo"/fechar o Warden, que pausam
                // (a rodada não conta e é refeita ao continuar).
                if self.control.cancel.load(Ordering::SeqCst) {
                    saved.search.cancel();
                } else {
                    saved.paused = true;
                }
                return Ok(());
            };
            let first_reproduce = matches!(plan.kind, RoundKind::Reproduce { .. });
            if first_reproduce && let Some(reached) = run.reached_ms {
                // W para a estimativa: o que a rodada 0 levou até entrar no mundo.
                saved.world_ms = Some(reached);
            }
            if plan.kind == RoundKind::Control && run.world_interval_ms.is_some() {
                saved.control_interval_ms = run.world_interval_ms;
            }
            if let Verdict::Inferred { from, to } = &verdict {
                self.remember_inferred(from, to, saved.search.universe());
            }
            saved.search.record(verdict.clone());
            saved.rounds.push(run.info);
            saved.elapsed_ms = saved
                .elapsed_ms
                .saturating_add(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
            self.save(saved).await?;
            self.notify_round(saved, plan.number, Some(verdict), false);
        }
    }

    /// Guarda a dependência inferida no grafo do pack (vale para o pack, não só para a busca).
    fn remember_inferred(&self, from: &str, to: &str, universe: &Universe) {
        let path = crate::commands::graph::inferred_file(self.state, self.pack_id);
        let mut store = InferredStore::load(&path).unwrap_or_default();
        let names = (item_of(universe, from).name, item_of(universe, to).name);
        store.add(InferredEdge {
            from: from.to_owned(),
            to: to.to_owned(),
            source: InferredSource::Bisect,
            note: format!(
                "a busca do culpado viu {} procurar uma classe ou mod de {} sem declarar",
                names.0, names.1
            ),
        });
        if store.save(&path).is_err() {
            tracing::warn!("dependência inferida não gravada no grafo");
        }
    }

    fn notify_round(&self, saved: &BisectState, index: u32, result: Option<Verdict>, asking: bool) {
        let view = view_of(saved, Some(self.control));
        let enabled = view
            .current
            .as_ref()
            .map_or(0, |current| current.enabled_count);
        (self.sink.events)(OperationEvent::Round {
            index,
            total_estimate: (view.estimate.rounds_min, view.estimate.rounds_max),
            enabled,
            suspects: u32::try_from(view.suspects.len()).unwrap_or(u32::MAX),
            result,
            asking,
        });
        self.handle.progress(Progress::items(
            u64::from(index) + 1,
            Some(u64::from(view.estimate.rounds_max)),
        ));
    }

    fn set_current(&self, current: Option<BisectCurrent>) {
        *self
            .control
            .current
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = current;
    }

    fn set_asking(&self, asking: bool) {
        if let Some(current) = self
            .control
            .current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            current.asking = asking;
        }
    }

    /// Uma rodada: prepara a instância (e o servidor local, D-13), abre o jogo, acompanha e
    /// encerra no veredito.
    async fn round(
        &self,
        saved: &mut BisectState,
        game: &mut BisectGame,
        lookup: &AppLookup,
        plan: &warden_bisect::RoundPlan,
        server: Option<&ServerBase>,
    ) -> Result<RoundRun, AppError> {
        let round_started = Instant::now();
        self.handle.stage(STAGE_ROUND, STAGE_ROUND_LABEL);
        self.set_current(Some(BisectCurrent {
            number: plan.number,
            kind: plan.kind.clone(),
            repeat: plan.repeat,
            enabled_count: u32::try_from(plan.enabled.len()).unwrap_or(u32::MAX),
            added: plan.added.clone(),
            found: plan.found.clone(),
            asking: false,
            started_at_ms: now_ms(),
        }));
        self.notify_round(saved, plan.number, None, false);
        let enabled: BTreeSet<String> = plan.enabled.iter().cloned().collect();
        let log_dir = self.dirs.log_dir(&saved.id, plan.number);
        std::fs::create_dir_all(&log_dir).map_err(|error| {
            domain(&BisectError::Io {
                path: log_dir.clone(),
                source: error,
            })
        })?;
        let (command, prepared_server) = self
            .prepare_round(saved, game, &enabled, server, &log_dir)
            .await?;
        let watched = self
            .watch_round(saved, plan, &command, &log_dir, prepared_server)
            .await?;
        let duration_ms = u64::try_from(round_started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let observed = &watched.watcher;
        let relative = |file: &str| format!("{LOGS_DIR}/{}/round-{}/{file}", saved.id, plan.number);
        let mut info = RoundInfo {
            number: plan.number,
            duration_ms,
            reached_ms: observed.world_ms().or(observed.loaded_ms()),
            anchor: None,
            signature: None,
            // Com o servidor local, o jogo só abre depois do "Done".
            log: log_dir
                .join("output.log")
                .is_file()
                .then(|| relative("output.log")),
            server_log: server.is_some().then(|| relative("server/output.log")),
            server_failed: false,
        };
        let world_interval_ms = observed
            .started_world_ms()
            .zip(observed.world_ms())
            .map(|(start, world)| world.saturating_sub(start));
        let reached_ms = info.reached_ms;
        let verdict = if watched.interrupted {
            None
        } else {
            Some(
                self.conclude(saved, game, lookup, watched, &enabled, &log_dir, &mut info)
                    .await?,
            )
        };
        Ok(RoundRun {
            verdict,
            info,
            reached_ms,
            world_interval_ms,
        })
    }

    /// A instância da rodada e a linha de comando; com o servidor local (D-13), também a pasta
    /// do servidor com os mesmos itens desligados e a cópia descartável do mundo, e o jogo entra
    /// nele (`--server`/`--port` ou Quick Play).
    async fn prepare_round(
        &self,
        saved: &BisectState,
        game: &mut BisectGame,
        enabled: &BTreeSet<String>,
        server: Option<&ServerBase>,
        log_dir: &Path,
    ) -> Result<
        (
            warden_launcher::LaunchCommand,
            Option<crate::bisect_server::PreparedRound>,
        ),
        AppError,
    > {
        let excluded: BTreeSet<String> = saved
            .search
            .universe()
            .items()
            .iter()
            .map(|item| item.path.clone())
            .filter(|path| !enabled.contains(path))
            .collect();
        let loader = warden_bisect::prepare::perf_loader(saved.loader.as_deref());
        let world_entry = saved.goal == Goal::World;
        let enters_world = world_entry || saved.goal == Goal::Server;
        let prepared_server = match server {
            Some(base) => {
                let spec = crate::bisect_server::RoundSpec {
                    excluded: excluded.clone(),
                    world: self.dirs.world.world(),
                    eula_accepted_at: crate::server_session::eula_accepted_at(self.state),
                    first_port: self.state.servers.first_port(),
                    motd: self
                        .state
                        .packs
                        .get(self.pack_id)
                        .map(|record| record.name)
                        .unwrap_or_default(),
                    log: crate::bisect_server::server_log(log_dir),
                };
                let base = base.clone();
                let dirs = self.dirs.server.clone();
                Some(
                    blocking(move || crate::bisect_server::prepare_round(&base, &dirs, &spec))
                        .await?,
                )
            }
            None => None,
        };
        let options = SubsetOptions {
            excluded,
            disable_onboarding: enters_world
                && warden_launcher::quick_play::needs_accessibility_onboarding_off(
                    &saved.minecraft,
                ),
            disable_forge_load_warnings: warden_bisect::prepare::disable_forge_load_warnings(
                &saved.minecraft,
                loader,
            ),
            world: saved
                .world
                .as_ref()
                .filter(|_| world_entry)
                .map(|name| SubsetWorld {
                    source: self.dirs.world.world(),
                    name: name.clone(),
                }),
        };
        let base = self.dirs.base.clone();
        let round_dir = self.dirs.round.game_dir.clone();
        blocking(move || {
            materialize_subset(&base, &round_dir, &options)
                .map_err(|error| AppError::from_domain(&error))
        })
        .await?;
        let extra = LaunchExtra {
            system_props: warden_bisect::prepare::system_props(&saved.minecraft, loader),
            quick_play_path: world_entry.then(|| QUICK_PLAY_FILE.to_owned()),
            server_port: prepared_server
                .as_ref()
                .map(crate::bisect_server::PreparedRound::port),
        };
        let command = game.command(
            self.state,
            self.pack_id,
            self.handle,
            saved.world.clone().filter(|_| world_entry),
            &extra,
        )?;
        Ok((command, prepared_server))
    }

    /// O acompanhamento da rodada: o que é "passou", os limites e o modo.
    fn watch_config(saved: &BisectState) -> WatchConfig {
        WatchConfig {
            minecraft: saved.minecraft.clone(),
            loader: warden_bisect::prepare::perf_loader(saved.loader.as_deref()),
            goal: saved.goal,
            log_line: match &saved.symptom {
                Symptom::LogLine { text } => Some(text.clone()),
                _ => None,
            },
            assisted: saved.symptom == Symptom::Assisted,
            freeze_ms: warden_bisect::prepare::freeze_ms(saved.control_interval_ms),
            max_ms: warden_bisect::prepare::max_round_ms(saved.world_ms),
            stability_ms: stability_ms(),
        }
    }

    /// Abre o jogo e acompanha até o veredito, a resposta (modo assistido) ou a interrupção;
    /// encerra o jogo na hora (várias falhas não fecham o jogo sozinhas). Com o servidor local
    /// (D-13), ele abre antes, o jogo só abre depois do "Done", as linhas dos dois vão para o
    /// mesmo acompanhamento e o servidor fecha no fim da rodada, antes de a cópia do mundo sair.
    async fn watch_round(
        &self,
        saved: &BisectState,
        plan: &warden_bisect::RoundPlan,
        command: &warden_launcher::LaunchCommand,
        log_dir: &Path,
        server: Option<crate::bisect_server::PreparedRound>,
    ) -> Result<Watched, AppError> {
        let mut watched = Watched::new(Self::watch_config(saved));
        let mut open_server = None;
        if let Some(prepared) = server {
            let Some(open) = self
                .open_server(saved, plan, &mut watched, prepared)
                .await?
            else {
                return Ok(watched);
            };
            open_server = Some(open);
        }
        let spawned = GameProcess::spawn(
            command,
            SpawnOptions {
                output_log: Some(log_dir.join("output.log")),
                stdin: false,
            },
        )
        .await;
        let (process, mut events) = match spawned {
            Ok(spawned) => spawned,
            Err(error) => {
                if let Some(open) = open_server {
                    open.close(true).await;
                }
                return Err(AppError::from_domain(&error));
            }
        };
        if open_server.is_some() {
            watched.game_offset_ms = elapsed_since(watched.started_at);
        } else {
            watched.started_at = process.started_at();
        }
        let game_handle = process.handle();
        self.state
            .tests
            .attach_game(Some(game_handle.clone()), Some(self.handle.token().clone()));
        tracing::info!(
            round = plan.number,
            pid = process.pid(),
            "rodada da busca aberta"
        );
        self.follow(
            saved,
            plan,
            &mut watched,
            Some(&mut events),
            open_server.as_mut(),
        )
        .await;
        self.control
            .answer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.set_asking(false);
        let ended = self
            .end_game(&mut watched, &game_handle, &mut events, process)
            .await;
        // "Parar jogo" (botão do teste ou fechar o app) no meio da rodada também interrompe; o
        // jogo encerrado porque o servidor saiu, não.
        if watched
            .exit
            .as_ref()
            .is_some_and(|exit| exit.stop_requested)
            && watched.decision.is_none()
            && watched.user_verdict.is_none()
            && watched.server_exit.is_none()
        {
            watched.interrupted = true;
        }
        if let Some(open) = open_server {
            open.close(watched.interrupted || ended.is_err()).await;
        }
        ended.map(|()| watched)
    }

    /// Encerra o jogo da rodada (se ainda estiver aberto) e espera a saída.
    async fn end_game(
        &self,
        watched: &mut Watched,
        game_handle: &warden_launcher::process::GameHandle,
        events: &mut warden_launcher::process::GameEvents,
        process: GameProcess,
    ) -> Result<(), AppError> {
        if watched.exit.is_none() {
            game_handle.stop().await;
            while let Some(event) = events.recv().await {
                if let GameEvent::Exited(done) = event {
                    watched.exit = Some(done);
                    break;
                }
            }
        }
        self.state
            .tests
            .attach_game(None, Some(self.handle.token().clone()));
        if watched.exit.is_none() {
            watched.exit = Some(
                process
                    .wait()
                    .await
                    .map_err(|error| AppError::from_domain(&error))?,
            );
        }
        Ok(())
    }

    /// Abre o servidor local da rodada (D-13) e espera o "Done", com as linhas dele no
    /// acompanhamento. `None`: a rodada se decidiu antes (o servidor travou, saiu sozinho ou a
    /// rodada foi interrompida), o jogo não abre e o servidor já fechou.
    async fn open_server(
        &self,
        saved: &BisectState,
        plan: &warden_bisect::RoundPlan,
        watched: &mut Watched,
        prepared: crate::bisect_server::PreparedRound,
    ) -> Result<Option<OpenServer>, AppError> {
        let mut open = prepared.open(&self.state.servers, self.pack_id).await?;
        watched.started_at = open.started_at();
        tracing::info!(
            round = plan.number,
            port = open.port(),
            "servidor local da rodada aberto"
        );
        if self
            .follow(saved, plan, watched, None, Some(&mut open))
            .await
            == Ended::ServerReady
        {
            return Ok(Some(open));
        }
        open.close(watched.interrupted).await;
        Ok(None)
    }

    /// Lê a saída do jogo, a do servidor local (D-13) e o relógio até o veredito, a resposta, a
    /// saída de um dos dois, a interrupção ou, sem jogo ainda, o "Done" do servidor.
    async fn follow(
        &self,
        saved: &BisectState,
        plan: &warden_bisect::RoundPlan,
        watched: &mut Watched,
        mut game: Option<&mut warden_launcher::process::GameEvents>,
        mut server: Option<&mut OpenServer>,
    ) -> Ended {
        let waiting_server = game.is_none();
        let started_at = watched.started_at;
        let quick_play_file = self.dirs.round.game_dir.join(QUICK_PLAY_FILE);
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut answer_rx: Option<oneshot::Receiver<AssistedAnswer>> = None;
        loop {
            tokio::select! {
                event = next_game(game.as_deref_mut()) => match event {
                    Some(GameEvent::Line(line)) => {
                        let at = line.elapsed_ms.saturating_add(watched.game_offset_ms);
                        if observe(watched, &line.text, at, false) {
                            return Ended::Settled;
                        }
                    }
                    Some(GameEvent::Exited(done)) => {
                        watched.exit = Some(done);
                        return Ended::GameExited;
                    }
                    None => return Ended::GameExited,
                },
                signal = next_server(server.as_deref_mut()) => match signal {
                    ServerSignal::Line(line) => {
                        if observe(watched, &line.text, line.elapsed_ms, true) {
                            return Ended::Settled;
                        }
                    }
                    ServerSignal::Ready => {
                        if waiting_server {
                            return Ended::ServerReady;
                        }
                    }
                    ServerSignal::Exited(exit) => {
                        watched.server_exit = Some(exit);
                        return Ended::ServerExited;
                    }
                },
                _ = ticker.tick() => {
                    let now = elapsed_since(started_at);
                    if let Ok(text) = std::fs::read_to_string(&quick_play_file) {
                        watched.watcher.quick_play_file(&text, now);
                    }
                    if watched.watcher.take_question() {
                        let (tx, rx) = oneshot::channel();
                        *self.control.answer.lock().unwrap_or_else(PoisonError::into_inner) = Some(tx);
                        answer_rx = Some(rx);
                        self.set_asking(true);
                        self.notify_round(saved, plan.number, None, true);
                    }
                    if let Some(found) = watched.watcher.tick(now) {
                        watched.decision = Some(found);
                        return Ended::Settled;
                    }
                }
                answered = async {
                    match answer_rx.as_mut() {
                        Some(rx) => rx.await.ok(),
                        None => std::future::pending().await,
                    }
                } => {
                    if let Some(answered) = answered {
                        watched.user_verdict = Some(answered.verdict());
                        return Ended::Settled;
                    }
                    answer_rx = None;
                }
                () = self.handle.token().cancelled() => {
                    watched.interrupted = true;
                    return Ended::Settled;
                }
            }
        }
    }

    /// O veredito da rodada acompanhada.
    #[allow(clippy::too_many_arguments)]
    async fn conclude(
        &self,
        saved: &mut BisectState,
        game: &BisectGame,
        lookup: &AppLookup,
        mut watched: Watched,
        enabled: &BTreeSet<String>,
        log_dir: &Path,
        info: &mut RoundInfo,
    ) -> Result<Verdict, AppError> {
        if let Some(verdict) = watched.user_verdict.take() {
            return Ok(verdict);
        }
        let mut on_server = watched.anchor_on_server;
        let decision = if let Some(decision) = watched.decision.take() {
            decision
        } else if let Some(exit) = watched.server_exit.as_ref() {
            // O servidor local saiu sozinho (D-13): o veredito é o dele.
            on_server = true;
            watched.watcher.exit(
                exit.exit_code,
                first_crash_report(exit).as_deref(),
                exit.duration_ms,
            )
        } else {
            let exit = watched.exit.as_ref();
            watched.watcher.exit(
                exit.and_then(|exit| exit.exit_code),
                exit.and_then(first_crash_report).as_deref(),
                exit.map_or(0, |exit| exit.duration_ms)
                    .saturating_add(watched.game_offset_ms),
            )
        };
        let failure = match decision {
            Decision::Passed { .. } => return Ok(Verdict::Passed),
            Decision::Timeout => return Ok(Verdict::Timeout),
            Decision::Failed(failure) => failure,
        };
        info.anchor = Some(failure.anchor);
        info.signature = Some(failure.signature.clone());
        info.server_failed = on_server;
        let started_at = watched
            .started_at
            .checked_sub(Duration::from_secs(5))
            .unwrap_or(watched.started_at);
        // A assinatura da D-06 com o log e os crash reports do lado que falhou.
        let (files, exit_code) = if on_server {
            (
                crate::bisect_server::server_files(log_dir, &self.dirs.server, started_at),
                watched.server_exit.as_ref().and_then(|exit| exit.exit_code),
            )
        } else {
            (
                SessionFiles {
                    session_dir: Some(log_dir.to_path_buf()),
                    game_dir: self.dirs.round.game_dir.clone(),
                    started_at,
                },
                watched.exit.as_ref().and_then(|exit| exit.exit_code),
            )
        };
        let spec = game.spec().clone();
        let crash_hash = blocking(move || {
            Ok(
                crate::commands::crashes::signature_of(&files, &spec, exit_code)
                    .ok()
                    .filter(|signature| signature.rule.is_some() || signature.exception.is_some())
                    .map(|signature| signature.hash),
            )
        })
        .await?;
        Ok(judge_failure(
            &failure,
            crash_hash.as_deref(),
            &saved.symptom,
            &mut saved.target,
            enabled,
            lookup,
        ))
    }

    /// Fim (ou pausa): grava e apaga o que não precisa ficar.
    async fn finish(&self, saved: &mut BisectState) -> Result<(), AppError> {
        self.set_current(None);
        let finished = saved.search.outcome().is_some();
        if finished {
            saved.finished_at_ms = Some(now_ms());
            saved.paused = false;
        }
        self.save(saved).await?;
        let dirs_round = self.dirs.round_root.clone();
        let remove_all = finished;
        let paths = self.state.paths.clone();
        let pack_id = self.pack_id;
        blocking(move || {
            if remove_all {
                Dirs::of(&paths, pack_id).remove_all()
            } else {
                // Pausada: a base fica para continuar depressa; a rodada sai.
                remove_dir(&dirs_round).map_err(|error| AppError::from_domain(&error))
            }
        })
        .await?;
        let last = u32::try_from(saved.search.rounds().len()).unwrap_or_default();
        self.notify_round(saved, last, None, false);
        Ok(())
    }
}

/// Uma linha do jogo ou do servidor local no acompanhamento. Devolve se a rodada se decidiu
/// (falha). Guarda de que lado veio o sinal de falha que o acompanhamento está fechando (D-13).
fn observe(watched: &mut Watched, text: &str, at_ms: u64, from_server: bool) -> bool {
    let settling = watched.watcher.is_settling();
    let found = watched.watcher.observe(text, at_ms);
    if !settling && (watched.watcher.is_settling() || found.is_some()) {
        watched.anchor_on_server = from_server;
    }
    if let Some(found) = found {
        watched.decision = Some(found);
        return true;
    }
    false
}

/// O texto do primeiro crash report novo de um processo que saiu.
fn first_crash_report(exit: &warden_launcher::process::GameExit) -> Option<String> {
    exit.artifacts
        .crash_reports
        .first()
        .and_then(|path| std::fs::read_to_string(path).ok())
}

/// A estabilidade de cada rodada (o E2E encurta pelo ambiente, só em build de debug).
fn stability_ms() -> u64 {
    if cfg!(test) {
        return TEST_STABILITY_MS;
    }
    if cfg!(debug_assertions)
        && let Some(value) = std::env::var(STABILITY_ENV)
            .ok()
            .and_then(|text| text.trim().parse().ok())
    {
        return value;
    }
    warden_bisect::estimate::STABILITY_MS
}

/// Identificador da busca a partir do começo (`20261018-153012-123`, UTC).
fn search_id(started_ms: u64) -> String {
    let stamp =
        warden_launcher::session::rfc3339_utc(UNIX_EPOCH + Duration::from_millis(started_ms));
    // `2026-10-18T15:30:12Z` → `20261018153012`.
    let digits: String = stamp.chars().filter(char::is_ascii_digit).collect();
    let (date, time) = digits.split_at_checked(8).unwrap_or((&digits, ""));
    format!("{date}-{time}-{:03}", started_ms % 1_000)
}

// Os testes da busca com o servidor local (D-13) usam os mesmos ajudantes.
#[cfg(test)]
pub(crate) mod tests;
