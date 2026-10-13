//! Crate `warden-perf`.
//!
//! Desempenho do jogo: RAM do processo, hsperfdata, tempos de carregamento e perfis do spark
//! (ARCHITECTURE §7.7; ADR-0038).
//!
//! - [`process`]: a RAM do processo do jogo e dos filhos dele (L-10);
//! - [`load_time`]: os tempos até "carregou" e até "entrou no mundo", pelos marcadores do log
//!   (L-10);
//! - [`SessionPerf`]: o que cada sessão de teste grava no `session.json` (campo `perf`).
//!
//! O resto (heap da JVM pelo `hsperfdata`, achados de memória, tempo por mod, spark,
//! histórico entre versões) vem das tarefas que o `docs/ROADMAP.md` atribui a esta crate.

mod error;
pub mod load_time;
pub mod process;

use serde::{Deserialize, Serialize};

pub use error::PerfErrorCode;
pub use load_time::{LoadTimes, LoadTracker, Loader, PreviousLoad};
pub use process::{MemorySample, MemoryStats, ProcessTree};

/// Intervalo entre duas amostras de desempenho (ARCHITECTURE §7.7).
pub const SAMPLE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// O desempenho de uma sessão de teste, gravado no `session.json` (campo `perf`) e mostrado
/// no resultado ("Abriu em …").
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
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
                }
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
    }
}
