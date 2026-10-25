//! Configs pessoais no Testar (ADR-0067; SPEC T12 e T15): o `options.txt` e parentes ficam nos
//! dados do Warden e voltam para a instância em todo teste.
//!
//! - `before_launch`: copia as cópias guardadas para a instância, antes da linha de base da
//!   captura (por isso este gancho vem antes dela em [`super::registered`]): a cópia devolvida
//!   não aparece como mudança do teste.
//! - `after_exit`: guarda o que o jogo deixou nesses arquivos (a pessoa mudou o vídeo ou as
//!   teclas durante o teste). Erro aqui vira aviso do teste.
//!
//! O teste como servidor não tem preferências de quem joga: os dois momentos não fazem nada.

use serde_json::{Map, Value, json};

use super::{ExitContext, HookContext, TestHooks};
use crate::commands::configs_places::personal_of;
use crate::error::AppError;
use crate::test_session::TestMode;

/// Os ganchos das configs pessoais.
pub(crate) struct PersonalHooks;

/// Roda trabalho de disco fora do executor.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("configs pessoais: {error}")))?
}

#[async_trait::async_trait]
impl TestHooks for PersonalHooks {
    fn name(&self) -> &'static str {
        "personal"
    }

    async fn before_launch(&self, context: &HookContext<'_>) -> Result<(), AppError> {
        if context.mode == TestMode::Server {
            return Ok(());
        }
        let personal = personal_of(context.state, context.pack_id)?;
        let game = context.dirs.game_dir.clone();
        let restored = blocking(move || {
            personal
                .restore_into(&game)
                .map_err(|error| AppError::from_domain(&error))
        })
        .await?;
        if !restored.is_empty() {
            tracing::info!(
                arquivos = restored.len(),
                "configs pessoais devolvidas ao teste"
            );
        }
        Ok(())
    }

    async fn after_exit(
        &self,
        context: &HookContext<'_>,
        _exit: &ExitContext<'_>,
    ) -> Result<Map<String, Value>, AppError> {
        let mut fields = Map::new();
        if context.mode == TestMode::Server {
            return Ok(fields);
        }
        let personal = personal_of(context.state, context.pack_id)?;
        let game = context.dirs.game_dir.clone();
        let saved = blocking(move || {
            personal
                .save_from(&game)
                .map_err(|error| AppError::from_domain(&error))
        })
        .await?;
        if !saved.is_empty() {
            fields.insert("personalSaved".into(), json!(saved));
        }
        Ok(fields)
    }
}
