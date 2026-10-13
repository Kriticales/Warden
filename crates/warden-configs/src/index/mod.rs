//! Busca em todas as configs (C-05; SPEC T12; ARCHITECTURE §10.1).
//!
//! Um índice em memória por origem (pack ou instância de teste) com uma entrada por chave nos
//! formatos que a crate lê (TOML, JSON/JSONC, JSON5, `.properties`, `.cfg` do Forge,
//! `options.txt`) e uma por linha nos outros arquivos de texto (YAML, SNBT, scripts, `.cfg` que
//! não é do Forge, arquivos que o leitor recusa e os acima de 2 MB). Cada entrada guarda a
//! chave, o valor, o comentário e a linha; o rótulo traduzido do mod chega com a C-04.
//!
//! - [`LiveIndex`]: lê a origem em segundo plano e se atualiza sozinho quando um arquivo muda
//!   (observador `notify` com espera do último aviso). A busca responde já durante a primeira
//!   leitura, com o que foi lido ([`IndexStatus`]).
//! - A busca é por palavras, sem diferenciar maiúsculas nem acentos, aproximada no nome da
//!   chave, com pontuação chave > rótulo > comentário > valor (`nucleo-matcher`), e cancelável
//!   ([`CancellationToken`](warden_core::CancellationToken)): a busca seguinte cancela a
//!   anterior. Os resultados vêm agrupados por arquivo, com a linha de cada chave e os trechos
//!   que casaram destacados ([`ConfigSearchResults`]).
//!
//! Diferente do resto da crate, este módulo lê o disco: os arquivos vêm da lista da origem
//! que quem chama fornece ([`Lister`]), a mesma do editor (`warden-project::configs`).

mod extract;
mod live;
mod matcher;
mod mod_id;
mod model;
mod store;

pub use live::{ChangeListener, IndexChange, Lister, LiveIndex, LiveIndexOptions, Relevance};
pub use model::{
    ConfigSearchGroup, ConfigSearchHit, ConfigSearchMod, ConfigSearchResults, HitField,
    IndexStatus, MarkedPart, MarkedText, SearchQuery, SourceFile,
};
