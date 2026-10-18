//! Crate `warden-bisect`: a busca do culpado (SPEC T25; ARCHITECTURE §9.7; ADR-0031).
//!
//! Algoritmo puro, pausável e testado com um executor simulado; quem abre o jogo é a
//! `warden-app` (`bisect_session.rs`).
//!
//! - [`universe`]: itens, papéis (sempre ligados, desligados, procurados) e a ordem
//!   topológica estável em que todo prefixo já tem as dependências;
//! - [`graph`]: a entrada a partir do grafo da D-07 (com `fabric-api` = `fabric_api`);
//! - [`search`]: a máquina de estados das rodadas (0, controle, busca binária, parceiro,
//!   confirmação, repetições, contradições e dependências inferidas);
//! - [`watch`]: o veredito de cada rodada a partir da saída do jogo (marcadores do S-R5-3,
//!   sinais de falha com prioridade, assinatura, jogo vivo e calado);
//! - [`judge`]: da falha ao veredito (mesma assinatura, outra, ambiente, inferida);
//! - [`estimate`]: rodadas e minutos do diálogo e da tela;
//! - [`prepare`]: o que cada rodada precisa além dos mods (`fabric.noGui`, `fml.queryResult`,
//!   `showLoadWarnings`, limites de tempo);
//! - [`state`]: `state/bisect.json` e os resultados terminados.
//!
//! A busca com o servidor local (versões anteriores a 1.20 e problemas de servidor) é da D-13:
//! o ponto de extensão é [`BisectErrorCode::NeedsServer`] e o [`watch::Goal`] da rodada.

mod error;
pub mod estimate;
pub mod graph;
pub mod judge;
pub mod prepare;
pub mod search;
pub mod state;
pub mod universe;
pub mod watch;

pub use error::{BisectError, BisectErrorCode, Result};
pub use estimate::Estimate;
pub use judge::{PackLookup, Symptom, Target};
pub use search::{Confidence, Outcome, RoundKind, RoundPlan, RoundRecord, Search, Verdict};
pub use state::{BisectState, RoundInfo};
pub use universe::{ItemSpec, Requirement, Role, Universe};
pub use watch::{Anchor, Decision, Failure, Goal, RoundWatcher, Stage, WatchConfig};
