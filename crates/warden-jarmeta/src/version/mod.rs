//! Comparadores de versão e dialetos de faixa (R3 §4 e §6.4 item 6).
//!
//! - [`fabric`]: `SemVer` estendido e predicados do Fabric (também usados para o Quilt);
//! - [`maven`]: versões e faixas Maven do Forge e do NeoForge;
//! - [`mod_annotation`]: texto de `@Mod.dependencies` (Forge 1.7.10 a 1.12.2);
//! - [`flexver`]: comparação de versões que não seguem nenhum padrão.

pub mod fabric;
pub mod flexver;
pub mod maven;
pub mod mod_annotation;
