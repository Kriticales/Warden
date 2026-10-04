//! Erros da crate (ARCHITECTURE §5).
//!
//! Cada erro de formato cita o arquivo (caminho relativo à pasta do pack, quando conhecido) e a
//! chave TOML envolvida, para a interface dizer exatamente o que está errado. A leitura tolerante
//! do pack ([`crate::read_pack`]) devolve esses erros por arquivo, sem interromper os demais.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Códigos do domínio `packwiz`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PackwizErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O arquivo não é um TOML válido.
    InvalidToml,
    /// Um campo existe, mas com o tipo errado (texto onde se esperava número etc.).
    InvalidFieldType,
    /// Um campo tem um valor que o packwiz não aceita (`pack-format` desconhecido etc.).
    InvalidFieldValue,
    /// Um caminho do índice ou de um metafile tenta sair da pasta do pack.
    UnsafePath,
    /// Um link não pôde ser interpretado.
    InvalidUrl,
    /// Formato de hash que o packwiz não conhece.
    UnknownHashFormat,
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// TOML com erro de sintaxe.
    #[error("{}TOML inválido: {message}", file_prefix(.file.as_deref()))]
    InvalidToml {
        /// Arquivo (relativo à pasta do pack), se conhecido.
        file: Option<String>,
        /// Mensagem do leitor de TOML, com linha e coluna.
        message: String,
    },
    /// Campo com o tipo errado.
    #[error(
        "{}o campo `{key}` deveria ser {expected}, mas é {found}",
        file_prefix(.file.as_deref())
    )]
    InvalidFieldType {
        /// Arquivo (relativo à pasta do pack), se conhecido.
        file: Option<String>,
        /// Chave completa, com pontos (`download.hash`).
        key: String,
        /// Tipo esperado ("texto", "número inteiro"…).
        expected: &'static str,
        /// Tipo encontrado.
        found: &'static str,
    },
    /// Campo com um valor que o packwiz recusa.
    #[error("{}valor inválido em `{key}`: {reason}", file_prefix(.file.as_deref()))]
    InvalidFieldValue {
        /// Arquivo (relativo à pasta do pack), se conhecido.
        file: Option<String>,
        /// Chave completa, com pontos.
        key: String,
        /// Por que o valor foi recusado.
        reason: String,
    },
    /// Caminho que sai da pasta do pack (`..`, absoluto, letra de unidade).
    #[error("caminho inseguro {path:?}: {reason}")]
    UnsafePath {
        /// O caminho recebido.
        path: String,
        /// Por que foi recusado.
        reason: &'static str,
    },
    /// Link que não pôde ser interpretado.
    #[error("link inválido {url:?}: {reason}")]
    InvalidUrl {
        /// O link recebido.
        url: String,
        /// Por que foi recusado.
        reason: &'static str,
    },
    /// Formato de hash desconhecido.
    #[error("formato de hash desconhecido: {0:?}")]
    UnknownHashFormat(String),
    /// Falha de disco.
    #[error("falha ao {action} {}", .path.display())]
    Io {
        /// O que se tentava fazer ("ler", "listar"…).
        action: &'static str,
        /// O caminho envolvido.
        path: PathBuf,
        /// O erro do sistema.
        #[source]
        source: io::Error,
    },
}

fn file_prefix(file: Option<&str>) -> String {
    file.map(|file| format!("{file}: ")).unwrap_or_default()
}

impl Error {
    /// Código estável do erro. `None` para falha de disco, que usa o código comum `IO` do
    /// domínio `core` (ARCHITECTURE §5).
    #[must_use]
    pub fn code(&self) -> Option<PackwizErrorCode> {
        Some(match self {
            Self::InvalidToml { .. } => PackwizErrorCode::InvalidToml,
            Self::InvalidFieldType { .. } => PackwizErrorCode::InvalidFieldType,
            Self::InvalidFieldValue { .. } => PackwizErrorCode::InvalidFieldValue,
            Self::UnsafePath { .. } => PackwizErrorCode::UnsafePath,
            Self::InvalidUrl { .. } => PackwizErrorCode::InvalidUrl,
            Self::UnknownHashFormat(_) => PackwizErrorCode::UnknownHashFormat,
            Self::Io { .. } => return None,
        })
    }

    /// Valores para a frase traduzida: `file`, `key`, `path`, `url` ou `format`, conforme o
    /// erro.
    #[must_use]
    pub fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        let mut put = |name: &str, value: &str| {
            params.insert(name.to_owned(), value.to_owned());
        };
        match self {
            Self::InvalidToml { file, .. } => {
                if let Some(file) = file {
                    put("file", file);
                }
            }
            Self::InvalidFieldType { file, key, .. }
            | Self::InvalidFieldValue { file, key, .. } => {
                if let Some(file) = file {
                    put("file", file);
                }
                put("key", key);
            }
            Self::UnsafePath { path, .. } => put("path", path),
            Self::InvalidUrl { url, .. } => put("url", url),
            Self::UnknownHashFormat(format) => put("format", format),
            Self::Io { path, .. } => put("path", &path.display().to_string()),
        }
        params
    }

    /// O mesmo erro, citando o arquivo em que aconteceu (para os erros de formato).
    #[must_use]
    pub fn in_file(mut self, name: &str) -> Self {
        match &mut self {
            Self::InvalidToml { file, .. }
            | Self::InvalidFieldType { file, .. }
            | Self::InvalidFieldValue { file, .. } => *file = Some(name.to_owned()),
            Self::UnsafePath { .. }
            | Self::InvalidUrl { .. }
            | Self::UnknownHashFormat(_)
            | Self::Io { .. } => {}
        }
        self
    }

    /// O arquivo citado pelo erro de formato, se houver.
    #[must_use]
    pub fn file(&self) -> Option<&str> {
        match self {
            Self::InvalidToml { file, .. }
            | Self::InvalidFieldType { file, .. }
            | Self::InvalidFieldValue { file, .. } => file.as_deref(),
            Self::UnsafePath { .. }
            | Self::InvalidUrl { .. }
            | Self::UnknownHashFormat(_)
            | Self::Io { .. } => None,
        }
    }
}

/// Resultado das funções desta crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mensagem_cita_arquivo_e_chave() {
        let error = Error::InvalidFieldType {
            file: None,
            key: "download.hash".to_owned(),
            expected: "texto",
            found: "número inteiro",
        }
        .in_file("mods/sodium.pw.toml");
        assert_eq!(
            error.to_string(),
            "mods/sodium.pw.toml: o campo `download.hash` deveria ser texto, mas é número inteiro"
        );
        assert_eq!(error.file(), Some("mods/sodium.pw.toml"));
        assert_eq!(error.code(), Some(PackwizErrorCode::InvalidFieldType));
        let params = error.params();
        assert_eq!(params["file"], "mods/sodium.pw.toml");
        assert_eq!(params["key"], "download.hash");
    }

    #[test]
    fn codigos_e_parametros_de_cada_variante() {
        let io = Error::Io {
            action: "ler",
            path: PathBuf::from("pack.toml"),
            source: io::Error::other("x"),
        };
        assert_eq!(io.code(), None);
        assert_eq!(io.params()["path"], "pack.toml");
        assert!(io.to_string().starts_with("falha ao ler pack.toml"));
        assert_eq!(io.in_file("a").file(), None);

        let cases = [
            (
                Error::InvalidToml {
                    file: None,
                    message: "m".to_owned(),
                },
                PackwizErrorCode::InvalidToml,
                None,
            ),
            (
                Error::InvalidFieldValue {
                    file: None,
                    key: "pack-format".to_owned(),
                    reason: "r".to_owned(),
                },
                PackwizErrorCode::InvalidFieldValue,
                Some(("key", "pack-format")),
            ),
            (
                Error::UnsafePath {
                    path: "../x".to_owned(),
                    reason: "r",
                },
                PackwizErrorCode::UnsafePath,
                Some(("path", "../x")),
            ),
            (
                Error::InvalidUrl {
                    url: "u".to_owned(),
                    reason: "r",
                },
                PackwizErrorCode::InvalidUrl,
                Some(("url", "u")),
            ),
            (
                Error::UnknownHashFormat("crc".to_owned()),
                PackwizErrorCode::UnknownHashFormat,
                Some(("format", "crc")),
            ),
        ];
        for (error, code, param) in cases {
            assert_eq!(error.code(), Some(code));
            if let Some((name, value)) = param {
                assert_eq!(error.params()[name], value);
            }
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn codigo_serializa_em_maiusculas() {
        let json = serde_json::to_string(&PackwizErrorCode::InvalidFieldType).unwrap();
        assert_eq!(json, "\"INVALID_FIELD_TYPE\"");
    }
}
