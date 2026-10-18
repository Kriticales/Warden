//! Comandos do domínio `health` (SPEC T14 "Saúde do pack" e T02 coluna Saúde; ROADMAP D-08;
//! ADR-0034; decisão D24).
//!
//! - `health_get`: a nota do pack agora, com "O que tirou pontos". Calculada só com o que já está
//!   guardado (o último relatório do pré-teste com os ignorados de agora, as sessões do Testar, o
//!   resumo do pack e o último "Verificar atualizações", que fica na memória): não roda o
//!   diagnóstico. A nota calculada vira a última nota do pack em `packs.json`.
//! - `health_list`: as últimas notas guardadas, para a coluna Saúde de Meus packs, sem calcular
//!   nada (CA-T02-05). Pack sem nota guardada fica de fora (a interface mostra "sem dados").
//!
//! Sem relatório do pré-teste (o pack nunca foi verificado neste computador) não há nota: a
//! guardada é apagada, para Meus packs nunca mostrar uma nota inventada.
//!
//! A nota guardada também é refeita sem a interface pedir ([`refresh`]): quando o pack muda
//! (`pack-changed`, ligado em [`connect`]: edições, "Ignorar neste pack", mudanças por fora), no
//! fim de cada teste e depois de "Verificar atualizações".
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Listener as _, Manager as _, State};
use warden_core::PackId;
use warden_diagnostics::crash_history::SessionEnd;
use warden_diagnostics::health::{
    HealthBand, HealthFinding, HealthInput, HealthScore, HealthSession, LatestRelease,
    SessionMemory, score, weights,
};
use warden_launcher::session::{SessionRecord, list_sessions};
use warden_project::updates::{ModUpdateReport, UpdateStatus};

use crate::commands::crashes::{session_entry, sessions_dir};
use crate::commands::diagnostics::current_pack_hash;
use crate::commands::diagnostics::report::{apply_ignored, load_report, read_ignored};
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::settings::TestMemory;
use crate::state::AppState;

/// Campo de `packs.json` (o `extra` do registro do pack) com a última nota.
pub(crate) const REGISTRY_KEY: &str = "health";

/// A última nota de um pack, como fica em `packs.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HealthSummary {
    /// De 0 a 100.
    pub(crate) score: u8,
    /// A faixa.
    pub(crate) band: HealthBand,
}

impl From<&HealthScore> for HealthSummary {
    fn from(health: &HealthScore) -> Self {
        Self {
            score: health.score,
            band: health.band,
        }
    }
}

/// A última nota guardada de um pack (Meus packs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackHealth {
    /// O pack.
    pub(crate) pack_id: PackId,
    /// A nota.
    pub(crate) health: HealthSummary,
}

/// A nota do pack agora (`None`: o pack nunca foi verificado). Grava a nota em `packs.json`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn health_get(
    app: AppHandle,
    pack_id: PackId,
) -> Result<Option<HealthScore>, AppError> {
    // Lê o relatório e as sessões do disco: fora da thread do IPC.
    tauri::async_runtime::spawn_blocking(move || refresh(&app.state::<AppState>(), pack_id))
        .await
        .map_err(|error| AppError::internal(format!("nota de saúde: {error}")))?
}

/// As últimas notas guardadas, sem calcular nada.
#[tauri::command]
#[specta::specta]
pub(crate) fn health_list(state: State<'_, AppState>) -> Vec<PackHealth> {
    stored(&state.packs)
}

/// As notas guardadas em `packs.json` (valor ilegível, de outra versão, fica de fora).
fn stored(packs: &warden_project::registry::Registry) -> Vec<PackHealth> {
    packs
        .records()
        .into_iter()
        .filter_map(|record| {
            let value = record.extra.get(REGISTRY_KEY)?;
            let health = serde_json::from_value(value.clone()).ok()?;
            Some(PackHealth {
                pack_id: record.id,
                health,
            })
        })
        .collect()
}

/// Uma nota calculada por vez: a última a gravar leu o estado mais novo.
static REFRESHING: Mutex<()> = Mutex::new(());

/// Calcula a nota agora e a guarda em `packs.json` (só quando mudou).
pub(crate) fn refresh(state: &AppState, pack_id: PackId) -> Result<Option<HealthScore>, AppError> {
    let _one = REFRESHING.lock().unwrap_or_else(PoisonError::into_inner);
    let health = compute(state, pack_id)?;
    let summary = health.as_ref().map(HealthSummary::from);
    let stored = state
        .packs
        .get(pack_id)
        .ok()
        .and_then(|record| record.extra.get(REGISTRY_KEY).cloned())
        .and_then(|value| serde_json::from_value::<HealthSummary>(value).ok());
    if stored != summary {
        let value = summary
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| AppError::internal(format!("nota de saúde: {error}")))?;
        state
            .packs
            .set_extra(pack_id, REGISTRY_KEY, value)
            .map_err(|error| AppError::from_domain(&error))?;
    }
    Ok(health)
}

/// [`refresh`] quando a falha só pode ser registrada (fim do teste, verificação de
/// atualizações).
pub(crate) fn refresh_logged(state: &AppState, pack_id: PackId) {
    if let Err(error) = refresh(state, pack_id) {
        tracing::info!(code = %error.code, "nota de saúde não recalculada");
    }
}

/// Refaz a nota guardada a cada `pack-changed` (fora da thread do evento).
pub(crate) fn connect(app: &AppHandle) {
    let handle = app.clone();
    app.listen("pack-changed", move |event| {
        let Some(pack_id) = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|payload| serde_json::from_value::<PackId>(payload.get("packId")?.clone()).ok())
        else {
            return;
        };
        let handle = handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Some(state) = handle.try_state::<AppState>() {
                refresh_logged(&state, pack_id);
            }
        });
    });
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
}

/// A nota com o que está guardado agora.
fn compute(state: &AppState, pack_id: PackId) -> Result<Option<HealthScore>, AppError> {
    let root = pack_root(state, pack_id)?;
    let Some(input) = input(state, pack_id, &root, now_ms()) else {
        return Ok(None);
    };
    let weights = weights().map_err(|error| AppError::from_domain(&error))?;
    Ok(Some(score(&input, weights)))
}

/// A entrada da nota (`None` sem relatório do pré-teste).
fn input(state: &AppState, pack_id: PackId, root: &Path, now_ms: u64) -> Option<HealthInput> {
    let mut report = load_report(state.paths.data_dir(), pack_id)?;
    apply_ignored(&mut report.findings, &read_ignored(root));
    let findings = report
        .findings
        .iter()
        .map(|entry| {
            let item = entry.items.iter().find_map(|item| item.path.clone());
            HealthFinding::new(entry.id.clone(), &entry.finding, entry.ignored, item)
        })
        .collect();
    Some(HealthInput {
        findings,
        sessions: sessions(&sessions_dir(state, pack_id)),
        current_tree: current_pack_hash(root),
        releases: state
            .update_reports
            .get(pack_id)
            .map(|report| releases(&report))
            .unwrap_or_default(),
        now_ms,
    })
}

/// As sessões gravadas do Testar (as que ainda não têm `session.json` ficam de fora).
fn sessions(dir: &Path) -> Vec<HealthSession> {
    let stored = match list_sessions(dir) {
        Ok(stored) => stored,
        Err(error) => {
            // Só o código: o erro traz a pasta de dados, com o nome do usuário.
            let code = AppError::from_domain(&error).code;
            tracing::info!(%code, "sessões do teste ilegíveis para a nota de saúde");
            return Vec::new();
        }
    };
    stored
        .iter()
        .filter_map(|session| {
            let record = session.record.as_ref()?;
            let entry = session_entry(session, record);
            Some(HealthSession {
                id: entry.id,
                started_at_ms: entry.started_at_ms,
                end: entry.end,
                signature: (entry.end == SessionEnd::Crashed)
                    .then_some(entry.signature)
                    .flatten()
                    .map(|signature| signature.hash),
                tree: entry.pack.tree,
                loaded_ms: crate::test_hooks::session_perf(record)
                    .and_then(|perf| perf.times.loaded_ms),
                memory: memory(record),
            })
        })
        .collect()
}

/// A memória do teste e a sugestão do Warden para a quantidade de mods dele.
fn memory(record: &SessionRecord) -> Option<SessionMemory> {
    let memory = record.extra.get("memory")?;
    let mb = u32::try_from(memory.get("mb")?.as_u64()?).ok()?;
    let auto = memory.get("auto")?.as_bool()?;
    let mods = usize::try_from(record.extra.get("modCount")?.as_u64()?).ok()?;
    let suggested_mb = crate::test_session::choose_memory(
        TestMemory::Auto,
        TestMemory::Auto,
        mods,
        crate::test_session::total_memory_mb(),
    )
    .mb;
    Some(SessionMemory {
        mb,
        auto,
        suggested_mb,
    })
}

/// A data da versão mais nova de cada item no último "Verificar atualizações": a instalada,
/// quando está em dia, ou a nova, quando há atualização.
fn releases(report: &ModUpdateReport) -> Vec<LatestRelease> {
    report
        .items
        .iter()
        .filter_map(|item| {
            let published = match item.status {
                UpdateStatus::UpToDate => item.current_published.as_deref()?,
                UpdateStatus::Available => item.new_version.as_ref()?.published.as_deref()?,
                _ => return None,
            };
            Some(LatestRelease {
                item: item.path.clone(),
                name: item.name.clone(),
                published_ms: published_ms(published)?,
            })
        })
        .collect()
}

/// `2024-03-01T10:20:30.123456Z` (Modrinth) ou `2024-03-01T10:20:30.12Z` (CurseForge) → ms
/// desde 1970 (a nota só precisa do mês).
fn published_ms(text: &str) -> Option<u64> {
    crate::test_session::parse_rfc3339_utc_ms(&format!("{}Z", text.get(..19)?))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::collections::BTreeMap;
    use std::fs;

    use warden_diagnostics::health::HealthCategory;
    use warden_diagnostics::{Evidence, Finding, RuleCode, Severity};
    use warden_launcher::outcome::{CrashArtifacts, Outcome};
    use warden_launcher::session::{SESSION_SCHEMA, SessionDir};
    use warden_launcher::{GameSpec, LoaderSpec};
    use warden_project::inventory::ItemSource;
    use warden_project::registry::PackRecord;
    use warden_project::updates::{NewVersion, UpdateChannel, UpdateItem};

    use super::*;
    use crate::commands::diagnostics::report::{
        CheckPass, DiagnosticsReport, ReportFinding, ReportItem, store_report, write_ignored,
    };
    use crate::state::tests::test_state;

    fn finding(rule: &str, severity: Severity, path: &str) -> ReportFinding {
        let finding = Finding::new(
            RuleCode::parse(rule).unwrap(),
            severity,
            "t",
            Evidence::PackFile {
                path: path.into(),
                excerpt: String::new(),
            },
        );
        ReportFinding {
            id: format!("{rule}|{path}"),
            finding,
            items: vec![ReportItem {
                path: Some(path.into()),
                name: path.into(),
            }],
            pattern: None,
            labels: BTreeMap::new(),
            ignored: false,
        }
    }

    fn session_record(outcome: Outcome, started_ms: u64, tree: &str) -> SessionRecord {
        let mut extra = serde_json::Map::new();
        extra.insert("startedAtMs".into(), started_ms.into());
        extra.insert(
            "packTree".into(),
            serde_json::json!({ "hash": tree, "head": null, "unsavedFiles": 0 }),
        );
        extra.insert("memory".into(), serde_json::json!({ "mb": 1024, "auto": false }));
        extra.insert("modCount".into(), 250.into());
        SessionRecord {
            schema: SESSION_SCHEMA,
            started_at: "2026-10-01T00:00:00Z".into(),
            ended_at: "2026-10-01T00:01:00Z".into(),
            duration_ms: 60_000,
            outcome,
            exit_code: (outcome == Outcome::Crashed).then_some(-1),
            pack_version: Some("1.0.0".into()),
            game: GameSpec::new("1.20.1", LoaderSpec::Forge {
                version: "47.4.10".into(),
            })
            .unwrap(),
            java_major: 17,
            artifacts: CrashArtifacts::default(),
            extra,
        }
    }

    /// Um pack registrado com `pack.toml` e índice, sem relatório.
    fn registered_pack(state: &AppState, dir: &Path) -> (PackId, std::path::PathBuf) {
        let root = dir.join("pack");
        fs::create_dir_all(&root).unwrap();
        let manifest = warden_packwiz::PackManifest::new("Saúde", "1.20.1");
        fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
        fs::write(root.join("index.toml"), "hash-format = \"sha256\"\n").unwrap();
        let id = PackId::new();
        state
            .packs
            .insert(PackRecord {
                id,
                name: "Saúde".into(),
                path: root.clone(),
                last_test: None,
                extra: BTreeMap::new(),
            })
            .unwrap();
        (id, root)
    }

    fn report(findings: Vec<ReportFinding>) -> DiagnosticsReport {
        DiagnosticsReport {
            pass: CheckPass::Complete,
            checked_at_ms: 1,
            item_count: 3,
            checked_mods: true,
            findings,
            pack_hash: None,
            stale: false,
        }
    }

    fn listed(state: &AppState) -> Vec<(PackId, u8, HealthBand)> {
        stored(&state.packs)
            .into_iter()
            .map(|entry| (entry.pack_id, entry.health.score, entry.health.band))
            .collect()
    }

    #[test]
    fn sem_relatorio_nao_ha_nota_e_a_guardada_some() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let (id, _root) = registered_pack(&state, dir.path());
        state
            .packs
            .set_extra(id, REGISTRY_KEY, Some(serde_json::json!({"score": 50, "band": "attention"})))
            .unwrap();
        assert_eq!(refresh(&state, id).unwrap(), None);
        assert!(listed(&state).is_empty());
    }

    #[test]
    fn nota_com_relatorio_sessoes_e_ignorados_fica_em_packs_json() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let (id, root) = registered_pack(&state, dir.path());
        store_report(
            state.paths.data_dir(),
            id,
            &report(vec![
                finding("E_MISSING_DEP", Severity::Error, "mods/a.pw.toml"),
                finding("W_SIDE", Severity::Warning, "mods/b.pw.toml"),
                finding("W_DOWNLOAD_BLOCKED", Severity::Warning, "mods/c.pw.toml"),
            ]),
        );
        // Nunca testado: −15 −3 −2 −10.
        let health = refresh(&state, id).unwrap().unwrap();
        assert_eq!(health.score, 70);
        assert_eq!(listed(&state), [(id, 70, HealthBand::Attention)]);

        // Um teste que travou com a árvore de agora e a memória abaixo da sugestão.
        let sessions = sessions_dir(&state, id);
        let session = sessions.join("2026-10-01_00-00-00");
        fs::create_dir_all(&session).unwrap();
        let tree = current_pack_hash(&root).unwrap();
        SessionDir::open(session)
            .write_record(&session_record(Outcome::Crashed, 1_000, &tree))
            .unwrap();
        let health = refresh(&state, id).unwrap().unwrap();
        let categories: Vec<HealthCategory> =
            health.lines.iter().map(|line| line.category).collect();
        assert_eq!(categories[..3], [
            HealthCategory::Problems,
            HealthCategory::LastTest,
            HealthCategory::Crashes,
        ]);
        assert!(categories.contains(&HealthCategory::Performance));
        let lost: u32 = health.lines.iter().map(|line| line.points).sum();
        assert_eq!(u32::from(health.score), 100 - lost);
        assert_eq!(listed(&state), [(id, health.score, health.band)]);

        // "Ignorar neste pack" devolve os pontos do aviso, na hora.
        write_ignored(&root, "W_SIDE|mods/b.pw.toml", true).unwrap();
        let ignored = refresh(&state, id).unwrap().unwrap();
        assert_eq!(ignored.score, health.score + 3);
        assert!(ignored.lines.iter().any(|line| line.ignored));
        assert_eq!(listed(&state), [(id, ignored.score, ignored.band)]);
    }

    #[test]
    fn lista_so_le_o_que_esta_guardado() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let (id, _root) = registered_pack(&state, dir.path());
        let (other, _) = {
            let sub = dir.path().join("outro");
            fs::create_dir_all(&sub).unwrap();
            registered_pack(&state, &sub)
        };
        // Relatório gravado, mas nenhuma nota calculada: a lista não calcula.
        store_report(state.paths.data_dir(), id, &report(Vec::new()));
        assert!(listed(&state).is_empty());
        state
            .packs
            .set_extra(other, REGISTRY_KEY, Some(serde_json::json!({"score": 88, "band": "good"})))
            .unwrap();
        // Valor ilegível (de outra versão) é ignorado.
        state
            .packs
            .set_extra(id, REGISTRY_KEY, Some(serde_json::json!("lixo")))
            .unwrap();
        assert_eq!(listed(&state), [(other, 88, HealthBand::Good)]);
    }

    #[test]
    fn datas_das_versoes_do_relatorio_de_atualizacoes() {
        assert_eq!(
            published_ms("2024-03-01T10:20:30.123456Z"),
            published_ms("2024-03-01T10:20:30Z")
        );
        assert!(published_ms("2024-03-01T10:20:30.12+00:00").is_some());
        assert_eq!(published_ms("ontem"), None);
        let item = |path: &str, status, current_published: Option<&str>, new: Option<&str>| {
            UpdateItem {
                key: format!("modrinth:{path}"),
                path: path.into(),
                name: path.into(),
                source: ItemSource::Modrinth,
                status,
                current_id: Some("x".into()),
                current: Some("1".into()),
                current_published: current_published.map(Into::into),
                new_version: new.map(|published| NewVersion {
                    id: "z".into(),
                    number: "2".into(),
                    channel: UpdateChannel::Release,
                    published: Some(published.into()),
                }),
                reason: None,
                detail: None,
            }
        };
        let report = ModUpdateReport {
            checked_at: "2026-10-01T00:00:00Z".into(),
            available: 1,
            items: vec![
                item("mods/a.pw.toml", UpdateStatus::UpToDate, Some("2024-01-01T00:00:00.5Z"), None),
                item("mods/b.pw.toml", UpdateStatus::Available, None, Some("2026-09-01T00:00:00Z")),
                // CurseForge em dia: a verificação não traz a data.
                item("mods/c.pw.toml", UpdateStatus::UpToDate, None, None),
                item("mods/d.pw.toml", UpdateStatus::Failed, Some("2020-01-01T00:00:00Z"), None),
            ],
        };
        let found = releases(&report);
        assert_eq!(
            found.iter().map(|release| release.item.as_str()).collect::<Vec<_>>(),
            ["mods/a.pw.toml", "mods/b.pw.toml"]
        );
    }
}
