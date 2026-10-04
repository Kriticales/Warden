//! Crate `warden-core`: tipos e serviços pequenos que todas as outras crates usam
//! (ARCHITECTURE §3).
//!
//! - Identificadores: [`PackId`] e [`OperationId`] (ULID, gravados como texto).
//! - Progresso e cancelamento: [`ProgressSink`], [`Progress`] e [`CancellationToken`]
//!   (reexportado de `tokio-util`).
//! - Pastas do app: [`AppPaths`] (ARCHITECTURE §13; ADR-0048 para as pastas de
//!   desenvolvimento).
//! - Escrita atômica: [`atomic_write`] (`.warden-tmp` + fsync + renomeação).
//! - Caminhos seguros: [`resolve_inside`], que recusa qualquer caminho que saia da raiz.
//! - Erros: o trait [`DomainError`], implementado pelo erro de cada crate, e o erro desta
//!   crate, [`CoreError`], com os códigos comuns a todas ([`CoreErrorCode`]).
//! - Injeção de falha: o módulo `fault` (feature `fault-injection`, ou nos testes desta
//!   crate) arma pontos de falha nomeados para provar que uma falha no meio deixa o estado
//!   anterior (QUALITY §3 item 7 e §4.1).
//!
//! Limites: nada de Tauri, rede, processos ou formato de pack aqui. A crate não depende de
//! nenhuma outra crate do Warden.

mod error;
pub mod fault;
mod fs;
mod ids;
mod path;
mod paths;
mod progress;

pub use error::{CoreError, CoreErrorCode, DomainCode, DomainError, NoDomainCode, error_chain};
pub use fs::{TEMP_SUFFIX, atomic_write, atomic_write_private, remove_temp_files};
pub use ids::{InvalidId, OperationId, PackId};
pub use path::resolve_inside;
pub use paths::{AppPaths, DATA_ROOT_ENV, SystemDirs};
pub use progress::{NoProgress, Progress, ProgressSink, ProgressUnit, StageId};
pub use tokio_util::sync::CancellationToken;
