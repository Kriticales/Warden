//! Comandos do domínio `diagnostics` (SPEC T14 sem IA; ARCHITECTURE §9; ROADMAP D-03).
//!
//! - `diagnostics_run`: "Verificar agora" da seção Problemas (passagem completa com os jars que
//!   já existem: os arquivos guardados no pack e os da instância de teste com o mesmo nome).
//! - `diagnostics_report_get`: o último relatório, com os ignorados e a marca "ficou velho"
//!   (o pack mudou depois dele).
//! - `diagnostics_ignore`: "Ignorar este aviso neste pack" (grava em `.warden/project.toml`).
//! - `diagnostics_crash_get`: "Por que travou" de uma sessão do Testar.
//! - `diagnostics_dependency_find`: a correção "Adicionar dependência": o projeto da fonte
//!   para o mod que falta, que a interface leva ao diálogo de dependências de sempre.
//! - `diagnostics_redact`: "Copiar relatório" passa o texto pela redação antes de copiar.
//!
//! Finos: as regras são da `warden-diagnostics` (D-01, D-02, D-05); aqui ficam a montagem da
//! entrada ([`input`]), o relatório ([`report`]) e a análise das sessões ([`crash`]). As
//! passagens rápida e completa do Testar usam o mesmo [`check`] pelo gancho
//! `test_hooks/diagnostics.rs`.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

mod config_range;
pub(crate) mod crash;
pub(crate) mod input;
pub(crate) mod report;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, State};
use warden_core::PackId;
use warden_diagnostics::pretest::{Pass, build_index};
use warden_instance::InstanceDirs;
use warden_project::search::SourceId;

use self::crash::CrashDiagnosis;
use self::input::{build_input, read_snapshot};
use self::report::{
    CheckPass, DiagnosticsReport, ItemNames, apply_ignored, load_report, read_ignored,
    modrinth_dependency_ids, report_finding, sort_findings, store_report, write_ignored,
};
use crate::commands::inventory::{notify_changed, pack_root};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationHandle;
use crate::state::AppState;

/// "Verificar agora": confere o pack e grava o relatório.
#[tauri::command]
#[specta::specta]
pub(crate) async fn diagnostics_run(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<DiagnosticsReport, AppError> {
    check(&state, pack_id, CheckPass::Complete, None).await
}

/// O último relatório do pack (`None`: nunca verificado neste computador).
#[tauri::command]
#[specta::specta]
pub(crate) async fn diagnostics_report_get(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Option<DiagnosticsReport>, AppError> {
    last_report(&state, pack_id)
}

/// "Ignorar este aviso neste pack" (ou desfazer). Devolve o relatório atualizado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn diagnostics_ignore(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    finding_id: String,
    ignored: bool,
) -> Result<Option<DiagnosticsReport>, AppError> {
    if finding_id.trim().is_empty() || finding_id.len() > 2048 {
        return Err(AppError::new(warden_project::ProjectErrorCode::InvalidInput)
            .with_param("field", "findingId"));
    }
    let changed = {
        let _lock = state.locks.write(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        write_ignored(&root, &finding_id, ignored)?
    };
    if changed {
        notify_changed(&app, pack_id, vec![PackArea::Meta]);
    }
    last_report(&state, pack_id)
}

/// "Por que travou" de uma sessão do Testar. Erros: `app.TEST_SESSION_NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn diagnostics_crash_get(
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
) -> Result<CrashDiagnosis, AppError> {
    crash::crash_of_session(&state, pack_id, &session_id).await
}

/// O projeto que resolve uma dependência que falta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DependencyCandidate {
    /// A fonte.
    pub(crate) source: SourceId,
    /// ID do projeto na fonte (o `AddChoice` do diálogo de dependências).
    pub(crate) project_id: String,
    /// Nome do projeto.
    pub(crate) name: String,
}

/// O projeto do mod que falta (`modId`: id do mod no jar ou id do projeto na fonte que
/// declarou a dependência). `None`: a fonte não conhece o mod (a interface leva à busca).
#[tauri::command]
#[specta::specta]
pub(crate) async fn diagnostics_dependency_find(
    state: State<'_, AppState>,
    pack_id: PackId,
    mod_id: String,
    source: Option<SourceId>,
) -> Result<Option<DependencyCandidate>, AppError> {
    pack_root(&state, pack_id)?;
    Ok(find_dependency(&state, &mod_id, source).await)
}

/// "Copiar relatório": o texto sem dados pessoais (nome de usuário do computador, jogador,
/// IPs, e-mails, chaves).
#[tauri::command]
#[specta::specta]
pub(crate) async fn diagnostics_redact(
    state: State<'_, AppState>,
    pack_id: PackId,
    text: String,
) -> Result<String, AppError> {
    pack_root(&state, pack_id)?;
    let player = state.settings.get().player_name;
    Ok(crate::commands::report::redactor(&player, &state.secrets)
        .redact(&text)
        .text)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Confere o pack numa passagem e grava o relatório como o último do pack.
pub(crate) async fn check(
    state: &AppState,
    pack_id: PackId,
    pass: CheckPass,
    operation: Option<&OperationHandle>,
) -> Result<DiagnosticsReport, AppError> {
    let snapshot = read_snapshot(state, pack_id, operation).await?;
    let built = build_input(state, pack_id, &snapshot, pass.domain()).await?;
    let item_count = snapshot
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.kind,
                warden_project::inventory::ItemKind::Mod
                    | warden_project::inventory::ItemKind::ResourcePack
                    | warden_project::inventory::ItemKind::Shader
            )
        })
        .count();
    let player = state.settings.get().player_name;
    let redactor = crate::commands::report::redactor(&player, &state.secrets);

    let mut pass = pass;
    let (findings, checked_mods) = match built {
        None => (Vec::new(), false),
        Some(built) => {
            pass = match built.pass() {
                Pass::Quick => CheckPass::Quick,
                Pass::Complete => CheckPass::Complete,
            };
            let game_dir = InstanceDirs::for_pack(&state.paths, pack_id).game_dir;
            let cache_dir = state.paths.cache_dir();
            let input = built.input;
            let root = snapshot.root.clone();
            // Raio-x de mixins (D-10): "alteram o mesmo ponto do jogo", só aviso.
            let xray = (input.pass == Pass::Complete).then(|| {
                crate::commands::xray::PretestXray::new(state, pack_id, &snapshot.items, &snapshot.root)
            });
            let findings = tokio::task::spawn_blocking(move || {
                let mut findings = warden_diagnostics::pretest::analyze(&input)
                    .map_err(|error| AppError::from_domain(&error))?;
                // Configs com valor fora da faixa que o próprio arquivo declara (C-06).
                findings.extend(config_range::config_range_findings(&root));
                if let Some(xray) = xray {
                    findings.extend(xray.findings(input.pack.loader)?);
                }
                // Passagem completa com a instância pronta: monta (ou confere no cache) o índice
                // de pacotes e configs de mixin, usado depois pela análise de um travamento.
                if input.pass == Pass::Complete && game_dir.is_dir() {
                    let index = build_index(&input, &game_dir, &cache_dir);
                    tracing::debug!(stats = ?index.stats, "índice dos jars da instância");
                }
                Ok::<_, AppError>(findings)
            })
            .await
            .map_err(|error| AppError::internal(format!("conferir o pack: {error}")))??;
            let project_names = dependency_names(state, &findings).await;
            let names = ItemNames::new(&snapshot.items, &built.mod_paths, &built.mod_names)
                .with_project_names(project_names);
            let findings = findings
                .iter()
                .map(|finding| report_finding(finding, None, &names, &redactor))
                .collect();
            (findings, true)
        }
    };

    let mut report = DiagnosticsReport {
        pass,
        checked_at_ms: now_ms(),
        item_count: u32::try_from(item_count).unwrap_or(u32::MAX),
        checked_mods,
        findings,
        pack_hash: snapshot.pack_hash.clone(),
        stale: false,
    };
    sort_findings(&mut report.findings);
    apply_ignored(&mut report.findings, &read_ignored(&snapshot.root));
    store_report(state.paths.data_dir(), pack_id, &report);
    tracing::info!(
        pack = %pack_id,
        pass = ?pass,
        findings = report.findings.len(),
        errors = report.blocking_errors(),
        "pack conferido"
    );
    Ok(report)
}

/// Nomes das dependências que faltam, declaradas pelo Modrinth (do cache, quando houver).
async fn dependency_names(
    state: &AppState,
    findings: &[warden_diagnostics::Finding],
) -> std::collections::HashMap<String, String> {
    let ids = modrinth_dependency_ids(findings);
    if ids.is_empty() {
        return std::collections::HashMap::new();
    }
    match state.modrinth.projects(&ids, None).await {
        Ok(projects) => projects
            .into_iter()
            .map(|project| (project.id, project.title))
            .collect(),
        Err(error) => {
            tracing::info!(%error, "nomes das dependências indisponíveis");
            std::collections::HashMap::new()
        }
    }
}

/// O último relatório, com os ignorados de agora e a marca de velho.
fn last_report(state: &AppState, pack_id: PackId) -> Result<Option<DiagnosticsReport>, AppError> {
    let root = pack_root(state, pack_id)?;
    let Some(mut report) = load_report(state.paths.data_dir(), pack_id) else {
        return Ok(None);
    };
    apply_ignored(&mut report.findings, &read_ignored(&root));
    report.stale = report.pack_hash.is_none() || report.pack_hash != current_pack_hash(&root);
    Ok(Some(report))
}

/// O resumo do pack agora (o mesmo do Testar: `pack.toml` e o índice).
fn current_pack_hash(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(warden_packwiz::PACK_FILE)).ok()?;
    let manifest = warden_packwiz::PackManifest::parse(&text).ok()?.value;
    crate::test_session::pack_tree_hash(root, &manifest.index.file).ok()
}

/// O projeto da fonte para um mod que falta.
async fn find_dependency(
    state: &AppState,
    mod_id: &str,
    source: Option<SourceId>,
) -> Option<DependencyCandidate> {
    let mod_id = mod_id.trim();
    if mod_id.is_empty() {
        return None;
    }
    if source == Some(SourceId::Curseforge) {
        let id: u64 = mod_id.parse().ok()?;
        let client = crate::commands::inventory::curseforge(state).ok()?;
        return match client.project(id, None).await {
            Ok(project) => Some(DependencyCandidate {
                source: SourceId::Curseforge,
                project_id: project.id.to_string(),
                name: project.name,
            }),
            Err(error) => {
                tracing::info!(%error, "dependência não encontrada na CurseForge");
                None
            }
        };
    }
    // O id do mod no jar costuma ser o slug do Modrinth (às vezes com `_` no lugar de `-`).
    let mut tries = vec![mod_id.to_owned()];
    if mod_id.contains('_') {
        tries.push(mod_id.replace('_', "-"));
    }
    for id in tries {
        match state.modrinth.project(&id, None).await {
            Ok(project) if project.project_type == warden_modrinth::ProjectType::Mod => {
                return Some(DependencyCandidate {
                    source: SourceId::Modrinth,
                    project_id: project.id,
                    name: project.title,
                });
            }
            Ok(_) => {}
            Err(error) => tracing::info!(%error, id, "dependência não encontrada no Modrinth"),
        }
    }
    None
}

#[cfg(test)]
mod tests;
