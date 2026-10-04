//! Crate `warden-packwiz-cli`.
//!
//! Execução do sidecar do packwiz: argv, tempo-limite, cancelamento, staging e conformidade.
//!
//! Esqueleto criado pela tarefa F0-01. O conteúdo vem das tarefas que o `docs/ROADMAP.md`
//! atribui a esta crate; a responsabilidade e os limites estão na ARCHITECTURE §3.

mod error;

pub use error::PackwizCliErrorCode;
