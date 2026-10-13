//! O diagnóstico no Testar (SPEC T13 passos 1, 4 e 7; ROADMAP D-03):
//!
//! - `before_prepare` ("Verificar o pack"): passagem rápida, com os dados do pack e das APIs;
//! - `before_launch` ("Verificação final"): passagem completa, com os jars já na instância;
//! - `after_exit`: se o jogo travou, "Por que travou" (análise pós-crash), gravado no
//!   `session.json` antes que o próximo teste reescreva os logs da instância.
//!
//! Erros do pré-teste que valem (não ignorados) interrompem o teste com
//! `app.TEST_PACK_HAS_ERRORS`; os achados ficam no último relatório, que o diálogo da interface
//! lê. Com "Testar mesmo assim" (`ignore_pack_errors`), o teste segue. Só avisos: segue, e os
//! avisos aparecem em Problemas.

use std::time::{Duration, SystemTime};

use serde_json::{Map, Value};
use warden_core::ProgressSink as _;
use warden_launcher::outcome::Outcome;

use super::{ExitContext, HookContext, TestHooks};
use crate::commands::diagnostics::crash::{self, CrashRequest};
use crate::commands::diagnostics::report::{CheckPass, DiagnosticsReport};
use crate::error::{AppError, AppErrorCode};

/// Etapa "Verificar o pack".
pub(crate) const STAGE_CHECK: &str = "test.check";
/// Etapa "Verificação final".
pub(crate) const STAGE_FINAL_CHECK: &str = "test.finalCheck";

/// Folga para o início do processo: a sessão começa um pouco antes de o jogo escrever.
const START_SLACK: Duration = Duration::from_secs(5);

/// Os ganchos do diagnóstico.
pub(crate) struct DiagnosticsHooks;

/// O erro que interrompe o teste, ou nada.
fn blocking(report: &DiagnosticsReport, step: &str, ignore: bool) -> Result<(), AppError> {
    let errors = report.blocking_errors();
    if errors == 0 || ignore {
        if errors > 0 {
            tracing::info!(errors, step, "testando mesmo assim, apesar dos erros");
        }
        return Ok(());
    }
    Err(AppError::new(AppErrorCode::TestPackHasErrors)
        .with_param("count", errors.to_string())
        .with_param("step", step))
}

impl DiagnosticsHooks {
    async fn pass(
        context: &HookContext<'_>,
        pass: CheckPass,
        stage: &'static str,
        label: &'static str,
        step: &str,
    ) -> Result<(), AppError> {
        context.operation.stage(stage, label);
        let report = context
            .operation
            .run_cancellable(crate::commands::diagnostics::check(
                context.state,
                context.pack_id,
                pass,
                Some(context.operation),
            ))
            .await?;
        blocking(&report, step, context.ignore_pack_errors)
    }
}

#[async_trait::async_trait]
impl TestHooks for DiagnosticsHooks {
    fn name(&self) -> &'static str {
        "diagnostics"
    }

    async fn before_prepare(&self, context: &HookContext<'_>) -> Result<(), AppError> {
        Self::pass(
            context,
            CheckPass::Quick,
            STAGE_CHECK,
            "diagnostico:etapa.verificar",
            "check",
        )
        .await
    }

    async fn before_launch(&self, context: &HookContext<'_>) -> Result<(), AppError> {
        Self::pass(
            context,
            CheckPass::Complete,
            STAGE_FINAL_CHECK,
            "diagnostico:etapa.final",
            "final",
        )
        .await
    }

    async fn after_exit(
        &self,
        context: &HookContext<'_>,
        exit: &ExitContext<'_>,
    ) -> Result<Map<String, Value>, AppError> {
        if exit.exit.outcome != Outcome::Crashed {
            return Ok(Map::new());
        }
        let session_id = exit
            .session_dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let started_at = SystemTime::now()
            .checked_sub(Duration::from_millis(exit.exit.duration_ms) + START_SLACK)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let diagnosis = crash::diagnose(
            context.state,
            context.pack_id,
            &CrashRequest {
                session_id: &session_id,
                session_dir: exit.session_dir,
                started_at,
                exit_code: exit.exit.exit_code,
            },
            Some(context.operation),
        )
        .await?;
        let value = serde_json::to_value(&diagnosis).map_err(|error| {
            AppError::internal(format!("gravar a análise do travamento: {error}"))
        })?;
        let mut fields = Map::new();
        fields.insert(crash::SESSION_FIELD.to_owned(), value);
        Ok(fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::diagnostics::report::CheckPass;

    fn report(errors: usize, ignored: usize) -> DiagnosticsReport {
        use warden_diagnostics::{Evidence, Finding, RuleCode, Severity};
        let finding =
            |n: usize, ignored: bool| crate::commands::diagnostics::report::ReportFinding {
                id: format!("E_MISSING_DEP|mods/{n}.pw.toml"),
                finding: Finding::new(
                    RuleCode::parse("E_MISSING_DEP").unwrap(),
                    Severity::Error,
                    "diagnostics.pretest.e_missing_dep",
                    Evidence::PackFile {
                        path: "pack.toml".into(),
                        excerpt: "x".into(),
                    },
                ),
                items: Vec::new(),
                pattern: None,
                labels: std::collections::BTreeMap::new(),
                ignored,
            };
        DiagnosticsReport {
            pass: CheckPass::Quick,
            checked_at_ms: 0,
            item_count: 0,
            checked_mods: true,
            findings: (0..errors)
                .map(|n| finding(n, false))
                .chain((0..ignored).map(|n| finding(100 + n, true)))
                .collect(),
            pack_hash: None,
            stale: false,
        }
    }

    #[test]
    fn erros_que_valem_interrompem_o_teste() {
        let error = blocking(&report(2, 1), "check", false).unwrap_err();
        assert_eq!(
            error.code,
            crate::error::ErrorCode::App(AppErrorCode::TestPackHasErrors)
        );
        assert_eq!(error.params["count"], "2");
        assert_eq!(error.params["step"], "check");
    }

    #[test]
    fn testar_mesmo_assim_e_ignorados_nao_interrompem() {
        assert!(blocking(&report(2, 0), "final", true).is_ok());
        assert!(blocking(&report(0, 3), "check", false).is_ok());
    }
}
