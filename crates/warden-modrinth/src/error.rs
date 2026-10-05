//! Erros do domínio `modrinth` (ARCHITECTURE §5).
//!
//! As falhas de rede chegam da `warden-http` e são traduzidas para códigos do Modrinth, para a
//! frase dizer "o Modrinth" em vez de um servidor genérico; sem conexão, tempo esgotado e
//! cancelamento continuam com os códigos comuns do domínio `core`. As frases ficam em
//! `apps/desktop/src/i18n/errors/modrinth.ts`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use warden_core::{DomainCode, DomainError, error_chain};
use warden_http::HttpErrorCode;

/// Códigos do domínio `modrinth`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModrinthErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O projeto não existe (ou não é público).
    ProjectNotFound,
    /// A versão não existe.
    VersionNotFound,
    /// O Modrinth não encontrou o endereço pedido.
    NotFound,
    /// O Modrinth limitou as requisições (429) por mais tempo do que o Warden espera sozinho.
    RateLimited,
    /// O Modrinth está fora do ar ou com erro (5xx).
    Unavailable,
    /// O Modrinth recusou o pedido (4xx).
    RequestRejected,
    /// O Modrinth respondeu algo que o Warden não entende.
    InvalidResponse,
    /// O cache local do Modrinth (`metadata.sqlite`) não pôde ser lido ou gravado.
    CacheUnavailable,
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Projeto inexistente.
    #[error("projeto {id:?} não encontrado no Modrinth")]
    ProjectNotFound {
        /// ID ou slug pedido.
        id: String,
    },
    /// Versão inexistente.
    #[error("versão {id:?} não encontrada no Modrinth")]
    VersionNotFound {
        /// ID pedido.
        id: String,
    },
    /// Falha da requisição.
    #[error(transparent)]
    Http(#[from] warden_http::Error),
    /// Falha do cache local.
    #[error("cache do Modrinth: falha ao {action}")]
    Cache {
        /// O que se tentava fazer.
        action: &'static str,
        /// Erro do SQLite.
        #[source]
        source: rusqlite::Error,
    },
    /// Falha de disco ao abrir o cache.
    #[error("cache do Modrinth: falha ao {action} {}", .path.display())]
    CacheIo {
        /// O que se tentava fazer.
        action: &'static str,
        /// Caminho.
        path: std::path::PathBuf,
        /// Erro do sistema.
        #[source]
        source: std::io::Error,
    },
    /// Resposta que não pôde ser usada.
    #[error("resposta inválida do Modrinth: {0}")]
    InvalidResponse(String),
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

/// Atalho para os resultados da crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn cache(action: &'static str) -> impl FnOnce(rusqlite::Error) -> Self {
        move |source| Self::Cache { action, source }
    }

    /// Se a falha é de conexão com o Modrinth (sem rede, tempo esgotado, limite ou servidor
    /// fora do ar): nesses casos o cliente usa o que houver no cache, mesmo vencido.
    #[must_use]
    pub fn is_offline(&self) -> bool {
        matches!(
            self,
            Self::Http(
                warden_http::Error::Network { .. }
                    | warden_http::Error::Timeout { .. }
                    | warden_http::Error::RateLimited { .. }
                    | warden_http::Error::ServerError { .. }
            )
        )
    }
}

impl DomainError for Error {
    type Code = ModrinthErrorCode;

    fn code(&self) -> DomainCode<ModrinthErrorCode> {
        DomainCode::Domain(match self {
            Self::ProjectNotFound { .. } => ModrinthErrorCode::ProjectNotFound,
            Self::VersionNotFound { .. } => ModrinthErrorCode::VersionNotFound,
            Self::Cache { .. } | Self::CacheIo { .. } => ModrinthErrorCode::CacheUnavailable,
            Self::InvalidResponse(_) => ModrinthErrorCode::InvalidResponse,
            Self::Internal(_) => ModrinthErrorCode::Internal,
            Self::Http(error) => match error.code() {
                DomainCode::Core(code) => return DomainCode::Core(code),
                DomainCode::Domain(code) => match code {
                    HttpErrorCode::RateLimited => ModrinthErrorCode::RateLimited,
                    HttpErrorCode::ServerError => ModrinthErrorCode::Unavailable,
                    HttpErrorCode::NotFound => ModrinthErrorCode::NotFound,
                    HttpErrorCode::UnexpectedStatus => ModrinthErrorCode::RequestRejected,
                    HttpErrorCode::InvalidResponse | HttpErrorCode::ResponseTooLarge => {
                        ModrinthErrorCode::InvalidResponse
                    }
                    HttpErrorCode::Internal
                    | HttpErrorCode::HashMismatch
                    | HttpErrorCode::SizeMismatch
                    | HttpErrorCode::InvalidUrl
                    | HttpErrorCode::TooManyRedirects
                    | HttpErrorCode::InsecureRedirect => ModrinthErrorCode::Internal,
                },
            },
        })
    }

    /// Parâmetros: `id` (projeto ou versão) nos "não encontrado"; os da `warden-http`
    /// (`status`, `seconds`…) nos de rede.
    fn params(&self) -> BTreeMap<String, String> {
        match self {
            Self::ProjectNotFound { id } | Self::VersionNotFound { id } => {
                BTreeMap::from([("id".to_owned(), id.clone())])
            }
            Self::Http(error) => error.params(),
            _ => BTreeMap::new(),
        }
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::Http(error) => error.detail(),
            _ => Some(error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        match self {
            Self::Http(error) => error.retryable(),
            Self::Cache { .. } | Self::CacheIo { .. } => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use warden_core::CoreErrorCode;

    use super::*;

    #[test]
    fn codigos_em_maiusculas_com_sublinhado() {
        let json = serde_json::to_value([
            ModrinthErrorCode::Internal,
            ModrinthErrorCode::ProjectNotFound,
            ModrinthErrorCode::VersionNotFound,
            ModrinthErrorCode::NotFound,
            ModrinthErrorCode::RateLimited,
            ModrinthErrorCode::Unavailable,
            ModrinthErrorCode::RequestRejected,
            ModrinthErrorCode::InvalidResponse,
            ModrinthErrorCode::CacheUnavailable,
        ])
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                "INTERNAL",
                "PROJECT_NOT_FOUND",
                "VERSION_NOT_FOUND",
                "NOT_FOUND",
                "RATE_LIMITED",
                "UNAVAILABLE",
                "REQUEST_REJECTED",
                "INVALID_RESPONSE",
                "CACHE_UNAVAILABLE"
            ])
        );
    }

    fn http(error: warden_http::Error) -> Error {
        Error::Http(error)
    }

    #[test]
    fn erros_de_rede_viram_codigos_do_modrinth() {
        let host = || "api.modrinth.com".to_owned();
        let url = || "https://api.modrinth.com/v2/search".to_owned();
        let cases = [
            (
                http(warden_http::Error::RateLimited {
                    host: host(),
                    url: url(),
                    wait: Some(Duration::from_secs(400)),
                }),
                DomainCode::Domain(ModrinthErrorCode::RateLimited),
                true,
                true,
            ),
            (
                http(warden_http::Error::ServerError {
                    host: host(),
                    url: url(),
                    status: 503,
                }),
                DomainCode::Domain(ModrinthErrorCode::Unavailable),
                true,
                true,
            ),
            (
                http(warden_http::Error::NotFound {
                    host: host(),
                    url: url(),
                    status: 404,
                }),
                DomainCode::Domain(ModrinthErrorCode::NotFound),
                false,
                false,
            ),
            (
                http(warden_http::Error::UnexpectedStatus {
                    host: host(),
                    url: url(),
                    status: 400,
                    body: None,
                }),
                DomainCode::Domain(ModrinthErrorCode::RequestRejected),
                false,
                false,
            ),
            (
                http(warden_http::Error::ResponseTooLarge {
                    host: host(),
                    url: url(),
                    limit: 1,
                }),
                DomainCode::Domain(ModrinthErrorCode::InvalidResponse),
                false,
                false,
            ),
            (
                http(warden_http::Error::Timeout {
                    host: host(),
                    url: url(),
                    seconds: 30,
                }),
                DomainCode::Core(CoreErrorCode::Timeout),
                true,
                true,
            ),
            (
                http(warden_http::Error::Cancelled),
                DomainCode::Core(CoreErrorCode::Cancelled),
                false,
                false,
            ),
            (
                http(warden_http::Error::Internal("x".into())),
                DomainCode::Domain(ModrinthErrorCode::Internal),
                false,
                false,
            ),
        ];
        for (error, code, retryable, offline) in cases {
            assert_eq!(error.code(), code, "{error}");
            assert_eq!(error.retryable(), retryable, "{error}");
            assert_eq!(error.is_offline(), offline, "{error}");
        }
    }

    #[test]
    fn nao_encontrado_leva_o_id() {
        let error = Error::ProjectNotFound {
            id: "sodium".into(),
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(ModrinthErrorCode::ProjectNotFound)
        );
        assert_eq!(error.params()["id"], "sodium");
        assert!(!error.retryable());
        let error = Error::VersionNotFound { id: "abc".into() };
        assert_eq!(error.params()["id"], "abc");
        let error = http(warden_http::Error::RateLimited {
            host: "api.modrinth.com".into(),
            url: "u".into(),
            wait: Some(Duration::from_secs(9)),
        });
        assert_eq!(error.params()["seconds"], "9");
        assert!(error.detail().unwrap().contains("429"));
        let error = Error::InvalidResponse("faltou o campo".into());
        assert_eq!(
            error.code(),
            DomainCode::Domain(ModrinthErrorCode::InvalidResponse)
        );
        assert!(error.detail().unwrap().contains("faltou o campo"));
        let error = Error::cache("ler")(rusqlite::Error::InvalidQuery);
        assert_eq!(
            error.code(),
            DomainCode::Domain(ModrinthErrorCode::CacheUnavailable)
        );
        assert!(error.retryable());
    }
}
