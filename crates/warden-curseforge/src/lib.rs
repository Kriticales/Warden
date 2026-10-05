//! Crate `warden-curseforge`: cliente da API da CurseForge com a chave do usuário, respeitando
//! os termos da API (ARCHITECTURE §3 e §17; R3 §3.2; ADR-0051).
//!
//! - [`CurseforgeClient`]: busca ([`SearchQuery`]: classes de mods, resource packs e shaders,
//!   `modLoaderType`, `gameVersion`), projeto(s), descrição, arquivos, `download-url`,
//!   identificação por impressão digital (murmur2) e categorias; teste da chave para o
//!   "Testar" de Configurações ([`CurseforgeClient::check_key`]).
//! - Distribuição bloqueada: arquivo sem `downloadUrl` (o autor desligou a distribuição por
//!   terceiros) vira [`Distribution::Blocked`] ou [`Error::DistributionBlocked`], com a página
//!   do download manual ([`CurseforgeClient::distribution`]).
//! - Sem chave, nada sai: [`Error::KeyMissing`] antes de qualquer requisição; 401/403 viram
//!   [`Error::KeyInvalid`].
//! - Termos da CurseForge: cache **só em memória** ([`MemoryCache`]); nenhuma resposta, descrição
//!   ou endereço de download vai para o disco, para os registros ou para os erros.
//! - Downloads da CDN: [`CurseforgeClient::download_headers`] e
//!   [`CurseforgeClient::download_request`] levam o `x-api-key` também para `edge.forgecdn.net`
//!   e o redirecionamento (R7 §9 item 2), só para os servidores da CurseForge.
//!
//! Limites: a rede passa pela `warden-http` (User-Agent, no máximo 4 requisições simultâneas,
//! 429, redirecionamento resolvido sem `Range`); o murmur2 é o da `warden-packwiz` (P1-01),
//! sem cópia. Esta crate não grava nada em disco e não lê o cofre: quem chama passa a chave.

mod cache;
mod client;
mod error;
mod model;
mod query;

pub use cache::{ITEM_TTL, MAX_ENTRIES, MemoryCache, SEARCH_TTL};
pub use client::{
    API_KEY_HEADER, BASE_URL_ENV, CDN_SITES, CurseforgeClient, DEFAULT_BASE_URL, FINGERPRINT_BATCH,
    ID_BATCH,
};
pub use error::{CurseforgeErrorCode, Error, Result};
pub use model::{
    Asset, Author, Category, Distribution, File, FileDependency, FileDistribution, FileHash,
    FileIndex, FilePage, FileRef, FingerprintMatch, FingerprintMatches, HashAlgo,
    MINECRAFT_GAME_ID, Mod, ModLinks, ModLoaderType, Pagination, ProjectClass, RelationType,
    ReleaseType, SearchResults, SortableGameVersion,
};
pub use query::{
    FilesQuery, MAX_CATEGORY_IDS, MAX_GAME_VERSIONS, MAX_MOD_LOADER_TYPES, MAX_PAGE_SIZE,
    MAX_SEARCH_WINDOW, SearchQuery, SortField, SortOrder,
};
pub use secrecy::SecretString;
