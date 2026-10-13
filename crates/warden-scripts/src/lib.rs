//! Crate `warden-scripts`.
//!
//! Scripts KubeJS e CraftTweaker (ARCHITECTURE §10.2; ADR-0037; SPEC T26):
//!
//! - [`detect`]: a linha do KubeJS e do CraftTweaker pelos jars do pack ("KubeJS 6");
//! - [`snippets`]: os trechos prontos por versão, em dados (`data/snippets/*.toml`);
//! - [`ids`]: o índice de IDs de itens, blocos, tags e receitas dos jars, com cache por hash;
//! - [`problems`]: os erros e avisos dos scripts lidos dos logs da instância de teste;
//! - [`web`]: o cliente do servidor web local do KubeJS 7 (token lido só aqui);
//! - [`reload`]: o caminho certo para "Recarregar no jogo" e o ponto de extensão do console do
//!   servidor local (L-09);
//! - [`vscode`]: "Abrir no VS Code".
//!
//! Limites: a crate não conhece o pack (quem chama monta a lista de jars e passa as pastas) e
//! só fala com `127.0.0.1`.

pub mod detect;
mod error;
pub mod ids;
pub mod problems;
pub mod reload;
pub mod snippets;
pub mod vscode;
pub mod web;

pub use error::{Error, Result, ScriptsErrorCode};
