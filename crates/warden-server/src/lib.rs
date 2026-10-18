//! Crate `warden-server`: o servidor local sob demanda (ARCHITECTURE §7.6; ADR-0032; SPEC T27;
//! ROADMAP L-09).
//!
//! Peças, cada uma testável sozinha:
//! - [`install`]: instala (idempotente, com cache) o servidor do loader certo com a versão
//!   exata do `pack.toml`: vanilla, Fabric (meta), Forge e NeoForge (`--installServer`),
//!   Forge antigo com Java 8;
//! - [`eula`]: `eula=true` só com o aceite da pessoa (`settings.json`, dado na interface);
//! - [`properties`]: o `server.properties` controlado (só `127.0.0.1`, porta livre,
//!   `online-mode=false`, RCON e query desligados);
//! - [`process`]: abre e supervisiona o processo (o supervisor da `warden-launcher`, com o
//!   stdin para comandos), espera o "pronto" e para com `stop` (à força depois de 60 s);
//! - [`markers`]: "pronto", "um jogador entrou", a classe do cliente carregada no servidor e o
//!   trecho do log de um travamento;
//! - [`plan`]: a conta de memória do servidor e do jogo contra a memória livre;
//! - [`log4j`]: a configuração segura para versões até a 1.18.
//!
//! A materialização do pack só com os itens de servidor é da `warden-instance`
//! (`InstallSide::Server`), e quem junta tudo, com as pastas do app, o console e o evento
//! `server-state`, é a `warden-app` (`server_session.rs`).

mod error;
pub mod eula;
pub mod install;
pub mod log4j;
pub mod markers;
pub mod plan;
pub mod process;
pub mod properties;

pub use error::{Error, Result, ServerErrorCode, ServerSource};
pub use eula::{EULA_URL, write_eula};
pub use install::{InstalledServer, Installer, ServerDownload, ServerLaunch};
pub use plan::{MemoryPlan, memory_plan, server_memory_mb};
pub use process::{
    ReadyFailure, RunningServer, STOP_TIMEOUT, ServerCommand, ServerEvent, ServerEvents,
    ServerHandle, StopOutcome, wait_ready,
};
pub use properties::{ControlledProperties, LOCAL_HOST, free_port, write_properties};
