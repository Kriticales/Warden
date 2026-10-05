//! Erros da crate (ARCHITECTURE §5).
//!
//! [`DiagnosticsError`] implementa [`DomainError`]: cada variante tem um código estável de
//! [`DiagnosticsErrorCode`] e os parâmetros da frase em
//! `apps/desktop/src/i18n/errors/diagnostics.ts`. Falha de disco usa o código comum `IO` do
//! domínio `core`.
//!
//! O código `INTERNAL` foi criado pela F0-05; a D-02 acrescentou os demais.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreErrorCode, DomainCode, DomainError};

/// Resultado com [`DiagnosticsError`].
pub type Result<T, E = DiagnosticsError> = std::result::Result<T, E>;

/// Códigos do domínio `diagnostics`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticsErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// A pasta da sessão de teste não existe mais (podada ou apagada por fora).
    SessionNotFound,
    /// Uma regra extra de redação não é uma expressão regular válida.
    InvalidRedactionRule,
}

/// Erro da análise pós-crash e da redação.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DiagnosticsError {
    /// A pasta da sessão (ou a pasta do jogo) não existe.
    #[error("a pasta da sessão não existe: {path}")]
    SessionNotFound {
        /// Caminho procurado.
        path: PathBuf,
    },

    /// Falha ao ler um arquivo da sessão.
    #[error("falha ao ler {path}")]
    Io {
        /// Caminho do arquivo.
        path: PathBuf,
        /// Erro do sistema.
        #[source]
        source: io::Error,
    },

    /// Regra extra de redação inválida (gancho 1.1, ADR-0039).
    #[error("regra de redação inválida {pattern:?}: {reason}")]
    InvalidRedactionRule {
        /// Expressão recebida (truncada em 200 caracteres).
        pattern: String,
        /// Motivo dado pela biblioteca de expressões regulares.
        reason: String,
    },

    /// Invariante quebrada (por exemplo, o catálogo embutido não compila; os testes impedem).
    #[error("erro interno: {0}")]
    Internal(String),
}

impl DiagnosticsError {
    /// Atalho para [`DiagnosticsError::Io`].
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl DomainError for DiagnosticsError {
    type Code = DiagnosticsErrorCode;

    fn code(&self) -> DomainCode<DiagnosticsErrorCode> {
        match self {
            Self::SessionNotFound { .. } => {
                DomainCode::Domain(DiagnosticsErrorCode::SessionNotFound)
            }
            Self::Io { .. } => DomainCode::Core(CoreErrorCode::Io),
            Self::InvalidRedactionRule { .. } => {
                DomainCode::Domain(DiagnosticsErrorCode::InvalidRedactionRule)
            }
            Self::Internal(_) => DomainCode::Domain(DiagnosticsErrorCode::Internal),
        }
    }

    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        match self {
            Self::SessionNotFound { path } | Self::Io { path, .. } => {
                params.insert("path".to_owned(), path.display().to_string());
            }
            Self::InvalidRedactionRule { pattern, .. } => {
                params.insert("pattern".to_owned(), pattern.clone());
            }
            Self::Internal(_) => {}
        }
        params
    }

    fn retryable(&self) -> bool {
        matches!(self, Self::Io { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_em_maiusculas_com_sublinhado() {
        let json = serde_json::to_value([
            DiagnosticsErrorCode::Internal,
            DiagnosticsErrorCode::SessionNotFound,
            DiagnosticsErrorCode::InvalidRedactionRule,
        ])
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!(["INTERNAL", "SESSION_NOT_FOUND", "INVALID_REDACTION_RULE"])
        );
    }

    #[test]
    fn cada_variante_tem_codigo_e_parametros() {
        let session = DiagnosticsError::SessionNotFound {
            path: PathBuf::from("sessao"),
        };
        assert_eq!(
            session.code(),
            DomainCode::Domain(DiagnosticsErrorCode::SessionNotFound)
        );
        assert_eq!(session.params()["path"], "sessao");
        assert!(!session.retryable());

        let io = DiagnosticsError::io("logs/latest.log", io::Error::other("negado"));
        assert_eq!(io.code(), DomainCode::Core(CoreErrorCode::Io));
        assert!(io.retryable());
        assert!(io.detail().unwrap().contains("causa: negado"));

        let rule = DiagnosticsError::InvalidRedactionRule {
            pattern: "(".to_owned(),
            reason: "parêntese aberto".to_owned(),
        };
        assert_eq!(
            rule.code(),
            DomainCode::Domain(DiagnosticsErrorCode::InvalidRedactionRule)
        );
        assert_eq!(rule.params()["pattern"], "(");

        let internal = DiagnosticsError::Internal("x".to_owned());
        assert_eq!(
            internal.code(),
            DomainCode::Domain(DiagnosticsErrorCode::Internal)
        );
        assert!(internal.params().is_empty());
    }
}
