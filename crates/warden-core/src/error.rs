//! Modelo de erros comum (ARCHITECTURE §5; ADR-0018).
//!
//! Cada crate tem o seu `enum Error` (`thiserror`) e o seu `enum <Domínio>ErrorCode`, e
//! implementa [`DomainError`] para que a `warden-app` monte o `AppError` que atravessa o IPC:
//! código estável, parâmetros da frase traduzida, detalhe técnico e se vale tentar de novo.
//!
//! Esta crate define também os códigos comuns a todas ([`CoreErrorCode`]): uma crate devolve
//! [`DomainCode::Core`] para cancelamento, tempo esgotado, falha de disco, caminho fora da
//! raiz e rede indisponível, e [`DomainCode::Domain`] para os próprios.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Códigos do domínio `core`, comuns a todas as crates. O código é contrato: renomear é
/// mudança de contrato; acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoreErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// A operação foi cancelada pelo usuário.
    Cancelled,
    /// A operação passou do tempo-limite.
    Timeout,
    /// Falha ao ler ou gravar no disco.
    Io,
    /// Um caminho vindo de fora tentou sair da pasta permitida.
    PathOutsideRoot,
    /// Sem conexão com a internet (ou o servidor não respondeu).
    NetworkUnavailable,
}

/// Código de um erro de domínio: um dos comuns ou um da própria crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainCode<C> {
    /// Código comum (`core`).
    Core(CoreErrorCode),
    /// Código da crate.
    Domain(C),
}

/// Contrato que o erro de cada crate implementa para virar `AppError` na `warden-app`.
pub trait DomainError: std::error::Error + Send + Sync + Sized + 'static {
    /// O `enum <Domínio>ErrorCode` da crate.
    type Code: Copy + std::fmt::Debug + Send + Sync + 'static;

    /// Código estável deste erro.
    fn code(&self) -> DomainCode<Self::Code>;

    /// Valores para a frase traduzida (nome do mod, versão, caminho…).
    fn params(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    /// Evidência técnica para "Detalhes técnicos". Nunca contém segredo (ARCHITECTURE §14).
    /// O padrão é a mensagem com a cadeia de causas.
    fn detail(&self) -> Option<String> {
        Some(error_chain(self))
    }

    /// Se repetir a mesma ação pode dar certo.
    fn retryable(&self) -> bool {
        false
    }
}

/// A mensagem do erro seguida das causas, uma por linha (`causa: ...`).
#[must_use]
pub fn error_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        // Escrever numa `String` não falha.
        let _ = write!(text, "\ncausa: {cause}");
        source = cause.source();
    }
    text
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// Cancelada pelo usuário.
    #[error("operação cancelada")]
    Cancelled,
    /// Passou do tempo-limite.
    #[error("tempo esgotado: {what} (limite de {seconds} s)")]
    Timeout {
        /// O que estava sendo esperado.
        what: String,
        /// O limite, em segundos.
        seconds: u64,
    },
    /// Falha de disco num caminho.
    #[error("falha ao {action} {path}")]
    Io {
        /// O que se tentava fazer ("gravar", "renomear para"…).
        action: &'static str,
        /// O caminho envolvido.
        path: PathBuf,
        /// O erro do sistema.
        #[source]
        source: io::Error,
    },
    /// Caminho relativo recusado por [`crate::resolve_inside`].
    #[error("caminho fora da pasta permitida: {path:?} ({reason})")]
    PathOutsideRoot {
        /// O caminho recebido (truncado em 260 caracteres).
        path: String,
        /// Por que foi recusado.
        reason: &'static str,
    },
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

impl CoreError {
    /// Atalho para [`CoreError::Io`].
    pub fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }
}

/// Esta crate não tem códigos além dos comuns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoDomainCode {}

impl DomainError for CoreError {
    type Code = NoDomainCode;

    fn code(&self) -> DomainCode<Self::Code> {
        DomainCode::Core(match self {
            Self::Cancelled => CoreErrorCode::Cancelled,
            Self::Timeout { .. } => CoreErrorCode::Timeout,
            Self::Io { .. } => CoreErrorCode::Io,
            Self::PathOutsideRoot { .. } => CoreErrorCode::PathOutsideRoot,
            Self::Internal(_) => CoreErrorCode::Internal,
        })
    }

    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        match self {
            Self::Io { path, .. } => {
                params.insert("path".to_owned(), path.display().to_string());
            }
            Self::PathOutsideRoot { path, .. } => {
                params.insert("path".to_owned(), path.clone());
            }
            Self::Timeout { seconds, .. } => {
                params.insert("seconds".to_owned(), seconds.to_string());
            }
            Self::Cancelled | Self::Internal(_) => {}
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
        matches!(self, Self::Timeout { .. } | Self::Io { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_em_maiusculas_com_sublinhado() {
        let json = serde_json::to_value([
            CoreErrorCode::Internal,
            CoreErrorCode::Cancelled,
            CoreErrorCode::Timeout,
            CoreErrorCode::Io,
            CoreErrorCode::PathOutsideRoot,
            CoreErrorCode::NetworkUnavailable,
        ])
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                "INTERNAL",
                "CANCELLED",
                "TIMEOUT",
                "IO",
                "PATH_OUTSIDE_ROOT",
                "NETWORK_UNAVAILABLE"
            ])
        );
    }

    #[test]
    fn io_leva_caminho_nos_parametros_e_causa_no_detalhe() {
        let error = CoreError::io(
            "gravar",
            "C:/pack/pack.toml",
            io::Error::new(io::ErrorKind::PermissionDenied, "acesso negado"),
        );
        assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Io));
        assert_eq!(error.params()["path"], "C:/pack/pack.toml");
        let detail = error.detail().unwrap();
        assert!(
            detail.starts_with("falha ao gravar C:/pack/pack.toml"),
            "{detail}"
        );
        assert!(detail.contains("\ncausa: acesso negado"), "{detail}");
        assert!(error.retryable());
    }

    #[test]
    fn cancelado_nao_tem_detalhe_nem_nova_tentativa() {
        let error = CoreError::Cancelled;
        assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Cancelled));
        assert_eq!(error.detail(), None);
        assert!(!error.retryable());
        assert!(error.params().is_empty());
    }

    #[test]
    fn demais_variantes() {
        let timeout = CoreError::Timeout {
            what: "packwiz refresh".into(),
            seconds: 120,
        };
        assert_eq!(timeout.code(), DomainCode::Core(CoreErrorCode::Timeout));
        assert_eq!(timeout.params()["seconds"], "120");
        assert!(timeout.retryable());

        let outside = CoreError::PathOutsideRoot {
            path: "../x".into(),
            reason: "sobe de pasta",
        };
        assert_eq!(
            outside.code(),
            DomainCode::Core(CoreErrorCode::PathOutsideRoot)
        );
        assert_eq!(outside.params()["path"], "../x");
        assert!(!outside.retryable());

        let internal = CoreError::Internal("invariante".into());
        assert_eq!(internal.code(), DomainCode::Core(CoreErrorCode::Internal));
        assert_eq!(internal.detail().unwrap(), "erro interno: invariante");
    }
}
