//! A Anthropic, o provedor Claude (ROADMAP D-17; ADR-0060), pela API Messages oficial com a
//! chave do usuário.
//!
//! - [`client`]: o cliente REST, sem SDK (`v1/messages`, `v1/messages/count_tokens`,
//!   `v1/models`), e a leitura dos códigos de erro da API.
//! - [`models`]: o Sonnet e o Opus mais novos da chave.
//! - [`translate`]: o formato comum ↔ o corpo da API, com os blocos de raciocínio no campo
//!   opaco.

pub mod client;
pub mod models;
pub mod translate;

use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use warden_core::CancellationToken;

pub use client::{
    API_VERSION, AnthropicClient, BASE_URL_ENV, DEFAULT_BASE_URL, MAX_PER_MINUTE_WAIT, Problem,
    REQUEST_TIMEOUT, Reply, api_message, classify,
};

use crate::error::{Error, Result};
use crate::providers::{
    AiProvider, ChatRequest, ChatResponse, Failure, ModelChoices, Provider, ToolResult,
};

/// Um código de erro da API na classificação comum.
fn failure_of(reply: &Reply) -> Failure {
    match classify(reply.status, &reply.body, reply.retry_after) {
        Problem::Transient { status } => Failure::Overloaded { status },
        Problem::PerMinute { wait } => Failure::PerMinute { wait },
        // A Anthropic não tem plano gratuito pela API.
        Problem::Quota => Failure::Quota { free_tier: false },
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

fn serialize(body: &Value) -> std::result::Result<Vec<u8>, Failure> {
    serde_json::to_vec(body).map_err(|error| {
        Failure::Fatal(Error::Internal(format!(
            "corpo JSON da requisição: {error}"
        )))
    })
}

#[async_trait]
impl Provider for AnthropicClient {
    fn id(&self) -> AiProvider {
        AiProvider::Anthropic
    }

    fn timeout(&self) -> Duration {
        AnthropicClient::timeout(self)
    }

    async fn generate(
        &self,
        request: &ChatRequest,
        cancel: &CancellationToken,
    ) -> std::result::Result<ChatResponse, Failure> {
        let bytes = serialize(&translate::body(request))?;
        let sent = String::from_utf8_lossy(&bytes).into_owned();
        let reply = self
            .post("messages", bytes, Some(cancel))
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
            .post("messages/count_tokens", bytes, Some(cancel))
            .await
            .map_err(Failure::Fatal)?;
        if reply.status != 200 {
            return Err(failure_of(&reply));
        }
        reply
            .json()
            .get("input_tokens")
            .and_then(Value::as_u64)
            .map(Some)
            .ok_or_else(|| {
                Failure::Fatal(Error::BadResponse {
                    reason: "count_tokens sem input_tokens".into(),
                })
            })
    }

    async fn models(&self) -> Result<ModelChoices> {
        models::fetch_choices(self).await
    }

    fn tool_result_text(&self, result: &ToolResult) -> String {
        translate::tool_result(result).to_string()
    }
}
