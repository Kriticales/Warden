//! Crate `warden-configs`: lê, edita e compara configs de mods sem perder nada do arquivo.
//!
//! Formatos (ARCHITECTURE §10, ADR-0013): TOML, JSON/JSONC, JSON5, `.properties`, `.cfg` do
//! Forge antigo e `options.txt`.
//!
//! Princípio: **o arquivo nunca é regravado**. Cada parser devolve as entradas com a faixa de
//! bytes do valor; uma edição troca só esses bytes e o resto (comentários, ordem, espaços, fim
//! de linha, BOM) fica como estava. Abrir e salvar sem mudança devolve os mesmos bytes
//! (CA-T12-01); mudar um valor muda só a linha dele (CA-T12-02). Toda edição é conferida
//! relendo o resultado: só a chave pedida pode ter mudado, e com o valor pedido.
//!
//! Limites: arquivos acima de [`MAX_STRUCTURED_BYTES`], fora de UTF-8 ou que o parser não aceita
//! ficam só no editor de texto (o erro diz o motivo). Metadados lidos aqui: comentário de cada
//! chave, faixa e valores permitidos; padrões são da C-06 (`defaults.rs`), as camadas do
//! formulário da C-04 (`meta/`) e a busca da C-05 (`index/`).

mod annotations;
mod compare;
mod document;
mod error;
mod format;
pub(crate) mod formats;
mod path;
mod text;
mod tree;

pub use compare::{SemanticChange, SemanticDiff, compare};
pub use document::{ConfigDocument, ConfigEdit, RewriteProblem};
pub use error::{ConfigError, ConfigsErrorCode, Result};
pub use format::ConfigFormat;
pub use path::{KeyPath, PathSegment};
pub use tree::{
    CfgType, ConfigEntry, ConfigTree, ConfigValue, EntryKind, TextSpan, ValueKind, ValueRange,
};

/// Maior arquivo aceito pelo editor estruturado (2 MB, SPEC T12); acima disso, só texto.
pub const MAX_STRUCTURED_BYTES: usize = 2 * 1024 * 1024;
