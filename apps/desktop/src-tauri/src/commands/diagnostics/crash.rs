//! "Por que travou" (SPEC T13 passo 7 e T14; ARCHITECTURE §9.3): a análise pós-crash de uma
//! sessão do Testar, feita pela `warden-diagnostics` com o índice de configs de mixin da D-05
//! para apontar o mod dono, os itens ligados ao pack e tudo redigido.
//!
//! O gancho `after_exit` do Testar ([`crate::test_hooks`]) analisa logo que o jogo fecha (os
//! logs da instância são reescritos no próximo teste) e grava o resultado no `session.json`,
//! campo [`SESSION_FIELD`]. Sessões gravadas antes disso são analisadas na hora, com o que
//! ainda existe na instância.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use warden_core::PackId;
use warden_diagnostics::postcrash::{AnalysisContext, SessionFiles, SessionOutcome};
use warden_diagnostics::pretest::{attribute_mixin_owners, build_index};
use warden_diagnostics::{LogLoader, SourceKind};
use warden_instance::InstanceDirs;
use warden_jarmeta::Loader;

use super::input::{index_input, read_snapshot};
use super::report::{ItemNames, ReportFinding, report_finding};
use crate::error::AppError;
use crate::operations::OperationHandle;
use crate::state::AppState;

/// Campo do `session.json` com a análise.
pub(crate) const SESSION_FIELD: &str = "diagnosis";

/// Um arquivo lido na análise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashSource {
    /// Nome relativo à sessão (`logs/latest.log`, `crash-reports/crash-….txt`).
    pub(crate) name: String,
    /// Tipo.
    pub(crate) kind: SourceKind,
    /// Linhas lidas.
    pub(crate) lines: u32,
}

/// "Por que travou" de uma sessão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrashDiagnosis {
    /// A sessão.
    pub(crate) session_id: String,
    /// Algum padrão conhecido apontou a causa (sem isso: "Não encontramos a causa
    /// automaticamente").
    pub(crate) found_cause: bool,
    /// Achados, do mais provável ao menos provável.
    pub(crate) findings: Vec<ReportFinding>,
    /// Arquivos lidos.
    pub(crate) sources: Vec<CrashSource>,
}

/// O que se sabe da sessão para analisar.
pub(crate) struct CrashRequest<'a> {
    /// Nome da pasta da sessão.
    pub(crate) session_id: &'a str,
    /// A pasta da sessão (`output.log`).
    pub(crate) session_dir: &'a Path,
    /// Início do processo (arquivos mais antigos não contam).
    pub(crate) started_at: SystemTime,
    /// Código de saída.
    pub(crate) exit_code: Option<i32>,
}

fn log_loader(loader: Loader) -> LogLoader {
    match loader {
        Loader::Fabric => LogLoader::Fabric,
        Loader::Quilt => LogLoader::Quilt,
        Loader::Forge => LogLoader::Forge,
        Loader::NeoForge => LogLoader::NeoForge,
    }
}

/// Analisa a sessão agora.
pub(crate) async fn diagnose(
    state: &AppState,
    pack_id: PackId,
    request: &CrashRequest<'_>,
    operation: Option<&OperationHandle>,
) -> Result<CrashDiagnosis, AppError> {
    let game_dir = InstanceDirs::for_pack(&state.paths, pack_id).game_dir;
    diagnose_in(state, pack_id, request, &game_dir, operation).await
}

/// [`diagnose`] com outra pasta do jogo: a do servidor local (L-09), que tem os próprios
/// crash reports, logs e `mods/`.
pub(crate) async fn diagnose_in(
    state: &AppState,
    pack_id: PackId,
    request: &CrashRequest<'_>,
    game_dir: &Path,
    operation: Option<&OperationHandle>,
) -> Result<CrashDiagnosis, AppError> {
    let snapshot = read_snapshot(state, pack_id, operation).await?;
    let game_dir = game_dir.to_path_buf();
    let cache_dir = state.paths.cache_dir();
    let context = AnalysisContext {
        loader: snapshot.loader.map(log_loader),
        minecraft: snapshot.minecraft.clone(),
    };
    let files = SessionFiles {
        session_dir: Some(request.session_dir.to_path_buf()),
        game_dir: game_dir.clone(),
        started_at: request.started_at,
    };
    let outcome = SessionOutcome {
        exit_code: request.exit_code,
        stopped_by_user: false,
        froze: false,
    };
    let index = index_input(&snapshot);
    let (analysis, mod_paths) = tokio::task::spawn_blocking(move || {
        let mut analysis =
            warden_diagnostics::analyze_session(&files, &context, Some(&outcome))
                .map_err(|error| AppError::from_domain(&error))?;
        let mut mod_paths = HashMap::new();
        if let Some(input) = index {
            let pass = build_index(&input, &game_dir, &cache_dir);
            attribute_mixin_owners(&mut analysis, &pass.index);
            for (item, jar) in pass.index.items() {
                for indexed in &jar.jars {
                    for module in &indexed.mods {
                        mod_paths
                            .entry(module.id.clone())
                            .or_insert_with(|| item.clone());
                    }
                }
            }
        }
        Ok::<_, AppError>((analysis, mod_paths))
    })
    .await
    .map_err(|error| AppError::internal(format!("analisar o travamento: {error}")))??;

    let player = state.settings.get().player_name;
    let redactor = crate::commands::report::redactor(&player, &state.secrets);
    let mod_names = HashMap::new();
    let names = ItemNames::new(&snapshot.items, &mod_paths, &mod_names);
    Ok(CrashDiagnosis {
        session_id: request.session_id.to_owned(),
        found_cause: analysis.found_cause(),
        findings: analysis
            .findings
            .iter()
            .map(|entry| {
                report_finding(
                    &entry.finding,
                    Some(entry.pattern.clone()),
                    &names,
                    &redactor,
                )
            })
            .collect(),
        sources: analysis
            .sources
            .iter()
            .map(|source| CrashSource {
                name: redactor.redact(&source.name).text,
                kind: source.kind,
                lines: source.lines,
            })
            .collect(),
    })
}

/// "Por que travou" de uma sessão gravada: a análise do `session.json` ou, nas sessões sem
/// ela, a análise feita agora. Erros: `app.TEST_SESSION_NOT_FOUND`.
pub(crate) async fn crash_of_session(
    state: &AppState,
    pack_id: PackId,
    session_id: &str,
) -> Result<CrashDiagnosis, AppError> {
    let session = crate::test_session::session_dir(&state.paths, pack_id, session_id)?;
    let record = session.read_record().map_err(|error| AppError::from_domain(&error))?;
    if let Some(stored) = record.extra.get(SESSION_FIELD)
        && let Ok(diagnosis) = serde_json::from_value::<CrashDiagnosis>(stored.clone())
    {
        return Ok(diagnosis);
    }
    let started_ms = record
        .extra
        .get("startedAtMs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let path: PathBuf = session.path().to_path_buf();
    diagnose(
        state,
        pack_id,
        &CrashRequest {
            session_id,
            session_dir: &path,
            started_at: UNIX_EPOCH + Duration::from_millis(started_ms),
            exit_code: record.exit_code,
        },
        None,
    )
    .await
}
