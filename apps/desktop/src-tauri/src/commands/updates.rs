//! Comandos do domínio `updates` (ARCHITECTURE §4.1; SPEC T10; P1-12).
//!
//! - `updates_report`: o último relatório do pack (só memória, sem rede).
//! - `updates_check`: verifica as atualizações do pack inteiro. `auto` respeita o intervalo
//!   das Configurações (e "só quando eu pedir"); `manual` sempre consulta e aparece em Tarefas.
//! - `updates_plan`: a revisão (versões, novidades, dependências novas, incompatibilidades).
//! - `updates_apply`: grava as novas referências numa transação; com mais de um item, cria
//!   antes um ponto de segurança.
//!
//! O relatório fica só em memória: a CurseForge não permite guardar as respostas (SPEC T10) e o
//! do Modrinth é refeito em duas requisições. Finos: validam, pegam a trava do pack e chamam a
//! `warden-project`.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tauri::{AppHandle, State};
use warden_core::{CancellationToken, PackId};
use warden_project::ProjectErrorCode;
use warden_project::updates::{
    AppliedUpdates, ModUpdateReport, UpdateContext, UpdatePlan, UpdateSelection,
};

use super::inventory::{curseforge, notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Verificar atualizações" pedido pelo usuário.
pub(crate) const CHECK: OperationKind = OperationKind::new("updates.check");
/// "Atualizar" (um ou vários itens).
pub(crate) const APPLY: OperationKind = OperationKind::new("updates.apply");

#[derive(Debug)]
struct Cached {
    report: ModUpdateReport,
    at: Instant,
}

/// Último relatório de cada pack, só em memória.
#[derive(Debug, Default)]
pub struct UpdateReports {
    reports: Mutex<HashMap<PackId, Cached>>,
}

impl UpdateReports {
    fn get(&self, pack: PackId) -> Option<ModUpdateReport> {
        let reports = self.reports.lock().unwrap_or_else(PoisonError::into_inner);
        reports.get(&pack).map(|cached| cached.report.clone())
    }

    fn age(&self, pack: PackId) -> Option<Duration> {
        let reports = self.reports.lock().unwrap_or_else(PoisonError::into_inner);
        reports.get(&pack).map(|cached| cached.at.elapsed())
    }

    fn put(&self, pack: PackId, report: ModUpdateReport) {
        let mut reports = self.reports.lock().unwrap_or_else(PoisonError::into_inner);
        reports.insert(
            pack,
            Cached {
                report,
                at: Instant::now(),
            },
        );
    }

    fn update(&self, pack: PackId, change: impl FnOnce(&mut ModUpdateReport)) {
        let mut reports = self.reports.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = reports.get_mut(&pack) {
            change(&mut cached.report);
        }
    }
}

/// Quem pediu a verificação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CheckTrigger {
    /// Ao abrir o pack: respeita o intervalo das Configurações.
    Auto,
    /// Botão "Verificar atualizações": sempre consulta.
    Manual,
}

/// Se a verificação automática deve consultar agora: intervalo 0 = só quando o usuário pedir;
/// senão, quando não há relatório ou ele é mais velho que o intervalo.
fn auto_check_due(interval_hours: u32, age: Option<Duration>) -> bool {
    interval_hours != 0
        && age.is_none_or(|age| age >= Duration::from_secs(u64::from(interval_hours) * 3600))
}

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// O último relatório do pack, ou `null` se ainda não houve verificação neste uso do Warden.
#[tauri::command]
#[specta::specta]
pub(crate) fn updates_report(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Option<ModUpdateReport>, AppError> {
    // O pack precisa existir; o relatório em si é só da memória.
    state.packs.get(pack_id).map_err(domain)?;
    Ok(state.update_reports.get(pack_id))
}

/// Verifica as atualizações do pack. No modo `auto`, devolve o relatório guardado se ele é
/// mais novo que o intervalo das Configurações (0 = só quando o usuário pedir).
#[tauri::command]
#[specta::specta]
pub(crate) async fn updates_check(
    state: State<'_, AppState>,
    pack_id: PackId,
    trigger: CheckTrigger,
) -> Result<Option<ModUpdateReport>, AppError> {
    let settings = state.settings.get();
    if trigger == CheckTrigger::Auto
        && !auto_check_due(
            settings.update_check_interval_hours,
            state.update_reports.age(pack_id),
        )
    {
        return Ok(state.update_reports.get(pack_id));
    }
    let curseforge = curseforge(&state)?;
    let handle = (trigger == CheckTrigger::Manual)
        .then(|| state.operations.start(CHECK, Some(pack_id), true));
    let result = async {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        let token = handle
            .as_ref()
            .map_or_else(CancellationToken::new, |handle| handle.token().clone());
        let context = UpdateContext {
            modrinth: &state.modrinth,
            curseforge: Some(&curseforge),
            prereleases: settings.show_prerelease_versions,
        };
        warden_project::updates::check(&root, &context, Some(&token))
            .await
            .map_err(domain)
    }
    .await;
    let result = match handle {
        Some(handle) => handle.finish(result),
        None => result,
    };
    let report = result?;
    state.update_reports.put(pack_id, report.clone());
    Ok(Some(report))
}

/// A revisão de um ou mais itens com atualização disponível.
#[tauri::command]
#[specta::specta]
pub(crate) async fn updates_plan(
    state: State<'_, AppState>,
    pack_id: PackId,
    paths: Vec<String>,
) -> Result<UpdatePlan, AppError> {
    let report = report_for(&state, pack_id)?;
    let curseforge = curseforge(&state)?;
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let context = UpdateContext {
        modrinth: &state.modrinth,
        curseforge: Some(&curseforge),
        prereleases: state.settings.get().show_prerelease_versions,
    };
    warden_project::updates::plan(&root, &report, &paths, &context, None)
        .await
        .map_err(domain)
}

/// Atualiza os itens escolhidos para as versões que a revisão mostrou. Com mais de um item,
/// cria antes um ponto de segurança. Devolve o que foi atualizado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn updates_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    selections: Vec<UpdateSelection>,
) -> Result<AppliedUpdates, AppError> {
    if selections.is_empty() {
        return Err(AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "selections"));
    }
    let report = report_for(&state, pack_id)?;
    let cli = packwiz(&state)?;
    let curseforge = curseforge(&state)?;
    let handle = state.operations.start(APPLY, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(&state, pack_id)?;
        let author = (selections.len() > 1)
            .then(|| {
                warden_packwiz::read_pack(&root)
                    .map(|pack| pack.pack.value.author)
                    .map_err(|e| AppError::from_domain(&e))
            })
            .transpose()?;
        let context = UpdateContext {
            modrinth: &state.modrinth,
            curseforge: Some(&curseforge),
            prereleases: state.settings.get().show_prerelease_versions,
        };
        warden_project::updates::apply(
            &root,
            &report,
            &selections,
            &context,
            author.as_deref(),
            &cli,
            handle.token(),
        )
        .await
        .map_err(domain)
    }
    .await;
    let applied = handle.finish(result)?;
    state
        .update_reports
        .update(pack_id, |report| report.mark_applied(&applied.updated));
    notify_changed(&app, pack_id, vec![PackArea::Inventory]);
    Ok(applied)
}

fn report_for(state: &AppState, pack_id: PackId) -> Result<ModUpdateReport, AppError> {
    state.update_reports.get(pack_id).ok_or_else(|| {
        // Sem verificação feita, não há o que revisar: a interface verifica primeiro.
        AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "report")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> ModUpdateReport {
        ModUpdateReport {
            checked_at: "2026-10-06T17:01:00Z".into(),
            items: Vec::new(),
            available: 0,
        }
    }

    #[test]
    fn verificacao_automatica_respeita_o_intervalo_das_configuracoes() {
        let hour = Duration::from_secs(3600);
        // Nunca verificado: consulta, a não ser que o usuário tenha desligado.
        assert!(auto_check_due(24, None));
        assert!(!auto_check_due(0, None));
        // Relatório novo: não consulta de novo.
        assert!(!auto_check_due(24, Some(hour)));
        assert!(!auto_check_due(24, Some(hour * 23)));
        // Passou do intervalo.
        assert!(auto_check_due(24, Some(hour * 24)));
        assert!(auto_check_due(1, Some(hour * 2)));
        // Desligada: nem velho consulta.
        assert!(!auto_check_due(0, Some(hour * 1000)));
    }

    #[test]
    fn relatorios_ficam_por_pack_e_aceitam_ajuste() {
        let reports = UpdateReports::default();
        let (a, b) = (PackId::new(), PackId::new());
        assert!(reports.get(a).is_none());
        assert!(reports.age(a).is_none());
        reports.put(a, report());
        assert!(reports.get(a).is_some());
        assert!(reports.get(b).is_none());
        assert!(
            reports
                .age(a)
                .is_some_and(|age| age < Duration::from_secs(5))
        );
        reports.update(a, |report| report.available = 3);
        assert_eq!(reports.get(a).map(|r| r.available), Some(3));
        // Ajustar um pack sem relatório não cria nada.
        reports.update(b, |report| report.available = 9);
        assert!(reports.get(b).is_none());
    }
}
