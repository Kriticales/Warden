//! Crate `warden-discovery`.
//!
//! Página de descoberta: início, categorias, modpacks e compatibilidade com o pack.
//!
//! - [`categories`]: mapeamento curado de categorias (`data/categories.toml`).
//! - [`home`]: populares e atualizados para o pack (P1-16).
//! - [`preview`]: galeria, notas de versão sob demanda e dependências (P1-16).
//! - [`images`]: imagens da CurseForge só em memória, pelo protocolo `warden-img://` (P1-16).
//!
//! Modpacks (`src/modpacks/**`) são da P1-17.

pub mod categories;
mod error;
pub mod home;
pub mod images;
pub mod preview;

pub use error::DiscoveryErrorCode;
