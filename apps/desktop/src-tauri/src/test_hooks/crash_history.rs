//! A assinatura do travamento no Testar (ROADMAP D-06; ARCHITECTURE §9.6): quando o jogo
//! trava, o `after_exit` analisa os arquivos da sessão (os logs da instância ainda são deste
//! teste) e grava no `session.json` o registro do travamento (campo `crash`: origem `test` e a
//! assinatura), que a lista de Travamentos de Problemas agrupa.

use serde_json::{Map, Value};
use warden_diagnostics::postcrash::SessionFiles;
use warden_launcher::outcome::Outcome;

use super::{ExitContext, HookContext, TestHooks};
use crate::commands::crashes::{self, CrashRecord};
use crate::error::AppError;

/// O gancho da assinatura.
pub(crate) struct CrashHistoryHook;

#[async_trait::async_trait]
impl TestHooks for CrashHistoryHook {
    fn name(&self) -> &'static str {
        "crash_history"
    }

    async fn after_exit(
        &self,
        context: &HookContext<'_>,
        exit: &ExitContext<'_>,
    ) -> Result<Map<String, Value>, AppError> {
        if exit.exit.outcome != Outcome::Crashed {
            return Ok(Map::new());
        }
        let files = SessionFiles {
            session_dir: Some(exit.session_dir.to_path_buf()),
            game_dir: context.dirs.game_dir.clone(),
            started_at: crashes::started_at(exit.record),
        };
        let game = exit.record.game.clone();
        let exit_code = exit.exit.exit_code;
        let signature =
            tokio::task::spawn_blocking(move || crashes::signature_of(&files, &game, exit_code))
                .await
                .map_err(|error| {
                    AppError::internal(format!("assinatura do travamento: {error}"))
                })??;
        tracing::info!(hash = %signature.hash, rule = ?signature.rule, "assinatura do travamento");
        let record = CrashRecord {
            origin: warden_diagnostics::crash_history::CrashOrigin::Test,
            signature,
        };
        let value = serde_json::to_value(&record)
            .map_err(|error| AppError::internal(format!("gravar a assinatura: {error}")))?;
        let mut fields = Map::new();
        fields.insert(crashes::SESSION_FIELD.to_owned(), value);
        Ok(fields)
    }
}
