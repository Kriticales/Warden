//! Crate `warden-http`.
//!
//! Cliente HTTP único: User-Agent, limitador por host, novas tentativas e downloads com hash.
//!
//! Esqueleto criado pela tarefa F0-01. O conteúdo vem das tarefas que o `docs/ROADMAP.md`
//! atribui a esta crate; a responsabilidade e os limites estão na ARCHITECTURE §3.

mod error;

pub use error::HttpErrorCode;
