//! Desempenho em todo teste (ROADMAP L-10; ARCHITECTURE §7.7; ADR-0038).
//!
//! Com o jogo aberto, uma tarefa própria mede a RAM do processo do jogo e dos filhos
//! ([`warden_perf::ProcessTree`]) a cada [`SAMPLE_INTERVAL`], fora da thread da interface (a
//! leitura do sistema roda em `spawn_blocking`), e manda `OperationEvent::PerfSample` pelo canal
//! do teste. Cada linha do jogo passa pelo [`LoadTracker`]: quando "carregou" ou "entrou no
//! mundo" acontece, uma amostra sai na hora, sem esperar o próximo segundo.
//!
//! A medição para sozinha quando o processo principal sai (o "Parar jogo" e o cancelamento
//! encerram o jogo) e, de todo jeito, no `after_exit` (ou se o teste terminar sem ele).
//!
//! No fim, a sessão grava o campo `perf` ([`SessionPerf`]: pico de RAM, pico do Job Object no
//! Windows, tempos e a última sessão de outra versão do pack no mesmo modo) e acrescenta uma
//! linha em `perf/<pack-id>.jsonl` nos dados locais (gancho 1.1, ARCHITECTURE §21.6;
//! ADR-0039 e ADR-0047), que a poda das sessões não apaga.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};
use tokio::task::JoinHandle;
use warden_launcher::LoaderSpec;
use warden_launcher::log::LogLine;
use warden_launcher::outcome::Outcome;
use warden_launcher::process::GameHandle;
use warden_launcher::session::{SessionRecord, list_sessions};
use warden_perf::load_time::{PastSession, previous_of_other_version};
use warden_perf::process::to_mb;
use warden_perf::{
    LoadTracker, Loader, MemoryStats, PreviousLoad, ProcessTree, SAMPLE_INTERVAL, SessionPerf,
};

use super::{ExitContext, GameContext, HookContext, TestHooks};
use crate::error::AppError;
use crate::events::OperationEvent;
use crate::test_session::TestMode;

/// Campo do `session.json` com o desempenho.
pub(crate) const PERF_FIELD: &str = "perf";

/// O gancho de desempenho de um teste.
#[derive(Default)]
pub(crate) struct PerfHook {
    running: Mutex<Option<Running>>,
}

/// A medição em andamento.
struct Running {
    live: Arc<Mutex<Live>>,
    sampler: JoinHandle<()>,
    game: GameHandle,
    events: Events,
}

/// O canal do teste.
type Events = Arc<dyn Fn(OperationEvent) + Send + Sync>;

/// O que as amostras e as linhas acumulam.
struct Live {
    stats: MemoryStats,
    tracker: LoadTracker,
    previous: Option<PreviousLoad>,
}

impl Live {
    /// A amostra para o canal.
    fn event(&self) -> OperationEvent {
        let measured = self.stats.samples > 0;
        let times = self.tracker.times();
        OperationEvent::PerfSample {
            rss_mb: measured.then(|| self.stats.rss_mb()),
            peak_rss_mb: measured.then(|| self.stats.peak_rss_mb()),
            loaded_ms: times.loaded_ms,
            world_ms: times.world_ms,
            previous: self.previous.clone(),
            heap_used_mb: None,
            heap_max_mb: None,
            gc_count: None,
            gc_ms: None,
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl PerfHook {
    fn take(&self) -> Option<Running> {
        lock(&self.running).take()
    }
}

impl Drop for PerfHook {
    /// O teste terminou sem `after_exit` (erro no meio): a medição não fica rodando.
    fn drop(&mut self) {
        if let Some(running) = self.take() {
            running.sampler.abort();
        }
    }
}

#[async_trait::async_trait]
impl TestHooks for PerfHook {
    fn name(&self) -> &'static str {
        "perf"
    }

    async fn on_game_started(&self, context: &HookContext<'_>, game: &GameContext<'_>) {
        let live = Arc::new(Mutex::new(Live {
            stats: MemoryStats::default(),
            tracker: LoadTracker::new(game.spec.minecraft.as_str(), loader(&game.spec.loader)),
            previous: None,
        }));
        let sampler = tokio::spawn(sample_loop(SamplerInput {
            live: Arc::clone(&live),
            pid: game.pid,
            game: game.game.clone(),
            events: Arc::clone(game.events),
            sessions_dir: context.dirs.state_dir.join("sessions"),
            current_session: game.session_dir.to_path_buf(),
            mode: context.mode,
            pack_version: game.pack_version.map(ToOwned::to_owned),
        }));
        let previous = lock(&self.running).replace(Running {
            live,
            sampler,
            game: game.game.clone(),
            events: Arc::clone(game.events),
        });
        if let Some(previous) = previous {
            previous.sampler.abort();
        }
    }

    fn on_game_line(&self, line: &LogLine) {
        let changed = {
            let guard = lock(&self.running);
            let Some(running) = guard.as_ref() else {
                return;
            };
            let mut measured = lock(&running.live);
            measured
                .tracker
                .observe(&line.text, line.elapsed_ms)
                .then(|| (measured.event(), Arc::clone(&running.events)))
        };
        // Um tempo mudou: a faixa mostra na hora, sem esperar a próxima amostra.
        if let Some((event, events)) = changed {
            events(event);
        }
    }

    async fn after_exit(
        &self,
        context: &HookContext<'_>,
        exit: &ExitContext<'_>,
    ) -> Result<Map<String, Value>, AppError> {
        let Some(running) = self.take() else {
            return Ok(Map::new());
        };
        running.sampler.abort();
        let _ = running.sampler.await;
        let perf = {
            let live = lock(&running.live);
            SessionPerf {
                peak_rss_mb: (live.stats.samples > 0).then(|| live.stats.peak_rss_mb()),
                peak_commit_mb: running.game.peak_memory_bytes().map(to_mb),
                times: live.tracker.times(),
                samples: live.stats.samples,
                previous: live.previous.clone(),
            }
        };
        tracing::info!(
            peak_rss_mb = ?perf.peak_rss_mb,
            loaded_ms = ?perf.times.loaded_ms,
            world_ms = ?perf.times.world_ms,
            samples = perf.samples,
            "desempenho do teste"
        );
        let line = history_line(&perf, exit.record, context.mode);
        let file = context.paths.perf_file(context.pack_id);
        let written = tokio::task::spawn_blocking(move || append_line(&file, &line)).await;
        match written {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!(%error, "linha do histórico de desempenho não gravada")
            }
            Err(error) => tracing::warn!(%error, "histórico de desempenho interrompido"),
        }
        let mut fields = Map::new();
        fields.insert(
            PERF_FIELD.to_owned(),
            serde_json::to_value(&perf).map_err(|error| AppError::internal(error.to_string()))?,
        );
        Ok(fields)
    }
}

/// O que a tarefa de medição recebe.
struct SamplerInput {
    live: Arc<Mutex<Live>>,
    pid: u32,
    game: GameHandle,
    events: Events,
    sessions_dir: PathBuf,
    current_session: PathBuf,
    mode: TestMode,
    pack_version: Option<String>,
}

/// Mede a cada [`SAMPLE_INTERVAL`] até o processo principal sair.
async fn sample_loop(input: SamplerInput) {
    let SamplerInput {
        live,
        pid,
        game,
        events,
        sessions_dir,
        current_session,
        mode,
        pack_version,
    } = input;
    let previous = tokio::task::spawn_blocking(move || {
        previous_load(
            &sessions_dir,
            &current_session,
            mode,
            pack_version.as_deref(),
        )
    })
    .await
    .ok()
    .flatten();
    lock(&live).previous = previous;

    let Ok(Some(mut tree)) = tokio::task::spawn_blocking(move || ProcessTree::open(pid)).await
    else {
        tracing::info!(
            pid,
            "processo do jogo não pôde ser aberto para medir a memória"
        );
        return;
    };
    let mut ticker = tokio::time::interval(SAMPLE_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if game.has_exited() {
            break;
        }
        let Ok((back, sample)) = tokio::task::spawn_blocking(move || {
            let sample = tree.sample();
            (tree, sample)
        })
        .await
        else {
            break;
        };
        tree = back;
        let Some(sample) = sample else {
            break;
        };
        let event = {
            let mut live = lock(&live);
            live.stats.push(sample);
            live.event()
        };
        events(event);
    }
}

/// O loader do motor para os marcadores.
fn loader(spec: &LoaderSpec) -> Loader {
    match spec {
        LoaderSpec::Vanilla => Loader::Vanilla,
        LoaderSpec::Fabric { .. } => Loader::Fabric,
        LoaderSpec::Quilt { .. } => Loader::Quilt,
        LoaderSpec::Forge { .. } => Loader::Forge,
        LoaderSpec::NeoForge { .. } => Loader::NeoForge,
    }
}

/// A última sessão de outra versão do pack, no mesmo modo, que carregou.
fn previous_load(
    sessions_dir: &Path,
    current_session: &Path,
    mode: TestMode,
    pack_version: Option<&str>,
) -> Option<PreviousLoad> {
    let sessions = list_sessions(sessions_dir)
        .inspect_err(|error| tracing::warn!(%error, "sessões anteriores não lidas"))
        .ok()?;
    let past: Vec<(String, Option<String>, Option<u32>)> = sessions
        .iter()
        .filter(|session| session.dir.path() != current_session)
        .filter_map(|session| {
            let record = session.record.as_ref()?;
            if session_mode(record) != mode {
                return None;
            }
            let id = session
                .dir
                .path()
                .file_name()?
                .to_string_lossy()
                .into_owned();
            Some((
                id,
                record.pack_version.clone(),
                session_perf(record)?.times.loaded_ms,
            ))
        })
        .collect();
    previous_of_other_version(
        pack_version,
        past.iter().map(|(id, version, loaded_ms)| PastSession {
            session_id: id,
            pack_version: version.as_deref(),
            loaded_ms: *loaded_ms,
        }),
    )
}

/// O modo gravado na sessão (sessões antigas sem o campo eram testes normais).
fn session_mode(record: &SessionRecord) -> TestMode {
    record
        .extra
        .get("mode")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

/// O campo `perf` de uma sessão gravada (`None` nas sessões de antes da L-10).
pub(crate) fn session_perf(record: &SessionRecord) -> Option<SessionPerf> {
    record
        .extra
        .get(PERF_FIELD)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

/// Uma linha de `perf/<pack-id>.jsonl` (ARCHITECTURE §21.6). Os nomes são o contrato com o
/// histórico da 1.1 (W-10): não mude sem migrar.
#[derive(Debug, Serialize)]
struct HistoryLine {
    sessao: String,
    data: String,
    modo: TestMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    carregou_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mundo_ms: Option<u32>,
    ram_pico_mb: Option<u32>,
    perfil: HistoryProfile,
    computador: Option<String>,
    arvore: Option<String>,
    primeira_abertura: bool,
    resultado: Outcome,
}

/// O perfil do teste na linha do histórico.
#[derive(Debug, Serialize)]
struct HistoryProfile {
    id: String,
    /// Nome do perfil (os perfis com nome são da L-08; o Padrão fica sem).
    #[serde(skip_serializing_if = "Option::is_none")]
    nome: Option<String>,
    assinatura: Option<String>,
}

/// Monta a linha a partir do desempenho e do registro da sessão (com os campos do Testar).
fn history_line(perf: &SessionPerf, record: &SessionRecord, mode: TestMode) -> HistoryLine {
    let extra = &record.extra;
    let text = |value: Option<&Value>| value.and_then(Value::as_str).map(ToOwned::to_owned);
    HistoryLine {
        sessao: text(extra.get("sessionId")).unwrap_or_default(),
        data: record.started_at.clone(),
        modo: mode,
        carregou_ms: perf.times.loaded_ms,
        mundo_ms: perf.times.world_ms,
        ram_pico_mb: perf.peak_rss_mb,
        perfil: HistoryProfile {
            id: text(extra.get("profile")).unwrap_or_else(|| "default".to_owned()),
            nome: None,
            assinatura: text(
                extra
                    .get("profileSignature")
                    .and_then(|sig| sig.get("hash")),
            ),
        },
        // A impressão do computador: SHA-256 dos campos gravados na sessão, calculado aqui e
        // nunca enviado.
        computador: extra.get("machine").map(|machine| {
            let canonical = serde_json::to_vec(machine).unwrap_or_default();
            format!("sha256:{}", hex::encode(Sha256::digest(&canonical)))
        }),
        arvore: text(extra.get("packTree").and_then(|tree| tree.get("hash"))),
        primeira_abertura: extra
            .get("firstLaunch")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        resultado: record.outcome,
    }
}

/// Acrescenta uma linha JSON ao arquivo (criando a pasta).
fn append_line(file: &Path, line: &HistoryLine) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string(line).map_err(std::io::Error::other)?;
    text.push('\n');
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)?;
    out.write_all(text.as_bytes())
}
