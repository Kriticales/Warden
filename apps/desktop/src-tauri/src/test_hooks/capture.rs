//! "O que mudou durante o teste" no Testar (SPEC T15; ROADMAP C-03; ARCHITECTURE §8.3).
//!
//! - `before_prepare`: com mudanças do teste anterior esperando revisão (configs do pack ou
//!   `options.txt`), o teste não começa: a próxima cópia do pack desfaria essas mudanças. O erro
//!   `app.TEST_CAPTURE_PENDING` leva a tela do teste a perguntar "Revisar agora", "Descartar e
//!   testar" ou "Cancelar".
//! - `before_launch`: a linha de base da instância, logo antes de abrir o jogo.
//! - `after_exit`: a comparação com a linha de base e a captura gravada (`capture` no
//!   `session.json`, com quantas mudanças e se pede revisão). Um erro aqui vira aviso do teste
//!   (a sessão é gravada do mesmo jeito).

use serde_json::{Map, Value, json};
use warden_instance::{capture_after_exit, load_capture, take_baseline};

use super::{ExitContext, HookContext, TestHooks};
use crate::error::{AppError, AppErrorCode};

/// Os ganchos da captura.
pub(crate) struct CaptureHooks;

/// Roda trabalho de disco fora do executor.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("captura interrompida: {error}")))?
}

#[async_trait::async_trait]
impl TestHooks for CaptureHooks {
    fn name(&self) -> &'static str {
        "capture"
    }

    async fn before_prepare(&self, context: &HookContext<'_>) -> Result<(), AppError> {
        let state_dir = context.dirs.state_dir.clone();
        let pending = blocking(move || {
            load_capture(&state_dir).map_err(|error| AppError::from_domain(&error))
        })
        .await?
        .filter(warden_instance::Capture::needs_review);
        match pending {
            Some(capture) => Err(AppError::new(AppErrorCode::TestCapturePending)
                .with_param("sessionId", capture.session_id)),
            None => Ok(()),
        }
    }

    async fn before_launch(&self, context: &HookContext<'_>) -> Result<(), AppError> {
        let dirs = context.dirs.clone();
        let pack_root = context.pack_root.to_path_buf();
        blocking(move || {
            take_baseline(&dirs, &pack_root)
                .map(|_| ())
                .map_err(|error| AppError::from_domain(&error))
        })
        .await
    }

    async fn after_exit(
        &self,
        context: &HookContext<'_>,
        exit: &ExitContext<'_>,
    ) -> Result<Map<String, Value>, AppError> {
        let dirs = context.dirs.clone();
        let pack_root = context.pack_root.to_path_buf();
        let session_id = exit
            .session_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        let outcome = blocking(move || {
            capture_after_exit(&dirs, &pack_root, &session_id)
                .map_err(|error| AppError::from_domain(&error))
        })
        .await?;
        let mut fields = Map::new();
        if let Some(outcome) = outcome {
            tracing::info!(
                mudancas = outcome.capture.files.len(),
                reformatados = outcome.capture.reformatted,
                devolvidos = outcome.restored.len(),
                "captura do teste gravada"
            );
            fields.insert(
                "capture".into(),
                json!({
                    "changes": outcome.capture.files.len(),
                    "needsReview": outcome.capture.needs_review(),
                }),
            );
        }
        Ok(fields)
    }
}
