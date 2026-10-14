//! Erros do domínio `server` (ARCHITECTURE §5 e §7.6).
//!
//! As falhas de rede chegam da `warden-http` (e do catálogo) e viram
//! [`ServerErrorCode::DownloadFailed`], com a fonte na frase; sem conexão, tempo esgotado,
//! cancelamento e falha de disco continuam com os códigos comuns do domínio `core`. As frases
//! ficam em `apps/desktop/src/i18n/errors/server.ts`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreError, CoreErrorCode, DomainCode, DomainError, error_chain};

/// Códigos do domínio `server`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServerErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// A EULA do Minecraft ainda não foi aceita (ou foi revogada): o servidor não abre.
    EulaNotAccepted,
    /// Este loader não tem servidor local no Warden (Quilt, `LiteLoader`).
    LoaderUnsupported,
    /// A versão do loader do `pack.toml` não existe na fonte oficial.
    LoaderVersionNotFound,
    /// A versão do Minecraft não tem servidor oficial para baixar.
    NoServerDownload,
    /// Um download do servidor (Mojang, Fabric, Forge, NeoForge) falhou.
    DownloadFailed,
    /// O instalador oficial do loader terminou com erro.
    InstallerFailed,
    /// O instalador terminou, mas o servidor não ficou onde devia.
    InstallerOutputMissing,
    /// Nenhuma porta livre em `127.0.0.1`.
    NoFreePort,
    /// O servidor fechou antes de ficar pronto.
    ExitedBeforeReady,
    /// O servidor não ficou pronto a tempo.
    ReadyTimeout,
    /// Não há servidor aberto para receber o comando.
    NotRunning,
    /// Comando vazio, com quebra de linha ou longo demais.
    InvalidCommand,
    /// O processo do servidor não pôde ser aberto ou controlado.
    ProcessFailed,
}

/// De onde vinha um download do servidor, para a frase dizer qual fonte falhou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerSource {
    /// Servidor vanilla (`piston-data.mojang.com`).
    Mojang,
    /// Meta do Fabric (`meta.fabricmc.net`).
    Fabric,
    /// Maven do Forge (`maven.minecraftforge.net`).
    Forge,
    /// Maven do NeoForge (`maven.neoforged.net`).
    NeoForge,
}

impl ServerSource {
    /// Nome para as frases.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mojang => "Mojang",
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        }
    }
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Sem o aceite da EULA, nenhum `eula=true` é gravado.
    #[error("a EULA do Minecraft não foi aceita")]
    EulaNotAccepted,
    /// Loader sem servidor local.
    #[error("o servidor local não suporta o loader {loader}")]
    LoaderUnsupported {
        /// Loader do pack.
        loader: String,
    },
    /// Versão do loader inexistente.
    #[error("{loader} {version} não existe para o Minecraft {minecraft}")]
    LoaderVersionNotFound {
        /// Loader.
        loader: String,
        /// Versão do `pack.toml`.
        version: String,
        /// Versão do Minecraft.
        minecraft: String,
    },
    /// O JSON da versão não tem `downloads.server`.
    #[error("o Minecraft {minecraft} não tem servidor oficial")]
    NoServerDownload {
        /// Versão do Minecraft.
        minecraft: String,
    },
    /// Falha num download.
    #[error("download do servidor ({}): {message}", .source_name.label())]
    Download {
        /// Fonte.
        source_name: ServerSource,
        /// O erro, com a cadeia de causas.
        message: String,
        /// Código comum (sem conexão, tempo esgotado), quando o erro é desses.
        core: Option<CoreErrorCode>,
    },
    /// O instalador oficial falhou.
    #[error("o instalador do {loader} terminou com o código {code:?}")]
    InstallerFailed {
        /// Loader.
        loader: String,
        /// Código de saída (`None`: morto ou tempo esgotado).
        code: Option<i32>,
        /// O fim da saída do instalador (até 4 KB), para "Detalhes técnicos".
        output: String,
    },
    /// O instalador não deixou o servidor.
    #[error("o instalador do {loader} não deixou o servidor na pasta do servidor")]
    InstallerOutputMissing {
        /// Loader.
        loader: String,
        /// O fim da saída do instalador (até 4 KB), para "Detalhes técnicos".
        output: String,
    },
    /// Sem porta livre.
    #[error("nenhuma porta livre em 127.0.0.1")]
    NoFreePort,
    /// Fechou antes do "Done".
    #[error("o servidor fechou antes de ficar pronto (código {exit_code:?})")]
    ExitedBeforeReady {
        /// Código de saída.
        exit_code: Option<i32>,
    },
    /// "Done" não veio a tempo.
    #[error("o servidor não ficou pronto em {seconds} s")]
    ReadyTimeout {
        /// Tempo esperado.
        seconds: u64,
    },
    /// Sem servidor aberto.
    #[error("não há servidor aberto")]
    NotRunning,
    /// Comando recusado.
    #[error("comando inválido para o servidor")]
    InvalidCommand,
    /// Processo do servidor.
    #[error(transparent)]
    Process(#[from] warden_launcher::Error),
    /// Disco, caminho fora da pasta ou outro erro comum.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// Cancelado.
    #[error("operação cancelada")]
    Cancelled,
    /// Invariante quebrada.
    #[error("erro interno: {0}")]
    Internal(String),
}

/// Atalho para os resultados da crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Atalho para um erro de disco ([`CoreError::Io`]).
    pub(crate) fn io(
        action: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Core(CoreError::io(action, path, source))
    }

    /// Atalho para um erro de rede de uma fonte.
    pub(crate) fn http(source_name: ServerSource) -> impl FnOnce(warden_http::Error) -> Self {
        move |error| match error {
            warden_http::Error::Cancelled => Self::Cancelled,
            error => Self::Download {
                source_name,
                core: match error.code() {
                    DomainCode::Core(code) => Some(code),
                    DomainCode::Domain(_) => None,
                },
                message: error_chain(&error),
            },
        }
    }

    /// Atalho para um erro do catálogo (meta da Mojang, versões dos loaders).
    pub(crate) fn catalog(source_name: ServerSource) -> impl FnOnce(warden_catalog::Error) -> Self {
        move |error| {
            let code = error.code();
            if code == DomainCode::Core(CoreErrorCode::Cancelled) {
                return Self::Cancelled;
            }
            Self::Download {
                source_name,
                core: match code {
                    DomainCode::Core(code) => Some(code),
                    DomainCode::Domain(_) => None,
                },
                message: error_chain(&error),
            }
        }
    }
}

impl DomainError for Error {
    type Code = ServerErrorCode;

    fn code(&self) -> DomainCode<ServerErrorCode> {
        DomainCode::Domain(match self {
            Self::EulaNotAccepted => ServerErrorCode::EulaNotAccepted,
            Self::LoaderUnsupported { .. } => ServerErrorCode::LoaderUnsupported,
            Self::LoaderVersionNotFound { .. } => ServerErrorCode::LoaderVersionNotFound,
            Self::NoServerDownload { .. } => ServerErrorCode::NoServerDownload,
            Self::Download { core, .. } => match core {
                Some(
                    code @ (CoreErrorCode::NetworkUnavailable
                    | CoreErrorCode::Timeout
                    | CoreErrorCode::Io
                    | CoreErrorCode::Cancelled),
                ) => return DomainCode::Core(*code),
                _ => ServerErrorCode::DownloadFailed,
            },
            Self::InstallerFailed { .. } => ServerErrorCode::InstallerFailed,
            Self::InstallerOutputMissing { .. } => ServerErrorCode::InstallerOutputMissing,
            Self::NoFreePort => ServerErrorCode::NoFreePort,
            Self::ExitedBeforeReady { .. } => ServerErrorCode::ExitedBeforeReady,
            Self::ReadyTimeout { .. } => ServerErrorCode::ReadyTimeout,
            Self::NotRunning => ServerErrorCode::NotRunning,
            Self::InvalidCommand => ServerErrorCode::InvalidCommand,
            Self::Process(_) => ServerErrorCode::ProcessFailed,
            Self::Internal(_) => ServerErrorCode::Internal,
            Self::Cancelled => return DomainCode::Core(CoreErrorCode::Cancelled),
            Self::Core(error) => {
                return match error.code() {
                    DomainCode::Core(code) => DomainCode::Core(code),
                    DomainCode::Domain(never) => match never {},
                };
            }
        })
    }

    fn params(&self) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        match self {
            Self::LoaderUnsupported { loader } | Self::InstallerOutputMissing { loader, .. } => {
                params.insert("loader".into(), loader.clone());
            }
            Self::LoaderVersionNotFound {
                loader,
                version,
                minecraft,
            } => {
                params.insert("loader".into(), loader.clone());
                params.insert("version".into(), version.clone());
                params.insert("minecraft".into(), minecraft.clone());
            }
            Self::NoServerDownload { minecraft } => {
                params.insert("minecraft".into(), minecraft.clone());
            }
            Self::Download { source_name, .. } => {
                params.insert("source".into(), source_name.label().into());
            }
            Self::InstallerFailed { loader, code, .. } => {
                params.insert("loader".into(), loader.clone());
                params.insert(
                    "code".into(),
                    code.map_or_else(|| "?".to_owned(), |code| code.to_string()),
                );
            }
            Self::ExitedBeforeReady { exit_code } => {
                params.insert(
                    "code".into(),
                    exit_code.map_or_else(|| "?".to_owned(), |code| code.to_string()),
                );
            }
            Self::ReadyTimeout { seconds } => {
                params.insert("seconds".into(), seconds.to_string());
            }
            Self::Core(error) => return error.params(),
            Self::Process(error) => return error.params(),
            _ => {}
        }
        params
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::InstallerFailed { output, .. } | Self::InstallerOutputMissing { output, .. }
                if !output.is_empty() =>
            {
                Some(format!("{}\n\n{output}", error_chain(self)))
            }
            _ => Some(error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Download { .. } | Self::NoFreePort | Self::ReadyTimeout { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_e_parametros() {
        assert_eq!(
            Error::EulaNotAccepted.code(),
            DomainCode::Domain(ServerErrorCode::EulaNotAccepted)
        );
        assert_eq!(
            Error::Cancelled.code(),
            DomainCode::Core(CoreErrorCode::Cancelled)
        );
        let offline = Error::Download {
            source_name: ServerSource::Fabric,
            message: "sem rede".into(),
            core: Some(CoreErrorCode::NetworkUnavailable),
        };
        assert_eq!(
            offline.code(),
            DomainCode::Core(CoreErrorCode::NetworkUnavailable)
        );
        let failed = Error::Download {
            source_name: ServerSource::Forge,
            message: "404".into(),
            core: None,
        };
        assert_eq!(
            failed.code(),
            DomainCode::Domain(ServerErrorCode::DownloadFailed)
        );
        assert_eq!(failed.params()["source"], "Forge");
        assert!(failed.retryable());
        let installer = Error::InstallerFailed {
            loader: "Forge".into(),
            code: Some(1),
            output: "java.io.IOException: x".into(),
        };
        assert_eq!(installer.params()["code"], "1");
        assert!(installer.detail().unwrap().contains("IOException"));
        assert_eq!(
            serde_json::to_value(ServerErrorCode::EulaNotAccepted).unwrap(),
            "EULA_NOT_ACCEPTED"
        );
    }
}
