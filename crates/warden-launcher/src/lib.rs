//! Crate `warden-launcher`: instalar e abrir o Minecraft com perfil offline, e supervisionar o
//! processo do jogo (ARCHITECTURE §7; ADR-0010 e ADR-0057; ROADMAP L-02).
//!
//! - [`LauncherEngine`]: o motor instala (idempotente) versão, bibliotecas, assets e loader no
//!   armazenamento compartilhado. Implementação: [`PortableMcEngine`], sobre a crate
//!   `portablemc` 5.0.5 escolhida no spike S1, sempre com a versão exata do loader.
//! - [`command`]: a linha de comando é do Warden, não do motor: memória, codificação da saída,
//!   `-XX:ErrorFile`, log4j seguro por faixa, jogador offline ([`offline`]), entrada direta e
//!   `@argfile` quando a linha passa do limite do Windows.
//! - [`process`]: o Warden inicia e supervisiona o jogo: pipes lidos em tarefas separadas,
//!   linhas decodificadas em UTF-8 com recuo para Windows-1252 ([`decode`]), eventos do log4j
//!   em XML e texto na mesma saída ([`log`]), Job Object no Windows e grupo de processos no
//!   Linux, classificação da saída ([`outcome`]).
//! - [`session`]: cada abertura grava `output.log` e `session.json` em
//!   `instances/<id>/state/sessions/<data-hora>/`.
//!
//! Limites: esta crate não escolhe nem baixa Java (é a `warden-java`), não monta a instância
//! (é a `warden-instance`) e não decide quando o jogo "carregou" (marcadores, L-04/L-10/D-12):
//! ela entrega as linhas e o resultado.

pub mod command;
pub mod decode;
mod engine;
mod error;
pub mod log;
pub mod offline;
pub mod outcome;
pub mod process;
mod range;
pub mod session;
mod spec;

pub use engine::{
    CatalogForgeResolver, EngineDirs, ForgeArtifactResolver, LauncherEngine, PortableMcEngine,
    stages,
};
pub use error::{Error, LauncherErrorCode, Result};
pub use offline::{OfflineProfile, OfflineUuid, PlayerName};
pub use spec::{
    ArgFile, GameSpec, InstalledGame, JavaRuntime, LaunchCommand, LaunchOptions, LoaderSpec,
    LoggingConfig, MinecraftVersionId, QuickPlay,
};
