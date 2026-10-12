//! Login pelo *device flow* do GitHub App e renovação do token (ADR-0059).
//!
//! 1. [`request_code`]: `POST /login/device/code` devolve o código que a pessoa digita em
//!    `github.com/login/device` e o `device_code`, que fica só na memória.
//! 2. [`wait_for_token`]: consulta `POST /login/oauth/access_token` a cada `interval`
//!    segundos até a aprovação; `slow_down` aumenta o intervalo (o devolvido pelo GitHub ou
//!    mais 5 s); `expired_token`, `access_denied` e o fim do prazo terminam a espera; o
//!    cancelamento para na hora, sem nova requisição.
//! 3. [`refresh`]: troca o *refresh token* por um par novo (o GitHub invalida o antigo). Tokens
//!    do *device flow* renovam sem *client secret*.
//!
//! O GitHub responde esses pedidos com 200 e um campo `error` no corpo; os códigos que o
//! Warden não conhece viram [`Error::LoginAppRejected`] com o código.

use std::fmt;
use std::time::Duration;

use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;
use warden_core::CancellationToken;
use warden_http::{HeaderValue, HttpClient, Method, Timer, Url};

use crate::endpoints::Endpoints;
use crate::error::{Error, Result};

/// `grant_type` da consulta do código.
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// Intervalo usado quando o GitHub não diz (o mínimo que ele aceita).
const DEFAULT_INTERVAL: Duration = Duration::from_secs(5);

/// Quanto o `slow_down` soma ao intervalo quando não diz o novo (RFC 8628 §3.5).
const SLOW_DOWN_STEP: Duration = Duration::from_secs(5);

/// Maior resposta aceita dos pedidos de login.
const MAX_LOGIN_BODY: u64 = 64 * 1024;

/// O código pedido ao GitHub.
pub struct DeviceCode {
    /// Código que a pessoa digita ("WDJB-MJHT").
    pub user_code: String,
    /// Segredo da espera (nunca sai da memória).
    pub device_code: SecretString,
    /// Por quanto tempo o código vale.
    pub expires_in: Duration,
    /// Intervalo mínimo entre as consultas.
    pub interval: Duration,
}

impl fmt::Debug for DeviceCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceCode")
            .field("user_code", &self.user_code)
            .field("expires_in", &self.expires_in)
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

/// Os tokens de um login ou de uma renovação.
#[derive(Debug)]
pub struct TokenSet {
    /// Token de acesso (`ghu_…`).
    pub access_token: SecretString,
    /// Validade do token de acesso; `None` quando o app não usa tokens que expiram.
    pub expires_in: Option<Duration>,
    /// Token de renovação (`ghr_…`), quando o token expira.
    pub refresh_token: Option<SecretString>,
    /// Validade do token de renovação.
    pub refresh_token_expires_in: Option<Duration>,
}

/// O que aconteceu durante a espera (para a tela mostrar o progresso).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitEvent {
    /// Ainda esperando a aprovação.
    Pending,
    /// O GitHub pediu para consultar mais devagar; o novo intervalo.
    SlowDown(Duration),
}

/// Resposta dos dois pedidos de login: ou o resultado, ou `error`. Sem `Debug` (tem segredos).
#[derive(Deserialize)]
struct LoginReply {
    error: Option<String>,
    interval: Option<u64>,
    // `POST /login/device/code`
    device_code: Option<String>,
    user_code: Option<String>,
    expires_in: Option<u64>,
    // `POST /login/oauth/access_token`
    access_token: Option<String>,
    refresh_token: Option<String>,
    refresh_token_expires_in: Option<u64>,
}

/// Pede o código do dispositivo.
pub async fn request_code(http: &HttpClient, endpoints: &Endpoints) -> Result<DeviceCode> {
    if !endpoints.login_available() {
        return Err(Error::LoginUnavailable);
    }
    let reply = post(
        http,
        endpoints.device_code_url()?,
        &[("client_id", endpoints.client_id())],
        None,
    )
    .await?;
    if let Some(error) = reply.error {
        return Err(app_error(&error));
    }
    let (Some(device_code), Some(user_code)) = (reply.device_code, reply.user_code) else {
        return Err(Error::InvalidResponse(
            "o pedido de código não trouxe o código".to_owned(),
        ));
    };
    let user_code = user_code.trim().to_owned();
    if user_code.is_empty() || user_code.len() > 32 {
        return Err(Error::InvalidResponse(
            "o código do dispositivo veio num formato inesperado".to_owned(),
        ));
    }
    Ok(DeviceCode {
        user_code,
        device_code: SecretString::from(device_code),
        expires_in: Duration::from_secs(reply.expires_in.unwrap_or(900)),
        interval: reply
            .interval
            .map_or(DEFAULT_INTERVAL, Duration::from_secs)
            .max(DEFAULT_INTERVAL),
    })
}

/// Espera a aprovação. `timer` mede o prazo e as esperas (os testes usam um relógio virtual).
pub async fn wait_for_token(
    http: &HttpClient,
    endpoints: &Endpoints,
    code: &DeviceCode,
    timer: &dyn Timer,
    cancel: &CancellationToken,
    mut on_event: impl FnMut(WaitEvent) + Send,
) -> Result<TokenSet> {
    let started = timer.now();
    let mut interval = code.interval;
    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(Error::Cancelled),
            () = timer.sleep(interval) => {}
        }
        if timer.now().saturating_sub(started) >= code.expires_in {
            return Err(Error::LoginCodeExpired);
        }
        let reply = post(
            http,
            endpoints.access_token_url()?,
            &[
                ("client_id", endpoints.client_id()),
                ("device_code", code.device_code.expose_secret()),
                ("grant_type", DEVICE_GRANT),
            ],
            Some(cancel),
        )
        .await?;
        match reply.error.as_deref() {
            None => return tokens(reply),
            Some("authorization_pending") => on_event(WaitEvent::Pending),
            Some("slow_down") => {
                interval = reply
                    .interval
                    .map(Duration::from_secs)
                    .filter(|new| *new > interval)
                    .unwrap_or(interval + SLOW_DOWN_STEP);
                tracing::info!(
                    intervalo_s = interval.as_secs(),
                    "GitHub pediu para consultar mais devagar"
                );
                on_event(WaitEvent::SlowDown(interval));
            }
            Some("expired_token") => return Err(Error::LoginCodeExpired),
            Some("access_denied") => return Err(Error::LoginDenied),
            Some(other) => return Err(app_error(other)),
        }
    }
}

/// Troca o *refresh token* por um par novo. Recusa do GitHub = [`Error::SessionExpired`].
pub async fn refresh(
    http: &HttpClient,
    endpoints: &Endpoints,
    refresh_token: &SecretString,
) -> Result<TokenSet> {
    if !endpoints.login_available() {
        return Err(Error::LoginUnavailable);
    }
    let reply = post(
        http,
        endpoints.access_token_url()?,
        &[
            ("client_id", endpoints.client_id()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.expose_secret()),
        ],
        None,
    )
    .await?;
    match reply.error {
        None => tokens(reply),
        Some(reason) => Err(Error::SessionExpired { reason }),
    }
}

/// `POST` de formulário num dos endereços de login, com resposta em JSON. Os segredos vão no
/// corpo, nunca no endereço. Um 4xx aqui é o GitHub recusando o app (identificador
/// desconhecido): vira [`Error::LoginAppRejected`] com o código HTTP.
async fn post(
    http: &HttpClient,
    url: Url,
    fields: &[(&str, &str)],
    cancel: Option<&CancellationToken>,
) -> Result<LoginReply> {
    let mut request = http
        .request(Method::POST, url)
        .header(
            warden_http::HeaderName::from_static("accept"),
            HeaderValue::from_static("application/json"),
        )
        .form_body(fields)
        .max_body(MAX_LOGIN_BODY);
    if let Some(cancel) = cancel {
        request = request.cancel(cancel);
    }
    let response = request.send_unchecked().await?;
    let status = response.status();
    if status.is_client_error() && status.as_u16() != 429 {
        return Err(Error::LoginAppRejected {
            reason: format!("HTTP {}", status.as_u16()),
        });
    }
    let response = response.error_for_status().await?;
    let body = response.bytes_limited(MAX_LOGIN_BODY).await?;
    serde_json::from_slice(&body).map_err(|error| {
        Error::InvalidResponse(format!("resposta do login em JSON inválido: {error}"))
    })
}

fn tokens(reply: LoginReply) -> Result<TokenSet> {
    let Some(access_token) = reply.access_token.filter(|token| !token.is_empty()) else {
        return Err(Error::InvalidResponse(
            "a resposta do login não trouxe o token".to_owned(),
        ));
    };
    Ok(TokenSet {
        access_token: SecretString::from(access_token),
        expires_in: reply.expires_in.map(Duration::from_secs),
        refresh_token: reply
            .refresh_token
            .filter(|token| !token.is_empty())
            .map(SecretString::from),
        refresh_token_expires_in: reply.refresh_token_expires_in.map(Duration::from_secs),
    })
}

/// Erro do GitHub sobre o app (só o código, que é público: `device_flow_disabled`…).
fn app_error(code: &str) -> Error {
    let reason: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(64)
        .collect();
    Error::LoginAppRejected { reason }
}
