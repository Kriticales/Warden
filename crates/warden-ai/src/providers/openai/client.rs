//! Cliente REST da OpenAI, sem SDK: `POST v1/responses` e `GET v1/models` pela `warden-http`.
//!
//! - A chave vai **só** no cabeçalho `Authorization: Bearer …`, marcado como sensível, nunca na
//!   URL nem em log; o corpo de um erro passa por [`without_secret`] antes de virar detalhe.
//! - Sem novas tentativas aqui: cada requisição sai uma vez e volta como veio (código, corpo e
//!   a espera pedida por `Retry-After`), porque quem repete, espera ou para é o laço.

use std::time::{Duration, SystemTime};

use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use warden_core::CancellationToken;
use warden_http::{HeaderName, HttpClient, Url};

use crate::error::{Error, Result, without_secret};

/// Endereço oficial da API.
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/";

/// Variável que troca o endereço da API, só em build de debug (servidor simulado dos E2E).
pub const BASE_URL_ENV: &str = "WARDEN_API_BASE_OPENAI";

/// Tempo-limite de cada requisição. Os modelos da OpenAI raciocinam antes de responder; uma
/// rodada com muitas consultas passa de um minuto.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);

/// Maior corpo aceito numa resposta (o raciocínio criptografado ocupa dezenas de KB).
const MAX_BODY: u64 = 8 * 1024 * 1024;

/// Uma resposta como veio: código, corpo e a espera pedida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// Código HTTP.
    pub status: u16,
    /// Corpo (UTF-8; bytes inválidos trocados), já sem a chave.
    pub body: String,
    /// A espera de `retry-after-ms` ou `Retry-After`, se veio.
    pub retry_after: Option<Duration>,
}

impl Reply {
    /// O corpo como JSON (`Null` se não for JSON).
    #[must_use]
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

/// Cliente da OpenAI. Barato de clonar.
#[derive(Clone)]
pub struct OpenAiClient {
    http: HttpClient,
    base: Url,
    key: SecretString,
    timeout: Duration,
}

impl std::fmt::Debug for OpenAiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Sem a chave.
        f.debug_struct("OpenAiClient")
            .field("base", &self.base.as_str())
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl OpenAiClient {
    /// Cliente com o endereço oficial (ou o de `WARDEN_API_BASE_OPENAI`, em build de debug).
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

    /// `POST v1/responses` com o corpo JSON já serializado (o laço guarda estes bytes exatos na
    /// conversa).
    pub async fn post_response(
        &self,
        body: Vec<u8>,
        cancel: Option<&CancellationToken>,
    ) -> Result<Reply> {
        let url = self.url("v1/responses")?;
        let request = self.http.post_bytes(url, "application/json", body);
        self.send(request, cancel).await
    }

    /// `GET v1/models` (a lista inteira: a API não pagina).
    pub async fn list_models(&self) -> Result<Reply> {
        let url = self.url("v1/models")?;
        self.send(self.http.get(url), None).await
    }

    async fn send(
        &self,
        request: warden_http::Request,
        cancel: Option<&CancellationToken>,
    ) -> Result<Reply> {
        let bearer = SecretString::from(format!("Bearer {}", self.key.expose_secret()));
        let mut request = request
            .secret_header(HeaderName::from_static("authorization"), &bearer)?
            .total_timeout(Some(self.timeout))
            .max_body(MAX_BODY);
        if let Some(cancel) = cancel {
            request = request.cancel(cancel);
        }
        let response = request.send_unchecked().await?;
        let status = response.status().as_u16();
        let retry_after = retry_after_ms(response.headers())
            .or_else(|| warden_http::headers::retry_after(response.headers(), SystemTime::now()));
        let bytes = response.bytes_limited(MAX_BODY).await?;
        let body = without_secret(&String::from_utf8_lossy(&bytes), self.key.expose_secret());
        Ok(Reply {
            status,
            body,
            retry_after,
        })
    }
}

/// `retry-after-ms`: a espera em milissegundos que a OpenAI manda junto do `Retry-After`.
fn retry_after_ms(headers: &warden_http::HeaderMap) -> Option<Duration> {
    headers
        .get("retry-after-ms")?
        .to_str()
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|ms| ms.is_finite() && *ms >= 0.0)
        .map(|ms| Duration::from_secs_f64(ms / 1000.0))
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

/// O que um código de erro da API quer dizer para o laço (guia de erros da OpenAI, 10/2026).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// 500, 502, 503 (`server_is_overloaded`) e 504: passageiro.
    Transient {
        /// O código.
        status: u16,
    },
    /// 429 de limite de requisições ou tokens: esperar o tempo pedido.
    PerMinute {
        /// O tempo pedido.
        wait: Duration,
    },
    /// 429 de cota ou de limite de gasto: repetir não adianta.
    Quota,
    /// Créditos pré-pagos acabaram.
    Billing,
    /// Chave recusada (401; 403 que não é de país).
    KeyInvalid,
    /// Outro erro.
    Other,
}

/// Espera máxima de um 429 por minuto antes de tratar como cota (a mesma do Gemini, ADR-0055).
pub const MAX_PER_MINUTE_WAIT: Duration = Duration::from_secs(60);

/// Espera de um 429 por minuto sem `Retry-After` nem tempo na mensagem.
const DEFAULT_WAIT: Duration = Duration::from_secs(30);

/// Os `error.code` de cota e de limite de gasto (429 que não passa sozinho).
const QUOTA_CODES: [&str; 4] = [
    "insufficient_quota",
    "organization_usage_limit_exceeded",
    "organization_spend_limit_exceeded",
    "project_spend_limit_exceeded",
];

/// Os `error.code` de cobrança.
const BILLING_CODES: [&str; 2] = ["credit_balance_exhausted", "billing_hard_limit_reached"];

/// Lê um código de erro da API e o corpo (`retry_after`: a espera dos cabeçalhos).
#[must_use]
pub fn classify(status: u16, body: &str, retry_after: Option<Duration>) -> Problem {
    let json: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let field = |name: &str| {
        json.pointer(&format!("/error/{name}"))
            .and_then(Value::as_str)
            .unwrap_or("")
    };
    let (code, kind) = (field("code"), field("type"));
    if BILLING_CODES.contains(&code) || status == 402 {
        return Problem::Billing;
    }
    match status {
        403 if code.contains("unsupported_country") => Problem::Other,
        401 | 403 => Problem::KeyInvalid,
        429 if QUOTA_CODES.contains(&code) || kind == "insufficient_quota" => Problem::Quota,
        429 => match retry_after.or_else(|| wait_in_message(field("message"))) {
            Some(wait) if wait > MAX_PER_MINUTE_WAIT => Problem::Quota,
            Some(wait) => Problem::PerMinute { wait },
            None => Problem::PerMinute { wait: DEFAULT_WAIT },
        },
        500 | 502 | 503 | 504 => Problem::Transient { status },
        _ => Problem::Other,
    }
}

/// "Please try again in 1.234s" / "in 20ms" (o tempo que a mensagem do 429 traz).
fn wait_in_message(message: &str) -> Option<Duration> {
    let rest = &message[message.find("try again in ")? + "try again in ".len()..];
    let number: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let value: f64 = number.parse().ok()?;
    let unit = &rest[number.len()..];
    let seconds = if unit.starts_with("ms") {
        value / 1000.0
    } else if unit.starts_with('s') {
        value
    } else {
        return None;
    };
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds.ceil()))
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

    fn body(code: &str, kind: &str, message: &str) -> String {
        json!({ "error": { "message": message, "type": kind, "param": null, "code": code } })
            .to_string()
    }

    #[test]
    fn limite_por_minuto_com_a_espera_pedida() {
        let rate = body("rate_limit_exceeded", "requests", "Rate limit reached.");
        assert_eq!(
            classify(429, &rate, Some(Duration::from_secs(7))),
            Problem::PerMinute {
                wait: Duration::from_secs(7)
            }
        );
        let in_message = body(
            "rate_limit_exceeded",
            "tokens",
            "Rate limit reached for gpt-6-luna on tokens per min (TPM). Please try again in 1.5s.",
        );
        assert_eq!(
            classify(429, &in_message, None),
            Problem::PerMinute {
                wait: Duration::from_secs(2)
            }
        );
        assert_eq!(
            wait_in_message("Please try again in 20ms."),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            classify(429, "{}", None),
            Problem::PerMinute { wait: DEFAULT_WAIT }
        );
        // Espera longa: é cota, não limite por minuto.
        assert_eq!(
            classify(429, &rate, Some(Duration::from_secs(600))),
            Problem::Quota
        );
    }

    #[test]
    fn cota_cobranca_e_chave() {
        let quota = body(
            "insufficient_quota",
            "insufficient_quota",
            "You exceeded your current quota, please check your plan and billing details.",
        );
        assert_eq!(classify(429, &quota, None), Problem::Quota);
        let spend = body("project_spend_limit_exceeded", "requests", "Limit.");
        assert_eq!(classify(429, &spend, None), Problem::Quota);
        let credit = body("credit_balance_exhausted", "requests", "No credits.");
        assert_eq!(classify(429, &credit, None), Problem::Billing);
        let key = body(
            "invalid_api_key",
            "invalid_request_error",
            "Incorrect API key provided.",
        );
        assert_eq!(classify(401, &key, None), Problem::KeyInvalid);
        let country = body(
            "unsupported_country_region_territory",
            "request_forbidden",
            "x",
        );
        assert_eq!(classify(403, &country, None), Problem::Other);
        assert_eq!(classify(403, "{}", None), Problem::KeyInvalid);
    }

    #[test]
    fn sobrecarga_passa() {
        let overloaded = body(
            "server_is_overloaded",
            "service_unavailable_error",
            "Slow down.",
        );
        assert_eq!(
            classify(503, &overloaded, None),
            Problem::Transient { status: 503 }
        );
        assert_eq!(classify(500, "", None), Problem::Transient { status: 500 });
        assert_eq!(classify(400, "{}", None), Problem::Other);
    }
}
