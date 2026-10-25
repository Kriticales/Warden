//! Comandos do domínio `crashes` (SPEC T14 "Travamentos"; ARCHITECTURE §9.6; ROADMAP D-06).
//!
//! - `crashes_list`: as sessões do Testar que travaram, agrupadas pela assinatura (contagem,
//!   primeira e última vez, versões do pack, situação e o "Por que travou" da mais nova), mais o
//!   estado da retenção dos travamentos guardados.
//! - `crash_get`: um grupo com cada vez que travou.
//! - `crashes_prune`: apaga os travamentos mais antigos que passam do limite de espaço, depois
//!   que a pessoa confirma na interface (só os que ela confirmou e que ainda passam do limite).
//!
//! A assinatura é calculada pelo gancho `test_hooks/crash_history.rs` logo que o jogo fecha e
//! gravada no `session.json` (campo [`SESSION_FIELD`]). Sessões gravadas antes disso ganham a
//! assinatura na hora da leitura, só com o `output.log` da sessão (os logs da instância já são
//! de outro teste).
//!
//! Retenção (ARCHITECTURE §7.4): as 30 sessões mais novas que não travaram ficam (o Testar
//! apaga as outras ao gravar cada sessão); as que travaram ficam até 500 MB por pack. Passando
//! de 90% do limite a interface avisa; passando do limite, oferece apagar as mais antigas. "Mais
//! antigas" na ordem da lista de Travamentos (o início de cada teste); o travamento mais novo e
//! o do teste aberto nunca são apagados, mesmo sozinhos passando do limite.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;
use warden_core::PackId;
use warden_diagnostics::crash_history::{
    CrashGroup, CrashOrigin, CrashStatus, PackState, SessionEnd, SessionEntry, group_crashes,
    session_order,
};
use warden_diagnostics::postcrash::{SessionFiles, SessionOutcome, collect_session};
use warden_diagnostics::{AnalysisContext, CrashSignature, LogLoader, SIGNATURE_VERSION};
use warden_instance::InstanceDirs;
use warden_launcher::LoaderSpec;
use warden_launcher::outcome::Outcome;
use warden_launcher::session::{
    CRASHED_BUDGET_BYTES, SessionRecord, StoredSession, list_sessions, session_size,
};

use crate::commands::diagnostics::crash::{CrashDiagnosis, SESSION_FIELD as DIAGNOSIS_FIELD};
use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::state::AppState;
use crate::test_session::TestSessionSummary;

/// Campo do `session.json` com o registro do travamento (origem e assinatura).
pub(crate) const SESSION_FIELD: &str = "crash";

/// A partir de que fração do limite a interface avisa.
const NEAR_LIMIT_PERCENT: u64 = 90;

/// O registro do travamento no `session.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashRecord {
    /// De onde veio (gancho 1.2, ADR-0039: por enquanto sempre `test`).
    pub(crate) origin: CrashOrigin,
    /// A assinatura.
    pub(crate) signature: CrashSignature,
}

/// Uma versão do pack em que travou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashVersion {
    /// `version` do `pack.toml` (`None`: sem versão).
    pub(crate) version: Option<String>,
    /// Havia alterações não salvas como versão.
    pub(crate) unsaved: bool,
}

impl From<&PackState> for CrashVersion {
    fn from(state: &PackState) -> Self {
        Self {
            version: state.version.clone(),
            unsaved: state.unsaved,
        }
    }
}

/// A situação de um grupo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub(crate) enum CrashSituation {
    /// "Não voltou a acontecer desde a versão X".
    Resolved {
        /// A versão em que o jogo fechou normalmente depois do último travamento.
        since: CrashVersion,
    },
    /// "Causa encontrada".
    CauseFound,
    /// "Causa não encontrada".
    CauseUnknown,
}

/// Quantas vezes por origem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashOriginCount {
    /// A origem.
    pub(crate) origin: CrashOrigin,
    /// Quantas vezes.
    pub(crate) count: u32,
}

/// O que está ligado ao grupo: buscas do culpado (D-12) e conversas com a IA (D-04).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashLinks {
    /// Buscas do culpado deste travamento.
    pub(crate) culprit_searches: Vec<String>,
    /// Conversas com a IA sobre este travamento.
    pub(crate) conversations: Vec<String>,
}

/// Uma linha de Travamentos: as vezes que o jogo travou com a mesma causa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashGroupView {
    /// A assinatura (o `hash` identifica o grupo).
    pub(crate) signature: CrashSignature,
    /// Quantas vezes travou.
    pub(crate) count: u32,
    /// Primeira vez (milissegundos desde 1970).
    #[specta(type = specta_typescript::Number)]
    pub(crate) first_at_ms: u64,
    /// Última vez.
    #[specta(type = specta_typescript::Number)]
    pub(crate) last_at_ms: u64,
    /// Versões do pack em que travou, da mais antiga para a mais nova.
    pub(crate) versions: Vec<CrashVersion>,
    /// Quantas vezes por origem.
    pub(crate) origins: Vec<CrashOriginCount>,
    /// A situação.
    pub(crate) situation: CrashSituation,
    /// A sessão mais nova (o resultado do teste que "Ver o que causou" abre).
    pub(crate) latest_session_id: String,
    /// "Por que travou" da sessão mais nova, quando foi gravado.
    pub(crate) diagnosis: Option<CrashDiagnosis>,
    /// Busca do culpado e conversas ligadas.
    pub(crate) links: CrashLinks,
}

/// O estado dos travamentos guardados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashRetention {
    /// O limite de espaço dos travamentos guardados, por pack.
    #[specta(type = specta_typescript::Number)]
    pub(crate) budget_bytes: u64,
    /// Os travamentos guardados passaram de 90% do limite.
    pub(crate) near_limit: bool,
    /// Os travamentos guardados passaram do limite.
    pub(crate) over_limit: bool,
    /// As sessões que passam do limite e podem ser apagadas, da mais antiga para a mais nova
    /// (nunca a mais nova nem a do teste aberto). "Apagar os mais antigos" manda estas.
    pub(crate) prunable: Vec<String>,
}

/// Travamentos do pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashList {
    /// Os grupos, do que travou por último para o que travou primeiro.
    pub(crate) groups: Vec<CrashGroupView>,
    /// A retenção.
    pub(crate) retention: CrashRetention,
}

/// Uma vez em que travou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashOccurrence {
    /// A sessão.
    pub(crate) session_id: String,
    /// Quando começou.
    #[specta(type = specta_typescript::Number)]
    pub(crate) started_at_ms: u64,
    /// A versão do pack.
    pub(crate) version: CrashVersion,
    /// Código de saída.
    pub(crate) exit_code: Option<i32>,
    /// A origem.
    pub(crate) origin: CrashOrigin,
}

/// Um grupo com cada vez que travou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashDetail {
    /// O grupo.
    pub(crate) group: CrashGroupView,
    /// Cada vez, da mais nova para a mais antiga.
    pub(crate) occurrences: Vec<CrashOccurrence>,
}

/// Os travamentos do pack, agrupados pela causa.
#[tauri::command]
#[specta::specta]
pub(crate) async fn crashes_list(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<CrashList, AppError> {
    pack_root(&state, pack_id)?;
    let sessions = sessions_dir(&state, pack_id);
    let open = open_session(&state, pack_id);
    blocking(move || list_at(&sessions, CRASHED_BUDGET_BYTES, open.as_deref())).await
}

/// Um grupo de travamentos pelo `hash` da assinatura (`None`: não existe mais, por exemplo
/// depois de apagar os mais antigos).
#[tauri::command]
#[specta::specta]
pub(crate) async fn crash_get(
    state: State<'_, AppState>,
    pack_id: PackId,
    signature: String,
) -> Result<Option<CrashDetail>, AppError> {
    pack_root(&state, pack_id)?;
    let sessions = sessions_dir(&state, pack_id);
    blocking(move || detail_at(&sessions, &signature)).await
}

/// Apaga, das sessões que a pessoa confirmou (`sessions`, o `prunable` da lista), as que ainda
/// passam do limite de espaço. Devolve quantas apagou.
#[tauri::command]
#[specta::specta]
pub(crate) async fn crashes_prune(
    state: State<'_, AppState>,
    pack_id: PackId,
    sessions: Vec<String>,
) -> Result<u32, AppError> {
    pack_root(&state, pack_id)?;
    let dir = sessions_dir(&state, pack_id);
    let open = open_session(&state, pack_id);
    blocking(move || prune_crashed(&dir, CRASHED_BUDGET_BYTES, open.as_deref(), &sessions)).await
}

pub(crate) fn sessions_dir(state: &AppState, pack_id: PackId) -> PathBuf {
    InstanceDirs::for_pack(&state.paths, pack_id)
        .state_dir
        .join("sessions")
}

/// A sessão do teste aberto agora neste pack, se houver.
fn open_session(state: &AppState, pack_id: PackId) -> Option<String> {
    state
        .tests
        .current()
        .filter(|game| game.pack_id == pack_id)
        .and_then(|game| game.session_id)
}

/// Roda trabalho de disco fora da thread do IPC.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("ler os travamentos: {error}")))?
}

/// Os travamentos de uma pasta de sessões.
fn list_at(sessions: &Path, budget: u64, open: Option<&str>) -> Result<CrashList, AppError> {
    let history = History::read(sessions)?;
    Ok(CrashList {
        groups: history
            .groups
            .iter()
            .map(|group| history.view(group))
            .collect(),
        retention: retention(sessions, budget, open)?,
    })
}

/// Um grupo de uma pasta de sessões.
fn detail_at(sessions: &Path, signature: &str) -> Result<Option<CrashDetail>, AppError> {
    let history = History::read(sessions)?;
    Ok(history
        .groups
        .iter()
        .find(|group| group.signature.hash == signature)
        .map(|group| history.detail(group)))
}

/// O que a retenção vê nas sessões que travaram.
struct RetentionPlan {
    /// Espaço ocupado por todas.
    used: u64,
    /// As que passam do limite e podem ser apagadas, da mais antiga para a mais nova.
    prunable: Vec<(String, PathBuf)>,
}

/// Soma o espaço das sessões que travaram da mais nova para a mais antiga, na ordem da lista de
/// Travamentos (início do teste; no mesmo segundo, `-2` antes de `-10`); as que passam do limite
/// podem ser apagadas, menos a mais nova e a do teste aberto (`open`). Sessões sem
/// `session.json` (o jogo ainda aberto ou a gravação interrompida) ficam de fora.
fn retention_plan(
    sessions: &Path,
    budget: u64,
    open: Option<&str>,
) -> Result<RetentionPlan, AppError> {
    let stored = list_sessions(sessions).map_err(|error| AppError::from_domain(&error))?;
    let mut crashed: Vec<(u64, String, &StoredSession)> = stored
        .iter()
        .filter_map(|session| {
            let record = session.record.as_ref()?;
            if record.outcome != Outcome::Crashed {
                return None;
            }
            let id = session_name(session);
            let started = TestSessionSummary::from_record(&id, record).started_at_ms;
            Some((started, id, session))
        })
        .collect();
    crashed.sort_by(|a, b| session_order(b.0, &b.1, a.0, &a.1));
    let mut used = 0u64;
    let mut prunable = Vec::new();
    for (index, (_, id, session)) in crashed.iter().enumerate() {
        used = used.saturating_add(session_size(&session.dir));
        let kept = index == 0 || open == Some(id.as_str());
        if used > budget && !kept {
            prunable.push((id.clone(), session.dir.path().to_path_buf()));
        }
    }
    prunable.reverse();
    Ok(RetentionPlan { used, prunable })
}

/// O estado da retenção dos travamentos guardados.
fn retention(sessions: &Path, budget: u64, open: Option<&str>) -> Result<CrashRetention, AppError> {
    let plan = retention_plan(sessions, budget, open)?;
    Ok(CrashRetention {
        budget_bytes: budget,
        near_limit: plan.used > budget / 100 * NEAR_LIMIT_PERCENT,
        over_limit: plan.used > budget,
        prunable: plan.prunable.into_iter().map(|(id, _)| id).collect(),
    })
}

/// Apaga, das sessões confirmadas, as que ainda passam do limite (das mais antigas para as mais
/// novas). Uma confirmada que deixou de passar do limite (outra foi apagada antes, por exemplo)
/// fica.
fn prune_crashed(
    sessions: &Path,
    budget: u64,
    open: Option<&str>,
    confirmed: &[String],
) -> Result<u32, AppError> {
    let plan = retention_plan(sessions, budget, open)?;
    let mut removed = 0u32;
    for (id, path) in plan
        .prunable
        .iter()
        .filter(|(id, _)| confirmed.contains(id))
    {
        match std::fs::remove_dir_all(path) {
            Ok(()) => removed += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(%error, session = %id, "travamento antigo não apagado");
                return Err(AppError::internal(format!("apagar a sessão {id}: {error}")));
            }
        }
    }
    tracing::info!(removed, "travamentos antigos apagados");
    Ok(removed)
}

/// O nome da pasta de uma sessão (o id da sessão).
fn session_name(session: &StoredSession) -> String {
    session
        .dir
        .path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A assinatura de um travamento a partir dos arquivos da sessão (gancho do Testar ou, numa
/// sessão antiga, só o `output.log`).
pub(crate) fn signature_of(
    files: &SessionFiles,
    game: &warden_launcher::GameSpec,
    exit_code: Option<i32>,
) -> Result<CrashSignature, AppError> {
    let sources = collect_session(files).map_err(|error| AppError::from_domain(&error))?;
    let context = AnalysisContext {
        loader: log_loader(&game.loader),
        minecraft: Some(game.minecraft.as_str().to_owned()),
    };
    let outcome = SessionOutcome {
        exit_code,
        stopped_by_user: false,
        froze: false,
    };
    let analysis = warden_diagnostics::analyze(&sources, &context, Some(&outcome))
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(warden_diagnostics::signature(&analysis, &sources))
}

fn log_loader(loader: &LoaderSpec) -> Option<LogLoader> {
    match loader {
        LoaderSpec::Fabric { .. } => Some(LogLoader::Fabric),
        LoaderSpec::Quilt { .. } => Some(LogLoader::Quilt),
        LoaderSpec::Forge { .. } => Some(LogLoader::Forge),
        LoaderSpec::NeoForge { .. } => Some(LogLoader::NeoForge),
        LoaderSpec::Vanilla => None,
    }
}

/// As sessões do pack lidas para o histórico.
struct History {
    groups: Vec<CrashGroup>,
    entries: HashMap<String, Entry>,
    /// `state/` da instância (as buscas do culpado ligadas, D-12).
    state_dir: Option<PathBuf>,
}

/// O que o histórico guarda de cada sessão que travou.
struct Entry {
    started_at_ms: u64,
    version: CrashVersion,
    exit_code: Option<i32>,
    origin: CrashOrigin,
    diagnosis: Option<CrashDiagnosis>,
}

impl History {
    fn read(sessions: &Path) -> Result<Self, AppError> {
        let stored = list_sessions(sessions).map_err(|error| AppError::from_domain(&error))?;
        let mut entries = HashMap::new();
        let mut list = Vec::new();
        for session in &stored {
            let Some(record) = &session.record else {
                continue;
            };
            let entry = session_entry(session, record);
            if entry.end == SessionEnd::Crashed {
                entries.insert(
                    entry.id.clone(),
                    Entry {
                        started_at_ms: entry.started_at_ms,
                        version: CrashVersion::from(&entry.pack),
                        exit_code: record.exit_code,
                        origin: entry.origin,
                        diagnosis: record
                            .extra
                            .get(DIAGNOSIS_FIELD)
                            .and_then(|value| serde_json::from_value(value.clone()).ok()),
                    },
                );
            }
            list.push(entry);
        }
        Ok(Self {
            groups: group_crashes(&list),
            entries,
            state_dir: sessions.parent().map(Path::to_path_buf),
        })
    }

    fn view(&self, group: &CrashGroup) -> CrashGroupView {
        let latest = group.latest();
        CrashGroupView {
            signature: group.signature.clone(),
            count: u32::try_from(group.count()).unwrap_or(u32::MAX),
            first_at_ms: group.first_at_ms,
            last_at_ms: group.last_at_ms,
            versions: group.versions.iter().map(CrashVersion::from).collect(),
            origins: group
                .origins
                .iter()
                .map(|&(origin, count)| CrashOriginCount { origin, count })
                .collect(),
            situation: match &group.status {
                CrashStatus::Resolved { since, .. } => CrashSituation::Resolved {
                    since: CrashVersion::from(since),
                },
                CrashStatus::CauseFound => CrashSituation::CauseFound,
                CrashStatus::CauseUnknown => CrashSituation::CauseUnknown,
            },
            latest_session_id: latest.to_owned(),
            diagnosis: self
                .entries
                .get(latest)
                .and_then(|entry| entry.diagnosis.clone()),
            links: CrashLinks {
                culprit_searches: self.state_dir.as_deref().map_or_else(Vec::new, |dir| {
                    crate::bisect_session::searches_for(dir, &group.signature.hash)
                }),
                ..CrashLinks::default()
            },
        }
    }

    fn detail(&self, group: &CrashGroup) -> CrashDetail {
        CrashDetail {
            group: self.view(group),
            occurrences: group
                .sessions
                .iter()
                .filter_map(|id| {
                    let entry = self.entries.get(id)?;
                    Some(CrashOccurrence {
                        session_id: id.clone(),
                        started_at_ms: entry.started_at_ms,
                        version: entry.version.clone(),
                        exit_code: entry.exit_code,
                        origin: entry.origin,
                    })
                })
                .collect(),
        }
    }
}

/// Uma sessão gravada como o histórico a vê. Sessão que travou sem assinatura gravada (de antes
/// da D-06) ganha uma calculada do `output.log`. A nota de saúde (D-08) lê as sessões por aqui.
pub(crate) fn session_entry(session: &StoredSession, record: &SessionRecord) -> SessionEntry {
    let id = session_name(session);
    let summary = TestSessionSummary::from_record(&id, record);
    let end = match record.outcome {
        Outcome::Crashed => SessionEnd::Crashed,
        Outcome::ClosedNormally => SessionEnd::ClosedNormally,
        Outcome::StoppedByUser => SessionEnd::Stopped,
    };
    let tree = record.extra.get("packTree");
    let pack = PackState {
        version: record.pack_version.clone(),
        tree: tree
            .and_then(|tree| tree.get("hash"))
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        unsaved: tree
            .and_then(|tree| tree.get("unsavedFiles"))
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|files| files > 0),
    };
    let stored: Option<CrashRecord> = record
        .extra
        .get(SESSION_FIELD)
        .and_then(|value| serde_json::from_value(value.clone()).ok());
    let origin = stored
        .as_ref()
        .map_or(CrashOrigin::Test, |stored| stored.origin);
    let signature = match (end, stored) {
        (_, Some(stored)) if stored.signature.version == SIGNATURE_VERSION => {
            Some(stored.signature)
        }
        // Gravada antes da D-06 ou com regras antigas de assinatura (o hash não agruparia com
        // as novas): recalcula do `output.log`.
        (SessionEnd::Crashed, _) => legacy_signature(session, record),
        _ => None,
    };
    let found_cause = record
        .extra
        .get(DIAGNOSIS_FIELD)
        .and_then(|value| value.get("foundCause"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    SessionEntry {
        id,
        started_at_ms: summary.started_at_ms,
        end,
        pack,
        signature,
        found_cause,
        origin,
    }
}

/// A assinatura de uma sessão gravada antes da D-06 (ou com regras de assinatura antigas), só com
/// o `output.log` (a pasta da sessão faz as vezes da pasta do jogo: não tem `logs/` nem
/// `crash-reports/`).
fn legacy_signature(session: &StoredSession, record: &SessionRecord) -> Option<CrashSignature> {
    let dir = session.dir.path().to_path_buf();
    let files = SessionFiles {
        session_dir: Some(dir.clone()),
        game_dir: dir,
        started_at: UNIX_EPOCH,
    };
    match signature_of(&files, &record.game, record.exit_code) {
        Ok(signature) => Some(signature),
        Err(error) => {
            tracing::info!(%error, "assinatura de sessão antiga indisponível");
            None
        }
    }
}

/// Início do processo com folga, para a coleta da sessão (arquivos mais antigos não contam).
pub(crate) fn started_at(record: &SessionRecord) -> SystemTime {
    let started_ms = record
        .extra
        .get("startedAtMs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    UNIX_EPOCH + Duration::from_millis(started_ms).saturating_sub(Duration::from_secs(5))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use warden_launcher::GameSpec;
    use warden_launcher::outcome::CrashArtifacts;
    use warden_launcher::session::{KEEP_CLOSED_SESSIONS, SESSION_SCHEMA, SessionDir, prune_closed};

    use super::*;

    const NPE_REPORT: &str = "---- Minecraft Crash Report ----
// Who set us up the TNT?

Time: 2026-10-13 12:00:01
Description: Ticking entity

java.lang.NullPointerException: Cannot invoke \"Object.hashCode()\" because \"<local3>\" is null
\tat com.example.meumod.EntityTicker.tick(EntityTicker.java:41) ~[meumod-1.0.jar%23101!/:?]
\tat net.minecraft.world.level.Level.guardEntityTick(Level.java:501) ~[client-1.20.1-srg.jar%23185!/:?]

-- Entity being ticked --
Details:
\tEntity Type: meumod:golem (com.example.meumod.Golem)
\tEntity's Exact location: 13.37, 64.00, -7.10
";

    fn record(outcome: Outcome, started_ms: u64, version: &str, tree: &str) -> SessionRecord {
        let mut extra = serde_json::Map::new();
        extra.insert("startedAtMs".into(), started_ms.into());
        extra.insert(
            "packTree".into(),
            serde_json::json!({ "hash": tree, "head": null, "unsavedFiles": 0 }),
        );
        SessionRecord {
            schema: SESSION_SCHEMA,
            started_at: "2026-10-01T00:00:00Z".into(),
            ended_at: "2026-10-01T00:01:00Z".into(),
            duration_ms: 60_000,
            outcome,
            exit_code: (outcome == Outcome::Crashed).then_some(-1),
            pack_version: Some(version.into()),
            game: GameSpec::new("1.20.1", LoaderSpec::Forge { version: "47.4.10".into() })
                .unwrap(),
            java_major: 17,
            artifacts: CrashArtifacts::default(),
            extra,
        }
    }

    /// Grava uma sessão `2026-10-01_00-<n>` com `bytes` no `output.log`.
    fn write_session(
        sessions: &Path,
        minute: u32,
        mut record: SessionRecord,
        output: &[u8],
        crash: Option<&CrashSignature>,
    ) -> String {
        let id = format!("2026-10-01_00-{:02}-{:02}", minute / 60, minute % 60);
        let dir = sessions.join(&id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("output.log"), output).unwrap();
        if let Some(signature) = crash {
            let value = serde_json::to_value(CrashRecord {
                origin: CrashOrigin::Test,
                signature: signature.clone(),
            })
            .unwrap();
            record.extra.insert(SESSION_FIELD.into(), value);
        }
        SessionDir::open(dir).write_record(&record).unwrap();
        id
    }

    fn signature(rule: &str) -> CrashSignature {
        CrashSignature::new(Some(rule.into()), None, None, Vec::new(), Vec::new())
    }

    fn names(sessions: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(sessions)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn retencao_com_40_sessoes_apaga_as_certas_e_avisa_antes_do_limite() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path().join("sessions");
        // 40 sessões: a cada cinco, uma travou (8 travadas, 32 que fecharam).
        let big = vec![b'x'; 100_000];
        let mut closed = Vec::new();
        let mut crashed = Vec::new();
        for n in 0..40u32 {
            let started = u64::from(n) * 60_000;
            if n % 5 == 4 {
                let id = write_session(
                    &sessions,
                    n,
                    record(Outcome::Crashed, started, "1.0.0", "t1"),
                    &big,
                    Some(&signature("x")),
                );
                crashed.push(id);
            } else {
                let id = write_session(
                    &sessions,
                    n,
                    record(Outcome::ClosedNormally, started, "1.0.0", "t1"),
                    b"ok",
                    None,
                );
                closed.push(id);
            }
        }
        // Cada travada ocupa um pouco mais de 100 kB; o limite do teste cabe 4 delas.
        let budget = 440_000;

        // As que fecharam: o Testar guarda as 30 mais novas (apaga as 2 mais antigas).
        let removed = prune_closed(&sessions, KEEP_CLOSED_SESSIONS).unwrap();
        assert_eq!(removed.len(), 2);
        // As que travaram nunca são apagadas sem confirmação: só avisadas.
        assert_eq!(names(&sessions).len(), 38);
        let status = retention(&sessions, budget, None).unwrap();
        assert_eq!(
            status,
            CrashRetention {
                budget_bytes: budget,
                near_limit: true,
                over_limit: true,
                prunable: crashed[..4].to_vec(),
            }
        );
        // "Apagar os mais antigos" apaga as 4 travadas mais antigas.
        assert_eq!(
            prune_crashed(&sessions, budget, None, &status.prunable).unwrap(),
            4
        );
        let mut expected: Vec<String> = closed[2..].to_vec();
        expected.extend(crashed[4..].iter().cloned());
        expected.sort();
        assert_eq!(names(&sessions), expected);
        let after = retention(&sessions, budget, None).unwrap();
        assert!(after.prunable.is_empty());
        assert!(!after.over_limit);
        // 4 travadas (~402 kB) passam de 90% do limite: o aviso continua, sem nada a apagar.
        assert!(after.near_limit);
        // Com folga, nem aviso.
        assert!(!retention(&sessions, 1_000_000, None).unwrap().near_limit);
        assert_eq!(
            prune_crashed(&sessions, 1_000_000, None, &crashed).unwrap(),
            0
        );
    }

    /// Grava uma sessão que travou com o nome `id`, início `started_ms` e `bytes` de log.
    fn crashed_session(sessions: &Path, id: &str, started_ms: Option<u64>, bytes: usize) {
        let dir = sessions.join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("output.log"), vec![b'x'; bytes]).unwrap();
        let mut record = record(Outcome::Crashed, started_ms.unwrap_or(0), "1.0.0", "t1");
        if started_ms.is_none() {
            record.extra.remove("startedAtMs");
        }
        let value = serde_json::to_value(CrashRecord {
            origin: CrashOrigin::Test,
            signature: signature("x"),
        })
        .unwrap();
        record.extra.insert(SESSION_FIELD.into(), value);
        SessionDir::open(dir).write_record(&record).unwrap();
    }

    #[test]
    fn retencao_nunca_apaga_o_travamento_mais_novo_mesmo_sozinho_passando_do_limite() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path();
        crashed_session(sessions, "2026-10-01_00-00-00", Some(1_000), 10_000);
        crashed_session(sessions, "2026-10-01_00-01-00", Some(61_000), 10_000);
        // O mais novo sozinho ocupa mais que o limite (um log enorme).
        crashed_session(sessions, "2026-10-01_00-02-00", Some(121_000), 500_000);
        let budget = 100_000;

        let status = retention(sessions, budget, None).unwrap();
        assert!(status.over_limit);
        assert!(status.near_limit);
        // Na ponta da 1ª revisão o mais novo entrava na lista e era apagado.
        assert_eq!(
            status.prunable,
            ["2026-10-01_00-00-00", "2026-10-01_00-01-00"]
        );
        // Mesmo pedido, o mais novo fica.
        let everything = names(sessions);
        assert_eq!(
            prune_crashed(sessions, budget, None, &everything).unwrap(),
            2
        );
        assert_eq!(names(sessions), ["2026-10-01_00-02-00"]);
        let after = retention(sessions, budget, None).unwrap();
        assert!(after.over_limit);
        assert!(after.prunable.is_empty());
        assert_eq!(
            prune_crashed(sessions, budget, None, &everything).unwrap(),
            0
        );
        assert_eq!(names(sessions), ["2026-10-01_00-02-00"]);
    }

    #[test]
    fn retencao_ordena_pelo_inicio_do_teste_como_a_lista_e_nao_pelo_nome_da_pasta() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path();
        // O nome da pasta diz o contrário do início gravado (relógio do sistema acertado entre
        // um teste e outro): vale o início, que é o que a lista de Travamentos mostra.
        crashed_session(sessions, "2026-10-01_00-00-00", Some(300_000), 60_000);
        crashed_session(sessions, "2026-10-01_00-01-00", Some(200_000), 60_000);
        crashed_session(sessions, "2026-10-01_00-02-00", Some(100_000), 60_000);
        let budget = 130_000;
        let list = list_at(sessions, budget, None).unwrap();
        assert_eq!(list.groups[0].latest_session_id, "2026-10-01_00-00-00");
        let status = retention(sessions, budget, None).unwrap();
        assert_eq!(status.prunable, ["2026-10-01_00-02-00"]);
        prune_crashed(sessions, budget, None, &status.prunable).unwrap();
        assert_eq!(
            names(sessions),
            ["2026-10-01_00-00-00", "2026-10-01_00-01-00"]
        );
    }

    #[test]
    fn retencao_no_mesmo_segundo_a_decima_sessao_e_mais_nova_que_a_segunda() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path();
        // Sessões antigas sem `startedAtMs`: o início vem do `startedAt` (segundos), igual nas
        // três; desempata o sufixo da pasta como número.
        crashed_session(sessions, "2026-10-01_00-00-00", None, 60_000);
        crashed_session(sessions, "2026-10-01_00-00-00-2", None, 60_000);
        crashed_session(sessions, "2026-10-01_00-00-00-10", None, 60_000);
        let status = retention(sessions, 130_000, None).unwrap();
        assert_eq!(status.prunable, ["2026-10-01_00-00-00"]);
        let status = retention(sessions, 70_000, None).unwrap();
        assert_eq!(
            status.prunable,
            ["2026-10-01_00-00-00", "2026-10-01_00-00-00-2"]
        );
        prune_crashed(sessions, 70_000, None, &status.prunable).unwrap();
        assert_eq!(names(sessions), ["2026-10-01_00-00-00-10"]);
    }

    #[test]
    fn retencao_poupa_o_teste_aberto_as_sessoes_sem_registro_e_o_que_nao_foi_confirmado() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path();
        crashed_session(sessions, "2026-10-01_00-00-00", Some(1_000), 60_000);
        crashed_session(sessions, "2026-10-01_00-01-00", Some(61_000), 60_000);
        crashed_session(sessions, "2026-10-01_00-02-00", Some(121_000), 60_000);
        crashed_session(sessions, "2026-10-01_00-03-00", Some(181_000), 60_000);
        // O jogo do teste ainda aberto: a pasta existe, sem `session.json`.
        let running = sessions.join("2026-10-01_00-04-00");
        fs::create_dir_all(&running).unwrap();
        fs::write(running.join("output.log"), vec![b'x'; 900_000]).unwrap();
        let budget = 70_000;

        // Sem teste aberto: as três mais antigas passam do limite.
        let all = retention(sessions, budget, None).unwrap().prunable;
        assert_eq!(
            all,
            [
                "2026-10-01_00-00-00",
                "2026-10-01_00-01-00",
                "2026-10-01_00-02-00"
            ]
        );
        // O teste que acabou de gravar o registro, ainda aberto, fica de fora.
        let open = Some("2026-10-01_00-01-00");
        let status = retention(sessions, budget, open).unwrap();
        assert_eq!(
            status.prunable,
            ["2026-10-01_00-00-00", "2026-10-01_00-02-00"]
        );
        // Só o que foi confirmado sai.
        let confirmed = vec!["2026-10-01_00-00-00".to_owned()];
        assert_eq!(
            prune_crashed(sessions, budget, open, &confirmed).unwrap(),
            1
        );
        assert_eq!(
            names(sessions),
            [
                "2026-10-01_00-01-00",
                "2026-10-01_00-02-00",
                "2026-10-01_00-03-00",
                "2026-10-01_00-04-00"
            ]
        );
        // Confirmado, mas não passa mais do limite (nem é antigo): fica.
        let newest = vec!["2026-10-01_00-03-00".to_owned()];
        assert_eq!(prune_crashed(sessions, budget, None, &newest).unwrap(), 0);
        assert!(running.join("output.log").exists());
    }

    #[test]
    fn retencao_usa_o_limite_de_500_mb_por_pack() {
        let root = tempfile::tempdir().unwrap();
        let list = list_at(root.path(), CRASHED_BUDGET_BYTES, None).unwrap();
        assert_eq!(list.retention.budget_bytes, 500 * 1024 * 1024);
        assert!(!list.retention.near_limit);
        assert!(list.groups.is_empty());
    }

    #[test]
    fn lista_agrupa_pela_assinatura_com_versoes_situacao_e_diagnostico() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path();
        let npe = signature("npe");
        let oom = signature("oom");
        let a = write_session(
            sessions,
            1,
            record(Outcome::Crashed, 1_000, "1.4.0", "t0"),
            b"",
            Some(&oom),
        );
        let fixed = write_session(
            sessions,
            2,
            record(Outcome::ClosedNormally, 2_000, "1.4.1", "t1"),
            b"",
            None,
        );
        let b = write_session(
            sessions,
            3,
            record(Outcome::Crashed, 3_000, "1.4.1", "t1"),
            b"",
            Some(&npe),
        );
        let mut changed = record(Outcome::Crashed, 4_000, "1.4.2", "t2");
        changed.extra.insert(
            "packTree".into(),
            serde_json::json!({ "hash": "t9", "head": null, "unsavedFiles": 3 }),
        );
        changed.extra.insert(
            DIAGNOSIS_FIELD.into(),
            serde_json::json!({
                "sessionId": "x", "foundCause": false, "findings": [], "sources": []
            }),
        );
        let c = write_session(sessions, 4, changed, b"", Some(&npe));

        let list = list_at(sessions, CRASHED_BUDGET_BYTES, None).unwrap();
        assert_eq!(list.groups.len(), 2);
        let first = &list.groups[0];
        assert_eq!(first.signature.hash, npe.hash);
        assert_eq!(first.count, 2);
        assert_eq!((first.first_at_ms, first.last_at_ms), (3_000, 4_000));
        assert_eq!(first.latest_session_id, c);
        assert_eq!(
            first.versions,
            [
                CrashVersion { version: Some("1.4.1".into()), unsaved: false },
                CrashVersion { version: Some("1.4.2".into()), unsaved: true },
            ]
        );
        assert_eq!(first.situation, CrashSituation::CauseUnknown);
        assert_eq!(first.origins, [CrashOriginCount { origin: CrashOrigin::Test, count: 2 }]);
        assert!(first.diagnosis.as_ref().is_some_and(|d| !d.found_cause));
        assert_eq!(first.links, CrashLinks::default());
        let second = &list.groups[1];
        assert_eq!(second.latest_session_id, a);
        assert_eq!(
            second.situation,
            CrashSituation::Resolved {
                since: CrashVersion { version: Some("1.4.1".into()), unsaved: false }
            }
        );
        assert!(second.diagnosis.is_none());

        let detail = detail_at(sessions, &npe.hash).unwrap().unwrap();
        let ids: Vec<&str> = detail
            .occurrences
            .iter()
            .map(|occurrence| occurrence.session_id.as_str())
            .collect();
        assert_eq!(ids, [c.as_str(), b.as_str()]);
        assert_eq!(detail.occurrences[0].exit_code, Some(-1));
        assert!(detail_at(sessions, "nao-existe").unwrap().is_none());
        // A sessão que fechou normalmente não vira linha.
        assert!(list.groups.iter().all(|group| group.latest_session_id != fixed));
    }

    #[test]
    fn sessao_antiga_sem_assinatura_ganha_uma_do_output_log() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path();
        let first = write_session(
            sessions,
            1,
            record(Outcome::Crashed, 1_000, "1.0.0", "t1"),
            NPE_REPORT.as_bytes(),
            None,
        );
        let again = NPE_REPORT
            .replace("EntityTicker.java:41", "EntityTicker.java:88")
            .replace("13.37, 64.00, -7.10", "900.00, 12.00, 4.20");
        write_session(
            sessions,
            2,
            record(Outcome::Crashed, 2_000, "1.0.0", "t1"),
            again.as_bytes(),
            None,
        );
        let list = list_at(sessions, CRASHED_BUDGET_BYTES, None).unwrap();
        assert_eq!(list.groups.len(), 1);
        let group = &list.groups[0];
        assert_eq!(group.count, 2);
        assert_eq!(group.signature.rule.as_deref(), Some("crash_report.ticking_entity"));
        assert_eq!(group.signature.frame.as_deref(), Some("com.example.meumod.EntityTicker.tick"));
        assert_ne!(group.latest_session_id, first);
    }

    #[test]
    fn assinatura_gravada_com_regras_antigas_e_recalculada_e_agrupa_com_as_novas() {
        let root = tempfile::tempdir().unwrap();
        let sessions = root.path();
        // Gravada com a versão 1 das regras (hash que não agruparia com os novos).
        let mut old = signature("crash_report.ticking_entity");
        old.version = SIGNATURE_VERSION - 1;
        old.hash = "0000000000000001".into();
        write_session(
            sessions,
            1,
            record(Outcome::Crashed, 1_000, "1.0.0", "t1"),
            NPE_REPORT.as_bytes(),
            Some(&old),
        );
        write_session(
            sessions,
            2,
            record(Outcome::Crashed, 2_000, "1.0.0", "t1"),
            NPE_REPORT.replace("EntityTicker.java:41", "EntityTicker.java:7").as_bytes(),
            None,
        );
        let list = list_at(sessions, CRASHED_BUDGET_BYTES, None).unwrap();
        assert_eq!(list.groups.len(), 1);
        assert_eq!(list.groups[0].count, 2);
        assert_eq!(list.groups[0].signature.version, SIGNATURE_VERSION);
        assert_eq!(
            list.groups[0].signature.subject,
            [
                "entity_type=meumod:golem",
                "description=ticking entity",
                "entity_class=com.example.meumod.Golem"
            ]
        );
    }

    #[test]
    fn assinatura_da_sessao_le_a_pasta_do_jogo_e_ignora_os_numeros() {
        let root = tempfile::tempdir().unwrap();
        let game = GameSpec::new("1.20.1", LoaderSpec::Forge { version: "47.4.10".into() }).unwrap();
        let run = |name: &str, report: &str| {
            let session = root.path().join(name).join("sessao");
            let game_dir = root.path().join(name).join("minecraft");
            fs::create_dir_all(&session).unwrap();
            fs::create_dir_all(game_dir.join("crash-reports")).unwrap();
            fs::write(session.join("output.log"), "[main/INFO]: abrindo\n").unwrap();
            fs::write(game_dir.join("crash-reports").join("crash-1.txt"), report).unwrap();
            let files = SessionFiles {
                session_dir: Some(session),
                game_dir,
                started_at: UNIX_EPOCH,
            };
            signature_of(&files, &game, Some(-1)).unwrap()
        };
        let one = run("um", NPE_REPORT);
        let two = run(
            "dois",
            &NPE_REPORT
                .replace("Level.java:501", "Level.java:777")
                .replace("%23101", "%2340"),
        );
        assert_eq!(one, two);
        assert_eq!(one.exception.as_deref(), Some("java.lang.NullPointerException"));
        let missing = SessionFiles {
            session_dir: None,
            game_dir: root.path().join("nao-existe"),
            started_at: UNIX_EPOCH,
        };
        assert!(signature_of(&missing, &game, None).is_err());
    }

    #[test]
    fn inicio_com_folga_e_registro_serializado() {
        let mut record = record(Outcome::Crashed, 10_000, "1", "t");
        assert_eq!(started_at(&record), UNIX_EPOCH + Duration::from_secs(5));
        record.extra.remove("startedAtMs");
        assert_eq!(started_at(&record), UNIX_EPOCH);
        let json = serde_json::to_value(CrashRecord {
            origin: CrashOrigin::Test,
            signature: signature("r"),
        })
        .unwrap();
        assert_eq!(json["origin"], "test");
        assert_eq!(json["signature"]["rule"], "r");
        assert_eq!(log_loader(&LoaderSpec::Vanilla), None);
        assert_eq!(
            log_loader(&LoaderSpec::NeoForge { version: "21.1.1".into() }),
            Some(LogLoader::NeoForge)
        );
    }
}
