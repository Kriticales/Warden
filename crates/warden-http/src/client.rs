//! O cliente HTTP do Warden (ARCHITECTURE §17).
//!
//! - TLS pelo `rustls` com o provedor `ring` e a verificação de certificados do sistema
//!   (`rustls-platform-verifier`): sem OpenSSL nem compilador de C extra no Windows.
//! - User-Agent fixo ([`crate::user_agent`]), limitador por servidor ([`crate::HostPolicy`]).
//! - Novas tentativas com espera exponencial (no máximo 3) em 429, 5xx, falha de conexão e
//!   tempo esgotado, **só** em requisições idempotentes (`GET`, `HEAD` e as `POST` de consulta
//!   marcadas com [`Request::idempotent`]), respeitando `Retry-After` e `X-Ratelimit-Reset`.
//! - Redirecionamentos seguidos pelo próprio Warden, um a um: cabeçalhos sensíveis (como o
//!   `x-api-key` da CurseForge) só seguem para o mesmo site (`*.forgecdn.net` → `*.forgecdn.net`),
//!   e https nunca cai para http. Isso também permite resolver o destino final **sem** `Range`
//!   antes de pedir partes ([`HttpClient::resolve_redirects`]), porque o
//!   `edge.forgecdn.net` responde 404 a pedidos com `Range` (R7 §9, conferido em 04/10/2026).
//! - Nada de segredo em registro: cabeçalhos nunca são registrados, e os marcados como
//!   sensíveis ([`Request::secret_header`]) também não aparecem no `Debug` do `reqwest`.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, LOCATION};
use reqwest::{Method, StatusCode};
use secrecy::{ExposeSecret as _, SecretString};
use serde::Serialize;
use serde::de::DeserializeOwned;
use url::Url;
use warden_core::CancellationToken;

use crate::config::HttpConfig;
use crate::error::{Error, Result};
use crate::headers::{ratelimit_exhausted, ratelimit_reset, requested_wait, retry_after};
use crate::limiter::{HostLimiter, Slot, cancellable};
use crate::timer::{SystemTimer, Timer};

/// Quanto do corpo de uma resposta de erro vai para "Detalhes técnicos".
const ERROR_BODY_EXCERPT: usize = 1024;

/// Servidores cujo corpo de resposta nunca entra em erro nem em registro (termos da
/// CurseForge, ARCHITECTURE §16).
const PRIVATE_BODY_SITES: [&str; 2] = ["curseforge.com", "forgecdn.net"];

/// Cliente HTTP único do Warden. Barato de clonar: os clones dividem conexões e limites.
#[derive(Clone)]
pub struct HttpClient {
    inner: Arc<Inner>,
}

struct Inner {
    client: reqwest::Client,
    config: Arc<HttpConfig>,
    timer: Arc<dyn Timer>,
    limiter: HostLimiter,
    wall_clock: fn() -> SystemTime,
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClient")
            .field("user_agent", &self.inner.config.user_agent)
            .finish_non_exhaustive()
    }
}

impl HttpClient {
    /// Cliente com o relógio real.
    pub fn new(config: HttpConfig) -> Result<Self> {
        Self::with_timer(config, Arc::new(SystemTimer::new()))
    }

    /// Cliente com um relógio injetado (testes: [`crate::ManualTimer`]).
    pub fn with_timer(config: HttpConfig, timer: Arc<dyn Timer>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .use_preconfigured_tls(tls_config()?)
            .user_agent(config.user_agent.clone())
            .connect_timeout(config.connect_timeout)
            .read_timeout(config.read_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| Error::Internal(format!("cliente HTTP: {e}")))?;
        let config = Arc::new(config);
        Ok(Self {
            inner: Arc::new(Inner {
                client,
                limiter: HostLimiter::new(Arc::clone(&config), Arc::clone(&timer)),
                config,
                timer,
                wall_clock: SystemTime::now,
            }),
        })
    }

    /// A configuração em uso.
    #[must_use]
    pub fn config(&self) -> &HttpConfig {
        &self.inner.config
    }

    pub(crate) fn timer(&self) -> &dyn Timer {
        self.inner.timer.as_ref()
    }

    /// `GET` (idempotente).
    #[must_use]
    pub fn get(&self, url: Url) -> Request {
        Request::new(self.clone(), Method::GET, url)
    }

    /// `HEAD` (idempotente).
    #[must_use]
    pub fn head(&self, url: Url) -> Request {
        Request::new(self.clone(), Method::HEAD, url)
    }

    /// `POST` com corpo JSON. Não é repetido sozinho, a menos que se marque
    /// [`Request::idempotent`] (consultas como `POST /version_files`).
    pub fn post_json<T: Serialize + ?Sized>(&self, url: Url, body: &T) -> Result<Request> {
        let body = serde_json::to_vec(body)
            .map_err(|e| Error::Internal(format!("corpo JSON da requisição: {e}")))?;
        let mut request = Request::new(self.clone(), Method::POST, url);
        request.idempotent = false;
        request.body = Some(body);
        request
            .headers
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Ok(request)
    }

    /// Segue os redirecionamentos de `url` com `HEAD` e **sem** `Range`, e devolve o endereço
    /// final. Usado antes de pedir partes de um arquivo: o `edge.forgecdn.net` responde 404 a
    /// um `GET` com `Range`, mas redireciona normalmente sem ele. Servidor que recusa `HEAD`
    /// (405 ou 501) devolve o próprio `url`.
    pub async fn resolve_redirects(
        &self,
        url: Url,
        headers: &HeaderMap,
        cancel: Option<&CancellationToken>,
    ) -> Result<Url> {
        let mut request = self.head(url.clone());
        request.headers.extend(headers.clone());
        request.headers.remove(reqwest::header::RANGE);
        request.cancel = cancel.cloned();
        let response = request.send_raw().await?;
        let status = response.status();
        if status == StatusCode::METHOD_NOT_ALLOWED || status == StatusCode::NOT_IMPLEMENTED {
            return Ok(url);
        }
        Ok(response.final_url().clone())
    }
}

fn tls_config() -> Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .and_then(rustls_platform_verifier::BuilderVerifierExt::with_platform_verifier)
        .map_err(|e| Error::Internal(format!("TLS: {e}")))?;
    Ok(builder.with_no_client_auth())
}

/// Uma requisição sendo montada.
pub struct Request {
    client: HttpClient,
    method: Method,
    url: Url,
    pub(crate) headers: HeaderMap,
    body: Option<Vec<u8>>,
    idempotent: bool,
    pub(crate) cancel: Option<CancellationToken>,
    pub(crate) timeout: Option<Duration>,
    max_body: u64,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Sem cabeçalhos nem corpo: podem ter chave.
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("url", &self.url.as_str())
            .finish_non_exhaustive()
    }
}

impl Request {
    fn new(client: HttpClient, method: Method, url: Url) -> Self {
        let timeout = Some(client.inner.config.request_timeout);
        let max_body = client.inner.config.max_body_bytes;
        Self {
            client,
            method,
            url,
            headers: HeaderMap::new(),
            body: None,
            idempotent: true,
            cancel: None,
            timeout,
            max_body,
        }
    }

    /// Acrescenta um cabeçalho.
    #[must_use]
    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.insert(name, value);
        self
    }

    /// Acrescenta um cabeçalho com segredo (o `x-api-key` da CurseForge): marcado como
    /// sensível, nunca registrado e só repassado em redirecionamentos para o mesmo site.
    pub fn secret_header(mut self, name: HeaderName, secret: &SecretString) -> Result<Self> {
        let mut value = HeaderValue::from_str(secret.expose_secret())
            .map_err(|_| Error::Internal(format!("valor inválido no cabeçalho {name}")))?;
        value.set_sensitive(true);
        self.headers.insert(name, value);
        Ok(self)
    }

    /// Marca se a requisição pode ser repetida sozinha (padrão: sim para `GET`/`HEAD`, não
    /// para `POST`).
    #[must_use]
    pub fn idempotent(mut self, idempotent: bool) -> Self {
        self.idempotent = idempotent;
        self
    }

    /// Cancela a requisição (inclusive as esperas) quando o token for acionado.
    #[must_use]
    pub fn cancel(mut self, token: &CancellationToken) -> Self {
        self.cancel = Some(token.clone());
        self
    }

    /// Troca o maior corpo aceito.
    #[must_use]
    pub fn max_body(mut self, bytes: u64) -> Self {
        self.max_body = bytes;
        self
    }

    /// Manda e exige resposta 2xx. Erros: 404/410 → [`Error::NotFound`], 429 →
    /// [`Error::RateLimited`], 5xx → [`Error::ServerError`], outros → [`Error::UnexpectedStatus`].
    pub async fn send(self) -> Result<Response> {
        let response = self.send_raw().await?;
        response.error_for_status().await
    }

    /// Manda e lê o corpo inteiro (até o limite).
    pub async fn bytes(self) -> Result<Vec<u8>> {
        let max_body = self.max_body;
        self.send().await?.bytes_limited(max_body).await
    }

    /// Manda e lê o corpo como JSON.
    pub async fn json<T: DeserializeOwned>(self) -> Result<T> {
        let max_body = self.max_body;
        let response = self.send().await?;
        let host = response.host.clone();
        let url = response.request_url.to_string();
        let body = response.bytes_limited(max_body).await?;
        serde_json::from_slice(&body).map_err(|e| Error::InvalidResponse {
            host,
            url,
            reason: format!("JSON inválido: {e}"),
        })
    }

    /// Manda com novas tentativas e limitador, sem olhar o código da resposta (exceto 429 e
    /// 5xx, que são repetidos quando permitido).
    pub(crate) async fn send_raw(self) -> Result<Response> {
        let inner = Arc::clone(&self.client.inner);
        let host = host_of(&self.url)?;
        let cancel = self.cancel.clone();
        let mut attempt: u32 = 0;
        loop {
            let slot = inner.limiter.acquire(&host, cancel.as_ref()).await?;
            let result = self.send_following_redirects().await;
            let can_retry = self.idempotent && attempt < inner.config.max_retries;
            match result {
                Ok((response, final_url)) => {
                    let status = response.status();
                    let headers = response.headers();
                    tracing::debug!(
                        metodo = %self.method,
                        url = %self.url,
                        status = status.as_u16(),
                        tentativa = attempt + 1,
                        "resposta HTTP"
                    );
                    if ratelimit_exhausted(headers)
                        && let Some(reset) = ratelimit_reset(headers)
                    {
                        inner.limiter.block(&host, reset);
                    }
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let requested = requested_wait(headers, (inner.wall_clock)());
                        let wait = requested.unwrap_or(inner.config.default_rate_limit_wait);
                        if can_retry && wait <= inner.config.max_rate_limit_wait {
                            tracing::info!(host, espera_s = wait.as_secs_f64(), "429: aguardando");
                            inner.limiter.block(&host, wait);
                            attempt += 1;
                            continue;
                        }
                        return Err(Error::RateLimited {
                            host,
                            url: self.url.to_string(),
                            wait: requested,
                        });
                    }
                    if is_transient_status(status) && can_retry {
                        let wait = retry_after(headers, (inner.wall_clock)())
                            .filter(|wait| *wait <= inner.config.max_rate_limit_wait)
                            .unwrap_or_else(|| backoff(&inner.config, attempt));
                        drop(response);
                        drop(slot);
                        cancellable(cancel.as_ref(), inner.timer.sleep(wait)).await?;
                        attempt += 1;
                        continue;
                    }
                    return Ok(Response {
                        inner: response,
                        host,
                        request_url: self.url.clone(),
                        final_url,
                        cancel,
                        read_timeout: inner.config.read_timeout,
                        _slot: slot,
                    });
                }
                Err(error) if can_retry && is_transient_error(&error) => {
                    tracing::debug!(url = %self.url, %error, "falha transitória; nova tentativa");
                    drop(slot);
                    let wait = backoff(&inner.config, attempt);
                    cancellable(cancel.as_ref(), inner.timer.sleep(wait)).await?;
                    attempt += 1;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn send_following_redirects(&self) -> Result<(reqwest::Response, Url)> {
        let inner = &self.client.inner;
        let mut url = self.url.clone();
        let mut method = self.method.clone();
        let mut body = self.body.clone();
        let mut redirects = 0;
        loop {
            let mut builder = inner.client.request(method.clone(), url.clone());
            for (name, value) in &self.headers {
                if !value.is_sensitive() || same_site(&self.url, &url) {
                    builder = builder.header(name, value);
                }
            }
            if let Some(body) = &body {
                builder = builder.body(body.clone());
            }
            if let Some(timeout) = self.timeout {
                builder = builder.timeout(timeout);
            }
            let response = cancellable(self.cancel.as_ref(), builder.send())
                .await?
                .map_err(|e| map_reqwest_error(e, &url, self.timeout, inner.config.read_timeout))?;
            let status = response.status();
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok());
            let Some(location) = location.filter(|_| status.is_redirection()) else {
                return Ok((response, url));
            };
            let host = host_of(&url)?;
            if redirects >= inner.config.max_redirects {
                return Err(Error::TooManyRedirects {
                    host,
                    url: self.url.to_string(),
                });
            }
            let next = url.join(location).map_err(|e| Error::InvalidResponse {
                host: host.clone(),
                url: url.to_string(),
                reason: format!("redirecionamento para um endereço inválido: {e}"),
            })?;
            if url.scheme() == "https" && next.scheme() != "https" {
                return Err(Error::InsecureRedirect {
                    host,
                    from: url.to_string(),
                    to: next.to_string(),
                });
            }
            check_scheme(&next)?;
            if status == StatusCode::SEE_OTHER
                || (matches!(status, StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND)
                    && method == Method::POST)
            {
                method = Method::GET;
                body = None;
            }
            tracing::debug!(de = %url, para = %next, status = status.as_u16(), "redirecionamento");
            url = next;
            redirects += 1;
        }
    }
}

/// Resposta recebida. Segura a vaga do servidor até ser descartada.
pub struct Response {
    inner: reqwest::Response,
    host: String,
    request_url: Url,
    final_url: Url,
    cancel: Option<CancellationToken>,
    read_timeout: Duration,
    _slot: Slot,
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.inner.status())
            .field("url", &self.final_url.as_str())
            .finish_non_exhaustive()
    }
}

impl Response {
    /// Código HTTP.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    /// Cabeçalhos.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// Endereço que respondeu, depois dos redirecionamentos.
    #[must_use]
    pub fn final_url(&self) -> &Url {
        &self.final_url
    }

    /// Endereço pedido.
    #[must_use]
    pub fn request_url(&self) -> &Url {
        &self.request_url
    }

    /// Servidor pedido.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Tamanho anunciado do corpo.
    #[must_use]
    pub fn content_length(&self) -> Option<u64> {
        self.headers()
            .get(CONTENT_LENGTH)?
            .to_str()
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// Próximo pedaço do corpo (`None` no fim).
    pub async fn chunk(&mut self) -> Result<Option<bytes::Bytes>> {
        let cancel = self.cancel.clone();
        let result = cancellable(cancel.as_ref(), self.inner.chunk()).await?;
        // No meio do corpo, o que esgota é o tempo sem dados (downloads não têm tempo total).
        result.map_err(|e| map_reqwest_error(e, &self.final_url, None, self.read_timeout))
    }

    /// Lê o corpo inteiro, recusando o que passar de `limit` bytes.
    pub async fn bytes_limited(mut self, limit: u64) -> Result<Vec<u8>> {
        let too_large = |response: &Self| Error::ResponseTooLarge {
            host: response.host.clone(),
            url: response.request_url.to_string(),
            limit,
        };
        if self.content_length().is_some_and(|length| length > limit) {
            return Err(too_large(&self));
        }
        let mut body = Vec::new();
        while let Some(chunk) = self.chunk().await? {
            if body.len() as u64 + chunk.len() as u64 > limit {
                return Err(too_large(&self));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    /// Exige 2xx; caso contrário, o erro do código (com o começo do corpo nos detalhes,
    /// exceto da CurseForge).
    pub async fn error_for_status(mut self) -> Result<Self> {
        let status = self.status();
        if status.is_success() {
            return Ok(self);
        }
        let host = self.host.clone();
        let url = self.request_url.to_string();
        let code = status.as_u16();
        Err(match status {
            StatusCode::NOT_FOUND | StatusCode::GONE => Error::NotFound {
                host,
                url,
                status: code,
            },
            StatusCode::TOO_MANY_REQUESTS => Error::RateLimited {
                host,
                url,
                wait: requested_wait(self.headers(), SystemTime::now()),
            },
            _ if status.is_server_error() => Error::ServerError {
                host,
                url,
                status: code,
            },
            _ => {
                let body = if is_private_body_site(&host) {
                    None
                } else {
                    self.error_excerpt().await
                };
                Error::UnexpectedStatus {
                    host,
                    url,
                    status: code,
                    body,
                }
            }
        })
    }

    async fn error_excerpt(&mut self) -> Option<String> {
        let mut body = Vec::new();
        while body.len() < ERROR_BODY_EXCERPT {
            match self.chunk().await {
                Ok(Some(chunk)) => body.extend_from_slice(&chunk),
                _ => break,
            }
        }
        body.truncate(ERROR_BODY_EXCERPT);
        let text = String::from_utf8_lossy(&body).trim().to_owned();
        (!text.is_empty()).then_some(text)
    }
}

/// Valida um link vindo de fora: só `http` e `https`, com servidor.
pub fn parse_url(text: &str) -> Result<Url> {
    let url = Url::parse(text.trim()).map_err(|e| Error::InvalidUrl {
        url: text.chars().take(512).collect(),
        reason: e.to_string(),
    })?;
    check_scheme(&url)?;
    host_of(&url)?;
    Ok(url)
}

fn check_scheme(url: &Url) -> Result<()> {
    if matches!(url.scheme(), "http" | "https") {
        Ok(())
    } else {
        Err(Error::InvalidUrl {
            url: url.as_str().chars().take(512).collect(),
            reason: "só links http e https são aceitos".into(),
        })
    }
}

pub(crate) fn host_of(url: &Url) -> Result<String> {
    url.host_str()
        .filter(|host| !host.is_empty())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| Error::InvalidUrl {
            url: url.as_str().chars().take(512).collect(),
            reason: "o link não tem servidor".into(),
        })
}

/// Os dois links são do mesmo site: mesmo servidor, ou os dois últimos rótulos do nome iguais
/// (`edge.forgecdn.net` e `mediafilez.forgecdn.net`). Endereços IP só valem iguais.
#[must_use]
pub fn same_site(a: &Url, b: &Url) -> bool {
    let (Some(a), Some(b)) = (a.host(), b.host()) else {
        return false;
    };
    match (a, b) {
        (url::Host::Domain(a), url::Host::Domain(b)) => {
            let site = |host: &str| -> String {
                let host = host.trim_end_matches('.').to_ascii_lowercase();
                let labels: Vec<&str> = host.rsplit('.').take(2).collect();
                labels.into_iter().rev().collect::<Vec<_>>().join(".")
            };
            site(a) == site(b)
        }
        (a, b) => a == b,
    }
}

fn is_private_body_site(host: &str) -> bool {
    PRIVATE_BODY_SITES
        .iter()
        .any(|site| host == *site || host.ends_with(&format!(".{site}")))
}

fn is_transient_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

pub(crate) fn is_transient_error(error: &Error) -> bool {
    matches!(error, Error::Network { .. } | Error::Timeout { .. })
}

/// Espera antes da tentativa `attempt + 2`: base, 2 × base, 4 × base…
pub(crate) fn backoff(config: &HttpConfig, attempt: u32) -> Duration {
    config
        .retry_base_delay
        .saturating_mul(2_u32.saturating_pow(attempt.min(16)))
}

pub(crate) fn map_reqwest_error(
    error: reqwest::Error,
    url: &Url,
    timeout: Option<Duration>,
    read_timeout: Duration,
) -> Error {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if error.is_timeout() {
        return Error::Timeout {
            host,
            url: url.to_string(),
            seconds: timeout.unwrap_or(read_timeout).as_secs(),
        };
    }
    if error.is_builder() {
        return Error::Internal(format!("requisição inválida para {url}: {error}"));
    }
    Error::Network {
        host,
        url: url.to_string(),
        source: error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesmo_site() {
        let url = |s: &str| Url::parse(s).unwrap();
        assert!(same_site(
            &url("https://edge.forgecdn.net/files/1"),
            &url("https://mediafilez.forgecdn.net/files/1")
        ));
        assert!(same_site(
            &url("https://forgecdn.net/a"),
            &url("https://X.FORGECDN.NET./b")
        ));
        assert!(!same_site(
            &url("https://edge.forgecdn.net/a"),
            &url("https://evil.example.com/a")
        ));
        assert!(same_site(
            &url("http://127.0.0.1:1/a"),
            &url("http://127.0.0.1:2/b")
        ));
        assert!(!same_site(
            &url("http://127.0.0.1/a"),
            &url("http://127.0.0.2/b")
        ));
    }

    #[test]
    fn links_aceitos() {
        assert!(parse_url("https://api.modrinth.com/v2").is_ok());
        assert!(parse_url(" http://x.y/z ").is_ok());
        for bad in ["ftp://x/y", "file:///C:/x", "não é link", "https://"] {
            let error = parse_url(bad).unwrap_err();
            assert!(matches!(error, Error::InvalidUrl { .. }), "{bad}: {error}");
        }
    }

    #[test]
    fn espera_exponencial() {
        let config = HttpConfig::default();
        assert_eq!(backoff(&config, 0), Duration::from_millis(500));
        assert_eq!(backoff(&config, 1), Duration::from_secs(1));
        assert_eq!(backoff(&config, 2), Duration::from_secs(2));
        assert!(backoff(&config, 40) >= Duration::from_secs(60));
    }

    #[test]
    fn corpo_privado_da_curseforge() {
        assert!(is_private_body_site("api.curseforge.com"));
        assert!(is_private_body_site("edge.forgecdn.net"));
        assert!(!is_private_body_site("api.modrinth.com"));
        assert!(!is_private_body_site("notcurseforge.com"));
    }
}
