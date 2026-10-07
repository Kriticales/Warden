//! Crate `warden-project`.
//!
//! Serviço de domínio do pack: registro, criar e abrir, inventário, busca combinada,
//! adicionar, remover e atualizar.
//!
//! Esqueleto criado pela tarefa F0-01. O conteúdo vem das tarefas que o `docs/ROADMAP.md`
//! atribui a esta crate; a responsabilidade e os limites estão na ARCHITECTURE §3.

pub mod create;
pub mod details;
mod error;
pub mod hygiene;
pub mod inventory;
pub mod meta;
pub mod open;
pub mod registry;
pub mod remove;
pub mod side;
pub mod transaction;
pub mod trash;
pub mod updates;

pub use error::{Error, ProjectErrorCode, Result};
