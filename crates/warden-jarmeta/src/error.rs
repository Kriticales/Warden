//! Erros da crate (ARCHITECTURE §5).
//!
//! Só falhas que impedem ler o jar inteiro viram [`Error`]: arquivo ilegível, zip inválido e
//! limites de segurança do arquivo de cima. Problemas num descritor ou num jar embutido viram
//! [`crate::Warning`] dentro do resultado, para nunca derrubar o diagnóstico (R3 §4.4).
//!
//! [`Error`] implementa `warden_core::DomainError`: falha de disco usa o código comum `IO` do
//! domínio `core`; os demais são do domínio `jarmeta` (frases em
//! `apps/desktop/src/i18n/errors/jarmeta.ts`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreErrorCode, DomainCode, DomainError, error_chain};

/// Códigos do domínio `jarmeta`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JarmetaErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
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

impl DomainError for Error {
    type Code = JarmetaErrorCode;

    fn code(&self) -> DomainCode<JarmetaErrorCode> {
        match self {
            Self::Read { .. } => DomainCode::Core(CoreErrorCode::Io),
            Self::InvalidArchive { .. } => DomainCode::Domain(JarmetaErrorCode::InvalidArchive),
            Self::LimitExceeded { .. } => DomainCode::Domain(JarmetaErrorCode::LimitExceeded),
        }
    }

    /// `path` (falha de disco), `limit` e `actual` (limite).
    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        match self {
            Self::Read { path, .. } => {
                params.insert("path".to_owned(), path.display().to_string());
            }
            Self::InvalidArchive { .. } => {}
            Self::LimitExceeded { limit, actual, .. } => {
                params.insert("limit".to_owned(), limit.to_string());
                params.insert("actual".to_owned(), actual.to_string());
            }
        }
        params
    }

    fn detail(&self) -> Option<String> {
        Some(error_chain(self))
    }

    /// Só a falha de disco pode passar tentando de novo (arquivo travado por outro programa).
    fn retryable(&self) -> bool {
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
        assert_eq!(read.code(), DomainCode::Core(CoreErrorCode::Io));
        assert!(
            read.params()
                .get("path")
                .is_some_and(|p| p.ends_with("sodium.jar"))
        );
        assert!(read.retryable());
        assert!(read.detail().is_some_and(|d| d.contains("negado")));

        let invalid = Error::InvalidArchive {
            detail: "fim do diretório central não encontrado".into(),
        };
        assert_eq!(
            invalid.code(),
            DomainCode::Domain(JarmetaErrorCode::InvalidArchive)
        );
        assert!(invalid.params().is_empty());
        assert!(!invalid.retryable());

        let limit = Error::LimitExceeded {
            kind: LimitKind::EntryCount,
            limit: 10,
            actual: 11,
        };
        assert_eq!(
            limit.code(),
            DomainCode::Domain(JarmetaErrorCode::LimitExceeded)
        );
        assert_eq!(limit.params().get("actual").map(String::as_str), Some("11"));
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
