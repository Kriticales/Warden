//! Interface do motor do launcher (ARCHITECTURE §7.1; ADR-0010 e ADR-0057).
//!
//! O motor só **instala** e devolve o [`InstalledGame`]; a linha de comando é montada pelo
//! Warden ([`crate::command`]) e o processo é do Warden ([`crate::process`]). Trocar de motor
//! afeta só o adaptador ([`PortableMcEngine`]).

mod logging;
mod pmc;

use std::path::{Path, PathBuf};

use warden_core::{AppPaths, CancellationToken, ProgressSink};

use crate::command;
use crate::error::Result;
use crate::spec::{GameSpec, InstalledGame, JavaRuntime, LaunchCommand, LaunchOptions};

pub use self::pmc::{CatalogForgeResolver, ForgeArtifactResolver, PortableMcEngine};

/// Etapas da instalação, para a barra de progresso. O texto de cada uma vem do `label_key`
/// igual ao id.
pub mod stages {
    /// Lendo e baixando os JSON das versões (Minecraft e loader).
    pub const VERSION: &str = "launcher.version";
    /// Conferindo bibliotecas, assets e o Java.
    pub const VERIFY: &str = "launcher.verify";
    /// Baixando arquivos do jogo.
    pub const DOWNLOAD: &str = "launcher.download";
    /// Instalando o loader (instalador do Forge/NeoForge e processors).
    pub const LOADER: &str = "launcher.loader";
    /// Extraindo natives e preparando a linha de comando.
    pub const FINALIZE: &str = "launcher.finalize";
}

/// As pastas do motor, todas em `shared/` (ARCHITECTURE §13). Natives ficam fora do
/// `gameDir`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineDirs {
    /// JSON e jars das versões.
    pub versions: PathBuf,
    /// Bibliotecas.
    pub libraries: PathBuf,
    /// Assets e configurações de log (`assets/log_configs/`).
    pub assets: PathBuf,
    /// Natives extraídos (subpasta por conjunto de bibliotecas).
    pub natives: PathBuf,
    /// Javas do motor. O Warden sempre passa o Java dele; a pasta só existe porque o motor
    /// pede uma.
    pub jvm: PathBuf,
    /// Pasta de jogo provisória da instalação; a de verdade entra na linha de comando.
    pub placeholder_game_dir: PathBuf,
}

impl EngineDirs {
    /// As pastas dentro de `shared`.
    #[must_use]
    pub fn under(shared: &Path) -> Self {
        Self {
            versions: shared.join("versions"),
            libraries: shared.join("libraries"),
            assets: shared.join("assets"),
            natives: shared.join("natives"),
            jvm: shared.join("engine-jvm"),
            placeholder_game_dir: shared.join("engine-gamedir"),
        }
    }

    /// As pastas do app.
    #[must_use]
    pub fn for_app(paths: &AppPaths) -> Self {
        Self::under(&paths.shared_dir())
    }
}

/// Um motor de launcher.
#[async_trait::async_trait]
pub trait LauncherEngine: Send + Sync {
    /// Instala (idempotente) a versão, as bibliotecas, os assets e o loader no armazenamento
    /// compartilhado, com o Java dado (que roda os instaladores do Forge/NeoForge).
    ///
    /// Cancelar devolve [`crate::Error::Cancelled`] na hora; o motor termina o lote de
    /// downloads em andamento em segundo plano e a próxima instalação espera por ele.
    async fn install(
        &self,
        spec: &GameSpec,
        java: &JavaRuntime,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<InstalledGame>;

    /// Monta a linha de comando final (programa, argumentos, ambiente, diretório).
    ///
    /// Erros: [`crate::Error::QuickPlayUnsupported`] e [`crate::Error::CommandTooLong`].
    fn command(&self, game: &InstalledGame, opts: &LaunchOptions) -> Result<LaunchCommand> {
        command::build(game, opts, command::TargetOs::current())
    }
}
