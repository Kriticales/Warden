//! Tempo de carregamento por mod, lido do log do loader no "Testar com perfil de desempenho"
//! (ARCHITECTURE §7.7; R5A §7.2; ROADMAP L-12).
//!
//! Cada faixa de versão registra de um jeito ([`plan`]):
//! - **Forge 1.8 a 1.12.2**: `Bar Step: <fase> - <mod> took X.XXXs` no `logs/debug.log`, sempre
//!   ligado (o tempo já vem pronto, por fase e por mod, com o nome de exibição do mod);
//! - **Forge 1.7.10**: os pares `Sending event <evento> to mod <id>` / `Sent event …` do
//!   `logs/fml-client-latest.log` (o FML grava tudo nele), com horário em segundos;
//! - **Forge 1.13+ e NeoForge**: só com o nível `TRACE` no `debug.log`, que este teste liga
//!   ([`trace_jvm_args`]): os pares `Loading mod instance`/`Loaded mod instance` e
//!   `Firing event for modid`/`Fired event for modid`, casados **por thread** (a construção dos
//!   mods é paralela); o NeoForge também avisa `Mod '<id>' took <tempo> to run a deferred
//!   task.` quando passa de 1 s;
//! - **Fabric, Quilt e vanilla**: não há dado por mod ([`TimingStatus::NotRecorded`]).
//!
//! Só o código de cada mod nas fases do loader entra na medida: mixins e a recarga de recursos
//! (texturas, modelos) ficam de fora, e a interface diz isso.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::Loader;

/// Propriedade do Forge e do NeoForge que liga o `TRACE` no `debug.log` (`log4j2.xml` do
/// `fmlloader`: `${sys:forge.logging.debugFile.level:-debug}`; conferido no Forge 1.20.1-47.4.10
/// e no NeoForge 21.1.252).
pub const TRACE_PROPERTY: &str = "-Dforge.logging.debugFile.level=trace";

/// Quantos mods o resultado guarda (os mais demorados).
pub const MAX_MODS: usize = 200;

/// Fases do Forge 1.12.2 que não são carregamento (acontecem ao abrir e fechar um mundo).
const WORLD_PHASES: &[&str] = &[
    "ServerAboutToStart",
    "ServerStarting",
    "ServerStarted",
    "ServerStopping",
    "ServerStopped",
];

/// Ids (e nomes de exibição do Forge 1.12.2) do próprio jogo e do loader.
const GAME_AND_LOADER: &[&str] = &[
    "minecraft",
    "mcp",
    "fml",
    "forge",
    "neoforge",
    "minecraft coder pack",
    "forge mod loader",
    "minecraft forge",
];

/// Como o log registra o tempo de cada mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum TimingSource {
    /// `Bar Step` do Forge 1.8 a 1.12.2 (nome de exibição do mod).
    BarStep,
    /// `Sending event`/`Sent event` do FML 1.7.10 (id do mod; horário em segundos).
    FmlEvents,
    /// Pares em `TRACE` do Forge 1.13+ e do NeoForge (id do mod).
    Trace,
}

/// O que a leitura conseguiu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum TimingStatus {
    /// Tempos lidos.
    Measured,
    /// O loader não registra o tempo de cada mod (Fabric, Quilt, vanilla).
    NotRecorded,
    /// O arquivo de log do loader não existe (o jogo nem chegou a carregar os mods, ou a
    /// configuração de log não o grava).
    LogMissing,
    /// O log existe, mas não tem as linhas de tempo (o jogo fechou antes, ou o registro
    /// detalhado não ligou).
    NoLines,
}

/// Uma fase de um mod.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PhaseTime {
    /// Fase ou evento do loader (`PreInitialization`, `FMLCommonSetupEvent`, `instance`…).
    pub phase: String,
    /// Duração somada, em ms.
    pub ms: u32,
}

/// O tempo de um mod.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModLoadTime {
    /// Como o log chama o mod: o id (`jei`) ou, no Forge 1.8 a 1.12.2, o nome de exibição
    /// (`Just Enough Items`).
    pub key: String,
    /// Duração somada de todas as fases, em ms.
    pub ms: u32,
    /// As fases, na ordem em que apareceram.
    pub phases: Vec<PhaseTime>,
}

/// O tempo por mod de um teste.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModTimings {
    /// O que a leitura conseguiu.
    pub status: TimingStatus,
    /// Como o log registra (sem valor quando o loader não registra).
    pub source: Option<TimingSource>,
    /// O arquivo lido, relativo à pasta do jogo (`logs/debug.log`).
    pub log: Option<String>,
    /// Os mods (sem o jogo e o loader), do mais demorado ao menos, até [`MAX_MODS`].
    pub mods: Vec<ModLoadTime>,
    /// O tempo do próprio jogo e do loader, somado, em ms.
    pub game_and_loader_ms: u32,
    /// O tempo de todos os mods, somado (inclusive os que passaram de [`MAX_MODS`]), em ms.
    pub mods_ms: u32,
}

impl ModTimings {
    /// Sem dado, com o motivo.
    #[must_use]
    pub fn without_data(status: TimingStatus, plan: &TimingPlan) -> Self {
        Self {
            status,
            source: plan.source,
            log: None,
            mods: Vec::new(),
            game_and_loader_ms: 0,
            mods_ms: 0,
        }
    }
}

/// Onde e como ler o tempo por mod de um jogo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingPlan {
    /// Como o log registra (`None`: o loader não registra).
    pub source: Option<TimingSource>,
    /// Arquivos candidatos, relativos à pasta do jogo, na ordem de preferência.
    pub logs: Vec<&'static str>,
    /// Argumentos da JVM que este teste precisa passar para o log registrar.
    pub jvm_args: Vec<String>,
}

/// O plano de leitura para o Minecraft e o loader do teste (`server`: servidor dedicado, que
/// grava o `fml-server-latest.log` no 1.7.10).
#[must_use]
pub fn plan(minecraft: &str, loader: Loader, server: bool) -> TimingPlan {
    let none = TimingPlan {
        source: None,
        logs: Vec::new(),
        jvm_args: Vec::new(),
    };
    match loader {
        Loader::Vanilla | Loader::Fabric | Loader::Quilt => none,
        Loader::NeoForge => modern(),
        Loader::Forge => match minor(minecraft) {
            Some(minor) if minor <= 7 => TimingPlan {
                source: Some(TimingSource::FmlEvents),
                logs: vec![if server {
                    "logs/fml-server-latest.log"
                } else {
                    "logs/fml-client-latest.log"
                }],
                jvm_args: Vec::new(),
            },
            Some(minor) if minor <= 12 => TimingPlan {
                source: Some(TimingSource::BarStep),
                logs: vec!["logs/debug.log", "logs/latest.log"],
                jvm_args: Vec::new(),
            },
            _ => modern(),
        },
    }
}

fn modern() -> TimingPlan {
    TimingPlan {
        source: Some(TimingSource::Trace),
        logs: vec!["logs/debug.log"],
        jvm_args: vec![TRACE_PROPERTY.to_owned()],
    }
}

/// Os argumentos da JVM do teste com perfil de desempenho (o `TRACE` do Forge moderno e do
/// NeoForge; nada nos outros).
#[must_use]
pub fn trace_jvm_args(minecraft: &str, loader: Loader) -> Vec<String> {
    plan(minecraft, loader, false).jvm_args
}

/// A versão "menor" do Minecraft no esquema `1.X(.Y)` (`1.12.2` → 12). As versões no esquema
/// novo (`26.2`) e as que não dá para ler devolvem `None` (tratadas como as mais novas).
fn minor(minecraft: &str) -> Option<u32> {
    let rest = minecraft.trim().strip_prefix("1.")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Lê o tempo por mod do texto de um log no formato dado. `log` é o nome relativo gravado no
/// resultado.
#[must_use]
pub fn parse(source: TimingSource, text: &str, log: &str) -> ModTimings {
    let mut acc = Accumulator::default();
    match source {
        TimingSource::BarStep => parse_bar_steps(text, &mut acc),
        TimingSource::FmlEvents => parse_fml_events(text, &mut acc),
        TimingSource::Trace => parse_trace(text, &mut acc),
    }
    acc.finish(source, log)
}

/// Soma por mod e por fase, na ordem em que aparecem.
#[derive(Default)]
struct Accumulator {
    order: Vec<String>,
    mods: HashMap<String, Vec<PhaseTime>>,
}

impl Accumulator {
    fn add(&mut self, key: &str, phase: &str, ms: u64) {
        if is_world_phase(phase) {
            return;
        }
        let ms = u32::try_from(ms).unwrap_or(u32::MAX);
        let phases = self.mods.entry(key.to_owned()).or_insert_with(|| {
            self.order.push(key.to_owned());
            Vec::new()
        });
        match phases.iter_mut().find(|known| known.phase == phase) {
            Some(known) => known.ms = known.ms.saturating_add(ms),
            None => phases.push(PhaseTime {
                phase: phase.to_owned(),
                ms,
            }),
        }
    }

    fn finish(mut self, source: TimingSource, log: &str) -> ModTimings {
        let mut game_and_loader_ms = 0u32;
        let mut mods = Vec::new();
        for key in self.order {
            let mut phases = self.mods.remove(&key).unwrap_or_default();
            let ms = phases
                .iter()
                .fold(0u32, |sum, phase| sum.saturating_add(phase.ms));
            // As fases de 0 ms só ocupariam espaço no `session.json`.
            phases.retain(|phase| phase.ms > 0);
            if is_game_or_loader(&key) {
                game_and_loader_ms = game_and_loader_ms.saturating_add(ms);
            } else {
                mods.push(ModLoadTime { key, ms, phases });
            }
        }
        let status = if mods.is_empty() && game_and_loader_ms == 0 {
            TimingStatus::NoLines
        } else {
            TimingStatus::Measured
        };
        // Estável: empates ficam na ordem do log.
        mods.sort_by_key(|item| std::cmp::Reverse(item.ms));
        let mods_ms = mods
            .iter()
            .fold(0u32, |sum, item| sum.saturating_add(item.ms));
        mods.truncate(MAX_MODS);
        ModTimings {
            status,
            source: Some(source),
            log: Some(log.to_owned()),
            mods,
            game_and_loader_ms,
            mods_ms,
        }
    }
}

/// Se o id (ou nome de exibição) é do jogo ou do loader.
#[must_use]
pub fn is_game_or_loader(key: &str) -> bool {
    let key = key.trim().to_ascii_lowercase();
    GAME_AND_LOADER.contains(&key.as_str())
}

static BAR_STEP: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"Bar Step: (?P<phase>\S+) - (?P<name>.+?) took (?P<secs>\d+(?:\.\d+)?)s\s*$").ok()
});

fn parse_bar_steps(text: &str, acc: &mut Accumulator) {
    let Some(pattern) = BAR_STEP.as_ref() else {
        return;
    };
    for line in text.lines() {
        let Some(found) = pattern.captures(line) else {
            continue;
        };
        let phase = &found["phase"];
        if let Some(ms) = seconds_to_ms(&found["secs"]) {
            acc.add(found["name"].trim(), phase, ms);
        }
    }
}

/// `1.234` (segundos) → 1234 ms.
fn seconds_to_ms(text: &str) -> Option<u64> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let whole: u64 = whole.parse().ok()?;
    let mut digits: String = fraction.chars().take(3).collect();
    while digits.len() < 3 {
        digits.push('0');
    }
    let fraction: u64 = digits.parse().ok()?;
    Some(whole.saturating_mul(1000).saturating_add(fraction))
}

/// `[HH:MM:SS] [thread/NIVEL] [logger]: mensagem` (FML 1.7.10).
static FML_LINE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"^\[(?P<h>\d\d):(?P<m>\d\d):(?P<s>\d\d)\] \[(?P<thread>.+)/[A-Z]+\] \[[^\]]*\]: (?P<what>Sending|Sent) event (?P<event>\S+) to mod (?P<id>\S+)\s*$",
    )
    .ok()
});

fn parse_fml_events(text: &str, acc: &mut Accumulator) {
    let Some(pattern) = FML_LINE.as_ref() else {
        return;
    };
    let mut open: HashMap<(String, String, String), u64> = HashMap::new();
    for line in text.lines() {
        let Some(found) = pattern.captures(line) else {
            continue;
        };
        let Some(at) = clock_ms(&found["h"], &found["m"], &found["s"], "0") else {
            continue;
        };
        let key = (
            found["thread"].to_owned(),
            found["event"].to_owned(),
            found["id"].to_owned(),
        );
        if &found["what"] == "Sending" {
            open.insert(key, at);
        } else if let Some(start) = open.remove(&key) {
            acc.add(&key.2, short_event(&key.1), elapsed(start, at));
        }
    }
}

/// `[18Oct2026 00:41:42.240] [thread/NIVEL] [logger/MARCADOR]: mensagem` (Forge 1.13+ e
/// NeoForge, `debug.log`).
static TRACE_LINE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"^\[\d{1,2}[A-Za-z]{3}\d{4} (?P<h>\d\d):(?P<m>\d\d):(?P<s>\d\d)\.(?P<ms>\d{3})\] \[(?P<thread>.+?)/(?P<level>[A-Z]+)\] \[[^\]]*\]: (?P<msg>.*)$",
    )
    .ok()
});

static INSTANCE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^(?P<what>Loading|Loaded) mod instance (?P<id>\S+) of type ").ok()
});

static EVENT: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^(?P<what>Firing|Fired) event for modid (?P<id>\S+) : (?P<event>\S+)").ok()
});

static DEFERRED: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^Mod '(?P<id>[^']+)' took (?P<value>\d+(?:\.\d+)?) ?(?P<unit>ns|μs|us|ms|s|min|h|d) to run a deferred task\.").ok()
});

fn parse_trace(text: &str, acc: &mut Accumulator) {
    let (Some(line_pattern), Some(instance), Some(event), Some(deferred)) = (
        TRACE_LINE.as_ref(),
        INSTANCE.as_ref(),
        EVENT.as_ref(),
        DEFERRED.as_ref(),
    ) else {
        return;
    };
    let mut open: HashMap<(String, String, String), u64> = HashMap::new();
    for line in text.lines() {
        let Some(found) = line_pattern.captures(line) else {
            continue;
        };
        let Some(at) = clock_ms(&found["h"], &found["m"], &found["s"], &found["ms"]) else {
            continue;
        };
        let thread = &found["thread"];
        let message = &found["msg"];
        let (starting, id, phase) = if let Some(pair) = instance.captures(message) {
            (
                &pair["what"] == "Loading",
                pair["id"].to_owned(),
                "instance".to_owned(),
            )
        } else if let Some(pair) = event.captures(message) {
            (
                &pair["what"] == "Firing",
                pair["id"].to_owned(),
                short_event(&pair["event"]).to_owned(),
            )
        } else if let Some(task) = deferred.captures(message) {
            if let Some(ms) = duration_ms(&task["value"], &task["unit"]) {
                acc.add(&task["id"], "deferred", ms);
            }
            continue;
        } else {
            continue;
        };
        let key = (thread.to_owned(), id, phase);
        if starting {
            open.insert(key, at);
        } else if let Some(start) = open.remove(&key) {
            acc.add(&key.1, &key.2, elapsed(start, at));
        }
    }
}

/// `net.minecraftforge.fml.event.lifecycle.FMLCommonSetupEvent` → `FMLCommonSetupEvent`; sem o
/// `@59cfabaf` do `toString` de alguns eventos do Forge.
fn short_event(event: &str) -> &str {
    let name = event.split('@').next().unwrap_or(event);
    name.rsplit(['.', '$']).next().unwrap_or(name)
}

/// Se a fase (ou o evento) é de abrir ou fechar um mundo, não do carregamento
/// (`ServerStopped`, `FMLServerAboutToStartEvent`).
fn is_world_phase(phase: &str) -> bool {
    let name = phase.strip_prefix("FML").unwrap_or(phase);
    let name = name.strip_suffix("Event").unwrap_or(name);
    WORLD_PHASES.contains(&name)
}

/// Horário do dia em ms.
fn clock_ms(hours: &str, minutes: &str, seconds: &str, millis: &str) -> Option<u64> {
    let hours: u64 = hours.parse().ok()?;
    let minutes: u64 = minutes.parse().ok()?;
    let seconds: u64 = seconds.parse().ok()?;
    let millis: u64 = millis.parse().ok()?;
    Some(((hours * 60 + minutes) * 60 + seconds) * 1000 + millis)
}

/// De um horário do dia a outro, passando da meia-noite se preciso.
fn elapsed(start: u64, end: u64) -> u64 {
    const DAY_MS: u64 = 24 * 60 * 60 * 1000;
    if end >= start {
        end - start
    } else {
        end + DAY_MS - start
    }
}

/// `1.532` `s` (o `Stopwatch` do Guava) → ms.
fn duration_ms(value: &str, unit: &str) -> Option<u64> {
    let value: f64 = value.parse().ok()?;
    let factor = match unit {
        "ns" => 1e-6,
        "μs" | "us" => 1e-3,
        "ms" => 1.0,
        "s" => 1e3,
        "min" => 60e3,
        "h" => 3600e3,
        "d" => 86_400e3,
        _ => return None,
    };
    let ms = (value * factor).round();
    // Valores negativos ou fora da faixa não vêm do log; ficam de fora.
    if ms.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&ms) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // conferido acima
        Some(ms as u64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plano_por_faixa_de_versao() {
        let forge = |minecraft| plan(minecraft, Loader::Forge, false);
        assert_eq!(forge("1.12.2").source, Some(TimingSource::BarStep));
        assert_eq!(forge("1.12.2").logs, ["logs/debug.log", "logs/latest.log"]);
        assert!(forge("1.12.2").jvm_args.is_empty());
        assert_eq!(forge("1.8.9").source, Some(TimingSource::BarStep));
        assert_eq!(forge("1.7.10").source, Some(TimingSource::FmlEvents));
        assert_eq!(forge("1.7.10").logs, ["logs/fml-client-latest.log"]);
        assert_eq!(
            plan("1.7.10", Loader::Forge, true).logs,
            ["logs/fml-server-latest.log"]
        );
        for modern in ["1.13.2", "1.16.5", "1.20.1", "1.21.1", "26.2", "lixo"] {
            assert_eq!(forge(modern).source, Some(TimingSource::Trace), "{modern}");
            assert_eq!(forge(modern).jvm_args, [TRACE_PROPERTY]);
        }
        assert_eq!(
            plan("1.21.1", Loader::NeoForge, false).jvm_args,
            [TRACE_PROPERTY]
        );
        for loader in [Loader::Fabric, Loader::Quilt, Loader::Vanilla] {
            let none = plan("1.20.1", loader, false);
            assert_eq!(none.source, None);
            assert!(none.logs.is_empty() && none.jvm_args.is_empty());
        }
        assert!(trace_jvm_args("1.12.2", Loader::Forge).is_empty());
        assert_eq!(trace_jvm_args("1.20.1", Loader::Forge), [TRACE_PROPERTY]);
        assert!(trace_jvm_args("1.20.1", Loader::Fabric).is_empty());
    }

    #[test]
    fn bar_step_soma_fases_e_separa_jogo_e_loader() {
        let log = "\
[10:00:00] [Client thread/DEBUG] [FML]: Bar Step: Construction - Minecraft Forge took 0.031s
[10:00:00] [Client thread/DEBUG] [FML]: Bar Step: Construction - Just Enough Items took 0.051s
[10:00:01] [Client thread/DEBUG] [FML]: Bar Step: PreInitialization - Just Enough Items took 1.2s
[10:00:01] [Client thread/DEBUG] [FML]: Bar Step: PreInitialization - Mod - com hífen took 2.000s
[10:00:02] [Client thread/DEBUG] [FML]: Bar Step: ServerStopped - Just Enough Items took 9.000s
[10:00:02] [Client thread/DEBUG] [FML]: Bar Finished: Construction took 1.119s
";
        let timings = parse(TimingSource::BarStep, log, "logs/debug.log");
        assert_eq!(timings.status, TimingStatus::Measured);
        assert_eq!(timings.log.as_deref(), Some("logs/debug.log"));
        assert_eq!(timings.game_and_loader_ms, 31);
        assert_eq!(timings.mods_ms, 3251);
        let keys: Vec<(&str, u32)> = timings
            .mods
            .iter()
            .map(|item| (item.key.as_str(), item.ms))
            .collect();
        assert_eq!(
            keys,
            [("Mod - com hífen", 2000), ("Just Enough Items", 1251)]
        );
        assert_eq!(
            timings.mods[1].phases,
            [
                PhaseTime {
                    phase: "Construction".into(),
                    ms: 51
                },
                PhaseTime {
                    phase: "PreInitialization".into(),
                    ms: 1200
                }
            ]
        );
    }

    #[test]
    fn eventos_do_fml_1710_casados_por_thread_evento_e_mod() {
        let log = "\
[00:45:39] [Client thread/TRACE] [mcp/mcp]: Sending event FMLConstructionEvent to mod mcp
[00:45:40] [Client thread/TRACE] [mcp/mcp]: Sent event FMLConstructionEvent to mod mcp
[23:59:59] [Client thread/TRACE] [wand/wand]: Sending event FMLPreInitializationEvent to mod wand
[00:00:02] [Client thread/TRACE] [wand/wand]: Sent event FMLPreInitializationEvent to mod wand
[00:00:02] [Client thread/TRACE] [wand/wand]: Sent event FMLInitializationEvent to mod wand
";
        let timings = parse(TimingSource::FmlEvents, log, "logs/fml-client-latest.log");
        assert_eq!(timings.game_and_loader_ms, 1000);
        assert_eq!(timings.mods.len(), 1);
        assert_eq!(timings.mods[0].key, "wand");
        // Passa da meia-noite; o "Sent" sem "Sending" não conta.
        assert_eq!(timings.mods[0].ms, 3000);
        assert_eq!(timings.mods[0].phases[0].phase, "FMLPreInitializationEvent");
    }

    #[test]
    fn trace_casa_por_thread_e_le_a_tarefa_adiada_do_neoforge() {
        let log = "\
[18Oct2026 00:41:42.240] [modloading-worker-0/TRACE] [net.minecraftforge.fml.javafmlmod.FMLModContainer/LOADING]: Loading mod instance alfa of type a.Alfa
[18Oct2026 00:41:42.241] [modloading-worker-1/TRACE] [net.minecraftforge.fml.javafmlmod.FMLModContainer/LOADING]: Loading mod instance beta of type b.Beta
[18Oct2026 00:41:42.300] [modloading-worker-1/TRACE] [net.minecraftforge.fml.javafmlmod.FMLModContainer/LOADING]: Loaded mod instance beta of type b.Beta
[18Oct2026 00:41:42.640] [modloading-worker-0/TRACE] [net.minecraftforge.fml.javafmlmod.FMLModContainer/LOADING]: Loaded mod instance alfa of type a.Alfa
[18Oct2026 00:41:43.000] [modloading-worker-2/TRACE] [net.neoforged.fml.ModContainer/LOADING]: Firing event for modid alfa : net.neoforged.fml.event.lifecycle.FMLCommonSetupEvent
[18Oct2026 00:41:43.010] [modloading-worker-3/TRACE] [net.neoforged.fml.ModContainer/LOADING]: Fired event for modid alfa : net.neoforged.fml.event.lifecycle.FMLCommonSetupEvent
[18Oct2026 00:41:43.120] [modloading-worker-2/TRACE] [net.neoforged.fml.ModContainer/LOADING]: Fired event for modid alfa : net.neoforged.fml.event.lifecycle.FMLCommonSetupEvent
[18Oct2026 00:41:44.000] [Render thread/WARN] [net.neoforged.fml.DeferredWorkQueue/LOADING]: Mod 'beta' took 1.532 s to run a deferred task.
[18Oct2026 00:41:44.000] [Render thread/WARN] [net.neoforged.fml.DeferredWorkQueue/LOADING]: Mod 'beta' took 2.001 min to run a deferred task.
[18Oct2026 00:41:45.000] [main/TRACE] [x/LOADING]: Firing event for modid forge : FMLConstructModEvent
[18Oct2026 00:41:45.500] [main/TRACE] [x/LOADING]: Fired event for modid forge : FMLConstructModEvent
";
        let timings = parse(TimingSource::Trace, log, "logs/debug.log");
        let found: Vec<(&str, u32)> = timings
            .mods
            .iter()
            .map(|item| (item.key.as_str(), item.ms))
            .collect();
        // beta: 59 ms da instância + 1532 ms + 120060 ms das tarefas adiadas.
        assert_eq!(found, [("beta", 121_651), ("alfa", 520)]);
        assert_eq!(timings.mods[1].phases[1].phase, "FMLCommonSetupEvent");
        assert_eq!(timings.mods[1].phases[1].ms, 120);
        assert_eq!(timings.game_and_loader_ms, 500);
    }

    #[test]
    fn log_sem_linhas_de_tempo() {
        let timings = parse(TimingSource::Trace, "[x] nada\n", "logs/debug.log");
        assert_eq!(timings.status, TimingStatus::NoLines);
        assert!(timings.mods.is_empty());
        let plan = plan("1.20.1", Loader::Fabric, false);
        let none = ModTimings::without_data(TimingStatus::NotRecorded, &plan);
        assert_eq!(none.source, None);
        assert_eq!(none.status, TimingStatus::NotRecorded);
    }

    #[test]
    fn unidades_do_guava_e_valores_estranhos() {
        assert_eq!(duration_ms("1.532", "s"), Some(1532));
        assert_eq!(duration_ms("250.0", "ms"), Some(250));
        assert_eq!(duration_ms("900", "μs"), Some(1));
        assert_eq!(duration_ms("1", "h"), Some(3_600_000));
        assert_eq!(duration_ms("1", "anos"), None);
        assert_eq!(duration_ms("1e400", "s"), None);
        assert_eq!(seconds_to_ms("0.0071"), Some(7));
        assert_eq!(seconds_to_ms("2"), Some(2000));
        assert_eq!(seconds_to_ms("x"), None);
        assert_eq!(minor("1.7.10"), Some(7));
        assert_eq!(minor("26.2"), None);
        assert!(is_game_or_loader("Minecraft Coder Pack"));
        assert!(is_game_or_loader("FML"));
        assert!(!is_game_or_loader("jei"));
    }

    #[test]
    fn mais_mods_que_o_limite_somam_no_total() {
        use std::fmt::Write as _;
        let mut log = String::new();
        for index in 0..(MAX_MODS + 5) {
            let _ = writeln!(log, "Bar Step: Construction - Mod {index} took 0.001s");
        }
        let timings = parse(TimingSource::BarStep, &log, "logs/debug.log");
        assert_eq!(timings.mods.len(), MAX_MODS);
        assert_eq!(timings.mods_ms, u32::try_from(MAX_MODS + 5).unwrap());
    }
}
