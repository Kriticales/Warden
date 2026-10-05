//! Erros do domínio `instance` (ARCHITECTURE §5).
//!
//! Criado pela F0-05 só com `INTERNAL`; a L-03 acrescentou os códigos da materialização. As
//! frases ficam em `apps/desktop/src/i18n/errors/instance.ts`.
//!
//! Um item do pack que não pôde ser copiado não interrompe os outros: a materialização termina
//! o que dá, grava o manifesto e devolve [`Error::Incomplete`] com a lista do que faltou
//! ([`FailedItem`]), para a interface dizer exatamente o que falta (SPEC T13, "sem internet").

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreError, CoreErrorCode, DomainCode, DomainError, error_chain};

/// Resultado com [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Códigos do domínio `instance`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InstanceErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O `pack.toml` ou o `index.toml` do pack não puderam ser lidos.
    PackUnreadable,
    /// O `pack.toml` não diz a versão do Minecraft.
    MinecraftVersionMissing,
    /// O `pack.toml` não fixa a versão exata do loader (D7).
    LoaderVersionNotExact,
    /// O `pack.toml` tem mais de um loader.
    LoaderAmbiguous,
    /// Alguns itens do pack não foram copiados para a instância.
    SyncIncomplete,
    /// O arquivo escolhido para um download manual não é o esperado (hash diferente).
    ManualFileMismatch,
    /// O cache de downloads (`cache/downloads/index.sqlite`) não pôde ser usado.
    CacheUnavailable,
}

/// Por que um item do pack não foi copiado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum FailureReason {
    /// O metafile (`.pw.toml`) não pôde ser lido.
    InvalidMetafile {
        /// Mensagem técnica.
        message: String,
    },
    /// O campo `side` tem um valor que o packwiz não conhece.
    UnknownSide {
        /// O valor encontrado.
        side: String,
    },
    /// O caminho do item sai da pasta da instância.
    UnsafePath {
        /// Mensagem técnica.
        message: String,
    },
    /// O download falhou (rede, servidor, hash ou tamanho).
    DownloadFailed {
        /// Mensagem técnica, sem endereços da CurseForge.
        message: String,
        /// Se tentar de novo pode resolver (rede, servidor fora do ar).
        retryable: bool,
    },
    /// O item vem da CurseForge e não há chave da CurseForge.
    CurseforgeKeyMissing,
    /// A CurseForge recusou a chave.
    CurseforgeKeyInvalid,
    /// O arquivo não existe mais na CurseForge.
    CurseforgeFileMissing,
    /// Falha da CurseForge (rede, limite, servidor).
    CurseforgeUnavailable {
        /// Mensagem técnica.
        message: String,
        /// Se tentar de novo pode resolver.
        retryable: bool,
    },
    /// Falha de disco ao copiar.
    Io {
        /// Mensagem técnica.
        message: String,
    },
}

impl FailureReason {
    /// Se tentar de novo pode resolver.
    #[must_use]
    pub fn retryable(&self) -> bool {
        match self {
            Self::DownloadFailed { retryable, .. }
            | Self::CurseforgeUnavailable { retryable, .. } => *retryable,
            _ => false,
        }
    }
}

/// Um item do pack que não chegou à instância.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FailedItem {
    /// Caminho do item no índice do pack (`mods/sodium.pw.toml`, `config/x.toml`).
    pub path: String,
    /// Nome para mostrar (o `name` do metafile ou o nome do arquivo).
    pub name: String,
    /// Motivo.
    pub reason: FailureReason,
}

/// Erro da materialização e do cache de downloads.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Cancelada pelo usuário.
    #[error("operação cancelada")]
    Cancelled,
    /// Falha de disco.
    #[error("falha ao {action} {}", .path.display())]
    Io {
        /// O que se tentava fazer.
        action: &'static str,
        /// Caminho envolvido.
        path: PathBuf,
        /// Erro do sistema.
        #[source]
        source: io::Error,
    },
    /// Caminho fora da pasta permitida (ou outro erro do `warden-core`).
    #[error(transparent)]
    Core(#[from] CoreError),
    /// O pack não pôde ser lido.
    #[error("o pack não pôde ser lido")]
    Pack(#[source] warden_packwiz::Error),
    /// Sem versão do Minecraft no `pack.toml`.
    #[error("o pack.toml não tem a versão do Minecraft ([versions] minecraft)")]
    MinecraftVersionMissing,
    /// A versão do loader não é exata.
    #[error("a versão do {loader} no pack.toml não é exata: {version:?}")]
    LoaderVersionNotExact {
        /// Loader (`fabric`, `forge`…).
        loader: String,
        /// O valor encontrado.
        version: String,
    },
    /// Mais de um loader no `pack.toml`.
    #[error("o pack.toml tem mais de um loader: {}", .loaders.join(", "))]
    LoaderAmbiguous {
        /// Os loaders encontrados.
        loaders: Vec<String>,
    },
    /// Alguns itens não foram copiados (o resto foi, e o manifesto foi gravado).
    #[error("{} itens do pack não foram copiados para a instância", .items.len())]
    Incomplete {
        /// Os itens que faltaram.
        items: Vec<FailedItem>,
    },
    /// O arquivo escolhido para o download manual tem outro hash.
    #[error("o arquivo escolhido não é {expected_file} (hash {format} diferente)")]
    ManualFileMismatch {
        /// Nome do arquivo esperado.
        expected_file: String,
        /// Formato do hash conferido.
        format: String,
    },
    /// Falha no índice do cache de downloads.
    #[error("falha no índice do cache de downloads ({action})")]
    Cache {
        /// O que se tentava fazer.
        action: &'static str,
        /// Erro do SQLite.
        #[source]
        source: rusqlite::Error,
    },
    /// Falha no download de um arquivo para o cache (fora da materialização).
    #[error(transparent)]
    Http(#[from] warden_http::Error),
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

impl Error {
    /// Atalho para [`Error::Io`].
    pub(crate) fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }

    /// Atalho para [`Error::Cache`].
    pub(crate) fn cache(action: &'static str) -> impl FnOnce(rusqlite::Error) -> Self {
        move |source| Self::Cache { action, source }
    }
}

/// Até quantos nomes entram no parâmetro `names` de [`Error::Incomplete`].
const NAMES_IN_MESSAGE: usize = 5;

impl DomainError for Error {
    type Code = InstanceErrorCode;

    fn code(&self) -> DomainCode<InstanceErrorCode> {
        DomainCode::Domain(match self {
            Self::Cancelled => return DomainCode::Core(CoreErrorCode::Cancelled),
            Self::Io { .. } => return DomainCode::Core(CoreErrorCode::Io),
            Self::Core(error) => match error.code() {
                DomainCode::Core(code) => return DomainCode::Core(code),
                DomainCode::Domain(never) => match never {},
            },
            Self::Http(error) => match error.code() {
                DomainCode::Core(code) => return DomainCode::Core(code),
                DomainCode::Domain(_) => InstanceErrorCode::SyncIncomplete,
            },
            Self::Pack(_) => InstanceErrorCode::PackUnreadable,
            Self::MinecraftVersionMissing => InstanceErrorCode::MinecraftVersionMissing,
            Self::LoaderVersionNotExact { .. } => InstanceErrorCode::LoaderVersionNotExact,
            Self::LoaderAmbiguous { .. } => InstanceErrorCode::LoaderAmbiguous,
            Self::Incomplete { .. } => InstanceErrorCode::SyncIncomplete,
            Self::ManualFileMismatch { .. } => InstanceErrorCode::ManualFileMismatch,
            Self::Cache { .. } => InstanceErrorCode::CacheUnavailable,
            Self::Internal(_) => InstanceErrorCode::Internal,
        })
    }

    /// Parâmetros: `path` (falha de disco), `loader` e `version`, `loaders`, `count` e `names`
    /// (itens que faltaram, até cinco nomes, mais `more` com o resto), `file` (download manual).
    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        let mut put = |name: &str, value: String| {
            params.insert(name.to_owned(), value);
        };
        match self {
            Self::Io { path, .. } => put("path", path.display().to_string()),
            Self::Core(error) => return error.params(),
            Self::Http(error) => return error.params(),
            Self::LoaderVersionNotExact { loader, version } => {
                put("loader", loader.clone());
                put("version", version.clone());
            }
            Self::LoaderAmbiguous { loaders } => put("loaders", loaders.join(", ")),
            Self::Incomplete { items } => {
                put("count", items.len().to_string());
                let names: Vec<&str> = items
                    .iter()
                    .take(NAMES_IN_MESSAGE)
                    .map(|item| item.name.as_str())
                    .collect();
                put("names", names.join(", "));
                put(
                    "more",
                    items.len().saturating_sub(NAMES_IN_MESSAGE).to_string(),
                );
            }
            Self::ManualFileMismatch { expected_file, .. } => put("file", expected_file.clone()),
            _ => {}
        }
        params
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::Cancelled => None,
            Self::Incomplete { items } => {
                let mut text = error_chain(self);
                for item in items {
                    let _ = write!(text, "\n{}: {:?}", item.path, item.reason);
                }
                Some(text)
            }
            _ => Some(error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        match self {
            Self::Http(error) => error.retryable(),
            Self::Core(error) => error.retryable(),
            Self::Incomplete { items } => {
                !items.is_empty() && items.iter().all(|item| item.reason.retryable())
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, reason: FailureReason) -> FailedItem {
        FailedItem {
            path: format!("mods/{name}.pw.toml"),
            name: name.to_owned(),
            reason,
        }
    }

    fn network() -> FailureReason {
        FailureReason::DownloadFailed {
            message: "sem conexão".to_owned(),
            retryable: true,
        }
    }

    #[test]
    fn incompleto_lista_ate_cinco_nomes() {
        let items: Vec<FailedItem> = (0..7).map(|i| item(&format!("m{i}"), network())).collect();
        let error = Error::Incomplete { items };
        assert_eq!(
            error.code(),
            DomainCode::Domain(InstanceErrorCode::SyncIncomplete)
        );
        let params = error.params();
        assert_eq!(params["count"], "7");
        assert_eq!(params["names"], "m0, m1, m2, m3, m4");
        assert_eq!(params["more"], "2");
        assert!(error.retryable());
        assert!(error.detail().unwrap().contains("mods/m6.pw.toml"));
    }

    #[test]
    fn incompleto_so_repetivel_se_todos_forem() {
        let error = Error::Incomplete {
            items: vec![
                item("a", network()),
                item("b", FailureReason::CurseforgeKeyMissing),
            ],
        };
        assert!(!error.retryable());
        assert!(!Error::Incomplete { items: vec![] }.retryable());
    }

    #[test]
    fn codigos_comuns() {
        assert_eq!(
            Error::Cancelled.code(),
            DomainCode::Core(CoreErrorCode::Cancelled)
        );
        assert_eq!(Error::Cancelled.detail(), None);
        let io = Error::io("ler", "x", io::Error::other("falhou"));
        assert_eq!(io.code(), DomainCode::Core(CoreErrorCode::Io));
        assert_eq!(io.params()["path"], "x");
        let loader = Error::LoaderVersionNotExact {
            loader: "fabric".to_owned(),
            version: "latest".to_owned(),
        };
        assert_eq!(loader.params()["version"], "latest");
        let ambiguous = Error::LoaderAmbiguous {
            loaders: vec!["forge".to_owned(), "fabric".to_owned()],
        };
        assert_eq!(ambiguous.params()["loaders"], "forge, fabric");
        let manual = Error::ManualFileMismatch {
            expected_file: "jei.jar".to_owned(),
            format: "sha1".to_owned(),
        };
        assert_eq!(
            manual.code(),
            DomainCode::Domain(InstanceErrorCode::ManualFileMismatch)
        );
        assert_eq!(manual.params()["file"], "jei.jar");
    }

    #[test]
    fn json_do_motivo() {
        let json = serde_json::to_value(FailureReason::CurseforgeKeyMissing).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "curseforgeKeyMissing" }));
    }
}
