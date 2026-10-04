//! Erros da crate (ARCHITECTURE §5).
//!
//! Só falhas que impedem ler o jar inteiro viram [`Error`]: arquivo ilegível, zip inválido e
//! limites de segurança do arquivo de cima. Problemas num descritor ou num jar embutido viram
//! [`crate::Warning`] dentro do resultado, para nunca derrubar o diagnóstico (R3 §4.4).
//!
//! A ligação com `warden_core::DomainError` (trait da F0-05) fica para quando ela estiver na
//! `main`; os métodos `code`, `params`, `detail` e `retryable` já seguem o contrato.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Código estável dos erros desta crate, para a frase traduzida na interface.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JarmetaErrorCode {
    /// Erro interno (bug).
    Internal,
    /// Não deu para ler o arquivo do disco.
    ReadFailed,
    /// O arquivo não é um zip/jar válido (corrompido, truncado ou de outro formato).
    InvalidArchive,
    /// O jar passa de um limite de segurança (tamanho, número de entradas).
    LimitExceeded,
}

/// O que passou do limite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LimitKind {
    /// Tamanho do arquivo do jar.
    FileSize,
    /// Número de entradas do zip.
    EntryCount,
}

impl LimitKind {
    fn describe(self) -> &'static str {
        match self {
            Self::FileSize => "o tamanho do arquivo",
            Self::EntryCount => "o número de entradas do zip",
        }
    }
}

/// Erro ao ler um jar.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Falha de leitura do disco.
    #[error("não deu para ler {}: {source}", path.display())]
    Read {
        /// Caminho do arquivo.
        path: PathBuf,
        /// Erro do sistema.
        #[source]
        source: std::io::Error,
    },
    /// Não é um zip válido.
    #[error("o arquivo não é um jar válido: {detail}")]
    InvalidArchive {
        /// Mensagem técnica da biblioteca de zip.
        detail: String,
    },
    /// Passou de um limite de segurança.
    #[error("{} passa do limite ({actual} > {limit})", kind.describe())]
    LimitExceeded {
        /// O que passou.
        kind: LimitKind,
        /// Limite configurado.
        limit: u64,
        /// Valor encontrado.
        actual: u64,
    },
}

impl Error {
    /// Código estável.
    #[must_use]
    pub fn code(&self) -> JarmetaErrorCode {
        match self {
            Self::Read { .. } => JarmetaErrorCode::ReadFailed,
            Self::InvalidArchive { .. } => JarmetaErrorCode::InvalidArchive,
            Self::LimitExceeded { .. } => JarmetaErrorCode::LimitExceeded,
        }
    }

    /// Valores para a frase traduzida.
    #[must_use]
    pub fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        match self {
            Self::Read { path, .. } => {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                params.insert("file".to_owned(), name);
            }
            Self::InvalidArchive { .. } => {}
            Self::LimitExceeded {
                kind,
                limit,
                actual,
            } => {
                let kind = serde_json::to_value(kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                params.insert("kind".to_owned(), kind);
                params.insert("limit".to_owned(), limit.to_string());
                params.insert("actual".to_owned(), actual.to_string());
            }
        }
        params
    }

    /// Evidência técnica para "Detalhes técnicos".
    #[must_use]
    pub fn detail(&self) -> Option<String> {
        Some(self.to_string())
    }

    /// Nenhum destes erros melhora tentando de novo, exceto a leitura do disco.
    #[must_use]
    pub fn retryable(&self) -> bool {
        matches!(self, Self::Read { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_e_parametros() {
        let read = Error::Read {
            path: PathBuf::from("mods").join("sodium.jar"),
            source: std::io::Error::other("negado"),
        };
        assert_eq!(read.code(), JarmetaErrorCode::ReadFailed);
        assert_eq!(
            read.params().get("file").map(String::as_str),
            Some("sodium.jar")
        );
        assert!(read.retryable());
        assert!(read.detail().is_some_and(|d| d.contains("negado")));

        let invalid = Error::InvalidArchive {
            detail: "fim do diretório central não encontrado".into(),
        };
        assert_eq!(invalid.code(), JarmetaErrorCode::InvalidArchive);
        assert!(invalid.params().is_empty());
        assert!(!invalid.retryable());

        let limit = Error::LimitExceeded {
            kind: LimitKind::EntryCount,
            limit: 10,
            actual: 11,
        };
        assert_eq!(limit.code(), JarmetaErrorCode::LimitExceeded);
        assert_eq!(
            limit.params().get("kind").map(String::as_str),
            Some("entryCount")
        );
        assert!(limit.to_string().contains("11 > 10"));
        let size = Error::LimitExceeded {
            kind: LimitKind::FileSize,
            limit: 1,
            actual: 2,
        };
        assert!(size.to_string().contains("tamanho"));
    }

    #[test]
    fn codigo_serializa_em_maiusculas() {
        let json = serde_json::to_string(&JarmetaErrorCode::InvalidArchive).expect("json");
        assert_eq!(json, "\"INVALID_ARCHIVE\"");
    }
}
