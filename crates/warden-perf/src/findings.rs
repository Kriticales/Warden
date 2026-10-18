//! A memória do jogo ao longo do teste e os achados de memória (ROADMAP L-11; SPEC T13;
//! ARCHITECTURE §7.7; ADR-0038).
//!
//! [`HeapTracker`] recebe uma [`HeapReading`] por amostra e guarda o que a faixa e a sessão
//! mostram (a memória agora, o pico, as coletas, o mini-gráfico dos últimos 5 minutos) e o que
//! os achados precisam. A **memória depois da coleta** é a heap usada da primeira amostra depois
//! de cada aumento de `invocations`: é o valor confiável em todos os coletores, inclusive no
//! Java 25 com Serial ou Parallel, em que o `used` só muda nas coletas (S-R5-2 §5.3).
//!
//! O **pico depois das coletas** ([`HeapSummary::peak_after_gc_mb`], o `heap_pico_mb` do
//! histórico entre versões; SPEC T14 e T33) é o maior valor da heap que cada coleta deixou. Na
//! primeira amostra depois de cada aumento de `invocations`, ele lê a heap fora do eden
//! (sobreviventes e geração velha, [`HeapReading::retained_bytes`]): logo depois de uma coleta o
//! eden está vazio, e só ele recebe os objetos novos até a próxima, então o que está fora dele é
//! o que a coleta deixou, sem o que o jogo alocou no segundo até a amostra (o mesmo número do
//! "antes->depois" do log de GC; um array enorme do G1, que vai direto para a geração velha,
//! entra também). Vale no Serial, no Parallel (no Java 25 inclusive: os contadores mudam na
//! coleta), no G1 e no CMS. No ZGC e no Shenandoah os contadores são de pausas, que acontecem no
//! meio do ciclo, e não há eden; num coletor desconhecido, não se sabe: nesses, e quando nenhuma
//! coleta foi vista (teste curto), o pico fica `None`, nunca o uso bruto, que no G1 quase sempre
//! chega perto do máximo e diz pouco sobre o pack.
//!
//! Achados ([`HeapTracker::findings`]), com os limites e o motivo de cada um:
//!
//! - `W_LOW_HEAP` (aviso): a memória depois das coletas fica em [`LOW_HEAP_PERCENT`]% ou mais do
//!   máximo por mais de [`LOW_HEAP_FOR`] (ARCHITECTURE §7.7, R5A §7.3; CA-T13-11). Abaixo de 90%
//!   depois de uma coleta sobra memória para o jogo; acima disso, cada coleta libera pouco, as
//!   coletas ficam seguidas e o jogo engasga até faltar memória. Um minuto separa um pico curto
//!   (carregar um mundo) de um pack que não cabe. A sugestão é a memória automática do Warden
//!   para o tamanho do pack, ou 2 GB a mais, a que for maior.
//! - `I_HEAP_OVERSIZED` (informação): mais de [`OVERSIZED_ABOVE_MB`] MB configurados e a memória
//!   do jogo nunca passa de [`OVERSIZED_PEAK_PERCENT`]% do máximo (ARCHITECTURE §7.7). Só com o
//!   jogo carregado e pelo menos [`OVERSIZED_MIN_READING`] de leitura: um teste fechado no menu
//!   não mostra quanto o pack usa jogando.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use warden_diagnostics::{Evidence, Finding, RuleCode, Severity, SuggestedFix};

use crate::hsperf::{HeapReading, JvmInfo};
use crate::process::{MB, to_mb};

/// `W_LOW_HEAP`: memória depois das coletas, em % do máximo.
pub const LOW_HEAP_PERCENT: u64 = 90;
/// `W_LOW_HEAP`: por quanto tempo seguido (estritamente mais que isso).
pub const LOW_HEAP_FOR: Duration = Duration::from_secs(60);
/// `W_LOW_HEAP`: quanto a sugestão soma à memória de agora, em MB.
pub const LOW_HEAP_STEP_MB: u32 = 2048;
/// `I_HEAP_OVERSIZED`: memória configurada acima disto, em MB.
pub const OVERSIZED_ABOVE_MB: u32 = 8192;
/// `I_HEAP_OVERSIZED`: o pico nunca passa disto, em % do máximo.
pub const OVERSIZED_PEAK_PERCENT: u64 = 40;
/// `I_HEAP_OVERSIZED`: tempo mínimo de leitura.
pub const OVERSIZED_MIN_READING: Duration = Duration::from_secs(180);
/// Mini-gráfico: cada coluna junta este tempo (o maior valor dele).
pub const SERIES_BUCKET: Duration = Duration::from_secs(15);
/// Mini-gráfico: colunas (20 × 15 s = os últimos 5 minutos, como no protótipo).
pub const SERIES_LEN: usize = 20;

/// Código de `W_LOW_HEAP`.
pub const RULE_LOW_HEAP: &str = "W_LOW_HEAP";
/// Código de `I_HEAP_OVERSIZED`.
pub const RULE_HEAP_OVERSIZED: &str = "I_HEAP_OVERSIZED";

/// O que o acompanhamento sabe antes do jogo abrir.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrackerConfig {
    /// O `-Xmx` que o Warden passou, em MB (o máximo que vale; sem ele, a reserva pelos
    /// contadores).
    pub xmx_mb: Option<u32>,
    /// A memória automática do Warden para o tamanho do pack, em MB (base da sugestão do
    /// `W_LOW_HEAP`).
    pub pack_suggestion_mb: Option<u32>,
}

/// O que um teste precisa saber para os achados.
#[derive(Debug, Clone, Copy)]
pub struct FindingContext<'a> {
    /// A sessão do teste (a evidência aponta para ela).
    pub session_id: &'a str,
    /// O jogo chegou a carregar (marcador de "carregou").
    pub loaded: bool,
}

/// A memória do jogo para a faixa, a cada amostra.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeapLive {
    /// Memória do jogo usada agora, em MB (no Java 25 com Serial ou Parallel, a da última
    /// coleta: veja [`HeapDetails::at_last_gc`]).
    pub used_mb: u32,
    /// Máximo, em MB.
    pub max_mb: u32,
    /// Coletas (ou pausas) até agora.
    pub gc_count: u32,
    /// Tempo parado em coletas até agora, em ms.
    pub gc_ms: u32,
    /// O resto do que a faixa mostra.
    pub details: HeapDetails,
}

/// O que a faixa mostra da memória do jogo além dos números.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HeapDetails {
    /// O coletor conta pausas (ZGC, Shenandoah): a faixa diz "Pausas de coleta".
    pub counts_pauses: bool,
    /// A memória mostrada é a da última coleta (Java 25 com Serial ou Parallel).
    pub at_last_gc: bool,
    /// Os últimos 5 minutos: uma coluna por 15 s (o maior valor dela), em MB, da mais velha para
    /// a mais nova.
    pub series_mb: Vec<u32>,
    /// Há quantos segundos a memória depois das coletas está acima de 90% do máximo, quando já
    /// passou de 1 minuto (o aviso ao vivo do `W_LOW_HEAP`).
    pub low_heap_s: Option<u32>,
    /// A memória sugerida para o aviso, em MB.
    pub suggested_mb: Option<u32>,
}

/// A memória do jogo no fim do teste, gravada na sessão.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct HeapSummary {
    /// Pico da memória do jogo, em MB (o uso bruto, com o que a próxima coleta ainda limpa).
    pub peak_used_mb: u32,
    /// Pico da memória do jogo depois das coletas, em MB: o maior valor da heap fora do eden na
    /// primeira amostra depois de cada coleta (o método está no topo de `findings.rs`). `None`
    /// sem coleta vista ou com um coletor em que não dá para medir (ZGC, Shenandoah, outro).
    pub peak_after_gc_mb: Option<u32>,
    /// Máximo, em MB.
    pub max_mb: u32,
    /// Coletas (ou pausas).
    pub gc_count: u32,
    /// Tempo parado em coletas, em ms.
    pub gc_ms: u32,
    /// O coletor conta pausas.
    pub counts_pauses: bool,
    /// Amostras com leitura.
    pub readings: u32,
}

/// Um trecho em que a memória depois das coletas ficou alta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LowHeap {
    /// Desde quando (ms do teste).
    since_ms: u64,
    /// Até quando, até agora.
    until_ms: u64,
    /// O maior percentual depois de uma coleta no trecho.
    worst_percent: u64,
}

impl LowHeap {
    fn duration_ms(&self) -> u64 {
        self.until_ms.saturating_sub(self.since_ms)
    }
}

/// A memória do jogo ao longo de um teste.
#[derive(Debug, Clone, Default)]
pub struct HeapTracker {
    config: TrackerConfig,
    info: Option<JvmInfo>,
    first_ms: Option<u64>,
    last_ms: u64,
    readings: u32,
    last: Option<HeapReading>,
    /// Memória depois da última coleta, em bytes.
    after_gc: Option<u64>,
    /// O trecho alto em andamento.
    high: Option<LowHeap>,
    /// O trecho alto mais longo que passou de [`LOW_HEAP_FOR`].
    worst_high: Option<LowHeap>,
    peak_used: u64,
    /// O maior valor que uma coleta deixou, em bytes (o pico depois das coletas).
    peak_after_gc: Option<u64>,
    /// `(coluna, maior valor em bytes)`.
    series: VecDeque<(u64, u64)>,
}

fn percent(part: u64, whole: u64) -> u64 {
    if whole == 0 {
        0
    } else {
        u64::try_from(u128::from(part) * 100 / u128::from(whole)).unwrap_or(u64::MAX)
    }
}

fn clamp_u32(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

impl HeapTracker {
    /// Começa vazio.
    #[must_use]
    pub fn new(config: TrackerConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Junta uma leitura feita `elapsed_ms` depois de o teste abrir o jogo.
    pub fn push(&mut self, elapsed_ms: u64, reading: HeapReading, info: Option<JvmInfo>) {
        if info.is_some() {
            self.info = info;
        }
        self.first_ms.get_or_insert(elapsed_ms);
        self.last_ms = elapsed_ms;
        self.readings = self.readings.saturating_add(1);
        let collected = self
            .last
            .is_some_and(|before| reading.gc_count > before.gc_count);
        self.last = Some(reading);
        self.peak_used = self.peak_used.max(reading.used_bytes);
        if collected {
            self.after_gc = Some(reading.used_bytes);
            self.collection(elapsed_ms, reading.used_bytes);
            if let Some(retained) = reading
                .retained_bytes()
                .filter(|_| self.info.is_some_and(|info| info.collector.has_eden()))
            {
                self.peak_after_gc = Some(self.peak_after_gc.unwrap_or(0).max(retained));
            }
        }
        if let Some(high) = self.high.as_mut() {
            high.until_ms = elapsed_ms;
            if high.duration_ms() > ms(LOW_HEAP_FOR)
                && self
                    .worst_high
                    .is_none_or(|worst| high.duration_ms() >= worst.duration_ms())
            {
                self.worst_high = Some(*high);
            }
        }
        self.series_point(elapsed_ms, self.shown_used());
    }

    /// Uma coleta terminou: a memória agora é a que sobrou depois dela.
    fn collection(&mut self, elapsed_ms: u64, used: u64) {
        let share = percent(used, self.max_bytes());
        if share >= LOW_HEAP_PERCENT {
            let high = self.high.get_or_insert(LowHeap {
                since_ms: elapsed_ms,
                until_ms: elapsed_ms,
                worst_percent: share,
            });
            high.worst_percent = high.worst_percent.max(share);
        } else {
            self.high = None;
        }
    }

    fn series_point(&mut self, elapsed_ms: u64, used: u64) {
        let bucket = elapsed_ms / ms(SERIES_BUCKET).max(1);
        match self.series.back_mut() {
            Some((last, value)) if *last == bucket => *value = (*value).max(used),
            _ => {
                self.series.push_back((bucket, used));
                while self.series.len() > SERIES_LEN {
                    self.series.pop_front();
                }
            }
        }
    }

    /// O máximo que vale: o `-Xmx` do teste ou, sem ele, a reserva pelos contadores.
    fn max_bytes(&self) -> u64 {
        match self.config.xmx_mb {
            Some(mb) => u64::from(mb) * MB,
            None => self.last.map_or(0, |last| last.max_bytes),
        }
    }

    /// A memória mostrada: a de agora ou, no Java 25 com Serial ou Parallel, a da última coleta.
    fn shown_used(&self) -> u64 {
        let at_last_gc = self.info.is_some_and(JvmInfo::updates_only_on_gc);
        match (at_last_gc, self.after_gc, self.last) {
            (true, Some(after), _) => after,
            (_, _, Some(last)) => last.used_bytes,
            _ => 0,
        }
    }

    /// Houve alguma leitura.
    #[must_use]
    pub fn has_readings(&self) -> bool {
        self.readings > 0
    }

    /// A memória sugerida para o `W_LOW_HEAP`: a automática para o tamanho do pack ou 2 GB a
    /// mais que agora, a maior, arredondada para cima em GB.
    #[must_use]
    pub fn suggested_mb(&self) -> u32 {
        let now = to_mb(self.max_bytes());
        let wanted = now
            .saturating_add(LOW_HEAP_STEP_MB)
            .max(self.config.pack_suggestion_mb.unwrap_or(0));
        wanted.div_ceil(1024).saturating_mul(1024)
    }

    /// A faixa agora. `None` sem nenhuma leitura.
    #[must_use]
    pub fn live(&self) -> Option<HeapLive> {
        let last = self.last?;
        let low_heap_s = self
            .high
            .filter(|high| high.duration_ms() > ms(LOW_HEAP_FOR))
            .map(|high| clamp_u32(high.duration_ms() / 1000));
        Some(HeapLive {
            used_mb: to_mb(self.shown_used()),
            max_mb: to_mb(self.max_bytes()),
            gc_count: clamp_u32(last.gc_count),
            gc_ms: clamp_u32(last.gc_time_ms),
            details: HeapDetails {
                counts_pauses: self.info.is_some_and(|info| info.collector.counts_pauses()),
                at_last_gc: self.info.is_some_and(JvmInfo::updates_only_on_gc)
                    && self.after_gc.is_some(),
                series_mb: self.series.iter().map(|&(_, used)| to_mb(used)).collect(),
                low_heap_s,
                suggested_mb: low_heap_s.map(|_| self.suggested_mb()),
            },
        })
    }

    /// O resumo para a sessão. `None` sem nenhuma leitura.
    #[must_use]
    pub fn summary(&self) -> Option<HeapSummary> {
        let last = self.last?;
        Some(HeapSummary {
            peak_used_mb: to_mb(self.peak_used),
            peak_after_gc_mb: self.peak_after_gc.map(to_mb),
            max_mb: to_mb(self.max_bytes()),
            gc_count: clamp_u32(last.gc_count),
            gc_ms: clamp_u32(last.gc_time_ms),
            counts_pauses: self.info.is_some_and(|info| info.collector.counts_pauses()),
            readings: self.readings,
        })
    }

    /// Os achados de memória do teste (vazio sem leitura).
    #[must_use]
    pub fn findings(&self, context: FindingContext<'_>) -> Vec<Finding> {
        let mut findings = Vec::new();
        let max_mb = to_mb(self.max_bytes());
        if let Some(high) = self.worst_high {
            let seconds = high.duration_ms() / 1000;
            findings.push(
                finding(
                    RULE_LOW_HEAP,
                    Severity::Warning,
                    context.session_id,
                    "heapAfterGc",
                    [
                        ("percent", high.worst_percent.to_string()),
                        ("seconds", seconds.to_string()),
                        ("maxMb", max_mb.to_string()),
                    ],
                )
                .with_param("maxMb", max_mb.to_string())
                .with_param("suggestedMb", self.suggested_mb().to_string())
                .with_param("percent", high.worst_percent.to_string())
                .with_param("seconds", seconds.to_string())
                .with_fix(SuggestedFix::ChangeMemory),
            );
        }
        let read_long_enough = self
            .first_ms
            .is_some_and(|first| self.last_ms.saturating_sub(first) >= ms(OVERSIZED_MIN_READING));
        let peak = percent(self.peak_used, self.max_bytes());
        if context.loaded
            && read_long_enough
            && max_mb > OVERSIZED_ABOVE_MB
            && peak <= OVERSIZED_PEAK_PERCENT
        {
            let peak_mb = to_mb(self.peak_used).to_string();
            findings.push(
                finding(
                    RULE_HEAP_OVERSIZED,
                    Severity::Info,
                    context.session_id,
                    "heapPeak",
                    [
                        ("peakMb", peak_mb.clone()),
                        ("percent", peak.to_string()),
                        ("maxMb", max_mb.to_string()),
                    ],
                )
                .with_param("maxMb", max_mb.to_string())
                .with_param("peakMb", peak_mb)
                .with_param("percent", peak.to_string())
                .with_fix(SuggestedFix::ChangeMemory),
            );
        }
        findings
    }
}

/// Um achado de memória com a medida como evidência.
fn finding<const N: usize>(
    rule: &str,
    severity: Severity,
    session_id: &str,
    metric: &str,
    values: [(&str, String); N],
) -> Finding {
    let code = RuleCode::parse(rule).unwrap_or_else(|| unreachable!("código fixo e válido"));
    Finding::new(
        code,
        severity,
        format!("diagnostico.regras.{rule}"),
        Evidence::Measurement {
            session_id: session_id.to_owned(),
            metric: metric.to_owned(),
            values: values
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect::<BTreeMap<_, _>>(),
        },
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::hsperf::Collector;

    const G1: Option<JvmInfo> = Some(JvmInfo {
        major: Some(21),
        collector: Collector::G1,
    });

    fn reading(used_mb: u64, gc_count: u64, gc_time_ms: u64) -> HeapReading {
        HeapReading {
            used_bytes: used_mb * MB,
            max_bytes: 4096 * MB,
            gc_count,
            gc_time_ms,
            eden_bytes: Some(0),
        }
    }

    fn tracker(xmx_mb: u32) -> HeapTracker {
        HeapTracker::new(TrackerConfig {
            xmx_mb: Some(xmx_mb),
            pack_suggestion_mb: Some(6144),
        })
    }

    /// Uma coleta por segundo deixando `after_mb`, por `seconds` segundos a partir de `from`.
    fn high_for(tracker: &mut HeapTracker, from: u64, seconds: u64, after_mb: u64) -> u64 {
        let mut count = tracker.last.map_or(0, |last| last.gc_count);
        for second in 0..=seconds {
            count += 1;
            tracker.push((from + second) * 1000, reading(after_mb, count, 0), G1);
        }
        count
    }

    fn rules(findings: &[Finding]) -> Vec<&str> {
        findings
            .iter()
            .map(|finding| finding.rule().as_str())
            .collect()
    }

    const CONTEXT: FindingContext<'static> = FindingContext {
        session_id: "2026-10-13_14-02-11",
        loaded: true,
    };

    #[test]
    fn memoria_alta_por_70_s_gera_o_aviso_e_por_menos_de_1_minuto_nao() {
        // CA-T13-11: 4 GB, 3.800 MB depois de cada coleta (92%).
        let mut short = tracker(4096);
        short.push(0, reading(1000, 0, 0), G1);
        high_for(&mut short, 1, 59, 3800);
        assert!(short.findings(CONTEXT).is_empty());
        assert_eq!(short.live().unwrap().details.low_heap_s, None);

        let mut long = tracker(4096);
        long.push(0, reading(1000, 0, 0), G1);
        high_for(&mut long, 1, 70, 3800);
        let findings = long.findings(CONTEXT);
        assert_eq!(rules(&findings), [RULE_LOW_HEAP]);
        let low = &findings[0];
        assert_eq!(low.severity(), Severity::Warning);
        assert_eq!(low.param("seconds"), Some("70"));
        assert_eq!(low.param("percent"), Some("92"));
        assert_eq!(low.param("maxMb"), Some("4096"));
        // 4 GB + 2 GB = 6 GB, igual à sugestão para o tamanho do pack.
        assert_eq!(low.param("suggestedMb"), Some("6144"));
        assert_eq!(low.fixes(), [SuggestedFix::ChangeMemory]);
        assert!(matches!(
            low.primary_evidence(),
            Evidence::Measurement { session_id, metric, values }
                if session_id == CONTEXT.session_id && metric == "heapAfterGc" && values["seconds"] == "70"
        ));
        let live = long.live().unwrap();
        assert_eq!(live.details.low_heap_s, Some(70));
        assert_eq!(live.details.suggested_mb, Some(6144));
    }

    #[test]
    fn uma_coleta_que_libera_memoria_recomeca_a_contagem() {
        let mut tracker = tracker(4096);
        tracker.push(0, reading(1000, 0, 0), G1);
        let count = high_for(&mut tracker, 1, 40, 3800);
        // Uma coleta deixa só 50%: o trecho alto acaba.
        tracker.push(42_000, reading(2048, count + 1, 0), G1);
        high_for(&mut tracker, 43, 40, 3800);
        assert!(tracker.findings(CONTEXT).is_empty());
    }

    #[test]
    fn so_conta_a_memoria_logo_depois_das_coletas() {
        // A memória sobe até 99% entre coletas, mas cada coleta deixa 60%: sem aviso.
        let mut tracker = tracker(4096);
        let mut count = 0;
        for second in 0..200 {
            let used = if second % 10 == 0 {
                count += 1;
                2458
            } else {
                4050
            };
            tracker.push(second * 1000, reading(used, count, 0), G1);
        }
        assert!(tracker.findings(CONTEXT).is_empty());
    }

    #[test]
    fn sugestao_e_a_maior_entre_o_tamanho_do_pack_e_2_gb_a_mais() {
        let mut small = HeapTracker::new(TrackerConfig {
            xmx_mb: Some(2048),
            pack_suggestion_mb: Some(8192),
        });
        small.push(0, reading(1, 0, 0), G1);
        assert_eq!(small.suggested_mb(), 8192);
        let mut odd = HeapTracker::new(TrackerConfig {
            xmx_mb: Some(5000),
            pack_suggestion_mb: None,
        });
        odd.push(0, reading(1, 0, 0), G1);
        assert_eq!(odd.suggested_mb(), 7168);
    }

    /// Uma amostra com `used_mb` no total, `eden_mb` dele no eden.
    fn with_eden(used_mb: u64, eden_mb: u64, gc_count: u64) -> HeapReading {
        HeapReading {
            eden_bytes: Some(eden_mb * MB),
            ..reading(used_mb, gc_count, 0)
        }
    }

    #[test]
    fn pico_depois_das_coletas_ignora_o_uso_bruto() {
        // G1 com 4 GB: o uso bruto sobe até 3.950 MB entre as coletas, mas cada coleta deixa só
        // a geração velha (1.200 a 1.800 MB). Na amostra logo depois, o eden já tem o que o
        // jogo alocou nesse segundo (300 MB), e fica de fora.
        let mut tracker = tracker(4096);
        tracker.push(0, with_eden(900, 600, 0), G1);
        let mut count = 0;
        for (step, old_mb) in (1u64..).zip([1200u64, 1500, 1800, 1400]) {
            let at = step * 10_000;
            tracker.push(at, with_eden(3950, 3950 - old_mb, count), G1);
            count += 1;
            tracker.push(at + 1000, with_eden(old_mb + 300, 300, count), G1);
            // Entre coletas o eden cresce e a geração velha fica: não conta.
            tracker.push(at + 2000, with_eden(old_mb + 2000, 2000, count), G1);
        }
        let summary = tracker.summary().unwrap();
        assert_eq!(summary.peak_used_mb, 3950);
        assert_eq!(summary.peak_after_gc_mb, Some(1800));
    }

    #[test]
    fn pico_depois_das_coletas_sem_coleta_ou_sem_eden_fica_vazio() {
        // Teste curto, nenhuma coleta vista: sem pico, mesmo com o bruto alto.
        let mut short = tracker(4096);
        short.push(0, with_eden(3900, 3000, 4), G1);
        short.push(1000, with_eden(3990, 3090, 4), G1);
        assert_eq!(short.summary().unwrap().peak_used_mb, 3990);
        assert_eq!(short.summary().unwrap().peak_after_gc_mb, None);
        // ZGC e Shenandoah contam pausas no meio do ciclo, e um coletor desconhecido não se
        // sabe: sem pico.
        for collector in [Collector::Zgc, Collector::Shenandoah, Collector::Other] {
            let info = Some(JvmInfo {
                major: Some(25),
                collector,
            });
            let mut tracker = tracker(4096);
            tracker.push(0, with_eden(3000, 0, 1), info);
            tracker.push(1000, with_eden(1000, 0, 2), info);
            assert_eq!(
                tracker.summary().unwrap().peak_after_gc_mb,
                None,
                "{collector:?}"
            );
        }
        // Sem o contador do eden: também sem pico.
        let mut no_eden = tracker(4096);
        no_eden.push(0, reading(3000, 1, 0), G1);
        no_eden.push(
            1000,
            HeapReading {
                eden_bytes: None,
                ..reading(1000, 2, 0)
            },
            G1,
        );
        assert_eq!(no_eden.summary().unwrap().peak_after_gc_mb, None);
    }

    #[test]
    fn pico_depois_das_coletas_no_java_25_com_parallel() {
        // O `used` só muda nas coletas: a amostra depois dela tem o eden vazio.
        let info = Some(JvmInfo {
            major: Some(25),
            collector: Collector::Parallel,
        });
        let mut tracker = tracker(2048);
        tracker.push(0, with_eden(0, 0, 0), info);
        tracker.push(1000, with_eden(700, 0, 1), info);
        tracker.push(2000, with_eden(700, 0, 1), info);
        tracker.push(3000, with_eden(650, 10, 2), info);
        assert_eq!(tracker.summary().unwrap().peak_after_gc_mb, Some(700));
    }

    #[test]
    fn muito_tempo_em_coletas_nao_e_achado() {
        // 13% de um minuto parado em coletas (a carga de um pack grande pode ser assim) e a
        // memória folgada depois delas: nenhum achado. A SPEC só tem o `W_LOW_HEAP` e o
        // `I_HEAP_OVERSIZED` como achados automáticos de memória.
        let mut busy = tracker(4096);
        for second in 0..=90u64 {
            busy.push(second * 1000, reading(2000, second, second * 8000 / 60), G1);
        }
        assert!(busy.findings(CONTEXT).is_empty());
    }

    #[test]
    fn memoria_sobrando_so_com_jogo_carregado_e_tempo_suficiente() {
        let run = |xmx: u32, used_mb: u64, seconds: u64| {
            let mut tracker = tracker(xmx);
            for second in 0..=seconds {
                tracker.push(second * 1000, reading(used_mb, second / 10, 0), G1);
            }
            tracker
        };
        // 12 GB, nunca passou de 3 GB (25%), 4 minutos: informação.
        let big = run(12_288, 3072, 240);
        let findings = big.findings(CONTEXT);
        assert_eq!(rules(&findings), [RULE_HEAP_OVERSIZED]);
        assert_eq!(findings[0].severity(), Severity::Info);
        assert_eq!(findings[0].param("peakMb"), Some("3072"));
        assert_eq!(findings[0].param("percent"), Some("25"));
        // Sem carregar, com pouco tempo, com 8 GB ou passando de 40%: nada.
        assert!(
            big.findings(FindingContext {
                loaded: false,
                ..CONTEXT
            })
            .is_empty()
        );
        assert!(run(12_288, 3072, 100).findings(CONTEXT).is_empty());
        assert!(run(8192, 1024, 240).findings(CONTEXT).is_empty());
        assert!(run(12_288, 6000, 240).findings(CONTEXT).is_empty());
    }

    #[test]
    fn java_25_com_parallel_mostra_a_memoria_da_ultima_coleta() {
        let info = Some(JvmInfo {
            major: Some(25),
            collector: Collector::Parallel,
        });
        let mut tracker = tracker(1024);
        // Antes de qualquer coleta, o `used` parado em 0: mostra o que há.
        tracker.push(0, reading(0, 0, 0), info);
        assert!(!tracker.live().unwrap().details.at_last_gc);
        tracker.push(1000, reading(300, 1, 5), info);
        // O eden "parado" depois da coleta: a faixa fica com o valor da coleta.
        tracker.push(2000, reading(120, 1, 5), info);
        let live = tracker.live().unwrap();
        assert!(live.details.at_last_gc);
        assert_eq!(live.used_mb, 300);
        assert_eq!(live.max_mb, 1024);
    }

    #[test]
    fn grafico_junta_15_s_por_coluna_e_guarda_5_minutos() {
        let mut tracker = tracker(4096);
        for second in 0..400u64 {
            tracker.push(second * 1000, reading(second, 0, 0), G1);
        }
        let live = tracker.live().unwrap();
        assert_eq!(live.details.series_mb.len(), SERIES_LEN);
        // A última coluna é a de 390 s a 399 s (o maior valor, 399).
        assert_eq!(live.details.series_mb.last(), Some(&399));
        assert_eq!(live.details.series_mb[SERIES_LEN - 2], 389);
    }

    #[test]
    fn zgc_conta_pausas_e_sem_xmx_usa_a_reserva() {
        let mut tracker = HeapTracker::new(TrackerConfig::default());
        tracker.push(
            0,
            reading(100, 3, 1),
            Some(JvmInfo {
                major: Some(25),
                collector: Collector::Zgc,
            }),
        );
        let live = tracker.live().unwrap();
        assert!(live.details.counts_pauses);
        assert_eq!(live.max_mb, 4096);
        let summary = tracker.summary().unwrap();
        assert_eq!(summary.peak_used_mb, 100);
        assert_eq!(summary.gc_count, 3);
        assert!(summary.counts_pauses);
        assert!(HeapTracker::default().live().is_none());
        assert!(HeapTracker::default().findings(CONTEXT).is_empty());
    }
}
