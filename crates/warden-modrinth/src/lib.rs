//! Crate `warden-modrinth`: cliente da API v2 do Modrinth e cache persistente de metadados
//! (ARCHITECTURE §3 e §17; R3 §3.1).
//!
//! - [`ModrinthClient`]: busca com facets ([`SearchQuery`], [`Facets`]), projeto(s),
//!   versões de um projeto, versão(ões), identificação por hash (`/version_files`),
//!   verificação de atualizações em lote (`/version_files/update`: uma requisição para o pack
//!   inteiro) e listas de referência (loaders, versões do Minecraft, categorias).
//! - [`MetadataCache`]: `cache/metadata.sqlite` com projetos (24 h), versões (imutáveis) e
//!   listas (24 h); sem rede, o cliente usa o que houver. Gancho 1.1: `status`, `updated` e
//!   `game_versions` de cada projeto ficam em colunas ([`MetadataCache::project_summary`]).
//! - Modelos tolerantes ([`Project`], [`Version`]…), com o `environment` novo do projeto
//!   (lista) e da versão (valor).
//!
//! Limites: a rede passa pela `warden-http` (User-Agent, limitador de 300 por minuto, 429);
//! esta crate não baixa arquivos de mods (isso é da instância, com
//! `warden_http::HttpClient::download`) nem escreve no pack.

mod cache;
mod client;
mod error;
mod model;
mod query;

pub use cache::{Cached, MetadataCache, PROJECT_TTL, ProjectSummary, TAG_TTL, UnixClock};
pub use client::{BASE_URL_ENV, DEFAULT_BASE_URL, HASH_BATCH, ID_BATCH, ModrinthClient};
pub use error::{Error, ModrinthErrorCode, Result};
pub use model::{
    CategoryTag, Dependency, DependencyType, Environment, GalleryImage, GameVersionTag,
    HashAlgorithm, License, LoaderTag, Project, ProjectStatus, ProjectType, SearchHit,
    SearchResults, SideSupport, Version, VersionFile, VersionFileHashes, VersionType,
};
pub use query::{Facets, SearchIndex, SearchQuery, UpdateFilter, VersionFilter, json_list};
