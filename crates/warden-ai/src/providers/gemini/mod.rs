//! O Gemini (Google), o provedor padrão (ADR-0055 e ADR-0060; ARCHITECTURE §9.5).
//!
//! - [`client`]: o cliente REST, sem SDK (`generateContent`, `countTokens`, `models.list`), e a
//!   leitura dos códigos de erro da API.
//! - [`models`]: o Flash-Lite e o Flash estáveis mais novos pelo `models.list`.
//! - [`translate`]: o formato comum ↔ o corpo da API, idêntico byte a byte ao da D-04.

pub mod client;
pub mod models;
pub mod translate;

use std::time::Duration;

use async_trait::async_trait;
use warden_core::CancellationToken;

pub use client::{
    BASE_URL_ENV, DEFAULT_BASE_URL, GeminiClient, MAX_PER_MINUTE_WAIT, Problem, REQUEST_TIMEOUT,
    Reply, api_message, billing_signal, classify,
};

use crate::error::{Error, Result};
use crate::providers::{
    AiProvider, ChatRequest, ChatResponse, Failure, ModelChoices, Provider, ToolResult,
};

/// Um código de erro da API na classificação comum.
fn failure_of(reply: &Reply) -> Failure {
    match classify(reply.status, &reply.body) {
        Problem::Transient { status } => Failure::Overloaded { status },
        Problem::PerMinute { wait } => Failure::PerMinute { wait },
        Problem::Quota { free_tier } => Failure::Quota { free_tier },
        Problem::Billing => Failure::Billing {
            detail: api_message(reply),
        },
        Problem::KeyInvalid => Failure::KeyInvalid {
            status: reply.status,
        },
        Problem::Other => Failure::Rejected {
            status: reply.status,
            detail: api_message(reply),
        },
    }
}

fn serialize(body: &serde_json::Value) -> std::result::Result<Vec<u8>, Failure> {
    serde_json::to_vec(body).map_err(|error| {
        Failure::Fatal(Error::Internal(format!(
            "corpo JSON da requisição: {error}"
        )))
    })
}

#[async_trait]
impl Provider for GeminiClient {
    fn id(&self) -> AiProvider {
        AiProvider::Gemini
    }

    fn timeout(&self) -> Duration {
        GeminiClient::timeout(self)
    }

    async fn generate(
        &self,
        request: &ChatRequest,
        cancel: &CancellationToken,
    ) -> std::result::Result<ChatResponse, Failure> {
        let bytes = serialize(&translate::body(request))?;
        let sent = String::from_utf8_lossy(&bytes).into_owned();
        let reply = self
            .post_bytes(&request.model, "generateContent", bytes, Some(cancel))
            .await
            .map_err(Failure::from_http)?;
        if reply.status != 200 {
            return Err(failure_of(&reply));
        }
        let json = reply.json();
        if json.is_null() {
            return Err(Failure::Fatal(Error::BadResponse {
                reason: "resposta 200 sem JSON".into(),
            }));
        }
        translate::response(&json, sent)
    }

    async fn count_tokens(
        &self,
        request: &ChatRequest,
        cancel: &CancellationToken,
    ) -> std::result::Result<Option<u64>, Failure> {
        let bytes = serialize(&translate::count_tokens_body(request))?;
        let reply = self
            .post_bytes(&request.model, "countTokens", bytes, Some(cancel))
            .await
            .map_err(Failure::Fatal)?;
        if reply.status != 200 {
            return Err(failure_of(&reply));
        }
        reply
            .json()
            .get("totalTokens")
            .and_then(serde_json::Value::as_u64)
            .map(Some)
            .ok_or_else(|| {
                Failure::Fatal(Error::BadResponse {
                    reason: "countTokens sem totalTokens".into(),
                })
            })
    }

    async fn models(&self) -> Result<ModelChoices> {
        models::fetch_choices(self).await
    }

    fn tool_result_text(&self, result: &ToolResult) -> String {
        translate::function_response(result).to_string()
    }
}
