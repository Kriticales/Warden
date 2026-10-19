//! Cliente REST do Gemini, sem SDK (ARCHITECTURE §9.5 "API"): `generateContent`,
//! `countTokens` e `models.list` pela `warden-http`.
//!
//! - A chave vai **só** no cabeçalho `x-goog-api-key`, nunca na URL; o corpo de um erro passa por
//!   [`without_secret`] antes de virar detalhe.
//! - Sem novas tentativas aqui: cada requisição sai uma vez ([`warden_http::Request::send_unchecked`])
//!   e volta como veio, com o código e o corpo, porque quem decide repetir, esperar ou parar é o
//!   laço ([`crate::engine`]), que mostra as esperas na tela.
//! - Tempo-limite de 120 s por requisição (D-15b; ADR-0060, complemento de 19/10/2026).

use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use warden_core::CancellationToken;
use warden_http::{HeaderName, HttpClient, Url};

use crate::error::{Error, Result, without_secret};

/// Endereço oficial da API.
pub const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/";

/// Variável que troca o endereço da API, só em build de debug (servidor simulado dos E2E).
pub const BASE_URL_ENV: &str = "WARDEN_API_BASE_GEMINI";

/// Tempo-limite de cada requisição. O Flash pensa no nível médio (o padrão dele) e uma resposta
/// real já levou mais de 60 s (S-R5-4 §13): 120 s cabem nela com folga. Estourando, o laço troca
/// para o modelo mais leve naquela pergunta ([`crate::engine`]; D-15b).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Maior corpo aceito numa resposta (uma resposta real tem dezenas de KB).
const MAX_BODY: u64 = 8 * 1024 * 1024;

/// O cabeçalho da chave.
const KEY_HEADER: &str = "x-goog-api-key";

/// Uma resposta como veio: código e corpo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// Código HTTP.
    pub status: u16,
    /// Corpo (UTF-8; bytes inválidos trocados), já sem a chave.
    pub body: String,
}

impl Reply {
    /// O corpo como JSON (`Null` se não for JSON).
    #[must_use]
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

/// Cliente do Gemini. Barato de clonar.
#[derive(Clone)]
pub struct GeminiClient {
    http: HttpClient,
    base: Url,
    key: SecretString,
    timeout: Duration,
}

impl std::fmt::Debug for GeminiClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Sem a chave.
        f.debug_struct("GeminiClient")
            .field("base", &self.base.as_str())
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl GeminiClient {
    /// Cliente com o endereço oficial (ou o de `WARDEN_API_BASE_GEMINI`, em build de debug).
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

    /// `POST v1beta/models/{modelo}:{método}` com o corpo dado.
    pub async fn post(
        &self,
        model: &str,
        method: &str,
        body: &Value,
        cancel: Option<&CancellationToken>,
    ) -> Result<Reply> {
        let bytes = serde_json::to_vec(body)
            .map_err(|error| Error::Internal(format!("corpo JSON da requisição: {error}")))?;
        self.post_bytes(model, method, bytes, cancel).await
    }

    /// `POST v1beta/models/{modelo}:{método}` com o corpo JSON já serializado (o laço guarda
    /// estes bytes exatos na conversa).
    pub async fn post_bytes(
        &self,
        model: &str,
        method: &str,
        body: Vec<u8>,
        cancel: Option<&CancellationToken>,
    ) -> Result<Reply> {
        let url = self.url(&format!("v1beta/models/{model}:{method}"))?;
        let request = self.http.post_bytes(url, "application/json", body);
        self.send(request, cancel).await
    }

    /// `GET v1beta/models` (uma página).
    pub async fn list_models_page(&self, page_token: Option<&str>) -> Result<Reply> {
        let mut url = self.url("v1beta/models")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("pageSize", "1000");
            if let Some(token) = page_token {
                query.append_pair("pageToken", token);
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
            .secret_header(HeaderName::from_static(KEY_HEADER), &self.key)?
            .total_timeout(Some(self.timeout))
            .max_body(MAX_BODY);
        if let Some(cancel) = cancel {
            request = request.cancel(cancel);
        }
        let response = request.send_unchecked().await?;
        let status = response.status().as_u16();
        let bytes = response.bytes_limited(MAX_BODY).await?;
        let body = without_secret(&String::from_utf8_lossy(&bytes), self.key.expose_secret());
        Ok(Reply { status, body })
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

/// O que um código de erro da API quer dizer para o laço (S-R5-4 §13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// 503/500: sobrecarga passageira; nova tentativa com espera exponencial.
    Transient {
        /// O código.
        status: u16,
    },
    /// 429 por minuto: esperar o `retryDelay` (até 60 s) e repetir.
    PerMinute {
        /// O tempo pedido.
        wait: Duration,
    },
    /// 429 diário ou com espera longa: parar na hora, sem prometer horário.
    Quota {
        /// O `quotaId` diz `FreeTier`.
        free_tier: bool,
    },
    /// Sinal de cobrança (402, `BILLING_DISABLED`…): parar tudo.
    Billing,
    /// Chave recusada.
    KeyInvalid,
    /// Outro erro.
    Other,
}

/// Espera máxima de um 429 por minuto antes de tratar como cota (S-R5-4 §16 item 5).
pub const MAX_PER_MINUTE_WAIT: Duration = Duration::from_secs(60);

/// Lê um código de erro da API e o corpo.
#[must_use]
pub fn classify(status: u16, body: &str) -> Problem {
    if billing_signal(status, body) {
        return Problem::Billing;
    }
    let json: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let api_status = json
        .pointer("/error/status")
        .and_then(Value::as_str)
        .unwrap_or("");
    let invalid_key = details(&json)
        .iter()
        .any(|detail| detail.get("reason").and_then(Value::as_str) == Some("API_KEY_INVALID"));
    match status {
        401 | 403 => Problem::KeyInvalid,
        400 if invalid_key || api_status == "UNAUTHENTICATED" => Problem::KeyInvalid,
        429 => rate_limit(&json),
        500 | 503 => Problem::Transient { status },
        _ => Problem::Other,
    }
}

fn details(json: &Value) -> &[Value] {
    json.pointer("/error/details")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// 429: por minuto (com `retryDelay` até 60 s) ou cota (diária ou espera longa).
fn rate_limit(json: &Value) -> Problem {
    let mut wait = None;
    let mut per_day = false;
    let mut free_tier = false;
    for detail in details(json) {
        if let Some(delay) = detail.get("retryDelay").and_then(Value::as_str) {
            wait = parse_delay(delay);
        }
        let violations = detail
            .get("violations")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        for violation in violations {
            let id = violation
                .get("quotaId")
                .and_then(Value::as_str)
                .unwrap_or("");
            per_day |= id.contains("PerDay");
            free_tier |= id.contains("FreeTier");
        }
    }
    match wait {
        Some(wait) if !per_day && wait <= MAX_PER_MINUTE_WAIT => Problem::PerMinute { wait },
        None if !per_day => Problem::PerMinute {
            wait: Duration::from_secs(30),
        },
        _ => Problem::Quota { free_tier },
    }
}

/// `"38s"` / `"38.85s"` → duração (arredondada para cima).
fn parse_delay(text: &str) -> Option<Duration> {
    let seconds: f64 = text.trim().strip_suffix('s')?.parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds.ceil()))
}

/// Erro que indica cobrança ou pagamento. Não conta a frase genérica dos 429 do plano gratuito
/// ("check your plan and billing details").
#[must_use]
pub fn billing_signal(status: u16, body: &str) -> bool {
    let text = body
        .to_lowercase()
        .replace("check your plan and billing details", "");
    status == 402
        || [
            "billing_disabled",
            "billing account",
            "billing is not enabled",
            "payment",
            "prepay",
            "insufficient funds",
        ]
        .iter()
        .any(|signal| text.contains(signal))
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

    /// Corpos reais do S-R5-4 (`tests/fixtures/erros-http-reais.json`).
    fn real(name: &str) -> String {
        let text = include_str!("../../../tests/fixtures/erros-http-reais.json");
        let all: Value = serde_json::from_str(text).unwrap();
        all[name].to_string()
    }

    #[test]
    fn erros_reais_do_spike() {
        assert_eq!(
            classify(503, &real("http_503_alta_demanda")),
            Problem::Transient { status: 503 }
        );
        assert_eq!(
            classify(429, &real("http_429_cota_por_minuto_plano_gratuito")),
            Problem::PerMinute {
                wait: Duration::from_secs(38)
            }
        );
        // O retryDelay diário (8658 s) é enganoso: vira cota, sem horário.
        assert_eq!(
            classify(429, &real("http_429_cota_diaria_plano_gratuito")),
            Problem::Quota { free_tier: true }
        );
    }

    #[test]
    fn frase_generica_do_429_nao_e_cobranca() {
        assert!(!billing_signal(
            429,
            &real("http_429_cota_por_minuto_plano_gratuito")
        ));
        assert!(billing_signal(402, "{}"));
        let disabled = json!({"error": {"code": 403, "status": "PERMISSION_DENIED",
            "details": [{"reason": "BILLING_DISABLED"}]}});
        assert_eq!(classify(403, &disabled.to_string()), Problem::Billing);
    }

    #[test]
    fn chave_recusada() {
        let invalid = json!({"error": {"code": 400, "status": "INVALID_ARGUMENT",
            "message": "API key not valid. Please pass a valid API key.",
            "details": [{"@type": "type.googleapis.com/google.rpc.ErrorInfo", "reason": "API_KEY_INVALID"}]}});
        assert_eq!(classify(400, &invalid.to_string()), Problem::KeyInvalid);
        assert_eq!(classify(403, "{}"), Problem::KeyInvalid);
        assert_eq!(classify(400, "{}"), Problem::Other);
    }

    #[test]
    fn espera_longa_vira_cota() {
        let body = json!({"error": {"code": 429, "details": [
            {"@type": "type.googleapis.com/google.rpc.RetryInfo", "retryDelay": "61s"}]}});
        assert_eq!(
            classify(429, &body.to_string()),
            Problem::Quota { free_tier: false }
        );
        assert_eq!(parse_delay("2.1s"), Some(Duration::from_secs(3)));
        assert_eq!(parse_delay("x"), None);
    }
}
