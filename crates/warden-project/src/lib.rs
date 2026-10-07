//! Crate `warden-project`.
//!
//! Serviço de domínio do pack: registro, criar e abrir, inventário, busca combinada,
//! adicionar, remover e atualizar.
//!
//! Esqueleto criado pela tarefa F0-01. O conteúdo vem das tarefas que o `docs/ROADMAP.md`
//! atribui a esta crate; a responsabilidade e os limites estão na ARCHITECTURE §3.

pub mod add;
pub mod create;
pub mod dedup;
pub mod deps;
pub mod details;
mod error;
pub mod hygiene;
pub mod initial_mods;
pub mod inventory;
pub mod kits;
pub mod meta;
pub mod open;
pub mod player_tools;
pub mod registry;
pub mod remove;
pub mod search;
pub mod side;
pub mod transaction;
pub mod trash;

pub use error::{Error, ProjectErrorCode, Result};
