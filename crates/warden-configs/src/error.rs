//! Erros da crate.
//!
//! Cada variante tem um código estável ([`ConfigError::code`]) no estilo da ARCHITECTURE §5.
//! Quando a `warden-core` ganhar o trait `DomainError` (tarefa F0-05), este tipo passa a
//! implementá-lo com os mesmos códigos.

use serde::{Deserialize, Serialize};

use crate::format::ConfigFormat;
use crate::path::KeyPath;

/// Resultado com [`ConfigError`].
pub type Result<T, E = ConfigError> = std::result::Result<T, E>;

/// Códigos do domínio `configs`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfigsErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
}

/// Erro ao ler, editar ou comparar uma config.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// O arquivo é maior que o limite do editor estruturado
    /// ([`MAX_STRUCTURED_BYTES`](crate::MAX_STRUCTURED_BYTES)); ele continua editável como texto.
    #[error("arquivo com {size} bytes passa do limite de {limit} bytes do editor estruturado")]
    TooLarge {
        /// Tamanho do arquivo, em bytes.
        size: usize,
        /// Limite, em bytes.
        limit: usize,
    },

    /// O arquivo não é UTF-8 válido (com ou sem BOM).
    #[error("o arquivo não é texto UTF-8 válido (byte inválido na posição {offset})")]
    NotUtf8 {
        /// Posição do primeiro byte inválido.
        offset: usize,
    },

    /// O texto não segue o formato esperado.
    #[error("{format} inválido na linha {line}, coluna {column}: {message}")]
    Parse {
        /// Formato que se tentou ler.
        format: ConfigFormat,
        /// Linha (começa em 1).
        line: usize,
        /// Coluna em caracteres (começa em 1).
        column: usize,
        /// Descrição técnica do problema (em inglês quando vem da biblioteca de leitura).
        message: String,
    },

    /// Nenhuma chave com esse caminho existe no arquivo.
    #[error("a chave {path} não existe no arquivo")]
    PathNotFound {
        /// Caminho pedido.
        path: KeyPath,
    },

    /// O caminho existe, mas não é um valor editável (seção, tabela ou lista de objetos).
    #[error("a chave {path} não é um valor editável: {reason}")]
    NotEditable {
        /// Caminho pedido.
        path: KeyPath,
        /// Motivo.
        reason: String,
    },

    /// O novo valor não pode ser gravado nessa chave neste formato.
    #[error("valor inválido para {path}: {reason}")]
    InvalidValue {
        /// Caminho pedido.
        path: KeyPath,
        /// Motivo.
        reason: String,
    },

    /// A mesma chave apareceu mais de uma vez na lista de edições.
    #[error("a chave {path} aparece mais de uma vez nas edições")]
    DuplicateEdit {
        /// Caminho repetido.
        path: KeyPath,
    },

    /// A releitura do resultado de uma edição não confirmou a mudança pedida (e só ela).
    /// Nada é gravado; é um defeito do Warden para aquele arquivo.
    #[error("a edição não pôde ser confirmada na releitura: {detail}")]
    EditNotConfirmed {
        /// O que a releitura encontrou.
        detail: String,
    },
}

impl ConfigError {
    /// Código estável do erro (`SCREAMING_SNAKE_CASE`), para o `ConfigsErrorCode` do app.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::TooLarge { .. } => "TOO_LARGE",
            Self::NotUtf8 { .. } => "NOT_UTF8",
            Self::Parse { .. } => "PARSE_FAILED",
            Self::PathNotFound { .. } => "KEY_NOT_FOUND",
            Self::NotEditable { .. } => "KEY_NOT_EDITABLE",
            Self::InvalidValue { .. } => "INVALID_VALUE",
            Self::DuplicateEdit { .. } => "DUPLICATE_EDIT",
            Self::EditNotConfirmed { .. } => "EDIT_NOT_CONFIRMED",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_sao_estaveis_e_distintos() {
        let path = KeyPath::from_keys(["a"]);
        let all = [
            ConfigError::TooLarge { size: 2, limit: 1 },
            ConfigError::NotUtf8 { offset: 0 },
            ConfigError::Parse {
                format: ConfigFormat::Toml,
                line: 1,
                column: 1,
                message: String::new(),
            },
            ConfigError::PathNotFound { path: path.clone() },
            ConfigError::NotEditable {
                path: path.clone(),
                reason: String::new(),
            },
            ConfigError::InvalidValue {
                path: path.clone(),
                reason: String::new(),
            },
            ConfigError::DuplicateEdit { path },
            ConfigError::EditNotConfirmed {
                detail: String::new(),
            },
        ];
        let codes: Vec<_> = all.iter().map(ConfigError::code).collect();
        let mut unique = codes.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), codes.len());
        for error in &all {
            assert!(!error.to_string().is_empty());
        }
    }
}
