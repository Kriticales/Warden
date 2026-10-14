//! Erros do domínio `scripts` (ARCHITECTURE §5).
//!
//! [`Error`] implementa [`DomainError`]: cada variante tem um código estável de
//! [`ScriptsErrorCode`] e a frase em `apps/desktop/src/i18n/errors/scripts.ts`. Nenhuma variante
//! carrega o token do servidor web do KubeJS: os detalhes técnicos levam só o endereço local e o
//! status HTTP.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use warden_core::{DomainCode, DomainError};

/// Resultado com [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Códigos do domínio `scripts`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScriptsErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O servidor web do KubeJS 7 não respondeu na instância de teste.
    WebServerUnavailable,
    /// O servidor web do KubeJS recusou o pedido (token diferente ou outro erro HTTP).
    WebServerRejected,
    /// O console do servidor local não aceitou o comando.
    ServerConsoleFailed,
    /// O VS Code não foi encontrado neste computador.
    VsCodeNotFound,
    /// O VS Code não abriu.
    VsCodeFailed,
}

/// Erro do domínio `scripts`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Nenhum servidor web do KubeJS respondeu nas portas da configuração.
    #[error("o servidor web do KubeJS não respondeu em 127.0.0.1 (portas {first} a {last})")]
    WebServerUnavailable {
        /// Primeira porta tentada.
        first: u16,
        /// Última porta tentada.
        last: u16,
    },
    /// O servidor web respondeu com erro.
    #[error("o servidor web do KubeJS respondeu {status} em {path}")]
    WebServerRejected {
        /// Rota pedida (sem o token).
        path: String,
        /// Status HTTP.
        status: u16,
    },
    /// O console do servidor local recusou o comando.
    #[error("o console do servidor local não aceitou o comando: {detail}")]
    ServerConsole {
        /// Detalhe técnico.
        detail: String,
    },
    /// O VS Code não está no PATH nem nos caminhos padrão.
    #[error("o comando `code` do VS Code não foi encontrado")]
    VsCodeNotFound,
    /// O VS Code não abriu.
    #[error("o VS Code não abriu: {detail}")]
    VsCodeFailed {
        /// Detalhe técnico.
        detail: String,
    },
    /// Bug.
    #[error("{0}")]
    Internal(String),
}

impl DomainError for Error {
    type Code = ScriptsErrorCode;

    fn code(&self) -> DomainCode<ScriptsErrorCode> {
        DomainCode::Domain(match self {
            Self::WebServerUnavailable { .. } => ScriptsErrorCode::WebServerUnavailable,
            Self::WebServerRejected { .. } => ScriptsErrorCode::WebServerRejected,
            Self::ServerConsole { .. } => ScriptsErrorCode::ServerConsoleFailed,
            Self::VsCodeNotFound => ScriptsErrorCode::VsCodeNotFound,
            Self::VsCodeFailed { .. } => ScriptsErrorCode::VsCodeFailed,
            Self::Internal(_) => ScriptsErrorCode::Internal,
        })
    }

    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        if let Self::WebServerRejected { status, .. } = self {
            params.insert("status".to_owned(), status.to_string());
        }
        params
    }

    fn retryable(&self) -> bool {
        matches!(
            self,
            Self::WebServerUnavailable { .. } | Self::ServerConsole { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_e_parametros() {
        let cases = [
            (
                Error::WebServerUnavailable {
                    first: 61423,
                    last: 61432,
                },
                ScriptsErrorCode::WebServerUnavailable,
            ),
            (
                Error::WebServerRejected {
                    path: "/api/reload/server".into(),
                    status: 403,
                },
                ScriptsErrorCode::WebServerRejected,
            ),
            (
                Error::ServerConsole {
                    detail: "fechado".into(),
                },
                ScriptsErrorCode::ServerConsoleFailed,
            ),
            (Error::VsCodeNotFound, ScriptsErrorCode::VsCodeNotFound),
            (
                Error::VsCodeFailed { detail: "x".into() },
                ScriptsErrorCode::VsCodeFailed,
            ),
            (Error::Internal("x".into()), ScriptsErrorCode::Internal),
        ];
        for (error, code) in cases {
            assert_eq!(error.code(), DomainCode::Domain(code));
        }
        let rejected = Error::WebServerRejected {
            path: "/api/reload/server".into(),
            status: 403,
        };
        assert_eq!(rejected.params()["status"], "403");
        assert!(!rejected.retryable());
        assert!(Error::WebServerUnavailable { first: 1, last: 2 }.retryable());
    }
}
