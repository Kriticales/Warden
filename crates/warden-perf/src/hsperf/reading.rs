//! Os números que o Warden tira do `hsperfdata` (S-R5-2 §5; ARCHITECTURE §7.7):
//!
//! - **heap usada** = soma de `sun.gc.generation.*.space.*.used` (vale em todos os coletores,
//!   inclusive onde o `jstat` mostra "-");
//! - **máximo pelos contadores** (a reserva do `-Xmx`): com `sun.gc.policy.name` igual a
//!   `Copy:MSC` ou `ParScav:MSC` (Serial, Parallel), a **soma** dos
//!   `sun.gc.generation.N.maxCapacity`; nos outros (G1, ZGC, Shenandoah), o **maior** deles
//!   (cada geração informa a heap inteira);
//! - **coletas** = soma de `sun.gc.collector.*.invocations`; **tempo** = soma de
//!   `sun.gc.collector.*.time` ÷ `sun.os.hrt.frequency`. Os índices podem ter buracos (o ZGC
//!   não tem o 1). No ZGC e no Shenandoah os contadores são de **pausas**.
//!
//! [`Layout`] guarda onde cada número fica no buffer: enquanto `num_entries` e `mod_time_stamp`
//! do cabeçalho não mudam, cada amostra relê só esses números, sem percorrer as entradas.

use super::parse::{Header, ParseError, Reader, Snapshot, parse, parse_header};

/// O coletor de memória da JVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Collector {
    /// Serial (`Copy:MSC`).
    Serial,
    /// Parallel (`ParScav:MSC`), o padrão do Java 8.
    Parallel,
    /// G1 (`GarbageFirst`), o padrão do Java 9 em diante e do perfil do teste.
    G1,
    /// CMS (Java 8 a 13).
    Cms,
    /// ZGC.
    Zgc,
    /// Shenandoah.
    Shenandoah,
    /// Outro (nome desconhecido).
    Other,
}

impl Collector {
    /// O coletor pelo nome da política (`sun.gc.policy.name`) ou, sem ela (ZGC e Shenandoah no
    /// Java 21+), pelo nome do primeiro coletor.
    #[must_use]
    pub fn detect(policy: Option<&str>, first_collector: Option<&str>) -> Self {
        match policy {
            Some("Copy:MSC") => return Self::Serial,
            Some("ParScav:MSC") => return Self::Parallel,
            Some("GarbageFirst") => return Self::G1,
            Some(name) if name.contains("CMS") => return Self::Cms,
            _ => {}
        }
        // ZGC: "ZGC minor collection pauses" (25); "Z concurrent cycle pauses" (17 e 21 sem
        // gerações).
        let name = first_collector.unwrap_or("");
        if name.contains("ZGC") || name.starts_with("Z ") {
            Self::Zgc
        } else if name.contains("Shenandoah") {
            Self::Shenandoah
        } else {
            Self::Other
        }
    }

    /// As gerações somam a heap (Serial e Parallel) em vez de cada uma informar a heap inteira.
    #[must_use]
    pub fn sums_generations(self) -> bool {
        matches!(self, Self::Serial | Self::Parallel)
    }

    /// O espaço 0 da geração 0 é o eden, onde entram os objetos novos (Serial, Parallel, G1 e
    /// CMS). No ZGC e no Shenandoah não há eden assim, e um coletor desconhecido não se sabe.
    #[must_use]
    pub fn has_eden(self) -> bool {
        matches!(self, Self::Serial | Self::Parallel | Self::G1 | Self::Cms)
    }

    /// Os contadores do coletor são de pausas, não de ciclos (a interface diz "Pausas de
    /// coleta").
    #[must_use]
    pub fn counts_pauses(self) -> bool {
        matches!(self, Self::Zgc | Self::Shenandoah)
    }
}

/// O que se sabe da JVM pelos textos do buffer (lidos uma vez, na montagem do [`Layout`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JvmInfo {
    /// Versão principal do Java (8, 17, 21, 25), pelo `java.property.java.version`.
    pub major: Option<u32>,
    /// O coletor.
    pub collector: Collector,
}

impl JvmInfo {
    /// No Java 25 com Serial ou Parallel, o `used` só muda nas coletas (S-R5-2 §5.3): a memória
    /// ao vivo é a da última coleta.
    #[must_use]
    pub fn updates_only_on_gc(self) -> bool {
        self.major.is_some_and(|major| major >= 25)
            && matches!(self.collector, Collector::Serial | Collector::Parallel)
    }
}

/// A versão principal pelo texto do `java.version` (`1.8.0_504` → 8, `21.0.12.1` → 21).
#[must_use]
pub fn java_major(version: &str) -> Option<u32> {
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// Uma leitura da memória da JVM: só números.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct HeapReading {
    /// Heap usada, em bytes.
    pub used_bytes: u64,
    /// Máximo da heap pelos contadores (a reserva quando o `-Xmx` não é conhecido), em bytes.
    pub max_bytes: u64,
    /// Coletas (ou pausas, no ZGC e no Shenandoah) desde que a JVM abriu.
    pub gc_count: u64,
    /// Tempo parado nelas, em ms.
    pub gc_time_ms: u64,
    /// Parte da heap usada que está no eden (`sun.gc.generation.0.space.0.used`), em bytes.
    /// Só nos coletores com gerações em que o eden é esse espaço (Serial, Parallel, G1 e CMS):
    /// logo depois de uma coleta o eden está vazio e só ele recebe objetos novos até a próxima,
    /// então `used_bytes - eden_bytes` na primeira amostra depois dela é a heap que a coleta
    /// deixou (veja [`HeapReading::retained_bytes`]). `None` nos outros coletores.
    pub eden_bytes: Option<u64>,
}

impl HeapReading {
    /// A heap fora do eden (sobreviventes e geração velha), em bytes: lida logo depois de uma
    /// coleta, é a memória que ela deixou, sem o que o jogo alocou depois. `None` sem o eden.
    #[must_use]
    pub fn retained_bytes(&self) -> Option<u64> {
        self.eden_bytes
            .map(|eden| self.used_bytes.saturating_sub(eden))
    }
}

/// Onde ficam os números de que o Warden precisa, para reler só eles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    num_entries: i32,
    mod_time_stamp: i64,
    little_endian: bool,
    used: Vec<usize>,
    max_capacity: Vec<usize>,
    invocations: Vec<usize>,
    times: Vec<usize>,
    frequency: Option<usize>,
    /// O `used` do eden, nos coletores em que ele é o espaço 0 da geração 0.
    eden: Option<usize>,
    info: JvmInfo,
}

impl Layout {
    /// Monta a partir de um instantâneo completo.
    #[must_use]
    pub fn new(snapshot: &Snapshot) -> Self {
        let offsets = |keep: &dyn Fn(&str) -> bool| -> Vec<usize> {
            snapshot
                .counters
                .iter()
                .filter(|(name, counter)| {
                    keep(name) && matches!(counter.value, super::parse::Value::Long(_))
                })
                .map(|(_, counter)| counter.data_at)
                .collect()
        };
        let generation = |name: &str, suffix: &str| {
            name.strip_prefix("sun.gc.generation.")
                .and_then(|rest| rest.split_once('.'))
                .is_some_and(|(index, rest)| {
                    index.bytes().all(|b| b.is_ascii_digit()) && rest == suffix
                })
        };
        let space_used = |name: &str| {
            name.strip_prefix("sun.gc.generation.")
                .and_then(|rest| rest.rsplit_once('.'))
                .is_some_and(|(space, field)| space.contains(".space.") && field == "used")
        };
        let collector = |name: &str, field: &str| {
            name.strip_prefix("sun.gc.collector.")
                .and_then(|rest| rest.split_once('.'))
                .is_some_and(|(index, rest)| {
                    index.bytes().all(|b| b.is_ascii_digit()) && rest == field
                })
        };
        let info = JvmInfo {
            major: snapshot
                .text("java.property.java.version")
                .and_then(java_major),
            collector: Collector::detect(
                snapshot.text("sun.gc.policy.name"),
                snapshot.text("sun.gc.collector.0.name"),
            ),
        };
        Self {
            num_entries: snapshot.header.num_entries,
            mod_time_stamp: snapshot.header.mod_time_stamp,
            little_endian: snapshot.header.little_endian,
            used: offsets(&space_used),
            max_capacity: offsets(&|name| generation(name, "maxCapacity")),
            invocations: offsets(&|name| collector(name, "invocations")),
            times: offsets(&|name| collector(name, "time")),
            frequency: snapshot
                .counters
                .get("sun.os.hrt.frequency")
                .map(|counter| counter.data_at),
            eden: info
                .collector
                .has_eden()
                .then(|| snapshot.counters.get("sun.gc.generation.0.space.0.used"))
                .flatten()
                .filter(|counter| matches!(counter.value, super::parse::Value::Long(_)))
                .map(|counter| counter.data_at),
            info,
        }
    }

    /// O que se sabe da JVM.
    #[must_use]
    pub fn info(&self) -> JvmInfo {
        self.info
    }

    /// Se a lista de contadores do buffer ainda é a deste layout.
    #[must_use]
    pub fn matches(&self, header: &Header) -> bool {
        self.num_entries == header.num_entries
            && self.mod_time_stamp == header.mod_time_stamp
            && self.little_endian == header.little_endian
    }

    /// Lê os números do buffer pelos deslocamentos guardados. `None` se algum sair do buffer.
    #[must_use]
    pub fn read(&self, buf: &[u8]) -> Option<HeapReading> {
        let r = Reader::new(buf, self.little_endian);
        let sum = |offsets: &[usize]| -> Option<u64> {
            offsets.iter().try_fold(0u64, |total, &at| {
                Some(total.saturating_add(u64::try_from(r.i64(at)?).unwrap_or(0)))
            })
        };
        let max_bytes = if self.info.collector.sums_generations() {
            sum(&self.max_capacity)?
        } else {
            self.max_capacity.iter().try_fold(0u64, |most, &at| {
                Some(most.max(u64::try_from(r.i64(at)?).unwrap_or(0)))
            })?
        };
        let frequency = match self.frequency {
            Some(at) => u64::try_from(r.i64(at)?).unwrap_or(0),
            None => 0,
        };
        let ticks = sum(&self.times)?;
        let gc_time_ms = if frequency == 0 {
            0
        } else {
            u64::try_from(u128::from(ticks) * 1000 / u128::from(frequency)).unwrap_or(u64::MAX)
        };
        let eden_bytes = match self.eden {
            Some(at) => Some(u64::try_from(r.i64(at)?).unwrap_or(0)),
            None => None,
        };
        Some(HeapReading {
            used_bytes: sum(&self.used)?,
            max_bytes,
            gc_count: sum(&self.invocations)?,
            gc_time_ms,
            eden_bytes,
        })
    }
}

/// Lê um buffer: com o `layout` ainda válido, só os números; senão, tudo de novo (e troca o
/// layout). `Ok(None)`: o buffer ainda não está pronto (`accessible = 0`).
pub fn read_buffer(
    buf: &[u8],
    layout: &mut Option<Layout>,
) -> Result<Option<HeapReading>, ParseError> {
    let header = parse_header(buf)?;
    if !header.accessible {
        return Ok(None);
    }
    if let Some(current) = layout.as_ref().filter(|current| current.matches(&header))
        && let Some(reading) = current.read(buf)
    {
        return Ok(Some(reading));
    }
    let snapshot = parse(buf)?;
    let fresh = Layout::new(&snapshot);
    let reading = fresh.read(buf);
    *layout = Some(fresh);
    Ok(reading)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::super::parse::test_buffer;
    use super::*;

    const GIB: i64 = 1 << 30;

    fn generations(policy: &str) -> Vec<u8> {
        test_buffer(
            &[
                ("sun.gc.generation.0.space.0.used", 100 << 20),
                ("sun.gc.generation.0.space.1.used", 10 << 20),
                ("sun.gc.generation.1.space.0.used", 50 << 20),
                ("sun.gc.generation.0.maxCapacity", GIB / 3),
                ("sun.gc.generation.1.maxCapacity", GIB - GIB / 3),
                ("sun.gc.collector.0.invocations", 5),
                ("sun.gc.collector.0.time", 2_500_000),
                // Sem o 1 (como o ZGC).
                ("sun.gc.collector.2.invocations", 2),
                ("sun.gc.collector.2.time", 500_000),
                ("sun.os.hrt.frequency", 10_000_000),
                ("sun.gc.collector.0.lastEntryTime", 99),
            ],
            &[
                ("sun.gc.policy.name", policy),
                ("java.property.java.version", "25.0.4.1"),
            ],
        )
    }

    #[test]
    fn soma_os_espacos_e_os_coletores_com_buraco() {
        let mut layout = None;
        let reading = read_buffer(&generations("GarbageFirst"), &mut layout)
            .unwrap()
            .unwrap();
        assert_eq!(reading.used_bytes, 160 << 20);
        // O eden é o espaço 0 da geração 0; fora dele, 60 MB.
        assert_eq!(reading.eden_bytes, Some(100 << 20));
        assert_eq!(reading.retained_bytes(), Some(60 << 20));
        assert_eq!(reading.gc_count, 7);
        // (2.500.000 + 500.000) ticks ÷ 10.000.000 por segundo = 300 ms.
        assert_eq!(reading.gc_time_ms, 300);
        // G1: a maior geração.
        assert_eq!(reading.max_bytes, u64::try_from(GIB - GIB / 3).unwrap());
        let info = layout.unwrap().info();
        assert_eq!(info.collector, Collector::G1);
        assert_eq!(info.major, Some(25));
        assert!(!info.updates_only_on_gc());
    }

    #[test]
    fn serial_e_parallel_somam_as_geracoes() {
        for (policy, collector) in [
            ("Copy:MSC", Collector::Serial),
            ("ParScav:MSC", Collector::Parallel),
        ] {
            let mut layout = None;
            let reading = read_buffer(&generations(policy), &mut layout)
                .unwrap()
                .unwrap();
            assert_eq!(reading.max_bytes, u64::try_from(GIB).unwrap());
            let info = layout.unwrap().info();
            assert_eq!(info.collector, collector);
            assert!(info.updates_only_on_gc(), "Java 25 com {policy}");
        }
    }

    #[test]
    fn buffer_ainda_nao_pronto_nao_tem_leitura() {
        let mut buf = generations("GarbageFirst");
        buf[7] = 0;
        let mut layout = None;
        assert_eq!(read_buffer(&buf, &mut layout).unwrap(), None);
        assert!(layout.is_none());
    }

    #[test]
    fn layout_e_reaproveitado_e_refeito_quando_o_cabecalho_muda() {
        let mut buf = generations("GarbageFirst");
        let mut layout = None;
        read_buffer(&buf, &mut layout).unwrap();
        let first = layout.clone().unwrap();
        // Um número muda no lugar: o layout continua o mesmo e a leitura vê o valor novo.
        let at = first.used[0];
        buf[at..at + 8].copy_from_slice(&(200i64 << 20).to_le_bytes());
        let reading = read_buffer(&buf, &mut layout).unwrap().unwrap();
        assert_eq!(reading.used_bytes, (200 + 10 + 50) << 20);
        assert_eq!(layout.as_ref(), Some(&first));
        // A JVM acrescentou contadores (`mod_time_stamp` mudou): o layout é refeito.
        buf[16..24].copy_from_slice(&8i64.to_le_bytes());
        read_buffer(&buf, &mut layout).unwrap();
        assert_ne!(layout.as_ref(), Some(&first));
        assert_eq!(layout.as_ref().unwrap().mod_time_stamp, 8);
    }

    #[test]
    fn versao_do_java() {
        assert_eq!(java_major("1.8.0_504"), Some(8));
        assert_eq!(java_major("1.8.0_51-b16"), Some(8));
        assert_eq!(java_major("17.0.20.1"), Some(17));
        assert_eq!(java_major("21"), Some(21));
        assert_eq!(java_major("25-ea"), Some(25));
        assert_eq!(java_major("x"), None);
    }

    #[test]
    fn coletor_pelo_nome() {
        assert_eq!(Collector::detect(Some("ParNew:CMS"), None), Collector::Cms);
        assert_eq!(
            Collector::detect(None, Some("ZGC minor collection pauses")),
            Collector::Zgc
        );
        assert_eq!(
            Collector::detect(None, Some("Shenandoah partial")),
            Collector::Shenandoah
        );
        assert_eq!(Collector::detect(None, None), Collector::Other);
        assert!(Collector::Zgc.counts_pauses() && Collector::Shenandoah.counts_pauses());
        assert!(!Collector::G1.counts_pauses());
    }
}
