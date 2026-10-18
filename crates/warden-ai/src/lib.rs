//! Crate `warden-ai`: o ✦ Diagnóstico com IA (SPEC T14; ARCHITECTURE §9.5; ADR-0030, ADR-0055
//! e ADR-0060).
//!
//! - [`providers`]: o trait de provedor, o formato comum e os tradutores (Gemini; OpenAI na
//!   D-16; Anthropic na D-17), sem SDK, pela `warden-http` (chave só no cabeçalho); o modelo
//!   padrão de cada provedor pela lista de modelos da conta (nenhum nome fixo no código).
//! - [`tools`]: o registro das ferramentas (declarações fixas por conversa, escalonamento por
//!   `allowedFunctionNames`, ponto de extensão da D-14) e o trait [`tools::ToolSource`] que a
//!   `warden-app` implementa para ler os dados.
//! - [`engine`]: o laço sem estado, só com o trait de provedor, com as novas tentativas do
//!   S-R5-4.
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
pub mod proposals;
pub mod providers;
pub mod schema;
pub mod tools;

pub use error::{AiErrorCode, Error, Result, without_secret};
