//! Erros do domínio `http` (ARCHITECTURE §5).
//!
//! Falta de conexão, tempo esgotado, cancelamento e falha de disco usam os códigos comuns do
//! domínio `core`; o resto (limite de requisições, servidor fora do ar, resposta estranha,
//! download que não confere) tem código próprio. As frases ficam em
//! `apps/desktop/src/i18n/errors/http.ts`.
//!
//! Nenhuma mensagem leva segredo: cabeçalhos nunca entram no erro, e as URLs guardadas são as
//! dos pedidos, que não carregam chave (a da CurseForge vai só no cabeçalho `x-api-key`).

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use warden_core::{CoreErrorCode, DomainCode, DomainError, error_chain};

/// Códigos do domínio `http`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HttpErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O servidor pediu para esperar (429) mais do que o Warden pode esperar sozinho.
    RateLimited,
    /// O servidor respondeu com erro dele (5xx) mesmo depois das novas tentativas.
    ServerError,
    /// O endereço pedido não existe no servidor (404 ou 410).
    NotFound,
    /// O servidor recusou o pedido com um código que o Warden não esperava.
    UnexpectedStatus,
    /// A resposta veio num formato que o Warden não entende.
    InvalidResponse,
    /// A resposta passou do tamanho máximo aceito.
    ResponseTooLarge,
    /// O arquivo baixado não tem o hash esperado.
    HashMismatch,
    /// O arquivo baixado não tem o tamanho esperado.
    SizeMismatch,
    /// O link não pôde ser usado (não é http/https ou está malformado).
    InvalidUrl,
    /// O servidor redirecionou vezes demais.
    TooManyRedirects,
    /// O servidor tentou redirecionar de https para http.
    InsecureRedirect,
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Sem conexão, conexão recusada ou interrompida.
    #[error("sem conexão com {host} ao pedir {url}")]
    Network {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// Erro da biblioteca HTTP.
        #[source]
        source: reqwest::Error,
    },
    /// Passou do tempo-limite (conexão, resposta ou tempo sem dados num download).
    #[error("tempo esgotado ao pedir {url} (limite de {seconds} s)")]
    Timeout {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// O limite, em segundos.
        seconds: u64,
    },
    /// Cancelado pelo usuário.
    #[error("operação cancelada")]
    Cancelled,
    /// Falha de disco num download.
    #[error("falha ao {action} {}", .path.display())]
    Io {
        /// O que se tentava fazer ("gravar", "renomear para"…).
        action: &'static str,
        /// O caminho envolvido.
        path: PathBuf,
        /// O erro do sistema.
        #[source]
        source: io::Error,
    },
    /// 429 sem poder esperar mais.
    #[error("{host} limitou as requisições (429) ao pedir {url}{}", wait_suffix(*.wait))]
    RateLimited {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// Quanto o servidor pediu para esperar, se disse.
        wait: Option<Duration>,
    },
    /// 5xx depois das novas tentativas.
    #[error("{host} respondeu {status} ao pedir {url}")]
    ServerError {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// Código HTTP.
        status: u16,
    },
    /// 404 ou 410.
    #[error("{host} respondeu {status} (não encontrado) ao pedir {url}")]
    NotFound {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// Código HTTP (404 ou 410).
        status: u16,
    },
    /// Outro código de erro (4xx).
    #[error("{host} respondeu {status} ao pedir {url}{}", body_suffix(.body.as_deref()))]
    UnexpectedStatus {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// Código HTTP.
        status: u16,
        /// Começo do corpo da resposta (até 1 KB), para "Detalhes técnicos".
        body: Option<String>,
    },
    /// Corpo que não pôde ser lido (JSON inválido, `Content-Range` incoerente…).
    #[error("resposta inválida de {host} para {url}: {reason}")]
    InvalidResponse {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// O que estava errado.
        reason: String,
    },
    /// Corpo maior que o limite.
    #[error("a resposta de {url} passou de {limit} bytes")]
    ResponseTooLarge {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// O limite, em bytes.
        limit: u64,
    },
    /// Download com hash diferente do esperado.
    #[error(
        "o arquivo baixado de {url} não confere: {algorithm} esperado {expected}, obtido {actual}"
    )]
    HashMismatch {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// Algoritmo conferido (`sha1`, `sha512`…).
        algorithm: String,
        /// Hash esperado.
        expected: String,
        /// Hash calculado.
        actual: String,
    },
    /// Download com tamanho diferente do esperado.
    #[error("o arquivo baixado de {url} tem {actual} bytes; eram esperados {expected}")]
    SizeMismatch {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
        /// Tamanho esperado.
        expected: u64,
        /// Tamanho obtido.
        actual: u64,
    },
    /// Link inválido.
    #[error("link inválido {url:?}: {reason}")]
    InvalidUrl {
        /// O link recebido (até 512 caracteres).
        url: String,
        /// Por que foi recusado.
        reason: String,
    },
    /// Redirecionamentos demais.
    #[error("redirecionamentos demais a partir de {url}")]
    TooManyRedirects {
        /// Servidor.
        host: String,
        /// Endereço pedido.
        url: String,
    },
    /// Redirecionamento de https para http.
    #[error("redirecionamento inseguro de {from} para {to}")]
    InsecureRedirect {
        /// Servidor.
        host: String,
        /// Endereço que redirecionou.
        from: String,
        /// Destino recusado.
        to: String,
    },
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

fn wait_suffix(wait: Option<Duration>) -> String {
    wait.map(|wait| format!(" (pediu para esperar {} s)", wait.as_secs()))
        .unwrap_or_default()
}

fn body_suffix(body: Option<&str>) -> String {
    body.filter(|body| !body.is_empty())
        .map(|body| format!(": {body}"))
        .unwrap_or_default()
}

/// Atalho para os resultados da crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Atalho para [`Error::Io`].
    pub(crate) fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }

    /// Servidor envolvido, quando há um.
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        match self {
            Self::Network { host, .. }
            | Self::Timeout { host, .. }
            | Self::RateLimited { host, .. }
            | Self::ServerError { host, .. }
            | Self::NotFound { host, .. }
            | Self::UnexpectedStatus { host, .. }
            | Self::InvalidResponse { host, .. }
            | Self::ResponseTooLarge { host, .. }
            | Self::HashMismatch { host, .. }
            | Self::SizeMismatch { host, .. }
            | Self::TooManyRedirects { host, .. }
            | Self::InsecureRedirect { host, .. } => Some(host),
            Self::Cancelled | Self::Io { .. } | Self::InvalidUrl { .. } | Self::Internal(_) => None,
        }
    }

    /// Código HTTP da resposta, quando o erro veio de uma.
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::RateLimited { .. } => Some(429),
            Self::ServerError { status, .. }
            | Self::NotFound { status, .. }
            | Self::UnexpectedStatus { status, .. } => Some(*status),
            _ => None,
        }
    }
}

impl DomainError for Error {
    type Code = HttpErrorCode;

    fn code(&self) -> DomainCode<HttpErrorCode> {
        DomainCode::Domain(match self {
            Self::Network { .. } => return DomainCode::Core(CoreErrorCode::NetworkUnavailable),
            Self::Timeout { .. } => return DomainCode::Core(CoreErrorCode::Timeout),
            Self::Cancelled => return DomainCode::Core(CoreErrorCode::Cancelled),
            Self::Io { .. } => return DomainCode::Core(CoreErrorCode::Io),
            Self::RateLimited { .. } => HttpErrorCode::RateLimited,
            Self::ServerError { .. } => HttpErrorCode::ServerError,
            Self::NotFound { .. } => HttpErrorCode::NotFound,
            Self::UnexpectedStatus { .. } => HttpErrorCode::UnexpectedStatus,
            Self::InvalidResponse { .. } => HttpErrorCode::InvalidResponse,
            Self::ResponseTooLarge { .. } => HttpErrorCode::ResponseTooLarge,
            Self::HashMismatch { .. } => HttpErrorCode::HashMismatch,
            Self::SizeMismatch { .. } => HttpErrorCode::SizeMismatch,
            Self::InvalidUrl { .. } => HttpErrorCode::InvalidUrl,
            Self::TooManyRedirects { .. } => HttpErrorCode::TooManyRedirects,
            Self::InsecureRedirect { .. } => HttpErrorCode::InsecureRedirect,
            Self::Internal(_) => HttpErrorCode::Internal,
        })
    }

    /// Parâmetros das frases: `host` (servidor) sempre que houver; `status`, `seconds`,
    /// `url` e `path` conforme o erro.
    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        if let Some(host) = self.host() {
            params.insert("host".to_owned(), host.to_owned());
        }
        if let Some(status) = self.status() {
            params.insert("status".to_owned(), status.to_string());
        }
        match self {
            Self::Timeout { seconds, .. } => {
                params.insert("seconds".to_owned(), seconds.to_string());
            }
            Self::RateLimited {
                wait: Some(wait), ..
            } => {
                params.insert("seconds".to_owned(), wait.as_secs().to_string());
            }
            Self::Io { path, .. } => {
                params.insert("path".to_owned(), path.display().to_string());
            }
            Self::InvalidUrl { url, .. } => {
                params.insert("url".to_owned(), url.clone());
            }
            _ => {}
        }
        params
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::Cancelled => None,
            _ => Some(error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Network { .. }
                | Self::Timeout { .. }
                | Self::Io { .. }
                | Self::RateLimited { .. }
                | Self::ServerError { .. }
                | Self::HashMismatch { .. }
                | Self::SizeMismatch { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_codes() -> Vec<HttpErrorCode> {
        vec![
            HttpErrorCode::Internal,
            HttpErrorCode::RateLimited,
            HttpErrorCode::ServerError,
            HttpErrorCode::NotFound,
            HttpErrorCode::UnexpectedStatus,
            HttpErrorCode::InvalidResponse,
            HttpErrorCode::ResponseTooLarge,
            HttpErrorCode::HashMismatch,
            HttpErrorCode::SizeMismatch,
            HttpErrorCode::InvalidUrl,
            HttpErrorCode::TooManyRedirects,
            HttpErrorCode::InsecureRedirect,
        ]
    }

    #[test]
    fn codigos_em_maiusculas_com_sublinhado() {
        let json = serde_json::to_value(all_codes()).unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                "INTERNAL",
                "RATE_LIMITED",
                "SERVER_ERROR",
                "NOT_FOUND",
                "UNEXPECTED_STATUS",
                "INVALID_RESPONSE",
                "RESPONSE_TOO_LARGE",
                "HASH_MISMATCH",
                "SIZE_MISMATCH",
                "INVALID_URL",
                "TOO_MANY_REDIRECTS",
                "INSECURE_REDIRECT"
            ])
        );
    }

    fn sample(variant: &str) -> Error {
        let host = || "api.modrinth.com".to_owned();
        let url = || "https://api.modrinth.com/v2/project/x".to_owned();
        match variant {
            "rate" => Error::RateLimited {
                host: host(),
                url: url(),
                wait: Some(Duration::from_secs(42)),
            },
            "server" => Error::ServerError {
                host: host(),
                url: url(),
                status: 503,
            },
            "notfound" => Error::NotFound {
                host: host(),
                url: url(),
                status: 404,
            },
            "status" => Error::UnexpectedStatus {
                host: host(),
                url: url(),
                status: 400,
                body: Some("{\"error\":\"invalid_input\"}".into()),
            },
            "invalid" => Error::InvalidResponse {
                host: host(),
                url: url(),
                reason: "JSON inválido".into(),
            },
            "large" => Error::ResponseTooLarge {
                host: host(),
                url: url(),
                limit: 10,
            },
            "hash" => Error::HashMismatch {
                host: host(),
                url: url(),
                algorithm: "sha1".into(),
                expected: "aa".into(),
                actual: "bb".into(),
            },
            "size" => Error::SizeMismatch {
                host: host(),
                url: url(),
                expected: 2,
                actual: 1,
            },
            "redirects" => Error::TooManyRedirects {
                host: host(),
                url: url(),
            },
            "insecure" => Error::InsecureRedirect {
                host: host(),
                from: url(),
                to: "http://x".into(),
            },
            "timeout" => Error::Timeout {
                host: host(),
                url: url(),
                seconds: 30,
            },
            "url" => Error::InvalidUrl {
                url: "ftp://x".into(),
                reason: "só http e https".into(),
            },
            "io" => Error::io("gravar", "C:/cache/x.part", io::Error::other("disco cheio")),
            "cancel" => Error::Cancelled,
            _ => Error::Internal("x".into()),
        }
    }

    #[test]
    fn codigos_parametros_e_nova_tentativa() {
        let cases = [
            ("rate", DomainCode::Domain(HttpErrorCode::RateLimited), true),
            (
                "server",
                DomainCode::Domain(HttpErrorCode::ServerError),
                true,
            ),
            (
                "notfound",
                DomainCode::Domain(HttpErrorCode::NotFound),
                false,
            ),
            (
                "status",
                DomainCode::Domain(HttpErrorCode::UnexpectedStatus),
                false,
            ),
            (
                "invalid",
                DomainCode::Domain(HttpErrorCode::InvalidResponse),
                false,
            ),
            (
                "large",
                DomainCode::Domain(HttpErrorCode::ResponseTooLarge),
                false,
            ),
            (
                "hash",
                DomainCode::Domain(HttpErrorCode::HashMismatch),
                true,
            ),
            (
                "size",
                DomainCode::Domain(HttpErrorCode::SizeMismatch),
                true,
            ),
            (
                "redirects",
                DomainCode::Domain(HttpErrorCode::TooManyRedirects),
                false,
            ),
            (
                "insecure",
                DomainCode::Domain(HttpErrorCode::InsecureRedirect),
                false,
            ),
            ("timeout", DomainCode::Core(CoreErrorCode::Timeout), true),
            ("url", DomainCode::Domain(HttpErrorCode::InvalidUrl), false),
            ("io", DomainCode::Core(CoreErrorCode::Io), true),
            ("cancel", DomainCode::Core(CoreErrorCode::Cancelled), false),
            (
                "internal",
                DomainCode::Domain(HttpErrorCode::Internal),
                false,
            ),
        ];
        for (name, code, retryable) in cases {
            let error = sample(name);
            assert_eq!(error.code(), code, "{name}");
            assert_eq!(error.retryable(), retryable, "{name}");
            if !matches!(name, "url" | "io" | "cancel" | "internal") {
                assert_eq!(
                    error.params()["host"],
                    "api.modrinth.com",
                    "{name}: {error}"
                );
            }
        }
        assert_eq!(sample("rate").params()["seconds"], "42");
        assert_eq!(sample("rate").params()["status"], "429");
        assert_eq!(sample("server").params()["status"], "503");
        assert_eq!(sample("timeout").params()["seconds"], "30");
        assert_eq!(sample("url").params()["url"], "ftp://x");
        assert_eq!(sample("io").params()["path"], "C:/cache/x.part");
        assert_eq!(sample("cancel").detail(), None);
        assert!(sample("status").detail().unwrap().contains("invalid_input"));
        assert!(sample("rate").to_string().contains("esperar 42 s"));
    }
}
