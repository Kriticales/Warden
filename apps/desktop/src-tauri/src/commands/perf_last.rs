//! Comando `perf_last_test` (SPEC T14 "Desempenho do último teste"; T33): o bloco da coluna da
//! direita da seção Problemas. Só lê as sessões gravadas do Testar; nada muda no disco.
//!
//! - **O último teste** é a sessão mais nova que chegou a carregar (as que fecharam antes ficam
//!   de fora: não têm tempo para abrir). Dela saem os tempos, a diferença para a última versão
//!   testada (a mesma do resultado do teste, L-10), a memória do jogo (L-11) e o perfil do
//!   spark, quando o teste salvou um (L-12).
//! - **A comparação entre versões** aplica o critério de "mais pesada" da T33
//!   ([`warden_perf::heavier`]) aos testes comparáveis com o último: modo normal, mesmo perfil
//!   (assinatura), mesmo computador (impressão), fora a primeira abertura, que carregaram e sem
//!   alterações não salvas (cada um atribuído à versão salva que testou). A versão anterior é a
//!   outra versão com testes comparáveis testada mais recentemente.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use serde::Serialize;
use serde_json::Value;
use tauri::State;
use warden_core::PackId;
use warden_launcher::session::{SessionRecord, list_sessions};
use warden_perf::findings::RULE_LOW_HEAP;
use warden_perf::heavier::{self, Metric, Sample, Verdict};
use warden_perf::spark::{SparkKind, TickSummary};
use warden_perf::{PreviousLoad, SessionPerf};

use crate::commands::crashes::sessions_dir;
use crate::commands::perf::{PerfReport, SparkRow, report_of};
use crate::error::AppError;
use crate::state::AppState;
use crate::test_hooks::session_perf;
use crate::test_session::{TestMode, TestSessionSummary};

/// A memória do jogo no teste (a heap da JVM, L-11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameMemoryUse {
    /// O maior uso, em MB: depois das coletas quando houve medida, senão o bruto.
    pub(crate) used_mb: u32,
    /// O máximo que o jogo podia usar, em MB.
    pub(crate) max_mb: u32,
    /// A memória ficou perto do máximo por mais de um minuto (`W_LOW_HEAP`).
    pub(crate) near_limit: bool,
}

/// O que a comparação com a versão anterior deu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ComparisonStatus {
    /// Passou do critério de "mais pesada".
    Heavier,
    /// Menos de 2 testes comparáveis numa das versões.
    FewTests,
    /// Não passou do critério.
    NotHeavier,
}

/// A comparação da versão do último teste com a versão salva anterior (T33).
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VersionComparison {
    /// O resultado.
    pub(crate) status: ComparisonStatus,
    /// A versão do último teste.
    pub(crate) version: String,
    /// A versão anterior com testes comparáveis.
    pub(crate) previous_version: String,
    /// A métrica que passou do critério (só em [`ComparisonStatus::Heavier`]).
    pub(crate) metric: Option<Metric>,
    /// Mediana da versão do último teste (ms, MB ou ms por tick).
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) current: Option<f64>,
    /// Mediana da versão anterior.
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) previous: Option<f64>,
    /// Quanto a mais, em %.
    pub(crate) percent: Option<u32>,
}

/// O desempenho do último teste que carregou.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LastTestPerf {
    /// A sessão (nome da pasta).
    pub(crate) session_id: String,
    /// Início (ms desde 1970).
    #[specta(type = specta_typescript::Number)]
    pub(crate) started_at_ms: u64,
    /// Versão do pack testada.
    pub(crate) pack_version: Option<String>,
    /// Modo do teste.
    pub(crate) mode: TestMode,
    /// Tempo para abrir, em ms.
    pub(crate) loaded_ms: u32,
    /// Até entrar no mundo, em ms (`None`: não entrou).
    pub(crate) world_ms: Option<u32>,
    /// A última sessão de outra versão que carregou, para a variação do tempo para abrir.
    pub(crate) previous: Option<PreviousLoad>,
    /// A memória do jogo (`None` sem leitura).
    pub(crate) memory: Option<GameMemoryUse>,
    /// Pico da RAM do processo, em MB.
    pub(crate) ram_peak_mb: Option<u32>,
    /// O tempo por tick do perfil do spark salvo no teste.
    pub(crate) tick: Option<TickSummary>,
    /// Os mods que mais usaram processamento no perfil do spark (`None`: sem perfil do spark
    /// neste teste).
    pub(crate) spark_top: Option<Vec<SparkRow>>,
    /// A comparação com a versão anterior (`None`: não há outra versão com testes comparáveis,
    /// ou o último teste tinha alterações não salvas).
    pub(crate) comparison: Option<VersionComparison>,
}

/// O desempenho do último teste do pack que chegou a carregar (`null`: nenhum carregou).
#[tauri::command]
#[specta::specta]
pub(crate) async fn perf_last_test(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Option<LastTestPerf>, AppError> {
    state
        .packs
        .get(pack_id)
        .map_err(|error| AppError::from_domain(&error))?;
    let dir = sessions_dir(&state, pack_id);
    tauri::async_runtime::spawn_blocking(move || {
        let stored = list_sessions(&dir).map_err(|error| AppError::from_domain(&error))?;
        let entries: Vec<Entry> = stored
            .iter()
            .filter_map(|session| {
                let record = session.record.as_ref()?;
                let id = session
                    .dir
                    .path()
                    .file_name()?
                    .to_string_lossy()
                    .into_owned();
                Some(Entry::of(&id, record))
            })
            .collect();
        Ok(last_test(&entries))
    })
    .await
    .map_err(|error| AppError::internal(format!("desempenho do último teste: {error}")))?
}

/// Uma sessão gravada, com o que o bloco usa.
#[derive(Debug, Clone)]
struct Entry {
    id: String,
    started_at_ms: u64,
    mode: TestMode,
    pack_version: Option<String>,
    /// Testou uma versão salva, sem alterações (`false` sem a árvore gravada).
    saved: bool,
    signature: Option<String>,
    machine: Option<Value>,
    first_launch: bool,
    perf: Option<SessionPerf>,
    profile: Option<PerfReport>,
}

impl Entry {
    fn of(id: &str, record: &SessionRecord) -> Self {
        let summary = TestSessionSummary::from_record(id, record);
        let extra = &record.extra;
        Self {
            id: id.to_owned(),
            started_at_ms: summary.started_at_ms,
            mode: summary.mode,
            pack_version: record
                .pack_version
                .as_deref()
                .map(str::trim)
                .filter(|version| !version.is_empty())
                .map(ToOwned::to_owned),
            saved: extra
                .get("packTree")
                .and_then(|tree| tree.get("unsavedFiles"))
                .and_then(Value::as_u64)
                == Some(0),
            signature: extra
                .get("profileSignature")
                .and_then(|signature| signature.get("hash"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            machine: extra.get("machine").cloned(),
            first_launch: extra
                .get("firstLaunch")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            perf: session_perf(record),
            profile: report_of(record),
        }
    }

    fn loaded_ms(&self) -> Option<u32> {
        self.perf.as_ref()?.times.loaded_ms
    }

    /// O perfil de processamento mais novo do spark que deu para ler.
    fn spark_profile(&self) -> Option<&crate::commands::perf::SparkReport> {
        self.profile.as_ref()?.spark.iter().rev().find(|spark| {
            spark.readable && spark.kind == SparkKind::Profile
        })
    }

    fn tick(&self) -> Option<TickSummary> {
        self.profile
            .as_ref()?
            .spark
            .iter()
            .rev()
            .find_map(|spark| spark.ticks.clone())
    }

    fn sample(&self) -> Option<Sample> {
        Some(Sample {
            loaded_ms: self.loaded_ms()?,
            ram_peak_mb: self.perf.as_ref()?.peak_rss_mb,
            tick_ms: self.tick().map(|tick| tick.mspt_median),
        })
    }

    /// Comparável com `reference` (T33): modo normal, carregou, mesmo perfil e computador, fora
    /// a primeira abertura, numa versão salva.
    fn comparable_with(&self, reference: &Self) -> bool {
        self.mode == TestMode::Normal
            && self.loaded_ms().is_some()
            && !self.first_launch
            && self.saved
            && self.pack_version.is_some()
            && self.signature.is_some()
            && self.signature == reference.signature
            && self.machine.is_some()
            && self.machine == reference.machine
    }
}

/// O bloco a partir das sessões, da mais nova para a mais antiga.
fn last_test(entries: &[Entry]) -> Option<LastTestPerf> {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| b.started_at_ms.cmp(&a.started_at_ms).then(b.id.cmp(&a.id)));
    let last = *sorted.iter().find(|entry| entry.loaded_ms().is_some())?;
    let perf = last.perf.as_ref()?;
    let memory = perf.heap.as_ref().map(|heap| GameMemoryUse {
        used_mb: heap.peak_after_gc_mb.unwrap_or(heap.peak_used_mb),
        max_mb: heap.max_mb,
        near_limit: perf
            .findings
            .iter()
            .any(|finding| finding.rule().as_str() == RULE_LOW_HEAP),
    });
    Some(LastTestPerf {
        session_id: last.id.clone(),
        started_at_ms: last.started_at_ms,
        pack_version: last.pack_version.clone(),
        mode: last.mode,
        loaded_ms: perf.times.loaded_ms?,
        world_ms: perf.times.world_ms,
        previous: perf.previous.clone(),
        memory,
        ram_peak_mb: perf.peak_rss_mb,
        tick: last.tick(),
        spark_top: last.spark_profile().map(|spark| spark.top.clone()),
        comparison: comparison(last, &sorted),
    })
}

/// O critério de "mais pesada" entre a versão do último teste e a anterior.
fn comparison(last: &Entry, sorted: &[&Entry]) -> Option<VersionComparison> {
    if !last.saved {
        return None;
    }
    let version = last.pack_version.as_deref()?;
    let comparable: Vec<&Entry> = sorted
        .iter()
        .copied()
        .filter(|entry| entry.comparable_with(last))
        .collect();
    // A outra versão testada mais recentemente (as sessões já vêm da mais nova à mais antiga).
    let previous_version = comparable
        .iter()
        .filter_map(|entry| entry.pack_version.as_deref())
        .find(|other| *other != version)?;
    let samples = |of: &str| -> Vec<Sample> {
        comparable
            .iter()
            .filter(|entry| entry.pack_version.as_deref() == Some(of))
            .filter_map(|entry| entry.sample())
            .collect()
    };
    let verdict = heavier::compare(&samples(version), &samples(previous_version));
    let (status, heavier) = match verdict {
        Verdict::Heavier(heavier) => (ComparisonStatus::Heavier, Some(heavier)),
        Verdict::FewTests => (ComparisonStatus::FewTests, None),
        Verdict::NotHeavier => (ComparisonStatus::NotHeavier, None),
    };
    Some(VersionComparison {
        status,
        version: version.to_owned(),
        previous_version: previous_version.to_owned(),
        metric: heavier.map(|heavier| heavier.metric),
        current: heavier.map(|heavier| heavier.current),
        previous: heavier.map(|heavier| heavier.previous),
        percent: heavier.map(|heavier| heavier.percent()),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use warden_perf::{HeapSummary, LoadTimes};

    use super::*;

    fn perf(loaded_s: Option<u32>, ram_mb: u32) -> SessionPerf {
        SessionPerf {
            peak_rss_mb: Some(ram_mb),
            times: LoadTimes {
                loaded_ms: loaded_s.map(|s| s * 1000),
                world_ms: loaded_s.map(|s| s * 1000 + 20_000),
            },
            ..SessionPerf::default()
        }
    }

    /// Um teste normal comparável, na versão `version`, `minute` minutos depois do primeiro.
    fn entry(minute: u64, version: &str, loaded_s: Option<u32>) -> Entry {
        Entry {
            id: format!("2026-10-20_10-{minute:02}-00"),
            started_at_ms: 1_790_000_000_000 + minute * 60_000,
            mode: TestMode::Normal,
            pack_version: Some(version.to_owned()),
            saved: true,
            signature: Some("sha256:perfil".into()),
            machine: Some(json!({ "cpus": 8 })),
            first_launch: false,
            perf: Some(perf(loaded_s, 4000)),
            profile: None,
        }
    }

    #[test]
    fn sem_teste_que_carregou_nao_ha_bloco() {
        assert_eq!(last_test(&[]), None);
        assert_eq!(last_test(&[entry(1, "1.0.0", None)]), None);
    }

    #[test]
    fn o_ultimo_que_carregou_vale_mesmo_com_um_travamento_depois() {
        let entries = [
            entry(1, "1.0.0", Some(60)),
            entry(2, "1.0.0", Some(70)),
            entry(3, "1.0.0", None),
        ];
        let last = last_test(&entries).unwrap();
        assert_eq!(last.session_id, "2026-10-20_10-02-00");
        assert_eq!(last.loaded_ms, 70_000);
        assert_eq!(last.world_ms, Some(90_000));
        assert_eq!(last.ram_peak_mb, Some(4000));
        assert_eq!(last.spark_top, None);
        assert_eq!(last.memory, None);
        // Uma versão só: nada a comparar.
        assert_eq!(last.comparison, None);
    }

    /// CA-T33-03 pelas sessões: 1.4.2 com 100, 104 e 98 s e 1.5.0 com 125 e 128 s.
    #[test]
    fn versao_mais_lenta_pelo_criterio_da_t33() {
        let entries = [
            entry(1, "1.4.2", Some(100)),
            entry(2, "1.4.2", Some(104)),
            entry(3, "1.4.2", Some(98)),
            entry(4, "1.5.0", Some(125)),
            entry(5, "1.5.0", Some(128)),
        ];
        let comparison = last_test(&entries).unwrap().comparison.unwrap();
        assert_eq!(comparison.status, ComparisonStatus::Heavier);
        assert_eq!(comparison.version, "1.5.0");
        assert_eq!(comparison.previous_version, "1.4.2");
        assert_eq!(comparison.metric, Some(Metric::LoadTime));
        assert_eq!(comparison.percent, Some(26));
        assert_eq!(comparison.current, Some(126_500.0));
        assert_eq!(comparison.previous, Some(100_000.0));
    }

    #[test]
    fn poucos_testes_e_testes_que_nao_comparam() {
        let mut other_profile = entry(5, "1.5.0", Some(128));
        other_profile.signature = Some("sha256:outro".into());
        let mut first_launch = entry(6, "1.5.0", Some(200));
        first_launch.first_launch = true;
        let mut other_machine = entry(7, "1.5.0", Some(200));
        other_machine.machine = Some(json!({ "cpus": 4 }));
        let mut performance = entry(8, "1.5.0", Some(200));
        performance.mode = TestMode::Performance;
        let entries = [
            entry(1, "1.4.2", Some(100)),
            entry(2, "1.4.2", Some(104)),
            entry(4, "1.5.0", Some(125)),
            other_profile,
            first_launch,
            other_machine,
            performance,
        ];
        // O último que carregou é o do perfil de desempenho; dele vêm o perfil e o computador
        // de referência. Da 1.5.0 só o teste das 10:04 compara.
        let comparison = last_test(&entries).unwrap().comparison.unwrap();
        assert_eq!(comparison.status, ComparisonStatus::FewTests);
        assert_eq!(comparison.metric, None);
    }

    #[test]
    fn sem_aviso_quando_quase_igual_e_sem_comparacao_com_alteracoes_nao_salvas() {
        let entries = [
            entry(1, "1.4.2", Some(100)),
            entry(2, "1.4.2", Some(104)),
            entry(3, "1.5.0", Some(110)),
            entry(4, "1.5.0", Some(112)),
        ];
        let comparison = last_test(&entries).unwrap().comparison.unwrap();
        assert_eq!(comparison.status, ComparisonStatus::NotHeavier);

        let mut unsaved = entries.to_vec();
        unsaved.push(Entry {
            saved: false,
            ..entry(5, "1.5.0", Some(130))
        });
        assert_eq!(last_test(&unsaved).unwrap().comparison, None);
    }

    #[test]
    fn memoria_do_jogo_e_perfil_do_spark() {
        let mut last = entry(1, "1.0.0", Some(60));
        let perf = last.perf.as_mut().unwrap();
        perf.heap = Some(HeapSummary {
            peak_used_mb: 6100,
            peak_after_gc_mb: Some(5900),
            max_mb: 6144,
            gc_count: 10,
            gc_ms: 500,
            counts_pauses: true,
            readings: 100,
        });
        last.profile = Some(
            serde_json::from_value(json!({
                "side": "client",
                "load": null,
                "serverSpark": null,
                "spark": [{
                    "file": "a.sparkprofile",
                    "kind": "profile",
                    "savedAtMs": null,
                    "readable": true,
                    "platform": null,
                    "durationMs": 60000,
                    "ticks": { "msptMedian": 38.0, "msptMax": 120.0, "msptP95": 50.0, "tps": 19.5, "ticks": 1200 },
                    "top": [{ "name": "Create", "item": "mods/create.pw.toml", "modId": "create", "percent": 21.0 }],
                    "gamePercent": 40.0
                }],
                "sparkSkipped": 0
            }))
            .unwrap(),
        );
        let view = last_test(&[last]).unwrap();
        assert_eq!(
            view.memory,
            Some(GameMemoryUse {
                used_mb: 5900,
                max_mb: 6144,
                near_limit: false
            })
        );
        assert!((view.tick.unwrap().mspt_median - 38.0).abs() < f64::EPSILON);
        let top = view.spark_top.unwrap();
        assert_eq!(top[0].name, "Create");
    }

    #[test]
    fn camel_case_para_a_interface() {
        let view = last_test(&[entry(1, "1.0.0", Some(60))]).unwrap();
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["sessionId"], "2026-10-20_10-01-00");
        assert_eq!(json["loadedMs"], 60_000);
        assert!(json["sparkTop"].is_null());
    }
}
