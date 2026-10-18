//! Perfis do spark salvos na instância (`config/spark/*.sparkprofile` e `*.sparkheap`):
//! leitura com `prost` a partir dos `.proto` públicos do spark (`proto/`, GPL-3.0; o código Rust
//! em [`generated`]) e o resumo mostrado na parte "Desempenho" do resultado do teste
//! (ARCHITECTURE §7.7; R5A §7.4; ROADMAP L-12).
//!
//! - **Perfil de processamento** (`.sparkprofile`, [`SparkKind::Profile`]): o tempo por tick (das
//!   janelas de tempo do spark, ou das estatísticas da plataforma) e os mods que mais usaram
//!   processamento. Cada amostra vai para o mod do quadro **mais fundo** da pilha que tem dono
//!   (o mod que chamou o código do jogo, ou o mod chamado pelo jogo); amostras sem nenhum
//!   quadro de mod ficam com "o jogo, o loader e o Java". As amostras em que a thread só
//!   esperava (dormindo ou parada até o próximo tick) não contam: as porcentagens são do
//!   processamento de verdade.
//! - **Resumo da memória** (`.sparkheap`, [`SparkKind::Heap`]): os mods que mais ocupam memória,
//!   pelas classes de cada um.
//!
//! O dono de cada classe vem de quem chama ([`ClassOwner`], o índice pacote → jar → mod da D-05
//! na `warden-app`) e, quando ele não sabe, do próprio arquivo (`class_sources`, que o spark
//! preenche com o id do mod). Só números e nomes de mod saem daqui: o nome e o UUID de quem
//! rodou o comando, os argumentos da JVM e as configs do servidor que o arquivo traz nunca
//! entram no resumo.

#[allow(
    missing_docs,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    rust_2018_idioms
)]
pub mod generated;

use std::collections::HashMap;

use prost::Message as _;
use serde::{Deserialize, Serialize};

use crate::mod_timing::is_game_or_loader;

use self::generated::{
    HeapData, PlatformMetadata, PlatformStatistics, SamplerData, StackTraceNode, ThreadNode,
    WindowStatistics, platform_metadata,
};

/// Maior arquivo lido (os perfis normais têm de dezenas de KB a poucos MB).
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Quantos mods o resumo guarda.
pub const TOP_SOURCES: usize = 8;

/// Extensão dos perfis de processamento.
pub const PROFILE_EXTENSION: &str = "sparkprofile";
/// Extensão dos resumos de memória.
pub const HEAP_EXTENSION: &str = "sparkheap";

/// Métodos em que a thread está só esperando (quadro do topo da pilha).
const IDLE_FRAMES: &[(&str, &str)] = &[
    ("java.lang.Thread", "sleep"),
    ("java.lang.Thread", "sleep0"),
    ("java.lang.Thread", "sleepNanos"),
    ("java.lang.Thread", "sleepNanos0"),
    ("java.lang.Thread", "yield"),
    ("java.lang.Thread", "yield0"),
    ("java.lang.Thread", "onSpinWait"),
    ("java.lang.Object", "wait"),
    ("java.lang.Object", "wait0"),
    ("jdk.internal.misc.Unsafe", "park"),
    ("sun.misc.Unsafe", "park"),
    ("java.util.concurrent.locks.LockSupport", "park"),
    ("java.util.concurrent.locks.LockSupport", "parkNanos"),
    ("java.util.concurrent.locks.LockSupport", "parkUntil"),
];

/// O tipo de arquivo do spark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SparkKind {
    /// Perfil de processamento (`.sparkprofile`).
    Profile,
    /// Resumo da memória (`.sparkheap`).
    Heap,
}

impl SparkKind {
    /// O tipo pelo nome do arquivo (`profile-2026-10-18_00.42.48.sparkprofile`).
    #[must_use]
    pub fn of_file(name: &str) -> Option<Self> {
        let (_, extension) = name.rsplit_once('.')?;
        if extension.eq_ignore_ascii_case(PROFILE_EXTENSION) {
            Some(Self::Profile)
        } else if extension.eq_ignore_ascii_case(HEAP_EXTENSION) {
            Some(Self::Heap)
        } else {
            None
        }
    }
}

/// Onde o spark rodou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SparkSide {
    /// No jogo (`/sparkc`).
    Client,
    /// No servidor.
    Server,
    /// Outra plataforma (proxy, aplicação).
    Other,
}

/// O jogo em que o perfil foi salvo.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SparkPlatform {
    /// Jogo ou servidor.
    pub side: SparkSide,
    /// Plataforma (`Forge`, `NeoForge`, `Fabric`).
    pub name: String,
    /// Versão da plataforma.
    pub version: String,
    /// Versão do Minecraft, quando o arquivo diz.
    pub minecraft: Option<String>,
}

/// O tempo por tick (e os ticks por segundo) no período do perfil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TickSummary {
    /// Mediana do tempo por tick, em ms.
    #[specta(type = specta_typescript::Number)]
    pub mspt_median: f64,
    /// Pior tick, em ms.
    #[specta(type = specta_typescript::Number)]
    pub mspt_max: f64,
    /// 95% dos ticks levaram até este tempo, em ms (só nas estatísticas da plataforma).
    #[specta(type = Option<specta_typescript::Number>)]
    pub mspt_p95: Option<f64>,
    /// Ticks por segundo, em média (20 é o normal do jogo).
    #[specta(type = Option<specta_typescript::Number>)]
    pub tps: Option<f64>,
    /// Ticks medidos.
    pub ticks: Option<u32>,
}

/// A parte de um mod (ou do jogo) no processamento ou na memória.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceShare {
    /// O item do pack (caminho do metafile), quando quem chamou sabe.
    pub item: Option<String>,
    /// O id do mod, quando se sabe.
    pub mod_id: Option<String>,
    /// Porcentagem do total (processamento sem espera, ou bytes da memória), com uma casa.
    pub percent: f64,
    /// Quanto: ms de processamento (perfil) ou bytes (memória).
    pub amount: u64,
}

/// O resumo de um arquivo do spark.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SparkSummary {
    /// Perfil de processamento ou memória.
    pub kind: SparkKind,
    /// Onde rodou.
    pub platform: Option<SparkPlatform>,
    /// Início do perfil (ms desde 1970; a memória só tem o fim).
    pub started_at_ms: Option<i64>,
    /// Quando terminou (o perfil) ou foi gerado (a memória), em ms desde 1970.
    pub ended_at_ms: Option<i64>,
    /// O tempo por tick (só no perfil, quando o spark mediu).
    pub ticks: Option<TickSummary>,
    /// Os mods que mais usaram processamento ou memória (sem o jogo), do maior ao menor.
    pub top: Vec<SourceShare>,
    /// A parte do jogo, do loader e do Java (o que não é de nenhum mod), em %.
    pub game_percent: f64,
    /// Processamento medido sem as esperas (perfil), em ms; bytes somados (memória).
    pub total: u64,
}

/// O dono de uma classe, como quem chama sabe (o índice pacote → jar → mod).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ClassOwner {
    /// O item do pack.
    pub item: Option<String>,
    /// O id do mod.
    pub mod_id: Option<String>,
}

/// Erro ao ler um arquivo do spark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparkReadError {
    /// O arquivo passa de [`MAX_FILE_BYTES`].
    TooLarge,
    /// Não é um arquivo do spark (ou está corrompido).
    Invalid(String),
}

impl std::fmt::Display for SparkReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge => write!(formatter, "arquivo do spark grande demais"),
            Self::Invalid(reason) => write!(formatter, "arquivo do spark ilegível: {reason}"),
        }
    }
}

impl std::error::Error for SparkReadError {}

/// Lê e resume um arquivo do spark. `owner` diz o dono de uma classe (`None`: não sabe).
pub fn summarize(
    kind: SparkKind,
    bytes: &[u8],
    owner: &dyn Fn(&str) -> Option<ClassOwner>,
) -> Result<SparkSummary, SparkReadError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(SparkReadError::TooLarge);
    }
    match kind {
        SparkKind::Profile => {
            let data = SamplerData::decode(bytes)
                .map_err(|error| SparkReadError::Invalid(error.to_string()))?;
            if data.metadata.is_none() && data.threads.is_empty() {
                return Err(SparkReadError::Invalid("sem metadados nem threads".into()));
            }
            Ok(summarize_profile(&data, owner))
        }
        SparkKind::Heap => {
            let data = HeapData::decode(bytes)
                .map_err(|error| SparkReadError::Invalid(error.to_string()))?;
            if data.metadata.is_none() && data.entries.is_empty() {
                return Err(SparkReadError::Invalid("sem metadados nem classes".into()));
            }
            Ok(summarize_heap(&data, owner))
        }
    }
}

fn platform(metadata: Option<&PlatformMetadata>) -> Option<SparkPlatform> {
    let metadata = metadata?;
    let side = match platform_metadata::Type::try_from(metadata.r#type) {
        Ok(platform_metadata::Type::Client) => SparkSide::Client,
        Ok(platform_metadata::Type::Server) => SparkSide::Server,
        _ => SparkSide::Other,
    };
    Some(SparkPlatform {
        side,
        name: metadata.name.clone(),
        version: metadata.version.clone(),
        minecraft: Some(metadata.minecraft_version.clone()).filter(|version| !version.is_empty()),
    })
}

/// Tempo positivo em ms desde 1970, ou nada.
fn timestamp(value: i64) -> Option<i64> {
    (value > 0).then_some(value)
}

/// Com uma casa decimal.
fn one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn percent(part: f64, total: f64) -> f64 {
    if total > 0.0 {
        one_decimal(part * 100.0 / total)
    } else {
        0.0
    }
}

/// O tempo por tick das janelas de tempo do spark (uma por minuto) ou, sem elas, das
/// estatísticas da plataforma.
fn ticks(
    windows: &HashMap<i32, WindowStatistics>,
    platform: Option<&PlatformStatistics>,
) -> Option<TickSummary> {
    let mut measured: Vec<&WindowStatistics> = windows
        .values()
        .filter(|window| window.ticks > 0 && window.mspt_median.is_finite())
        .collect();
    if !measured.is_empty() {
        measured.sort_by(|a, b| a.mspt_median.total_cmp(&b.mspt_median));
        let ticks: u32 = measured
            .iter()
            .map(|window| u32::try_from(window.ticks).unwrap_or(0))
            .sum();
        let middle = measured.len() / 2;
        let median = if measured.len() % 2 == 1 {
            measured[middle].mspt_median
        } else {
            f64::midpoint(
                measured[middle - 1].mspt_median,
                measured[middle].mspt_median,
            )
        };
        let max = measured
            .iter()
            .map(|window| window.mspt_max)
            .fold(0.0_f64, f64::max);
        let weight: f64 = measured
            .iter()
            .map(|window| f64::from(window.ticks.max(0)))
            .sum();
        let tps = measured
            .iter()
            .filter(|window| window.tps.is_finite() && window.tps > 0.0)
            .map(|window| window.tps * f64::from(window.ticks.max(0)))
            .sum::<f64>()
            / weight;
        return Some(TickSummary {
            mspt_median: one_decimal(median),
            mspt_max: one_decimal(max),
            mspt_p95: None,
            tps: (tps.is_finite() && tps > 0.0).then(|| one_decimal(tps)),
            ticks: Some(ticks),
        });
    }
    let statistics = platform?;
    let mspt = statistics.mspt.as_ref()?.last1m.as_ref()?;
    if mspt.max <= 0.0 {
        return None;
    }
    Some(TickSummary {
        mspt_median: one_decimal(mspt.median),
        mspt_max: one_decimal(mspt.max),
        mspt_p95: Some(one_decimal(mspt.percentile95)),
        tps: statistics
            .tps
            .as_ref()
            .map(|tps| tps.last1m)
            .filter(|tps| tps.is_finite() && *tps > 0.0)
            .map(one_decimal),
        ticks: None,
    })
}

/// O dono de cada classe, perguntado uma vez só.
struct Owners<'a> {
    owner: &'a dyn Fn(&str) -> Option<ClassOwner>,
    spark: &'a HashMap<String, String>,
    known: HashMap<String, Option<ClassOwner>>,
}

impl<'a> Owners<'a> {
    fn new(
        owner: &'a dyn Fn(&str) -> Option<ClassOwner>,
        spark: &'a HashMap<String, String>,
    ) -> Self {
        Self {
            owner,
            spark,
            known: HashMap::new(),
        }
    }

    /// O dono (sem o jogo e o loader) de uma classe (`a.b.C`, `a.b.C$D`, `a.b.C[]`).
    fn of(&mut self, class_name: &str) -> Option<ClassOwner> {
        if let Some(known) = self.known.get(class_name) {
            return known.clone();
        }
        let base = class_name.trim_end_matches("[]");
        let found = (self.owner)(base)
            .or_else(|| {
                self.spark.get(base).map(|id| ClassOwner {
                    item: None,
                    mod_id: Some(id.clone()),
                })
            })
            .filter(|owner| {
                owner.item.is_some()
                    || owner
                        .mod_id
                        .as_deref()
                        .is_some_and(|id| !is_game_or_loader(id))
            });
        self.known.insert(class_name.to_owned(), found.clone());
        found
    }
}

/// Soma por dono.
#[derive(Default)]
struct Totals {
    order: Vec<ClassOwner>,
    amounts: HashMap<ClassOwner, f64>,
    game: f64,
}

impl Totals {
    fn add(&mut self, owner: Option<&ClassOwner>, amount: f64) {
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        let Some(owner) = owner else {
            self.game += amount;
            return;
        };
        // Um mod achado pelo índice e pelo spark é o mesmo dono: o item vence.
        let key = owner.clone();
        let entry = self.amounts.entry(key.clone()).or_insert_with(|| {
            self.order.push(key);
            0.0
        });
        *entry += amount;
    }

    fn into_summary_parts(self) -> (Vec<SourceShare>, f64, u64) {
        let total = self.game + self.amounts.values().sum::<f64>();
        let mut shares: Vec<SourceShare> = self
            .order
            .into_iter()
            .map(|owner| {
                let amount = self.amounts.get(&owner).copied().unwrap_or_default();
                SourceShare {
                    percent: percent(amount, total),
                    amount: to_u64(amount),
                    item: owner.item,
                    mod_id: owner.mod_id,
                }
            })
            .collect();
        // Empates em ordem de nome, para o resumo não depender da ordem do arquivo.
        shares.sort_by(|a, b| {
            b.amount
                .cmp(&a.amount)
                .then_with(|| a.mod_id.cmp(&b.mod_id))
                .then_with(|| a.item.cmp(&b.item))
        });
        shares.truncate(TOP_SOURCES);
        (shares, percent(self.game, total), to_u64(total))
    }
}

// Positivo e arredondado; acima do máximo, a conversão do Rust satura.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u64(value: f64) -> u64 {
    if value.is_finite() && value > 0.0 {
        value.round() as u64
    } else {
        0
    }
}

fn summarize_profile(
    data: &SamplerData,
    owner: &dyn Fn(&str) -> Option<ClassOwner>,
) -> SparkSummary {
    let metadata = data.metadata.as_ref();
    let mut owners = Owners::new(owner, &data.class_sources);
    let mut totals = Totals::default();
    for thread in &data.threads {
        walk_thread(thread, &mut owners, &mut totals);
    }
    let (top, game_percent, total) = totals.into_summary_parts();
    SparkSummary {
        kind: SparkKind::Profile,
        platform: platform(metadata.and_then(|metadata| metadata.platform_metadata.as_ref())),
        started_at_ms: metadata.and_then(|metadata| timestamp(metadata.start_time)),
        ended_at_ms: metadata.and_then(|metadata| timestamp(metadata.end_time)),
        ticks: ticks(
            &data.time_window_statistics,
            metadata.and_then(|metadata| metadata.platform_statistics.as_ref()),
        ),
        top,
        game_percent,
        total,
    }
}

/// Soma de um vetor de tempos (um por janela).
fn sum(times: &[f64]) -> f64 {
    times.iter().filter(|time| time.is_finite()).sum()
}

/// Percorre a árvore de uma thread: o tempo próprio de cada quadro (o total menos o dos
/// filhos) vai para o dono mais fundo do caminho até ele. Abaixo de um quadro de espera (e nele)
/// nada conta: no motor nativo do spark a espera aparece como quadros do sistema (`futex`,
/// `sched_yield`) debaixo do `Unsafe.park` ou do `Thread.yield`. Referências fora da lista ou
/// em ciclo são ignoradas (arquivo malformado não trava nem conta duas vezes).
fn walk_thread(thread: &ThreadNode, owners: &mut Owners<'_>, totals: &mut Totals) {
    let nodes = &thread.children;
    let mut visited = vec![false; nodes.len()];
    // (nó, dono herdado do caminho, se o caminho já está numa espera)
    let mut stack: Vec<(usize, Option<ClassOwner>, bool)> = thread
        .children_refs
        .iter()
        .filter_map(|index| usize::try_from(*index).ok())
        .map(|index| (index, None, false))
        .collect();
    while let Some((index, inherited, waiting)) = stack.pop() {
        let Some(node) = nodes.get(index) else {
            continue;
        };
        if std::mem::replace(&mut visited[index], true) {
            continue;
        }
        let waiting = waiting || is_idle(node);
        let current = if waiting {
            None
        } else {
            owners.of(&node.class_name).or(inherited)
        };
        let children: Vec<usize> = node
            .children_refs
            .iter()
            .filter_map(|child| usize::try_from(*child).ok())
            .filter(|child| *child < nodes.len())
            .collect();
        if !waiting {
            let children_time: f64 = children.iter().map(|child| sum(&nodes[*child].times)).sum();
            totals.add(current.as_ref(), sum(&node.times) - children_time);
        }
        for child in children {
            stack.push((child, current.clone(), waiting));
        }
    }
}

/// Se o quadro é uma espera (a thread não está processando).
fn is_idle(node: &StackTraceNode) -> bool {
    IDLE_FRAMES
        .iter()
        .any(|(class, method)| node.class_name == *class && node.method_name == *method)
}

fn summarize_heap(data: &HeapData, owner: &dyn Fn(&str) -> Option<ClassOwner>) -> SparkSummary {
    let metadata = data.metadata.as_ref();
    let empty = HashMap::new();
    let mut owners = Owners::new(owner, &empty);
    let mut totals = Totals::default();
    for entry in &data.entries {
        #[allow(clippy::cast_precision_loss)] // bytes: a precisão do f64 basta para a porcentagem
        let size = entry.size.max(0) as f64;
        let found = owners.of(&entry.r#type);
        totals.add(found.as_ref(), size);
    }
    let (top, game_percent, total) = totals.into_summary_parts();
    SparkSummary {
        kind: SparkKind::Heap,
        platform: platform(metadata.and_then(|metadata| metadata.platform_metadata.as_ref())),
        started_at_ms: None,
        ended_at_ms: metadata.and_then(|metadata| timestamp(metadata.generated_time)),
        ticks: None,
        top,
        game_percent,
        total,
    }
}

#[cfg(test)]
mod tests {
    // Os números do resumo já saem arredondados a uma casa.
    #![allow(clippy::float_cmp)]

    use super::generated::{HeapEntry, HeapMetadata, SamplerMetadata};
    use super::*;

    fn node(class: &str, method: &str, time: f64, children: &[i32]) -> StackTraceNode {
        StackTraceNode {
            class_name: class.into(),
            method_name: method.into(),
            times: vec![time],
            children_refs: children.to_vec(),
            ..StackTraceNode::default()
        }
    }

    fn by_package(class: &str) -> Option<ClassOwner> {
        let item = |path: &str, id: &str| ClassOwner {
            item: Some(path.into()),
            mod_id: Some(id.into()),
        };
        if class.starts_with("com.simibubi.create.") {
            Some(item("mods/create.pw.toml", "create"))
        } else if class.starts_with("mezz.jei.") {
            Some(item("mods/jei.pw.toml", "jei"))
        } else {
            None
        }
    }

    fn profile(nodes: Vec<StackTraceNode>, roots: &[i32]) -> SamplerData {
        SamplerData {
            metadata: Some(SamplerMetadata::default()),
            threads: vec![ThreadNode {
                name: "Server thread".into(),
                children: nodes,
                children_refs: roots.to_vec(),
                ..ThreadNode::default()
            }],
            class_sources: HashMap::from([
                ("dev.ftb.Quests".to_owned(), "ftbquests".to_owned()),
                (
                    "net.neoforged.neoforge.event.EventHooks".to_owned(),
                    "neoforge".to_owned(),
                ),
            ]),
            ..SamplerData::default()
        }
    }

    #[test]
    fn amostra_vai_para_o_mod_mais_fundo_e_esperas_nao_contam() {
        // 0 MinecraftServer.tick (100)
        //   1 Create.tick (60)
        //     2 Level.getBlock (20)      → Create (chamado pelo Create)
        //     3 jei.Hook (30)            → JEI (mais fundo)
        //   4 Quests.tick (10)           → ftbquests (pelo class_sources do spark)
        //   5 EventHooks.fire (10)       → jogo (neoforge não é dono)
        // 6 LockSupport.parkNanos (900)  → espera, fora
        let nodes = vec![
            node(
                "net.minecraft.server.MinecraftServer",
                "tick",
                100.0,
                &[1, 4, 5],
            ),
            node("com.simibubi.create.Create", "tick", 60.0, &[2, 3]),
            node("net.minecraft.world.level.Level", "getBlock", 20.0, &[]),
            node("mezz.jei.Hook", "run", 30.0, &[]),
            node("dev.ftb.Quests", "tick", 10.0, &[]),
            node("net.neoforged.neoforge.event.EventHooks", "fire", 10.0, &[]),
            node(
                "java.util.concurrent.locks.LockSupport",
                "parkNanos",
                900.0,
                &[],
            ),
        ];
        let data = profile(nodes, &[0, 6]);
        let summary = summarize(SparkKind::Profile, &data.encode_to_vec(), &by_package).unwrap();
        assert_eq!(summary.total, 100);
        let shares: Vec<(Option<&str>, f64)> = summary
            .top
            .iter()
            .map(|share| (share.mod_id.as_deref(), share.percent))
            .collect();
        assert_eq!(
            shares,
            [
                (Some("create"), 30.0),
                (Some("jei"), 30.0),
                (Some("ftbquests"), 10.0)
            ]
        );
        assert_eq!(summary.top[0].item.as_deref(), Some("mods/create.pw.toml"));
        assert_eq!(summary.top[2].item, None);
        // MinecraftServer.tick (20 próprios) + EventHooks (10).
        assert_eq!(summary.game_percent, 30.0);
    }

    #[test]
    fn referencias_quebradas_e_ciclos_nao_travam() {
        let nodes = vec![
            node("com.simibubi.create.A", "a", 10.0, &[1, 7, -3]),
            node("com.simibubi.create.B", "b", 5.0, &[0]),
        ];
        let data = profile(nodes, &[0, 0, 99]);
        let summary = summarize(SparkKind::Profile, &data.encode_to_vec(), &by_package).unwrap();
        assert_eq!(summary.top.len(), 1);
        assert_eq!(summary.top[0].mod_id.as_deref(), Some("create"));
    }

    #[test]
    fn memoria_agrupada_por_dono_das_classes() {
        let entry = |kind: &str, size: i64| HeapEntry {
            r#type: kind.into(),
            size,
            instances: 1,
            order: 0,
        };
        let data = HeapData {
            metadata: Some(HeapMetadata {
                generated_time: 1_792_284_173_000,
                ..HeapMetadata::default()
            }),
            entries: vec![
                entry("byte[]", 500),
                entry("com.simibubi.create.Kinetic[]", 300),
                entry("com.simibubi.create.Belt", 100),
                entry("mezz.jei.Ingredient", 100),
                entry("lixo", -5),
            ],
        };
        let summary = summarize(SparkKind::Heap, &data.encode_to_vec(), &by_package).unwrap();
        assert_eq!(summary.kind, SparkKind::Heap);
        assert_eq!(summary.total, 1000);
        assert_eq!(summary.top[0].mod_id.as_deref(), Some("create"));
        assert_eq!(summary.top[0].percent, 40.0);
        assert_eq!(summary.top[0].amount, 400);
        assert_eq!(summary.game_percent, 50.0);
        assert_eq!(summary.ended_at_ms, Some(1_792_284_173_000));
        assert_eq!(summary.ticks, None);
    }

    #[test]
    fn tempo_por_tick_das_janelas_ou_da_plataforma() {
        let window = |ticks, median, max, tps| WindowStatistics {
            ticks,
            mspt_median: median,
            mspt_max: max,
            tps,
            ..WindowStatistics::default()
        };
        let windows = HashMap::from([
            (1, window(1200, 10.0, 40.0, 20.0)),
            (2, window(600, 30.0, 90.04, 10.0)),
            (3, window(0, 99.0, 999.0, 0.0)),
        ]);
        let summary = ticks(&windows, None).unwrap();
        assert_eq!(summary.mspt_median, 20.0);
        assert_eq!(summary.mspt_max, 90.0);
        assert_eq!(summary.tps, Some(16.7));
        assert_eq!(summary.ticks, Some(1800));
        assert_eq!(ticks(&HashMap::new(), None), None);
    }

    #[test]
    fn arquivo_que_nao_e_do_spark() {
        assert!(matches!(
            summarize(SparkKind::Profile, b"\xff\xff\xff\xff", &by_package),
            Err(SparkReadError::Invalid(_))
        ));
        assert!(matches!(
            summarize(SparkKind::Heap, b"", &by_package),
            Err(SparkReadError::Invalid(_))
        ));
        assert_eq!(
            SparkKind::of_file("profile-1.sparkprofile"),
            Some(SparkKind::Profile)
        );
        assert_eq!(SparkKind::of_file("heap.SPARKHEAP"), Some(SparkKind::Heap));
        assert_eq!(SparkKind::of_file("config.json"), None);
        assert_eq!(SparkKind::of_file("semextensao"), None);
        assert!(SparkReadError::TooLarge.to_string().contains("grande"));
    }
}
