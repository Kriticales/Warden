//! RAM do processo do jogo e dos filhos dele (ARCHITECTURE §7.7; ADR-0038).
//!
//! [`ProcessTree`] mede o processo principal e os descendentes (se o Java for aberto por um
//! intermediário, ou se o jogo abrir outros processos): a RAM em uso é a soma da árvore.
//!
//! - **Windows**: `GetProcessMemoryInfo` com `PROCESS_MEMORY_COUNTERS_EX` (`WorkingSetSize`,
//!   `PrivateUsage` e `PeakWorkingSetSize`). O processo principal fica aberto do começo ao fim
//!   (um handle próprio, aberto logo depois do jogo: o pid não pode ser reaproveitado enquanto
//!   ele existir); os filhos vêm da lista de processos do sistema (Toolhelp). O `unsafe` fica
//!   todo em `windows.rs`.
//! - **Linux**: `/proc/<pid>/status` (`VmRSS` e `VmHWM`) e o `ppid` de `/proc/<pid>/stat`
//!   (`procfs.rs`; sem `/proc`, como no macOS, não há medição).
//!
//! Custo: a lista de processos é relida a cada [`RESCAN_EVERY`] amostras; nas outras, só os
//! processos já conhecidos são lidos. Cada amostra é uma leitura curta do sistema, feita fora
//! da thread da interface por quem chama.

#[cfg(not(windows))]
mod procfs;
#[cfg(windows)]
mod windows;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// A cada quantas amostras a lista de processos é relida para achar filhos novos.
pub const RESCAN_EVERY: u32 = 5;

/// Bytes em um MB (MiB, como o Gerenciador de Tarefas e o `-Xmx`).
pub const MB: u64 = 1024 * 1024;

/// A memória da árvore do jogo num instante.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemorySample {
    /// RAM em uso, somada na árvore (Windows: *working set*; Linux: `VmRSS`).
    pub rss_bytes: u64,
    /// Memória privada somada na árvore (Windows: `PrivateUsage`; `None` no Linux).
    pub private_bytes: Option<u64>,
    /// Pico de RAM do processo principal guardado pelo sistema (Windows:
    /// `PeakWorkingSetSize`; Linux: `VmHWM`). Pega picos entre duas amostras.
    pub root_peak_bytes: Option<u64>,
    /// Quantos processos foram medidos (o principal e os filhos vivos).
    pub processes: u32,
}

/// O processo do jogo e os descendentes dele.
#[derive(Debug)]
pub struct ProcessTree {
    root: u32,
    platform: platform::Root,
    /// Filhos conhecidos (sem o principal), da última leitura da lista de processos.
    children: Vec<u32>,
    /// Amostras desde a última leitura da lista.
    since_scan: u32,
}

#[cfg(not(windows))]
use self::procfs as platform;
#[cfg(windows)]
use self::windows as platform;

impl ProcessTree {
    /// Começa a medir o processo `pid` e os filhos. Chame logo depois de abrir o processo.
    ///
    /// `None` quando o processo não pode ser aberto (já saiu, ou o sistema não é suportado).
    #[must_use]
    pub fn open(pid: u32) -> Option<Self> {
        Some(Self {
            root: pid,
            platform: platform::Root::open(pid)?,
            children: Vec::new(),
            since_scan: RESCAN_EVERY,
        })
    }

    /// O pid do processo principal.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.root
    }

    /// Mede a árvore agora. `None` quando o processo principal já saiu.
    pub fn sample(&mut self) -> Option<MemorySample> {
        let root = self.platform.read_root()?;
        if self.since_scan >= RESCAN_EVERY {
            self.since_scan = 0;
            if let Some(pairs) = platform::process_list() {
                self.children = descendants(self.root, &pairs);
            }
        }
        self.since_scan += 1;
        let mut sample = MemorySample {
            rss_bytes: root.rss,
            private_bytes: root.private,
            root_peak_bytes: root.peak,
            processes: 1,
        };
        // Um filho que saiu desde a última leitura da lista só deixa de contar. Um processo
        // que começou antes do jogo não é filho dele (o pid do pai foi reaproveitado).
        self.children.retain(|&pid| {
            let Some(child) = self.platform.read_child(pid) else {
                return false;
            };
            sample.rss_bytes = sample.rss_bytes.saturating_add(child.rss);
            sample.private_bytes = sample
                .private_bytes
                .zip(child.private)
                .map(|(total, more)| total.saturating_add(more));
            sample.processes += 1;
            true
        });
        Some(sample)
    }
}

/// A memória de um processo, em bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ProcessMemory {
    /// RAM em uso.
    rss: u64,
    /// Memória privada (só no Windows).
    private: Option<u64>,
    /// Pico de RAM guardado pelo sistema.
    peak: Option<u64>,
}

/// Os descendentes de `root` (filhos, netos…), sem o próprio `root`, em ordem crescente.
/// Ciclos (pids reaproveitados) não fazem a busca rodar para sempre.
#[must_use]
pub fn descendants(root: u32, pairs: &[(u32, u32)]) -> Vec<u32> {
    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &(pid, ppid) in pairs {
        if pid != ppid {
            children.entry(ppid).or_default().push(pid);
        }
    }
    let mut found = BTreeSet::new();
    let mut queue = vec![root];
    while let Some(parent) = queue.pop() {
        for &child in children.get(&parent).map_or(&[][..], Vec::as_slice) {
            if child != root && found.insert(child) {
                queue.push(child);
            }
        }
    }
    found.into_iter().collect()
}

/// O que se acumula das amostras de um teste: a última e o pico.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryStats {
    /// A última amostra.
    pub last: Option<MemorySample>,
    /// Pico da RAM em uso: o maior entre a soma da árvore em cada amostra e o pico do
    /// processo principal guardado pelo sistema.
    pub peak_rss_bytes: u64,
    /// Amostras recebidas.
    pub samples: u32,
}

impl MemoryStats {
    /// Junta uma amostra.
    pub fn push(&mut self, sample: MemorySample) {
        self.peak_rss_bytes = self
            .peak_rss_bytes
            .max(sample.rss_bytes)
            .max(sample.root_peak_bytes.unwrap_or(0));
        self.last = Some(sample);
        self.samples = self.samples.saturating_add(1);
    }

    /// A RAM em uso agora, em MB (0 sem amostra).
    #[must_use]
    pub fn rss_mb(&self) -> u32 {
        self.last.map_or(0, |sample| to_mb(sample.rss_bytes))
    }

    /// O pico, em MB.
    #[must_use]
    pub fn peak_rss_mb(&self) -> u32 {
        to_mb(self.peak_rss_bytes)
    }
}

/// Bytes em MB, arredondando para o mais próximo (com teto em `u32::MAX`).
#[must_use]
pub fn to_mb(bytes: u64) -> u32 {
    u32::try_from(bytes.saturating_add(MB / 2) / MB).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use proptest::prelude::*;

    use super::*;

    #[test]
    fn descendentes_seguem_a_arvore_toda() {
        // 10 → 11 → 13, 10 → 12; 20 é de outro processo; 30 aponta para si mesmo.
        let pairs = [(11, 10), (12, 10), (13, 11), (20, 1), (30, 30), (10, 1)];
        assert_eq!(descendants(10, &pairs), [11, 12, 13]);
        assert_eq!(descendants(11, &pairs), [13]);
        assert!(descendants(99, &pairs).is_empty());
    }

    #[test]
    fn ciclo_de_pids_reaproveitados_nao_trava() {
        let pairs = [(2, 1), (1, 2), (3, 2)];
        assert_eq!(descendants(1, &pairs), [2, 3]);
    }

    #[test]
    fn estatisticas_guardam_o_pico_e_a_ultima() {
        let mut stats = MemoryStats::default();
        assert_eq!(stats.rss_mb(), 0);
        stats.push(MemorySample {
            rss_bytes: 900 * MB,
            private_bytes: None,
            root_peak_bytes: Some(1200 * MB),
            processes: 1,
        });
        stats.push(MemorySample {
            rss_bytes: 1000 * MB,
            private_bytes: None,
            root_peak_bytes: None,
            processes: 2,
        });
        assert_eq!(stats.rss_mb(), 1000);
        assert_eq!(stats.peak_rss_mb(), 1200);
        assert_eq!(stats.samples, 2);
        stats.push(MemorySample {
            rss_bytes: 1500 * MB,
            ..MemorySample::default()
        });
        assert_eq!(stats.peak_rss_mb(), 1500);
    }

    #[test]
    fn mb_arredonda() {
        assert_eq!(to_mb(0), 0);
        assert_eq!(to_mb(MB / 2 - 1), 0);
        assert_eq!(to_mb(MB / 2), 1);
        assert_eq!(to_mb(6 * 1024 * MB), 6144);
        assert_eq!(
            to_mb(u64::MAX),
            u32::try_from(u64::MAX / MB).unwrap_or(u32::MAX)
        );
    }

    proptest! {
        /// Os descendentes nunca incluem a raiz, não se repetem e cada um tem o pai na
        /// árvore (a raiz ou outro descendente).
        #[test]
        fn descendentes_sao_coerentes(pairs in proptest::collection::vec((0u32..40, 0u32..40), 0..80), root in 0u32..40) {
            let found = descendants(root, &pairs);
            prop_assert!(!found.contains(&root));
            let mut sorted = found.clone();
            sorted.dedup();
            prop_assert_eq!(&sorted, &found);
            for pid in &found {
                prop_assert!(pairs.iter().any(|&(child, parent)| child == *pid && child != parent && (parent == root || found.contains(&parent))));
            }
        }
    }

    /// O processo deste teste: tem memória, o pico não é menor que a RAM em uso e um
    /// processo filho entra na soma.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn mede_o_proprio_processo_e_um_filho() {
        let mut tree = ProcessTree::open(std::process::id()).expect("abrir o próprio processo");
        let alone = tree.sample().expect("amostra");
        assert!(alone.rss_bytes > MB, "{alone:?}");
        assert!(alone.root_peak_bytes.unwrap_or(u64::MAX) >= alone.rss_bytes / 2);
        let mut child = sleeper();
        tree.since_scan = RESCAN_EVERY;
        let with_child = tree.sample().expect("amostra com filho");
        let _ = child.kill();
        let _ = child.wait();
        assert!(with_child.processes >= 2, "{with_child:?}");
        assert!(with_child.rss_bytes > alone.rss_bytes / 2);
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn processo_que_saiu_nao_tem_amostra() {
        let mut child = sleeper();
        let mut tree = ProcessTree::open(child.id()).expect("abrir o filho");
        assert!(tree.sample().is_some());
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(tree.sample().is_none());
    }

    /// Um processo que fica parado até ser encerrado.
    #[cfg(any(windows, target_os = "linux"))]
    fn sleeper() -> std::process::Child {
        #[cfg(windows)]
        let mut command = {
            let mut command = std::process::Command::new("powershell");
            command.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
            command
        };
        #[cfg(not(windows))]
        let mut command = {
            let mut command = std::process::Command::new("sleep");
            command.arg("30");
            command
        };
        let child = command
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("abrir o processo de teste");
        // O processo precisa existir de fato antes da leitura.
        std::thread::sleep(std::time::Duration::from_millis(300));
        child
    }
}
