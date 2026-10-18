//! Crate `warden-perf`.
//!
//! Desempenho do jogo: RAM do processo, hsperfdata, tempos de carregamento e perfis do spark
//! (ARCHITECTURE §7.7; ADR-0038).
//!
//! - [`process`]: a RAM do processo do jogo e dos filhos dele (L-10);
//! - [`load_time`]: os tempos até "carregou" e até "entrou no mundo", pelos marcadores do log
//!   (L-10);
//! - [`hsperf`]: a memória da JVM do jogo pelo `hsperfdata`, sem JDK (L-11);
//! - [`findings`]: a memória do jogo ao longo do teste e os achados de memória (L-11);
//! - [`SessionPerf`]: o que cada sessão de teste grava no `session.json` (campo `perf`).
//!
//! O resto (tempo por mod, spark, histórico entre versões) vem das tarefas que o
//! `docs/ROADMAP.md` atribui a esta crate.

mod error;
pub mod findings;
pub mod hsperf;
pub mod load_time;
pub mod mod_timing;
pub mod process;

use serde::{Deserialize, Serialize};

pub use error::PerfErrorCode;
pub use findings::{HeapDetails, HeapLive, HeapSummary, HeapTracker, TrackerConfig};
pub use hsperf::JvmHeap;
pub use load_time::{LoadTimes, LoadTracker, Loader, PreviousLoad};
pub use process::{MemorySample, MemoryStats, ProcessTree};

/// Intervalo entre duas amostras de desempenho (ARCHITECTURE §7.7).
pub const SAMPLE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// O desempenho de uma sessão de teste, gravado no `session.json` (campo `perf`) e mostrado
/// no resultado ("Abriu em …").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionPerf {
    /// Pico da RAM do processo (com os filhos), em MB. `None` sem nenhuma amostra.
    pub peak_rss_mb: Option<u32>,
    /// Pico da memória reservada pelo jogo e pelos filhos (Job Object do Windows), em MB.
    pub peak_commit_mb: Option<u32>,
    /// Até carregar e até entrar no mundo.
    #[serde(flatten)]
    pub times: LoadTimes,
    /// Amostras de memória recebidas.
    pub samples: u32,
    /// A última sessão de outra versão do pack que carregou, para a diferença.
    pub previous: Option<PreviousLoad>,
    /// A memória do jogo (heap da JVM): `None` sem leitura e nas sessões de antes da L-11.
    pub heap: Option<HeapSummary>,
    /// Por que não houve leitura da memória do jogo (`None`: houve, ou a sessão é de antes da
    /// L-11).
    pub heap_unavailable: Option<HeapUnavailable>,
    /// Os achados de memória do teste (`W_LOW_HEAP`, `I_HEAP_OVERSIZED`).
    pub findings: Vec<warden_diagnostics::Finding>,
}

/// Por que a memória do jogo não pôde ser lida (S-R5-2 §7; ARCHITECTURE §7.7). De fora não dá
/// para distinguir uma JVM sem esse recurso (`OpenJ9`) de um nome de usuário ou pasta temporária
/// fora da code page do Windows: o Warden só diz o argumento quando ele está no teste.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HeapUnavailable {
    /// Os argumentos do teste desligam a leitura (`-XX:+PerfDisableSharedMem`,
    /// `-XX:-UsePerfData`).
    Argument {
        /// O argumento, como está no teste.
        argument: String,
    },
    /// O Java do teste não publica a memória do jogo.
    NotPublished,
}

impl SessionPerf {
    /// Diferença de tempo até carregar para a sessão anterior de outra versão, em ms
    /// (positivo = mais lento agora).
    #[must_use]
    pub fn loaded_delta_ms(&self) -> Option<i64> {
        let now = self.times.loaded_ms?;
        let before = self.previous.as_ref()?.loaded_ms;
        Some(i64::from(now) - i64::from(before))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn sessao_em_camel_case_com_os_tempos_no_mesmo_nivel() {
        let perf = SessionPerf {
            peak_rss_mb: Some(4300),
            peak_commit_mb: None,
            times: LoadTimes {
                loaded_ms: Some(102_000),
                world_ms: None,
            },
            samples: 720,
            previous: Some(PreviousLoad {
                session_id: "2026-10-12_20-00-00".into(),
                pack_version: "1.4.2".into(),
                loaded_ms: 84_000,
            }),
            ..SessionPerf::default()
        };
        let json = serde_json::to_value(&perf).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "peakRssMb": 4300,
                "peakCommitMb": null,
                "loadedMs": 102_000,
                "worldMs": null,
                "samples": 720,
                "previous": {
                    "sessionId": "2026-10-12_20-00-00",
                    "packVersion": "1.4.2",
                    "loadedMs": 84_000
                },
                "heap": null,
                "heapUnavailable": null,
                "findings": []
            })
        );
        let back: SessionPerf = serde_json::from_value(json).unwrap();
        assert_eq!(back, perf);
        assert_eq!(perf.loaded_delta_ms(), Some(18_000));
        assert_eq!(SessionPerf::default().loaded_delta_ms(), None);
    }

    #[test]
    fn sessao_antiga_sem_campos_le_com_padrao() {
        let back: SessionPerf =
            serde_json::from_value(serde_json::json!({ "samples": 3 })).unwrap();
        assert_eq!(back.samples, 3);
        assert_eq!(back.times, LoadTimes::default());
        assert_eq!(back.heap, None);
        assert!(back.findings.is_empty());
    }

    #[test]
    fn motivo_sem_leitura_tem_o_argumento() {
        let json = serde_json::to_value(HeapUnavailable::Argument {
            argument: "-XX:+PerfDisableSharedMem".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "argument", "argument": "-XX:+PerfDisableSharedMem" })
        );
        assert_eq!(
            serde_json::to_value(HeapUnavailable::NotPublished).unwrap(),
            serde_json::json!({ "kind": "notPublished" })
        );
    }
}
