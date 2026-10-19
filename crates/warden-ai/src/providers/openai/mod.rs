//! A OpenAI (ROADMAP D-16; ADR-0060), pela Responses API oficial com a chave do usuário.
//!
//! - [`client`]: o cliente REST, sem SDK (`v1/responses`, `v1/models`), e a leitura dos códigos
//!   de erro da API.
//! - [`models`]: o `gpt-N-luna` e o `gpt-N-sol` da maior geração inteira com os dois.
//! - [`translate`]: o formato comum ↔ o corpo da API, com os itens de raciocínio no campo opaco.

pub mod client;
pub mod models;
pub mod translate;

use std::time::Duration;

use async_trait::async_trait;
use warden_core::CancellationToken;

pub use client::{
    BASE_URL_ENV, DEFAULT_BASE_URL, MAX_PER_MINUTE_WAIT, OpenAiClient, Problem, REQUEST_TIMEOUT,
    Reply, api_message, classify,
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
        // A OpenAI não tem plano gratuito pela API.
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

#[async_trait]
impl Provider for OpenAiClient {
    fn id(&self) -> AiProvider {
        AiProvider::Openai
    }

    fn timeout(&self) -> Duration {
        OpenAiClient::timeout(self)
    }

    async fn generate(
        &self,
        request: &ChatRequest,
        cancel: &CancellationToken,
    ) -> std::result::Result<ChatResponse, Failure> {
        let bytes = serde_json::to_vec(&translate::body(request)).map_err(|error| {
            Failure::Fatal(Error::Internal(format!(
                "corpo JSON da requisição: {error}"
            )))
        })?;
        let sent = String::from_utf8_lossy(&bytes).into_owned();
        let reply = self
            .post_response(bytes, Some(cancel))
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

    async fn models(&self) -> Result<ModelChoices> {
        models::fetch_choices(self).await
    }

    fn tool_result_text(&self, result: &ToolResult) -> String {
        translate::function_call_output(result).to_string()
    }
}
