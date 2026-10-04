//! Erros da crate.
//!
//! [`ConfigError`] implementa [`DomainError`] (ARCHITECTURE §5): cada variante tem um código
//! estável de [`ConfigsErrorCode`] e os parâmetros da frase em
//! `apps/desktop/src/i18n/errors/configs.ts`. A crate trabalha sobre bytes já lidos e não toca
//! no disco, então não devolve códigos do domínio `core` (a falha de disco é de quem lê e grava
//! o arquivo).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use warden_core::{DomainCode, DomainError};

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
    /// Arquivo grande demais para o editor estruturado.
    TooLarge,
    /// Arquivo que não é texto UTF-8.
    NotUtf8,
    /// O texto não segue o formato esperado.
    ParseFailed,
    /// A chave pedida não existe no arquivo.
    KeyNotFound,
    /// A chave é uma seção, não um valor.
    KeyNotEditable,
    /// O valor novo não serve para essa chave nesse formato.
    InvalidValue,
    /// A mesma chave apareceu duas vezes num pedido de edição.
    DuplicateEdit,
    /// A releitura não confirmou a edição.
    EditNotConfirmed,
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

impl DomainError for ConfigError {
    type Code = ConfigsErrorCode;

    fn code(&self) -> DomainCode<ConfigsErrorCode> {
        DomainCode::Domain(match self {
            Self::TooLarge { .. } => ConfigsErrorCode::TooLarge,
            Self::NotUtf8 { .. } => ConfigsErrorCode::NotUtf8,
            Self::Parse { .. } => ConfigsErrorCode::ParseFailed,
            Self::PathNotFound { .. } => ConfigsErrorCode::KeyNotFound,
            Self::NotEditable { .. } => ConfigsErrorCode::KeyNotEditable,
            Self::InvalidValue { .. } => ConfigsErrorCode::InvalidValue,
            Self::DuplicateEdit { .. } => ConfigsErrorCode::DuplicateEdit,
            Self::EditNotConfirmed { .. } => ConfigsErrorCode::EditNotConfirmed,
        })
    }

    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        let mut put = |name: &str, value: String| {
            params.insert(name.to_owned(), value);
        };
        match self {
            Self::TooLarge { size, limit } => {
                put("sizeMb", megabytes(*size));
                put("limitMb", megabytes(*limit));
            }
            Self::NotUtf8 { offset } => put("offset", offset.to_string()),
            Self::Parse {
                format,
                line,
                column,
                ..
            } => {
                put("format", format.name().to_owned());
                put("line", line.to_string());
                put("column", column.to_string());
            }
            Self::PathNotFound { path } | Self::DuplicateEdit { path } => {
                put("key", path.to_string());
            }
            Self::NotEditable { path, reason } | Self::InvalidValue { path, reason } => {
                put("key", path.to_string());
                put("reason", reason.clone());
            }
            Self::EditNotConfirmed { .. } => {}
        }
        params
    }
}

/// Tamanho em MB com uma casa decimal (`2,0`), para a frase.
fn megabytes(bytes: usize) -> String {
    let tenths = bytes.saturating_mul(10) / (1024 * 1024);
    format!("{},{}", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<ConfigError> {
        let path = KeyPath::from_keys(["geral", "max"]);
        vec![
            ConfigError::TooLarge {
                size: 3 * 1024 * 1024 + 1,
                limit: 2 * 1024 * 1024,
            },
            ConfigError::NotUtf8 { offset: 7 },
            ConfigError::Parse {
                format: ConfigFormat::Toml,
                line: 3,
                column: 5,
                message: "expected `=`".into(),
            },
            ConfigError::PathNotFound { path: path.clone() },
            ConfigError::NotEditable {
                path: path.clone(),
                reason: "é uma seção".into(),
            },
            ConfigError::InvalidValue {
                path: path.clone(),
                reason: "só inteiro".into(),
            },
            ConfigError::DuplicateEdit { path },
            ConfigError::EditNotConfirmed {
                detail: "mudança inesperada".into(),
            },
        ]
    }

    #[test]
    fn cada_variante_tem_codigo_proprio_do_dominio() {
        let codes: Vec<_> = all()
            .iter()
            .map(|error| match error.code() {
                DomainCode::Domain(code) => code,
                DomainCode::Core(code) => panic!("{error}: código core {code:?}"),
            })
            .collect();
        let mut unique = codes.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), codes.len());
        assert!(!codes.contains(&ConfigsErrorCode::Internal));
    }

    #[test]
    fn codigos_na_forma_do_contrato() {
        let json = serde_json::to_string(&ConfigsErrorCode::EditNotConfirmed).unwrap();
        assert_eq!(json, "\"EDIT_NOT_CONFIRMED\"");
        let json = serde_json::to_string(&ConfigsErrorCode::NotUtf8).unwrap();
        assert_eq!(json, "\"NOT_UTF8\"");
    }

    #[test]
    fn parametros_e_detalhe() {
        let errors = all();
        let params: Vec<_> = errors.iter().map(DomainError::params).collect();
        assert_eq!(params[0]["sizeMb"], "3,0");
        assert_eq!(params[0]["limitMb"], "2,0");
        assert_eq!(params[1]["offset"], "7");
        assert_eq!(
            (
                params[2]["format"].as_str(),
                params[2]["line"].as_str(),
                params[2]["column"].as_str()
            ),
            ("TOML", "3", "5")
        );
        assert_eq!(params[3]["key"], "geral.max");
        assert_eq!(params[4]["reason"], "é uma seção");
        assert_eq!(params[5]["reason"], "só inteiro");
        assert_eq!(params[6]["key"], "geral.max");
        assert!(params[7].is_empty());
        for error in &errors {
            assert!(error.detail().is_some_and(|detail| !detail.is_empty()));
            assert!(!error.retryable());
        }
    }
}
