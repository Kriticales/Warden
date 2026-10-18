//! Testar com perfil de desempenho (SPEC T13; ARCHITECTURE §7.7; R5A §7.2 e §7.4; ROADMAP
//! L-12).
//!
//! Só age no teste com perfil ([`HookContext::perf_profile`]):
//! - **no jogo** (modo `Performance`): a abertura ganha o registro detalhado do Forge moderno e
//!   do NeoForge ([`jvm_args`], chamado pelo `plan_launch`); no fim, o tempo por mod sai do log
//!   do loader ([`warden_perf::mod_timing`]) e cada perfil do spark que a pessoa salvou no jogo
//!   (`/sparkc profiler stop --save-to-file`) vira um resumo;
//! - **no servidor local** ("Testar como servidor" com o perfil): quando o jogo abre (o servidor
//!   já está pronto), o Warden liga o spark do servidor pelo console ([`SPARK_START`]); no fim,
//!   antes de o servidor fechar, desliga ([`SPARK_STOP`]) se o perfil ainda não tiver sido
//!   salvo, e espera o arquivo.
//!
//! Os arquivos do spark salvos durante o teste (`config/spark/*.sparkprofile`, `*.sparkheap`,
//! com data a partir do início do jogo) são copiados para `<sessão>/spark/`, porque a instância
//! muda no próximo teste; nunca entram em "O que mudou durante o teste" (a captura da C-03 já os
//! ignora, CA-T15-06). Os mods do resumo são agrupados pelo jar com o índice pacote → jar → mod
//! da D-05 (o mesmo do "Por que travou"). O resultado vai para o `session.json`, campo
//! [`SESSION_FIELD`] (lido pelo `perf_report_get`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};
use warden_diagnostics::pretest::build_index;
use warden_jarmeta::index::PackIndex;
use warden_launcher::{GameSpec, LoaderSpec};
use warden_perf::Loader;
use warden_perf::mod_timing::{self, ModTimings, TimingStatus};
use warden_perf::spark::{self, ClassOwner, SparkKind};

use super::{ExitContext, GameContext, HookContext, TestHooks};
use crate::commands::diagnostics::input::{index_input, read_snapshot};
use crate::commands::perf::{
    LoadReport, LoadRow, PerfReport, PerfSide, SESSION_FIELD, SPARK_DIR, ServerSpark, SparkReport,
    SparkRow,
};
use crate::error::AppError;
use crate::test_session::TestMode;

/// Liga o spark do servidor por 60 s, salvando o perfil num arquivo (ARCHITECTURE §7.7).
pub(crate) const SPARK_START: &str = "spark profiler start --timeout 60 --save-to-file";
/// Desliga o spark do servidor, salvando o que mediu até ali.
pub(crate) const SPARK_STOP: &str = "spark profiler stop --save-to-file";

/// Quanto o fim do teste espera o servidor salvar o perfil depois do [`SPARK_STOP`].
const SAVE_WAIT: Duration = Duration::from_secs(20);

/// Folga para o relógio do arquivo (sistemas de arquivos com data em segundos).
const MTIME_SLACK: Duration = Duration::from_secs(2);

/// Quantos arquivos do spark uma sessão guarda (os mais novos).
const MAX_SPARK_FILES: usize = 10;

/// Maior log do loader lido (o `debug.log` em `TRACE` de um pack grande fica em dezenas de MB).
const MAX_LOG_BYTES: u64 = 256 * 1024 * 1024;

/// Os argumentos da JVM do teste com perfil de desempenho no jogo.
pub(crate) fn jvm_args(spec: &GameSpec) -> Vec<String> {
    mod_timing::trace_jvm_args(spec.minecraft.as_str(), loader(&spec.loader))
}

fn loader(spec: &LoaderSpec) -> Loader {
    match spec {
        LoaderSpec::Vanilla => Loader::Vanilla,
        LoaderSpec::Fabric { .. } => Loader::Fabric,
        LoaderSpec::Quilt { .. } => Loader::Quilt,
        LoaderSpec::Forge { .. } => Loader::Forge,
        LoaderSpec::NeoForge { .. } => Loader::NeoForge,
    }
}

fn loader_name(spec: &LoaderSpec) -> &'static str {
    match spec {
        LoaderSpec::Vanilla => "vanilla",
        LoaderSpec::Fabric { .. } => "fabric",
        LoaderSpec::Quilt { .. } => "quilt",
        LoaderSpec::Forge { .. } => "forge",
        LoaderSpec::NeoForge { .. } => "neoforge",
    }
}

/// O gancho de um teste.
#[derive(Default)]
pub(crate) struct PerfProfileHook {
    started: Mutex<Option<Started>>,
}

/// O que o começo do jogo guardou para o fim.
struct Started {
    spec: GameSpec,
    since: SystemTime,
    /// No servidor local: o que aconteceu com o spark dele.
    server: Option<ServerSpark>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[async_trait::async_trait]
impl TestHooks for PerfProfileHook {
    fn name(&self) -> &'static str {
        "perf_profile"
    }

    async fn on_game_started(&self, context: &HookContext<'_>, game: &GameContext<'_>) {
        if !context.perf_profile {
            return;
        }
        let server = if context.mode == TestMode::Server {
            Some(start_server_spark(context).await)
        } else {
            None
        };
        *lock(&self.started) = Some(Started {
            spec: game.spec.clone(),
            since: game.started_at,
            server,
        });
    }

    async fn after_exit(
        &self,
        context: &HookContext<'_>,
        exit: &ExitContext<'_>,
    ) -> Result<Map<String, Value>, AppError> {
        let Some(started) = lock(&self.started).take() else {
            return Ok(Map::new());
        };
        let game_dir = if started.server.is_some() {
            crate::server_session::server_dirs(&context.state.paths, context.pack_id).game_dir
        } else {
            context.dirs.game_dir.clone()
        };
        if started.server == Some(ServerSpark::Started) {
            stop_server_spark(context, &game_dir, started.since).await;
        }
        let collected = {
            let game_dir = game_dir.clone();
            let session_dir = exit.session_dir.to_path_buf();
            let since = started.since;
            let spec = started.spec.clone();
            let server = started.server.is_some();
            tokio::task::spawn_blocking(move || {
                collect(&game_dir, &session_dir, since, &spec, server)
            })
            .await
            .map_err(|error| AppError::internal(format!("perfil de desempenho: {error}")))?
        };
        let owners = if collected.needs_owners() {
            Owners::read(context).await
        } else {
            Owners::default()
        };
        let report = collected.into_report(&owners, started.server);
        tracing::info!(
            side = ?report.side,
            load = ?report.load.as_ref().map(|load| load.status),
            spark = report.spark.len(),
            "perfil de desempenho do teste"
        );
        let mut fields = Map::new();
        fields.insert(
            SESSION_FIELD.to_owned(),
            serde_json::to_value(&report).map_err(|error| AppError::internal(error.to_string()))?,
        );
        Ok(fields)
    }
}

/// Liga o spark do servidor local, se ele estiver entre os mods do servidor.
async fn start_server_spark(context: &HookContext<'_>) -> ServerSpark {
    let mods = crate::server_session::server_dirs(&context.state.paths, context.pack_id)
        .game_dir
        .join("mods");
    let installed = tokio::task::spawn_blocking(move || has_spark(&mods))
        .await
        .unwrap_or(false);
    if !installed {
        tracing::info!("perfil de desempenho no servidor sem o spark");
        return ServerSpark::NotInstalled;
    }
    match context
        .state
        .servers
        .send_command(context.pack_id, SPARK_START)
        .await
    {
        Ok(()) => {
            tracing::info!("spark do servidor ligado pelo console");
            ServerSpark::Started
        }
        Err(error) => {
            tracing::warn!(code = %error.code, "spark do servidor não ligou");
            ServerSpark::CommandFailed
        }
    }
}

/// Se algum jar de `mods/` é o spark (o nome do arquivo filtra; o descritor confirma).
fn has_spark(mods: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(mods) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        Path::new(&name)
            .extension()
            .is_some_and(|extension| extension == "jar")
            && name.contains("spark")
            && warden_jarmeta::index::index_jar_file(
                &entry.path(),
                &warden_jarmeta::Limits::default(),
            )
            .is_ok_and(|index| {
                index
                    .jars
                    .iter()
                    .any(|jar| jar.mods.iter().any(|module| module.id == "spark"))
            })
    })
}

/// Desliga o spark do servidor (se o perfil ainda não foi salvo) e espera o arquivo.
async fn stop_server_spark(context: &HookContext<'_>, game_dir: &Path, since: SystemTime) {
    let spark_dir = game_dir.join("config").join("spark");
    let saved = {
        let spark_dir = spark_dir.clone();
        move || !spark_files(&spark_dir, since).is_empty()
    };
    if tokio::task::spawn_blocking(saved.clone())
        .await
        .unwrap_or(false)
    {
        return;
    }
    if let Err(error) = context
        .state
        .servers
        .send_command(context.pack_id, SPARK_STOP)
        .await
    {
        tracing::info!(code = %error.code, "spark do servidor não desligou (servidor fechado)");
        return;
    }
    let deadline = tokio::time::Instant::now() + SAVE_WAIT;
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if tokio::task::spawn_blocking(saved.clone())
            .await
            .unwrap_or(false)
        {
            // O spark grava o arquivo e depois avisa; um instante para terminar de gravar.
            tokio::time::sleep(Duration::from_millis(500)).await;
            return;
        }
    }
    tracing::warn!("o servidor não salvou o perfil do spark a tempo");
}

/// Um arquivo do spark salvo durante o teste.
#[derive(Debug, Clone)]
struct SparkFile {
    path: PathBuf,
    name: String,
    kind: SparkKind,
    modified: SystemTime,
}

/// Os arquivos do spark de `config/spark/` (sem subpastas) com data a partir de `since`, do mais
/// antigo ao mais novo.
fn spark_files(spark_dir: &Path, since: SystemTime) -> Vec<SparkFile> {
    let since = since.checked_sub(MTIME_SLACK).unwrap_or(since);
    let Ok(entries) = std::fs::read_dir(spark_dir) else {
        return Vec::new();
    };
    let mut files: Vec<SparkFile> = entries
        .flatten()
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let name = entry.file_name().to_str()?.to_owned();
            let kind = SparkKind::of_file(&name)?;
            let modified = metadata.modified().ok()?;
            (modified >= since).then(|| SparkFile {
                path: entry.path(),
                name,
                kind,
                modified,
            })
        })
        .collect();
    files.sort_by(|a, b| {
        a.modified
            .cmp(&b.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
    files
}

/// O que o fim do teste leu do disco (sem o índice, que precisa da trava do pack).
struct Collected {
    side: PerfSide,
    load: Option<(ModTimings, String)>,
    spark: Vec<(SparkFile, Option<Vec<u8>>)>,
    spark_skipped: u32,
}

/// Lê o log do loader e copia os arquivos do spark para a sessão.
fn collect(
    game_dir: &Path,
    session_dir: &Path,
    since: SystemTime,
    spec: &GameSpec,
    server: bool,
) -> Collected {
    let load = (!server).then(|| {
        (
            load_times(game_dir, since, spec),
            loader_name(&spec.loader).to_owned(),
        )
    });
    let mut files = spark_files(&game_dir.join("config").join("spark"), since);
    let skipped = files.len().saturating_sub(MAX_SPARK_FILES);
    files.drain(..skipped);
    let copies = session_dir.join(SPARK_DIR);
    let spark = files
        .into_iter()
        .map(|file| {
            let bytes = read_spark(&file.path);
            if let Some(bytes) = &bytes {
                let written = std::fs::create_dir_all(&copies)
                    .and_then(|()| std::fs::write(copies.join(&file.name), bytes));
                if let Err(error) = written {
                    tracing::warn!(%error, "cópia do perfil do spark não gravada na sessão");
                }
            }
            (file, bytes)
        })
        .collect();
    Collected {
        side: if server {
            PerfSide::Server
        } else {
            PerfSide::Client
        },
        load,
        spark,
        spark_skipped: u32::try_from(skipped).unwrap_or(u32::MAX),
    }
}

fn read_spark(path: &Path) -> Option<Vec<u8>> {
    let size = std::fs::metadata(path).ok()?.len();
    if size > spark::MAX_FILE_BYTES {
        tracing::warn!(size, "perfil do spark grande demais; fica de fora");
        return None;
    }
    std::fs::read(path)
        .inspect_err(|error| tracing::warn!(%error, "perfil do spark não lido"))
        .ok()
}

/// O tempo por mod do log do loader deste teste (um log de antes do jogo abrir não conta).
fn load_times(game_dir: &Path, since: SystemTime, spec: &GameSpec) -> ModTimings {
    let plan = mod_timing::plan(spec.minecraft.as_str(), loader(&spec.loader), false);
    let Some(source) = plan.source else {
        return ModTimings::without_data(TimingStatus::NotRecorded, &plan);
    };
    let since = since.checked_sub(MTIME_SLACK).unwrap_or(since);
    let found = plan.logs.iter().find_map(|relative| {
        let path = game_dir.join(relative);
        let metadata = std::fs::metadata(&path).ok()?;
        let fresh = metadata.modified().is_ok_and(|modified| modified >= since);
        (metadata.is_file() && fresh && metadata.len() <= MAX_LOG_BYTES)
            .then_some((path, *relative))
    });
    let Some((path, relative)) = found else {
        return ModTimings::without_data(TimingStatus::LogMissing, &plan);
    };
    match std::fs::read(&path) {
        Ok(bytes) => mod_timing::parse(source, &String::from_utf8_lossy(&bytes), relative),
        Err(error) => {
            tracing::warn!(%error, log = relative, "log do loader não lido");
            ModTimings::without_data(TimingStatus::LogMissing, &plan)
        }
    }
}

impl Collected {
    /// Se há algo para ligar aos itens do pack.
    fn needs_owners(&self) -> bool {
        self.spark.iter().any(|(_, bytes)| bytes.is_some())
            || self
                .load
                .as_ref()
                .is_some_and(|(timings, _)| !timings.mods.is_empty())
    }

    fn into_report(self, owners: &Owners, server: Option<ServerSpark>) -> PerfReport {
        let load = self.load.map(|(timings, loader)| LoadReport {
            status: timings.status,
            source: timings.source,
            loader,
            log: timings.log,
            mods: timings
                .mods
                .into_iter()
                .map(|item| {
                    let path = owners.item_of_mod(&item.key);
                    LoadRow {
                        name: owners.name(path.as_deref(), Some(&item.key)),
                        item: path,
                        key: item.key,
                        ms: item.ms,
                        phases: item.phases,
                    }
                })
                .collect(),
            game_and_loader_ms: timings.game_and_loader_ms,
            mods_ms: timings.mods_ms,
        });
        let spark = self
            .spark
            .into_iter()
            .map(|(file, bytes)| spark_report(&file, bytes.as_deref(), owners))
            .collect();
        PerfReport {
            side: self.side,
            load,
            server_spark: server,
            spark,
            spark_skipped: self.spark_skipped,
        }
    }
}

fn millis(time: SystemTime) -> Option<i64> {
    let elapsed = time.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(elapsed.as_millis()).ok()
}

fn spark_report(file: &SparkFile, bytes: Option<&[u8]>, owners: &Owners) -> SparkReport {
    let summary = bytes.and_then(|bytes| {
        spark::summarize(file.kind, bytes, &|class| owners.class(class))
            .inspect_err(|error| tracing::warn!(%error, "perfil do spark ilegível"))
            .ok()
    });
    let mut report = SparkReport {
        file: file.name.clone(),
        kind: file.kind,
        saved_at_ms: millis(file.modified),
        readable: summary.is_some(),
        platform: None,
        duration_ms: None,
        ticks: None,
        top: Vec::new(),
        game_percent: 0.0,
    };
    if let Some(summary) = summary {
        report.platform = summary.platform;
        report.duration_ms = summary
            .started_at_ms
            .zip(summary.ended_at_ms)
            .and_then(|(start, end)| u64::try_from(end - start).ok());
        report.ticks = summary.ticks;
        report.game_percent = summary.game_percent;
        report.top = summary
            .top
            .into_iter()
            .map(|share| SparkRow {
                name: owners.name(share.item.as_deref(), share.mod_id.as_deref()),
                item: share.item,
                mod_id: share.mod_id,
                percent: share.percent,
            })
            .collect();
    }
    report
}

/// O índice pacote → jar → mod do pack (D-05) e os nomes dos itens.
#[derive(Default)]
struct Owners {
    index: Option<PackIndex>,
    /// id do mod → item do pack.
    mods: HashMap<String, String>,
    /// item do pack → nome.
    names: HashMap<String, String>,
}

impl Owners {
    /// Lê o pack e monta o índice com os jars da instância de teste (o cache por hash da D-05
    /// deixa isso rápido: a "Verificação final" já indexou os mesmos jars). Sem pack legível,
    /// o resumo fica com os ids do próprio spark.
    async fn read(context: &HookContext<'_>) -> Self {
        let snapshot =
            match read_snapshot(context.state, context.pack_id, Some(context.operation)).await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    tracing::warn!(code = %error.code, "pack não lido para o perfil de desempenho");
                    return Self::default();
                }
            };
        let names: HashMap<String, String> = snapshot
            .items
            .iter()
            .map(|item| (item.path.clone(), item.name.clone()))
            .collect();
        let Some(input) = index_input(&snapshot) else {
            return Self {
                names,
                ..Self::default()
            };
        };
        let game_dir = context.dirs.game_dir.clone();
        let cache_dir = context.state.paths.cache_dir();
        let index =
            tokio::task::spawn_blocking(move || build_index(&input, &game_dir, &cache_dir).index)
                .await
                .ok();
        let mut mods = HashMap::new();
        if let Some(index) = &index {
            for (item, jar) in index.items() {
                for indexed in &jar.jars {
                    for module in &indexed.mods {
                        mods.entry(module.id.clone())
                            .or_insert_with(|| item.clone());
                    }
                }
            }
        }
        Self { index, mods, names }
    }

    fn class(&self, class: &str) -> Option<ClassOwner> {
        let owner = self.index.as_ref()?.class_owner(class)?.owner()?;
        Some(ClassOwner {
            item: Some(owner.item.to_owned()),
            mod_id: owner.mod_id.map(ToOwned::to_owned),
        })
    }

    fn item_of_mod(&self, mod_id: &str) -> Option<String> {
        self.mods.get(mod_id).cloned()
    }

    /// O nome do item, ou o id do mod, ou o caminho.
    fn name(&self, item: Option<&str>, mod_id: Option<&str>) -> String {
        item.and_then(|item| self.names.get(item).cloned())
            .or_else(|| mod_id.map(ToOwned::to_owned))
            .or_else(|| item.map(ToOwned::to_owned))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forge(minecraft: &str) -> GameSpec {
        GameSpec::new(
            minecraft,
            LoaderSpec::Forge {
                version: "1".into(),
            },
        )
        .unwrap()
    }

    #[test]
    fn registro_detalhado_so_no_forge_moderno_e_no_neoforge() {
        assert_eq!(jvm_args(&forge("1.20.1")), [mod_timing::TRACE_PROPERTY]);
        assert!(jvm_args(&forge("1.12.2")).is_empty());
        let neo = GameSpec::new(
            "1.21.1",
            LoaderSpec::NeoForge {
                version: "21.1.252".into(),
            },
        )
        .unwrap();
        assert_eq!(jvm_args(&neo), [mod_timing::TRACE_PROPERTY]);
        let fabric = GameSpec::new(
            "1.20.1",
            LoaderSpec::Fabric {
                version: "0.16.10".into(),
            },
        )
        .unwrap();
        assert!(jvm_args(&fabric).is_empty());
    }

    #[test]
    fn arquivos_do_spark_desde_o_inicio_do_jogo() {
        let dir = tempfile::tempdir().unwrap();
        let spark_dir = dir.path().join("config/spark");
        std::fs::create_dir_all(spark_dir.join("tmp")).unwrap();
        for name in [
            "a.sparkprofile",
            "b.sparkheap",
            "config.json",
            "activity.json",
        ] {
            std::fs::write(spark_dir.join(name), b"x").unwrap();
        }
        let names = |since| -> Vec<String> {
            spark_files(&spark_dir, since)
                .into_iter()
                .map(|file| file.name)
                .collect()
        };
        let mut found = names(SystemTime::now() - Duration::from_secs(60));
        found.sort();
        assert_eq!(found, ["a.sparkprofile", "b.sparkheap"]);
        // Salvos antes do jogo abrir (um teste anterior): ficam de fora.
        assert!(names(SystemTime::now() + Duration::from_secs(60)).is_empty());
        assert!(spark_files(&dir.path().join("nada"), SystemTime::UNIX_EPOCH).is_empty());
    }

    #[test]
    fn log_velho_ou_ausente_e_fabric_sem_dado() {
        let dir = tempfile::tempdir().unwrap();
        let since = SystemTime::now() + Duration::from_secs(60);
        assert_eq!(
            load_times(dir.path(), since, &forge("1.20.1")).status,
            TimingStatus::LogMissing
        );
        std::fs::create_dir_all(dir.path().join("logs")).unwrap();
        std::fs::write(dir.path().join("logs/debug.log"), "x\n").unwrap();
        // O debug.log do teste anterior não vale.
        assert_eq!(
            load_times(dir.path(), since, &forge("1.20.1")).status,
            TimingStatus::LogMissing
        );
        assert_eq!(
            load_times(dir.path(), SystemTime::UNIX_EPOCH, &forge("1.20.1")).status,
            TimingStatus::NoLines
        );
        let fabric = GameSpec::new(
            "1.20.1",
            LoaderSpec::Fabric {
                version: "0.16.10".into(),
            },
        )
        .unwrap();
        assert_eq!(
            load_times(dir.path(), SystemTime::UNIX_EPOCH, &fabric).status,
            TimingStatus::NotRecorded
        );
    }

    #[test]
    fn nomes_vem_do_item_e_caem_para_o_id() {
        let owners = Owners {
            index: None,
            mods: HashMap::from([("jei".to_owned(), "mods/jei.pw.toml".to_owned())]),
            names: HashMap::from([(
                "mods/jei.pw.toml".to_owned(),
                "Just Enough Items (JEI)".to_owned(),
            )]),
        };
        assert_eq!(
            owners.item_of_mod("jei").as_deref(),
            Some("mods/jei.pw.toml")
        );
        assert_eq!(
            owners.name(Some("mods/jei.pw.toml"), Some("jei")),
            "Just Enough Items (JEI)"
        );
        assert_eq!(owners.name(None, Some("spark")), "spark");
        assert_eq!(owners.name(Some("mods/x.pw.toml"), None), "mods/x.pw.toml");
        assert_eq!(owners.class("mezz.jei.Jei"), None);
    }

    #[test]
    fn spark_no_servidor_pelo_descritor_do_jar() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_spark(dir.path()));
        // Um jar com "spark" no nome mas sem o descritor do spark não conta.
        std::fs::write(dir.path().join("sparkles-1.0.jar"), b"PK\x05\x06").unwrap();
        assert!(!has_spark(dir.path()));
        let jar = zip_with(
            "META-INF/mods.toml",
            "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"GPL-3.0\"\n[[mods]]\nmodId=\"spark\"\nversion=\"1.10.53\"\n",
        );
        std::fs::write(dir.path().join("spark-1.10.53-forge.jar"), jar).unwrap();
        assert!(has_spark(dir.path()));
    }

    fn zip_with(name: &str, text: &str) -> Vec<u8> {
        use std::io::Write as _;
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(text.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        buffer.into_inner()
    }
}
