//! Crate `warden-packwiz-cli`: execução do sidecar do packwiz com segurança e verificação de
//! que um pack está "como o packwiz produziria" (ARCHITECTURE §6.1 e §6.3; ADR-0007).
//!
//! - Construtores de argv puros ([`PackwizCommand`], [`Invocation`]): só os comandos que o
//!   Warden usa (`refresh`, `refresh --build`, `curseforge add`, `list`, `modrinth export`,
//!   `curseforge export`), com `--pack-file pack.toml` relativo e a pasta do pack como
//!   diretório de trabalho.
//! - Execução ([`Packwiz`]): ambiente limpo, chave da CurseForge só em
//!   `WARDEN_CURSEFORGE_API_KEY` e só nos comandos que falam com a CurseForge, entrada padrão
//!   controlada, sem janela no Windows, tempo-limite e cancelamento que matam o processo e os
//!   filhos (Job Object no Windows, grupo de processos no Linux).
//! - Saída ([`output`]): bytes cortados em linhas, UTF-8 com substituição, sem ANSI e sem a
//!   barra de progresso (que vira [`warden_core::Progress`]).
//! - Resultado pelo código de saída **e** por pós-condição relida do disco.
//! - Cópia de staging ([`Staging`]) e conformidade ([`check_conformance`]).
//!
//! Limites: esta crate não decide o que gravar no pack (isso é da `warden-project`, pela
//! `PackTransaction`) nem guarda a chave (vem do cofre, pela `warden-secrets`). Quem chama
//! segura a trava do pack (ADR-0019).

mod command;
mod conformance;
mod error;
pub mod output;
mod process;
mod runner;
mod staging;

pub use command::{
    CURSEFORGE_KEY_ENV, ExportSide, INHERITED_ENV, Invocation, PackwizCommand, Stdin, Timeouts,
};
pub use conformance::{
    ConformanceReport, Difference, check_conformance, file_differences, index_differences,
};
pub use error::{DETAIL_LINES, Error, ManualDownload, PackwizCliErrorCode, Result};
pub use runner::{
    AddReport, BINARY_ENV, BINARY_NAME, CurseForgeTarget, ExportReport, MAX_LINES, Packwiz,
    RefreshReport, RunContext, RunOutput,
};
pub use staging::{PackFile, SKIPPED_DIR, Staging, list_files, snapshot};
