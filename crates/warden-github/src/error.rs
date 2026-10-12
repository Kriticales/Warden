//! Erros do domínio `github` (ARCHITECTURE §5; UI-03).
//!
//! As falhas de rede chegam da `warden-http`: sem conexão, tempo esgotado e cancelamento
//! continuam com os códigos comuns do domínio `core`; os demais viram códigos do GitHub, para
//! a frase dizer "o GitHub". As frases ficam em `apps/desktop/src/i18n/errors/github.ts`.
//!
//! Nenhuma mensagem leva token, *refresh token* nem `device_code`: só endereços (que nunca
//! têm segredo), códigos HTTP e os códigos de erro do próprio GitHub.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use warden_core::{DomainCode, DomainError, error_chain};
use warden_http::HttpErrorCode;

/// Códigos do domínio `github`. Só acréscimo.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GithubErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O Warden ainda não tem o identificador do GitHub App (constante vazia).
    LoginUnavailable,
    /// O GitHub recusou o app do Warden no login (device flow desligado, identificador errado);
    /// `params.reason` traz o código do GitHub.
    LoginAppRejected,
    /// O código do dispositivo venceu antes da aprovação.
    LoginCodeExpired,
    /// A pessoa recusou o acesso na página do GitHub.
    LoginDenied,
    /// Não há login do GitHub (nem token guardado).
    NotSignedIn,
    /// O token foi recusado e não deu para renovar: é preciso entrar de novo.
    SessionExpired,
    /// O GitHub recusou a ação por falta de permissão (403).
    PermissionDenied,
    /// O repositório do Warden não foi encontrado (ou o login não tem acesso a ele).
    RepositoryNotFound,
    /// O limite de pedidos da API acabou; `params.seconds` diz quanto falta, quando o GitHub
    /// informa.
    RateLimited,
    /// O GitHub está fora do ar ou com erro (5xx).
    Unavailable,
    /// O GitHub recusou o pedido (outro 4xx); `params.status`.
    RequestRejected,
    /// O GitHub respondeu algo que o Warden não entende.
    InvalidResponse,
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Sem *client ID* (nenhuma requisição saiu).
    #[error("login com o GitHub indisponível: o Warden não tem o identificador do GitHub App")]
    LoginUnavailable,
    /// O GitHub recusou o app no login (`device_flow_disabled`, `incorrect_client_credentials`…).
    #[error("o GitHub recusou o login do app do Warden ({reason})")]
    LoginAppRejected {
        /// Código de erro do GitHub.
        reason: String,
    },
    /// `expired_token` ou o prazo do código acabou.
    #[error("o código de login do GitHub venceu antes da aprovação")]
    LoginCodeExpired,
    /// `access_denied`.
    #[error("o acesso foi recusado na página do GitHub")]
    LoginDenied,
    /// Não há token.
    #[error("sem login do GitHub")]
    NotSignedIn,
    /// Token recusado (401) ou renovação recusada.
    #[error("o GitHub recusou o token ({reason})")]
    SessionExpired {
        /// O que foi recusado ("HTTP 401" ou o código de erro da renovação).
        reason: String,
    },
    /// 403 sem ser limite de pedidos.
    #[error("o GitHub recusou {url} por falta de permissão (HTTP 403)")]
    PermissionDenied {
        /// Endereço pedido.
        url: String,
    },
    /// 404 num endereço do repositório.
    #[error("o GitHub não encontrou {url} (HTTP 404)")]
    RepositoryNotFound {
        /// Endereço pedido.
        url: String,
    },
    /// 403 ou 429 com o limite esgotado.
    #[error("o GitHub limitou os pedidos ao pedir {url}")]
    RateLimited {
        /// Endereço pedido.
        url: String,
        /// Quanto falta para o limite voltar, se o GitHub disse.
        wait: Option<Duration>,
    },
    /// Outro 4xx.
    #[error("o GitHub recusou {url} (HTTP {status}){}", message_suffix(.message.as_deref()))]
    Rejected {
        /// Endereço pedido.
        url: String,
        /// Código HTTP.
        status: u16,
        /// O `message` do GitHub, quando veio.
        message: Option<String>,
    },
    /// Falha da requisição.
    #[error(transparent)]
    Http(#[from] warden_http::Error),
    /// Resposta que não pôde ser usada.
    #[error("resposta inválida do GitHub: {0}")]
    InvalidResponse(String),
    /// Cancelado.
    #[error("operação cancelada")]
    Cancelled,
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

fn message_suffix(message: Option<&str>) -> String {
    message.map(|text| format!(": {text}")).unwrap_or_default()
}

/// Atalho para os resultados da crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Se o login precisa ser feito de novo.
    #[must_use]
    pub fn needs_login(&self) -> bool {
        matches!(self, Self::NotSignedIn | Self::SessionExpired { .. })
    }
}

impl DomainError for Error {
    type Code = GithubErrorCode;

    fn code(&self) -> DomainCode<GithubErrorCode> {
        DomainCode::Domain(match self {
            Self::LoginUnavailable => GithubErrorCode::LoginUnavailable,
            Self::LoginAppRejected { .. } => GithubErrorCode::LoginAppRejected,
            Self::LoginCodeExpired => GithubErrorCode::LoginCodeExpired,
            Self::LoginDenied => GithubErrorCode::LoginDenied,
            Self::NotSignedIn => GithubErrorCode::NotSignedIn,
            Self::SessionExpired { .. } => GithubErrorCode::SessionExpired,
            Self::PermissionDenied { .. } => GithubErrorCode::PermissionDenied,
            Self::RepositoryNotFound { .. } => GithubErrorCode::RepositoryNotFound,
            Self::RateLimited { .. } => GithubErrorCode::RateLimited,
            Self::Rejected { .. } => GithubErrorCode::RequestRejected,
            Self::InvalidResponse(_) => GithubErrorCode::InvalidResponse,
            Self::Internal(_) => GithubErrorCode::Internal,
            Self::Cancelled => return DomainCode::Core(warden_core::CoreErrorCode::Cancelled),
            Self::Http(error) => match error.code() {
                DomainCode::Core(code) => return DomainCode::Core(code),
                DomainCode::Domain(code) => match code {
                    HttpErrorCode::RateLimited => GithubErrorCode::RateLimited,
                    HttpErrorCode::ServerError => GithubErrorCode::Unavailable,
                    HttpErrorCode::NotFound => GithubErrorCode::RepositoryNotFound,
                    HttpErrorCode::UnexpectedStatus => GithubErrorCode::RequestRejected,
                    HttpErrorCode::InvalidResponse | HttpErrorCode::ResponseTooLarge => {
                        GithubErrorCode::InvalidResponse
                    }
                    HttpErrorCode::Internal
                    | HttpErrorCode::HashMismatch
                    | HttpErrorCode::SizeMismatch
                    | HttpErrorCode::InvalidUrl
                    | HttpErrorCode::TooManyRedirects
                    | HttpErrorCode::InsecureRedirect => GithubErrorCode::Internal,
                },
            },
        })
    }

    /// Parâmetros: `reason` no app recusado, `status` no pedido recusado, `seconds` no limite;
    /// os da `warden-http` nos de rede.
    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        match self {
            Self::LoginAppRejected { reason } => {
                params.insert("reason".to_owned(), reason.clone());
            }
            Self::Rejected { status, .. } => {
                params.insert("status".to_owned(), status.to_string());
            }
            Self::RateLimited {
                wait: Some(wait), ..
            } => {
                params.insert("seconds".to_owned(), wait.as_secs().to_string());
            }
            Self::Http(error) => return error.params(),
            _ => {}
        }
        params
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::LoginUnavailable | Self::NotSignedIn | Self::Cancelled => None,
            _ => Some(error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        match self {
            Self::RateLimited { .. } => true,
            Self::Http(error) => error.retryable(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(error: &Error) -> String {
        match error.code() {
            DomainCode::Domain(code) => serde_json::to_value(code)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned(),
            DomainCode::Core(code) => format!("core.{}", serde_json::to_value(code).unwrap()),
        }
    }

    #[test]
    fn codigos_e_parametros() {
        assert_eq!(code(&Error::LoginUnavailable), "LOGIN_UNAVAILABLE");
        assert_eq!(code(&Error::LoginCodeExpired), "LOGIN_CODE_EXPIRED");
        assert_eq!(code(&Error::LoginDenied), "LOGIN_DENIED");
        assert_eq!(code(&Error::NotSignedIn), "NOT_SIGNED_IN");
        let expired = Error::SessionExpired {
            reason: "HTTP 401".into(),
        };
        assert_eq!(code(&expired), "SESSION_EXPIRED");
        assert!(expired.needs_login());
        assert!(!Error::LoginDenied.needs_login());
        let rejected = Error::LoginAppRejected {
            reason: "device_flow_disabled".into(),
        };
        assert_eq!(code(&rejected), "LOGIN_APP_REJECTED");
        assert_eq!(rejected.params()["reason"], "device_flow_disabled");
        let limited = Error::RateLimited {
            url: "u".into(),
            wait: Some(Duration::from_secs(90)),
        };
        assert_eq!(code(&limited), "RATE_LIMITED");
        assert_eq!(limited.params()["seconds"], "90");
        assert!(limited.retryable());
        let rejected = Error::Rejected {
            url: "u".into(),
            status: 422,
            message: Some("Validation Failed".into()),
        };
        assert_eq!(code(&rejected), "REQUEST_REJECTED");
        assert_eq!(rejected.params()["status"], "422");
        assert_eq!(
            rejected.to_string(),
            "o GitHub recusou u (HTTP 422): Validation Failed"
        );
        assert_eq!(
            code(&Error::PermissionDenied { url: "u".into() }),
            "PERMISSION_DENIED"
        );
        assert_eq!(
            code(&Error::RepositoryNotFound { url: "u".into() }),
            "REPOSITORY_NOT_FOUND"
        );
        assert_eq!(code(&Error::Cancelled), "core.\"CANCELLED\"");
        assert_eq!(
            code(&Error::InvalidResponse("x".into())),
            "INVALID_RESPONSE"
        );
        assert_eq!(code(&Error::Internal("x".into())), "INTERNAL");
        assert!(Error::NotSignedIn.detail().is_none());
        assert!(rejected.detail().is_some());
    }

    #[test]
    fn erros_de_rede_viram_do_github_ou_comuns() {
        let server = Error::Http(warden_http::Error::ServerError {
            host: "api.github.com".into(),
            url: "u".into(),
            status: 502,
        });
        assert_eq!(code(&server), "UNAVAILABLE");
        let timeout = Error::Http(warden_http::Error::Timeout {
            host: "api.github.com".into(),
            url: "u".into(),
            seconds: 30,
        });
        assert_eq!(code(&timeout), "core.\"TIMEOUT\"");
        let cancelled = Error::Http(warden_http::Error::Cancelled);
        assert_eq!(code(&cancelled), "core.\"CANCELLED\"");
    }
}
