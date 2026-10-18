//! Erros do domínio `bisect` (ARCHITECTURE §5).
//!
//! [`BisectError`] implementa [`DomainError`]: cada variante tem um código estável de
//! [`BisectErrorCode`] e os parâmetros da frase em `apps/desktop/src/i18n/errors/bisect.ts`.
//! Falha de disco usa o código comum `IO` do domínio `core`.
//!
//! O código `INTERNAL` foi criado pela F0-05; a D-12 acrescentou os demais.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreErrorCode, DomainCode, DomainError};

/// Resultado com [`BisectError`].
pub type Result<T, E = BisectError> = std::result::Result<T, E>;

/// Códigos do domínio `bisect`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BisectErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// Não há busca do culpado guardada para o pack.
    NotFound,
    /// O pack não tem mods para procurar.
    NothingToSearch,
    /// O estado guardado (`state/bisect.json`) não pôde ser lido.
    StateUnreadable,
    /// A busca não está esperando a resposta do modo assistido.
    NoQuestion,
    /// A busca já terminou (ou não está pausada) e não pode continuar.
    NotResumable,
    /// A busca precisa entrar no mundo, não há mundo de teste gerado e a EULA do Minecraft
    /// (exigida para gerá-lo com o servidor vanilla) não foi aceita.
    EulaNeeded,
    /// O servidor vanilla não conseguiu gerar o mundo de teste (download, porta, tempo esgotado,
    /// servidor que fechou antes do "pronto").
    TestWorldFailed,
    /// A versão do Minecraft só entra no mundo pelo servidor local (D-13).
    NeedsServer,
}

/// Erro da busca do culpado.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BisectError {
    /// Nenhuma busca guardada.
    #[error("nenhuma busca do culpado guardada")]
    NotFound,
    /// Nada a procurar.
    #[error("o pack não tem mods para procurar")]
    NothingToSearch,
    /// Estado ilegível.
    #[error("estado da busca ilegível em {}: {reason}", short(path))]
    StateUnreadable {
        /// O arquivo.
        path: PathBuf,
        /// Motivo.
        reason: String,
    },
    /// Resposta sem pergunta.
    #[error("a busca não está esperando resposta")]
    NoQuestion,
    /// Não dá para continuar.
    #[error("a busca não pode continuar")]
    NotResumable,
    /// Sem mundo de teste e sem o aceite da EULA para gerá-lo.
    #[error("sem mundo de teste e sem o aceite da EULA para gerá-lo")]
    EulaNeeded,
    /// A geração do mundo de teste falhou.
    #[error("o mundo de teste do Minecraft {minecraft} não pôde ser gerado")]
    TestWorldFailed {
        /// Versão do jogo.
        minecraft: String,
    },
    /// Precisa do servidor local.
    #[error("o Minecraft {minecraft} só entra no mundo pelo servidor local")]
    NeedsServer {
        /// Versão do jogo.
        minecraft: String,
    },
    /// Falha de disco.
    #[error("falha ao gravar ou ler {}", short(path))]
    Io {
        /// O arquivo.
        path: PathBuf,
        /// Erro do sistema.
        #[source]
        source: io::Error,
    },
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

/// Só o nome do arquivo: o caminho inteiro tem a pasta do usuário (QUALITY §14).
fn short(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl BisectError {
    /// Atalho para [`BisectError::Io`].
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

impl DomainError for BisectError {
    type Code = BisectErrorCode;

    fn code(&self) -> DomainCode<BisectErrorCode> {
        DomainCode::Domain(match self {
            Self::NotFound => BisectErrorCode::NotFound,
            Self::NothingToSearch => BisectErrorCode::NothingToSearch,
            Self::StateUnreadable { .. } => BisectErrorCode::StateUnreadable,
            Self::NoQuestion => BisectErrorCode::NoQuestion,
            Self::NotResumable => BisectErrorCode::NotResumable,
            Self::EulaNeeded => BisectErrorCode::EulaNeeded,
            Self::TestWorldFailed { .. } => BisectErrorCode::TestWorldFailed,
            Self::NeedsServer { .. } => BisectErrorCode::NeedsServer,
            Self::Io { .. } => return DomainCode::Core(CoreErrorCode::Io),
            Self::Internal(_) => BisectErrorCode::Internal,
        })
    }

    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        match self {
            Self::NeedsServer { minecraft } | Self::TestWorldFailed { minecraft } => {
                params.insert("minecraft".to_owned(), minecraft.clone());
            }
            Self::Io { path, .. } | Self::StateUnreadable { path, .. } => {
                params.insert("path".to_owned(), short(path));
            }
            _ => {}
        }
        params
    }

    fn retryable(&self) -> bool {
        // Gerar o mundo de novo costuma dar certo (internet de volta, porta liberada).
        matches!(self, Self::Io { .. } | Self::TestWorldFailed { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_e_parametros_sem_a_pasta_do_usuario() {
        let error = BisectError::io(
            PathBuf::from("/home/fulano/dados/instances/x/state/bisect.json"),
            io::Error::other("x"),
        );
        assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Io));
        assert_eq!(error.params()["path"], "bisect.json");
        assert!(!error.detail().unwrap().contains("fulano"));
        assert!(error.retryable());
        let error = BisectError::NeedsServer {
            minecraft: "1.16.5".into(),
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(BisectErrorCode::NeedsServer)
        );
        assert_eq!(error.params()["minecraft"], "1.16.5");
        for (error, code) in [
            (BisectError::NotFound, BisectErrorCode::NotFound),
            (
                BisectError::NothingToSearch,
                BisectErrorCode::NothingToSearch,
            ),
            (BisectError::NoQuestion, BisectErrorCode::NoQuestion),
            (BisectError::NotResumable, BisectErrorCode::NotResumable),
            (BisectError::EulaNeeded, BisectErrorCode::EulaNeeded),
            (BisectError::Internal("x".into()), BisectErrorCode::Internal),
        ] {
            assert_eq!(error.code(), DomainCode::Domain(code));
            assert!(!error.retryable());
        }
        let error = BisectError::TestWorldFailed {
            minecraft: "1.21.1".into(),
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(BisectErrorCode::TestWorldFailed)
        );
        assert_eq!(error.params()["minecraft"], "1.21.1");
        assert!(error.retryable());
        let json = serde_json::to_string(&BisectErrorCode::EulaNeeded).unwrap();
        assert_eq!(json, "\"EULA_NEEDED\"");
    }
}
