//! Erros do domínio `launcher` (ARCHITECTURE §5).
//!
//! Criado pela F0-05 só com `INTERNAL`; a L-02 acrescentou os códigos do motor, do perfil
//! offline e do processo do jogo. Cancelamento, disco e falta de conexão continuam com os
//! códigos comuns do domínio `core`. As frases ficam em
//! `apps/desktop/src/i18n/errors/launcher.ts`.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warden_core::{CoreError, CoreErrorCode, DomainCode, DomainError, error_chain};

/// Códigos do domínio `launcher`. O código é contrato: renomear é mudança de contrato;
/// acrescentar é permitido (só acréscimo, ROADMAP §1).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LauncherErrorCode {
    /// Bug: invariante quebrada sem código específico.
    Internal,
    /// O nome do jogador não segue `^[A-Za-z0-9_]{3,16}$`.
    InvalidPlayerName,
    /// O pack não diz a versão exata do loader (vazia, "stable", "latest"…).
    LoaderVersionMissing,
    /// A versão do Minecraft não existe (nem na Mojang, nem no loader).
    MinecraftVersionNotFound,
    /// A versão do loader não existe para essa versão do Minecraft.
    LoaderVersionNotFound,
    /// A instalação do loader (instalador do Forge/NeoForge) falhou.
    LoaderInstallFailed,
    /// Um ou mais arquivos do jogo não puderam ser baixados ou vieram corrompidos.
    DownloadFailed,
    /// Os arquivos do jogo instalados estão inconsistentes (JSON da versão, bibliotecas).
    GameFilesInvalid,
    /// O Java não pôde ser usado para instalar ou abrir o jogo.
    JavaUnusable,
    /// O modo de entrada direta não existe nesta versão do jogo.
    QuickPlayUnsupported,
    /// A linha de comando passa do limite do Windows e o Java não aceita arquivo de argumentos.
    CommandTooLong,
    /// O processo do jogo não pôde ser iniciado.
    LaunchFailed,
    /// O Warden não conseguiu controlar o processo do jogo (Job Object, grupo de processos).
    ProcessControlFailed,
}

/// Erro das funções desta crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Nome de jogador fora do padrão.
    #[error("nome de jogador inválido: {name:?}")]
    InvalidPlayerName {
        /// O nome recebido.
        name: String,
    },
    /// Versão do loader ausente ou não exata.
    #[error("o pack não fixa a versão do {loader} (recebido: {version:?})")]
    LoaderVersionMissing {
        /// Loader (`Fabric`, `Forge`…).
        loader: String,
        /// O que veio no lugar da versão.
        version: String,
    },
    /// Versão do Minecraft desconhecida.
    #[error("versão do Minecraft não encontrada: {minecraft}")]
    MinecraftVersionNotFound {
        /// Versão pedida.
        minecraft: String,
        /// Mensagem do motor.
        engine: String,
    },
    /// Versão do loader desconhecida.
    #[error("{loader} {version} não existe para o Minecraft {minecraft}")]
    LoaderVersionNotFound {
        /// Loader.
        loader: String,
        /// Versão do loader pedida.
        version: String,
        /// Versão do Minecraft.
        minecraft: String,
        /// Mensagem do motor.
        engine: String,
    },
    /// Instalação do loader falhou.
    #[error("a instalação do {loader} falhou: {engine}")]
    LoaderInstallFailed {
        /// Loader.
        loader: String,
        /// Mensagem do motor (cadeia completa).
        engine: String,
    },
    /// Downloads falharam.
    #[error("falha ao baixar os arquivos do jogo: {engine}")]
    DownloadFailed {
        /// Mensagem do motor (cadeia completa).
        engine: String,
    },
    /// Sem conexão durante a instalação.
    #[error("sem conexão ao instalar o jogo: {engine}")]
    NetworkUnavailable {
        /// Mensagem do motor (cadeia completa).
        engine: String,
    },
    /// Arquivos do jogo inconsistentes.
    #[error("arquivos do jogo inválidos: {message}")]
    GameFilesInvalid {
        /// O que estava errado.
        message: String,
    },
    /// Java inutilizável.
    #[error("o Java em {} não pôde ser usado: {message}", .path.display())]
    JavaUnusable {
        /// O programa `java`.
        path: PathBuf,
        /// O que deu errado.
        message: String,
    },
    /// Quick Play pedido numa versão sem ele.
    #[error("entrar direto no mundo não existe no Minecraft {minecraft} (só a partir de 1.20)")]
    QuickPlayUnsupported {
        /// Versão do Minecraft.
        minecraft: String,
    },
    /// Linha de comando longa demais para o Java 8 no Windows.
    #[error(
        "a linha de comando tem {length} caracteres e o Java {java_major} não aceita arquivo de argumentos"
    )]
    CommandTooLong {
        /// Tamanho da linha.
        length: usize,
        /// Major do Java.
        java_major: u32,
    },
    /// O processo não abriu.
    #[error("o jogo não pôde ser iniciado com {}", .program.display())]
    LaunchFailed {
        /// O programa.
        program: PathBuf,
        /// Erro do sistema.
        #[source]
        source: io::Error,
    },
    /// Falha no controle do processo.
    #[error("falha ao controlar o processo do jogo ({action})")]
    ProcessControl {
        /// O que estava sendo feito.
        action: &'static str,
        /// Erro do sistema.
        #[source]
        source: io::Error,
    },
    /// A operação foi cancelada.
    #[error("operação cancelada")]
    Cancelled,
    /// Erro comum vindo da `warden-core`.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// Bug.
    #[error("erro interno do launcher: {message}")]
    Internal {
        /// O que estava errado.
        message: String,
    },
}

/// Resultado das funções desta crate.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Erro de disco ([`CoreError::Io`]).
    pub(crate) fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Core(CoreError::io(action, path, source))
    }

    /// Bug, com a explicação.
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

impl DomainError for Error {
    type Code = LauncherErrorCode;

    fn code(&self) -> DomainCode<LauncherErrorCode> {
        use LauncherErrorCode as C;
        DomainCode::Domain(match self {
            Self::InvalidPlayerName { .. } => C::InvalidPlayerName,
            Self::LoaderVersionMissing { .. } => C::LoaderVersionMissing,
            Self::MinecraftVersionNotFound { .. } => C::MinecraftVersionNotFound,
            Self::LoaderVersionNotFound { .. } => C::LoaderVersionNotFound,
            Self::LoaderInstallFailed { .. } => C::LoaderInstallFailed,
            Self::DownloadFailed { .. } => C::DownloadFailed,
            Self::GameFilesInvalid { .. } => C::GameFilesInvalid,
            Self::JavaUnusable { .. } => C::JavaUnusable,
            Self::QuickPlayUnsupported { .. } => C::QuickPlayUnsupported,
            Self::CommandTooLong { .. } => C::CommandTooLong,
            Self::LaunchFailed { .. } => C::LaunchFailed,
            Self::ProcessControl { .. } => C::ProcessControlFailed,
            Self::Internal { .. } => C::Internal,
            Self::NetworkUnavailable { .. } => {
                return DomainCode::Core(CoreErrorCode::NetworkUnavailable);
            }
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
        let mut put = |key: &str, value: String| {
            params.insert(key.to_owned(), value);
        };
        match self {
            Self::InvalidPlayerName { name } => put("name", name.clone()),
            Self::LoaderVersionMissing { loader, .. }
            | Self::LoaderInstallFailed { loader, .. } => {
                put("loader", loader.clone());
            }
            Self::MinecraftVersionNotFound { minecraft, .. }
            | Self::QuickPlayUnsupported { minecraft } => put("minecraft", minecraft.clone()),
            Self::LoaderVersionNotFound {
                loader,
                version,
                minecraft,
                ..
            } => {
                put("loader", loader.clone());
                put("version", version.clone());
                put("minecraft", minecraft.clone());
            }
            Self::JavaUnusable { path, .. } => put("path", path.display().to_string()),
            Self::CommandTooLong { java_major, .. } => put("javaMajor", java_major.to_string()),
            Self::Core(error) => return error.params(),
            _ => {}
        }
        params
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::Core(error) => error.detail(),
            _ => Some(error_chain(self)),
        }
    }

    fn retryable(&self) -> bool {
        match self {
            Self::DownloadFailed { .. } | Self::NetworkUnavailable { .. } => true,
            Self::Core(error) => error.retryable(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_serializados_como_contrato() {
        let text = serde_json::to_string(&LauncherErrorCode::ProcessControlFailed).unwrap();
        assert_eq!(text, "\"PROCESS_CONTROL_FAILED\"");
        let text = serde_json::to_string(&LauncherErrorCode::InvalidPlayerName).unwrap();
        assert_eq!(text, "\"INVALID_PLAYER_NAME\"");
    }

    #[test]
    fn erros_comuns_viram_codigos_do_core() {
        assert_eq!(
            Error::Cancelled.code(),
            DomainCode::Core(CoreErrorCode::Cancelled)
        );
        let network = Error::NetworkUnavailable {
            engine: "tcp connect error".into(),
        };
        assert_eq!(
            network.code(),
            DomainCode::Core(CoreErrorCode::NetworkUnavailable)
        );
        assert!(network.retryable());
        let io = Error::io("gravar", "x", io::Error::other("disco"));
        assert_eq!(io.code(), DomainCode::Core(CoreErrorCode::Io));
    }

    #[test]
    fn parametros_para_a_frase() {
        let error = Error::LoaderVersionNotFound {
            loader: "Forge".into(),
            version: "47.9.9".into(),
            minecraft: "1.20.1".into(),
            engine: "installer not found".into(),
        };
        let params = error.params();
        assert_eq!(params["loader"], "Forge");
        assert_eq!(params["version"], "47.9.9");
        assert_eq!(params["minecraft"], "1.20.1");
        assert!(error.detail().unwrap().contains("47.9.9"));
        assert!(!error.retryable());
    }
}
