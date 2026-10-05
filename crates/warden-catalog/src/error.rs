//! Erros do domínio `catalog` (ARCHITECTURE §5).
//!
//! As falhas de rede chegam da `warden-http` e viram códigos do catálogo com a fonte
//! ([`Source`]: Mojang, Fabric, Forge ou NeoForge) no parâmetro `source`, para a frase dizer
//! quem falhou; sem conexão, tempo esgotado e cancelamento continuam com os códigos comuns do
//! domínio `core`. As frases ficam em `apps/desktop/src/i18n/errors/catalog.ts`.
//!
//! Uma fonte fora do ar só vira erro quando não há nada no cache: com cache, o catálogo
//! devolve a lista guardada e a data dela ([`crate::Freshness`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use warden_core::{DomainCode, DomainError, error_chain};
use warden_http::HttpErrorCode;

use crate::model::Source;

/// Códigos do domínio `catalog`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// A fonte (`params.source`) está fora do ar ou com erro (5xx) e não há lista no cache.
    SourceUnavailable,
    /// A fonte limitou as requisições (429) por mais tempo do que o Warden espera sozinho.
    RateLimited,
    /// A fonte recusou o pedido ou não achou o endereço (4xx).
    RequestRejected,
    /// A fonte respondeu algo que o Warden não entende.
    InvalidResponse,
    /// A versão do Minecraft (`params.id`) não existe no manifesto da Mojang.
    VersionNotFound,
    /// O JSON da versão baixado não bate com o `sha1` do manifesto (`params.id`).
    VersionJsonCorrupted,
    /// O cache local do catálogo (`metadata.sqlite`) não pôde ser lido ou gravado.
    CacheUnavailable,
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Falha da requisição a uma fonte.
    #[error("{origin}: {error}")]
    Http {
        /// Quem foi consultado.
        origin: Source,
        /// O erro do cliente HTTP.
        #[source]
        error: warden_http::Error,
    },
    /// Resposta que não pôde ser usada.
    #[error("resposta inválida de {origin}: {reason}")]
    InvalidResponse {
        /// Quem respondeu.
        origin: Source,
        /// O que estava errado.
        reason: String,
    },
    /// Versão do Minecraft inexistente no manifesto.
    #[error("a versão {id:?} do Minecraft não está no manifesto da Mojang")]
    VersionNotFound {
        /// A versão pedida.
        id: String,
    },
    /// O JSON da versão não tem o `sha1` do manifesto.
    #[error("JSON da versão {id:?} corrompido: sha1 esperado {expected}, obtido {actual}")]
    VersionJsonCorrupted {
        /// A versão.
        id: String,
        /// O `sha1` do manifesto.
        expected: String,
        /// O `sha1` do que chegou.
        actual: String,
    },
    /// Falha do cache local.
    #[error("cache do catálogo: falha ao {action}")]
    Cache {
        /// O que se tentava fazer.
        action: &'static str,
        /// Erro do SQLite.
        #[source]
        source: rusqlite::Error,
    },
    /// Falha de disco ao abrir o cache.
    #[error("cache do catálogo: falha ao {action} {}", .path.display())]
    CacheIo {
        /// O que se tentava fazer.
        action: &'static str,
        /// Caminho.
        path: std::path::PathBuf,
        /// Erro do sistema.
        #[source]
        source: std::io::Error,
    },
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

    pub(crate) fn http(origin: Source) -> impl FnOnce(warden_http::Error) -> Self {
        move |error| Self::Http { origin, error }
    }

    /// Se foi o cancelamento pedido pelo usuário (o único caso em que o catálogo não cai no
    /// cache).
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(
            self,
            Self::Http {
                error: warden_http::Error::Cancelled,
                ..
            }
        )
    }
}

impl DomainError for Error {
    type Code = CatalogErrorCode;

    fn code(&self) -> DomainCode<CatalogErrorCode> {
        DomainCode::Domain(match self {
            Self::InvalidResponse { .. } => CatalogErrorCode::InvalidResponse,
            Self::VersionNotFound { .. } => CatalogErrorCode::VersionNotFound,
            Self::VersionJsonCorrupted { .. } => CatalogErrorCode::VersionJsonCorrupted,
            Self::Cache { .. } | Self::CacheIo { .. } => CatalogErrorCode::CacheUnavailable,
            Self::Internal(_) => CatalogErrorCode::Internal,
            Self::Http { error, .. } => match error.code() {
                DomainCode::Core(code) => return DomainCode::Core(code),
                DomainCode::Domain(code) => match code {
                    HttpErrorCode::RateLimited => CatalogErrorCode::RateLimited,
                    HttpErrorCode::ServerError => CatalogErrorCode::SourceUnavailable,
                    HttpErrorCode::NotFound | HttpErrorCode::UnexpectedStatus => {
                        CatalogErrorCode::RequestRejected
                    }
                    HttpErrorCode::InvalidResponse | HttpErrorCode::ResponseTooLarge => {
                        CatalogErrorCode::InvalidResponse
                    }
                    HttpErrorCode::Internal
                    | HttpErrorCode::HashMismatch
                    | HttpErrorCode::SizeMismatch
                    | HttpErrorCode::InvalidUrl
                    | HttpErrorCode::TooManyRedirects
                    | HttpErrorCode::InsecureRedirect => CatalogErrorCode::Internal,
                },
            },
        })
    }

    /// Parâmetros: `source` (Mojang, Fabric, Forge, NeoForge) nos erros de uma fonte, com os
    /// da `warden-http` (`status`, `seconds`…); `id` nos erros de uma versão.
    fn params(&self) -> BTreeMap<String, String> {
        match self {
            Self::Http { origin, error } => {
                let mut params = error.params();
                params.insert("source".to_owned(), origin.to_string());
                params
            }
            Self::InvalidResponse { origin, .. } => {
                BTreeMap::from([("source".to_owned(), origin.to_string())])
            }
            Self::VersionNotFound { id } | Self::VersionJsonCorrupted { id, .. } => {
                BTreeMap::from([("id".to_owned(), id.clone())])
            }
            Self::Cache { .. } | Self::CacheIo { .. } | Self::Internal(_) => BTreeMap::new(),
        }
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::Http { origin, error } => Some(format!(
                "{origin}: {}",
                error.detail().unwrap_or_else(|| error.to_string())
            )),
            _ => Some(error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        match self {
            Self::Http { error, .. } => error.retryable(),
            Self::Cache { .. }
            | Self::CacheIo { .. }
            | Self::InvalidResponse { .. }
            | Self::VersionJsonCorrupted { .. } => true,
            Self::VersionNotFound { .. } | Self::Internal(_) => false,
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
            CatalogErrorCode::Internal,
            CatalogErrorCode::SourceUnavailable,
            CatalogErrorCode::RateLimited,
            CatalogErrorCode::RequestRejected,
            CatalogErrorCode::InvalidResponse,
            CatalogErrorCode::VersionNotFound,
            CatalogErrorCode::VersionJsonCorrupted,
            CatalogErrorCode::CacheUnavailable,
        ])
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                "INTERNAL",
                "SOURCE_UNAVAILABLE",
                "RATE_LIMITED",
                "REQUEST_REJECTED",
                "INVALID_RESPONSE",
                "VERSION_NOT_FOUND",
                "VERSION_JSON_CORRUPTED",
                "CACHE_UNAVAILABLE"
            ])
        );
    }

    fn http(error: warden_http::Error) -> Error {
        Error::Http {
            origin: Source::Forge,
            error,
        }
    }

    #[test]
    fn erros_de_rede_viram_codigos_do_catalogo_com_a_fonte() {
        let host = || "maven.minecraftforge.net".to_owned();
        let url = || "https://maven.minecraftforge.net/x".to_owned();
        let cases = [
            (
                http(warden_http::Error::RateLimited {
                    host: host(),
                    url: url(),
                    wait: Some(Duration::from_secs(400)),
                }),
                DomainCode::Domain(CatalogErrorCode::RateLimited),
                true,
            ),
            (
                http(warden_http::Error::ServerError {
                    host: host(),
                    url: url(),
                    status: 503,
                }),
                DomainCode::Domain(CatalogErrorCode::SourceUnavailable),
                true,
            ),
            (
                http(warden_http::Error::NotFound {
                    host: host(),
                    url: url(),
                    status: 404,
                }),
                DomainCode::Domain(CatalogErrorCode::RequestRejected),
                false,
            ),
            (
                http(warden_http::Error::UnexpectedStatus {
                    host: host(),
                    url: url(),
                    status: 400,
                    body: None,
                }),
                DomainCode::Domain(CatalogErrorCode::RequestRejected),
                false,
            ),
            (
                http(warden_http::Error::ResponseTooLarge {
                    host: host(),
                    url: url(),
                    limit: 1,
                }),
                DomainCode::Domain(CatalogErrorCode::InvalidResponse),
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
            ),
            (
                http(warden_http::Error::Cancelled),
                DomainCode::Core(CoreErrorCode::Cancelled),
                false,
            ),
            (
                http(warden_http::Error::Internal("x".into())),
                DomainCode::Domain(CatalogErrorCode::Internal),
                false,
            ),
        ];
        for (error, code, retryable) in cases {
            assert_eq!(error.code(), code, "{error}");
            assert_eq!(error.retryable(), retryable, "{error}");
            assert_eq!(error.params()["source"], "Forge", "{error}");
            assert!(error.detail().unwrap().starts_with("Forge: "), "{error}");
        }
        assert!(http(warden_http::Error::Cancelled).is_cancelled());
        assert!(
            !http(warden_http::Error::Timeout {
                host: host(),
                url: url(),
                seconds: 1
            })
            .is_cancelled()
        );
    }

    #[test]
    fn erros_de_versao_levam_o_id() {
        let error = Error::VersionNotFound { id: "9.9".into() };
        assert_eq!(
            error.code(),
            DomainCode::Domain(CatalogErrorCode::VersionNotFound)
        );
        assert_eq!(error.params()["id"], "9.9");
        assert!(!error.retryable());
        let error = Error::VersionJsonCorrupted {
            id: "1.7.10".into(),
            expected: "aa".into(),
            actual: "bb".into(),
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(CatalogErrorCode::VersionJsonCorrupted)
        );
        assert_eq!(error.params()["id"], "1.7.10");
        assert!(error.retryable());
        assert!(error.detail().unwrap().contains("bb"));
    }

    #[test]
    fn outros_erros() {
        let error = Error::InvalidResponse {
            origin: Source::Mojang,
            reason: "faltou o campo".into(),
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(CatalogErrorCode::InvalidResponse)
        );
        assert_eq!(error.params()["source"], "Mojang");
        assert!(error.detail().unwrap().contains("faltou o campo"));
        let error = Error::cache("ler")(rusqlite::Error::InvalidQuery);
        assert_eq!(
            error.code(),
            DomainCode::Domain(CatalogErrorCode::CacheUnavailable)
        );
        assert!(error.retryable());
        assert!(error.params().is_empty());
        let error = Error::CacheIo {
            action: "criar a pasta",
            path: "x".into(),
            source: std::io::Error::other("disco"),
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(CatalogErrorCode::CacheUnavailable)
        );
        let error = Error::Internal("bug".into());
        assert_eq!(error.code(), DomainCode::Domain(CatalogErrorCode::Internal));
        assert!(!error.retryable());
    }
}
