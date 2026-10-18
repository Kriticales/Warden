//! Crate `warden-ai`: o ✦ Diagnóstico com IA (SPEC T14; ARCHITECTURE §9.5; ADR-0030 e ADR-0055).
//!
//! - [`gemini`]: cliente REST do Gemini, sem SDK, pela `warden-http` (chave só no cabeçalho).
//! - [`models`]: o modelo padrão resolvido pelo `models.list` (nenhum nome fixo no código).
//! - [`tools`]: o registro das ferramentas (declarações fixas por conversa, escalonamento por
//!   `allowedFunctionNames`, ponto de extensão da D-14) e o trait [`tools::ToolSource`] que a
//!   `warden-app` implementa para ler os dados.
//! - [`engine`]: o laço `generateContent` sem estado com as novas tentativas do S-R5-4.
//! - [`evidence`]: a conferência das evidências e o medidor de confiança.
//! - [`conversation`]: as conversas guardadas nos dados do Warden.
//! - [`schema`]: o *system prompt* e o esquema da resposta final.
//!
//! Limites: esta crate não lê o pack nem os testes (quem lê é a `warden-app`, em `ai_tools/`) e
//! não aplica nada no pack (as propostas aplicáveis são da D-14).

pub mod conversation;
pub mod engine;
mod error;
pub mod evidence;
pub mod external;
pub mod gemini;
pub mod models;
pub mod proposals;
pub mod schema;
pub mod tools;

pub use error::{AiErrorCode, Error, Result, without_secret};
