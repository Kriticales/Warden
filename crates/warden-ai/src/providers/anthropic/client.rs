//! Cliente REST da Anthropic, sem SDK: `POST v1/messages`, `POST v1/messages/count_tokens` e
//! `GET v1/models` pela `warden-http`.
//!
//! - A chave vai **só** no cabeçalho `x-api-key`, marcado como sensível, nunca na URL nem em log;
//!   o corpo de um erro passa por [`without_secret`] antes de virar detalhe. Toda requisição leva
//!   o `anthropic-version` fixo da API.
//! - Sem novas tentativas aqui: cada requisição sai uma vez e volta como veio (código, corpo e
//!   a espera pedida por `retry-after`), porque quem repete, espera ou para é o laço.

use std::time::{Duration, SystemTime};

use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use warden_core::CancellationToken;
use warden_http::{HeaderName, HeaderValue, HttpClient, Url};

use crate::error::{Error, Result, without_secret};

/// Endereço oficial da API.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/";

/// Variável que troca o endereço da API, só em build de debug (servidor simulado dos E2E).
pub const BASE_URL_ENV: &str = "WARDEN_API_BASE_ANTHROPIC";

/// A versão da API (cabeçalho `anthropic-version`, obrigatório).
pub const API_VERSION: &str = "2023-06-01";

/// Tempo-limite de cada requisição. O Claude raciocina antes de responder (no Opus o
/// raciocínio não se desliga); uma rodada com muitas consultas passa de um minuto.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);

/// Maior corpo aceito numa resposta.
const MAX_BODY: u64 = 8 * 1024 * 1024;

/// Uma resposta como veio: código, corpo e a espera pedida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// Código HTTP.
    pub status: u16,
    /// Corpo (UTF-8; bytes inválidos trocados), já sem a chave.
    pub body: String,
    /// A espera de `retry-after`, se veio.
    pub retry_after: Option<Duration>,
}

impl Reply {
    /// O corpo como JSON (`Null` se não for JSON).
    #[must_use]
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

/// Cliente da Anthropic. Barato de clonar.
#[derive(Clone)]
pub struct AnthropicClient {
    http: HttpClient,
    base: Url,
    key: SecretString,
    timeout: Duration,
}

impl std::fmt::Debug for AnthropicClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Sem a chave.
        f.debug_struct("AnthropicClient")
            .field("base", &self.base.as_str())
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl AnthropicClient {
    /// Cliente com o endereço oficial (ou o de `WARDEN_API_BASE_ANTHROPIC`, em build de debug).
    pub fn new(http: HttpClient, key: SecretString) -> Result<Self> {
        let base = base_url_from_env().unwrap_or_else(|| DEFAULT_BASE_URL.to_owned());
        Self::with_base_url(http, &base, key)
    }

    /// Cliente com outro endereço (testes).
    pub fn with_base_url(http: HttpClient, base: &str, key: SecretString) -> Result<Self> {
        let mut base = warden_http::parse_url(base)?;
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        Ok(Self {
            http,
            base,
            key,
            timeout: REQUEST_TIMEOUT,
        })
    }

    /// Troca o tempo-limite por requisição (testes do tempo esgotado).
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// O tempo-limite por requisição.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    fn url(&self, path: &str) -> Result<Url> {
        self.base
            .join(path)
            .map_err(|error| Error::Internal(format!("endereço {path:?}: {error}")))
    }

    /// `POST v1/{caminho}` com o corpo JSON já serializado (o laço guarda estes bytes exatos na
    /// conversa): `messages` ou `messages/count_tokens`.
    pub async fn post(
        &self,
        path: &str,
        body: Vec<u8>,
        cancel: Option<&CancellationToken>,
    ) -> Result<Reply> {
        let url = self.url(&format!("v1/{path}"))?;
        let request = self.http.post_bytes(url, "application/json", body);
        self.send(request, cancel).await
    }

    /// `GET v1/models` (uma página).
    pub async fn list_models_page(&self, after_id: Option<&str>) -> Result<Reply> {
        let mut url = self.url("v1/models")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("limit", "1000");
            if let Some(after) = after_id {
                query.append_pair("after_id", after);
            }
        }
        self.send(self.http.get(url), None).await
    }

    async fn send(
        &self,
        request: warden_http::Request,
        cancel: Option<&CancellationToken>,
    ) -> Result<Reply> {
        let mut request = request
            .secret_header(HeaderName::from_static("x-api-key"), &self.key)?
            .header(
                HeaderName::from_static("anthropic-version"),
                HeaderValue::from_static(API_VERSION),
            )
            .total_timeout(Some(self.timeout))
            .max_body(MAX_BODY);
        if let Some(cancel) = cancel {
            request = request.cancel(cancel);
        }
        let response = request.send_unchecked().await?;
        let status = response.status().as_u16();
        let retry_after = warden_http::headers::retry_after(response.headers(), SystemTime::now());
        let bytes = response.bytes_limited(MAX_BODY).await?;
        let body = without_secret(&String::from_utf8_lossy(&bytes), self.key.expose_secret());
        Ok(Reply {
            status,
            body,
            retry_after,
        })
    }
}

fn base_url_from_env() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::env::var(BASE_URL_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// O que um código de erro da API quer dizer para o laço (erros da API Messages, 10/2026).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// 529 (`overloaded_error`), 500 (`api_error`), 502, 503 e 504: passageiro.
    Transient {
        /// O código.
        status: u16,
    },
    /// 429 (`rate_limit_error`) com `retry-after` até 60 s: esperar e repetir.
    PerMinute {
        /// O tempo pedido.
        wait: Duration,
    },
    /// 429 com espera longa: parar, sem prometer horário.
    Quota,
    /// Cobrança (`billing_error`, 402, saldo de créditos baixo).
    Billing,
    /// Chave recusada (401 `authentication_error`, 403 `permission_error`).
    KeyInvalid,
    /// Outro erro.
    Other,
}

/// Espera máxima de um 429 antes de tratar como cota (a mesma do Gemini, ADR-0055).
pub const MAX_PER_MINUTE_WAIT: Duration = Duration::from_secs(60);

/// Espera de um 429 sem `retry-after`.
const DEFAULT_WAIT: Duration = Duration::from_secs(30);

/// Lê um código de erro da API e o corpo (`retry_after`: a espera do cabeçalho).
#[must_use]
pub fn classify(status: u16, body: &str, retry_after: Option<Duration>) -> Problem {
    let json: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let kind = json
        .pointer("/error/type")
        .and_then(Value::as_str)
        .unwrap_or("");
    let message = json
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    if status == 402 || kind == "billing_error" || message.contains("credit balance") {
        return Problem::Billing;
    }
    match status {
        401 | 403 => Problem::KeyInvalid,
        429 => match retry_after {
            Some(wait) if wait > MAX_PER_MINUTE_WAIT => Problem::Quota,
            Some(wait) => Problem::PerMinute { wait },
            None => Problem::PerMinute { wait: DEFAULT_WAIT },
        },
        500 | 502 | 503 | 504 | 529 => Problem::Transient { status },
        _ => Problem::Other,
    }
}

/// A mensagem de erro da API (`error.message`), sem a chave, para o detalhe técnico.
#[must_use]
pub fn api_message(reply: &Reply) -> String {
    reply
        .json()
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map_or_else(
            || reply.body.chars().take(1024).collect(),
            ToOwned::to_owned,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn body(kind: &str, message: &str) -> String {
        json!({ "type": "error", "error": { "type": kind, "message": message } }).to_string()
    }

    #[test]
    fn sobrecarga_e_limite_por_minuto() {
        assert_eq!(
            classify(529, &body("overloaded_error", "Overloaded"), None),
            Problem::Transient { status: 529 }
        );
        assert_eq!(
            classify(500, &body("api_error", "Internal server error"), None),
            Problem::Transient { status: 500 }
        );
        let rate = body(
            "rate_limit_error",
            "Number of request tokens has exceeded your per-minute rate limit",
        );
        assert_eq!(
            classify(429, &rate, Some(Duration::from_secs(12))),
            Problem::PerMinute {
                wait: Duration::from_secs(12)
            }
        );
        assert_eq!(
            classify(429, &rate, None),
            Problem::PerMinute { wait: DEFAULT_WAIT }
        );
        assert_eq!(
            classify(429, &rate, Some(Duration::from_secs(3600))),
            Problem::Quota
        );
    }

    #[test]
    fn chave_e_cobranca() {
        assert_eq!(
            classify(
                401,
                &body("authentication_error", "invalid x-api-key"),
                None
            ),
            Problem::KeyInvalid
        );
        assert_eq!(
            classify(403, &body("permission_error", "no access"), None),
            Problem::KeyInvalid
        );
        let low = body(
            "invalid_request_error",
            "Your credit balance is too low to access the Anthropic API.",
        );
        assert_eq!(classify(400, &low, None), Problem::Billing);
        assert_eq!(classify(402, "{}", None), Problem::Billing);
        assert_eq!(
            classify(400, &body("invalid_request_error", "messages: bad"), None),
            Problem::Other
        );
    }
}
