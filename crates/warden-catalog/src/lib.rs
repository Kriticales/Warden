//! Crate `warden-catalog`: catálogo de versões do Minecraft e dos loaders a partir das fontes
//! primárias (ARCHITECTURE §3 e §17; R2 §1.2, §3.2, §3.4, §3.5 e §8.1 item 1).
//!
//! - [`Catalog::minecraft_versions`]: o manifesto da Mojang **na ordem oficial**, nunca por
//!   texto ou semver (`26.3` vem antes de `1.21.11`, que vem antes de `1.21.2`), com a
//!   etiqueta "melhor esforço" nas anteriores a 1.7.10 ([`SUPPORTED_SINCE`]);
//!   [`MinecraftVersions::compare`] compara duas versões por essa ordem.
//! - [`Catalog::loader_versions`]: Fabric (meta: versões do jogo + versões do loader), Forge
//!   (`maven-metadata.xml` + `promotions_slim.json`, com o sufixo `-1.7.10` tirado da versão
//!   do `pack.toml` e a recomendada marcada) e NeoForge (`net.neoforged:forge` na 1.20.1,
//!   `net.neoforged:neoforge` depois, com a versão do Minecraft calculada de cada número, sem
//!   misturar `21.1` com `21.11`). A lista de quebradas ([`DENYLIST`]) fica de fora.
//! - [`Catalog::version_json`]: o JSON de uma versão, conferido e guardado pelo `sha1`.
//! - [`CatalogCache`]: as respostas ficam em `cache/metadata.sqlite` (tabelas `catalog_*`).
//!   Valem 6 h; sem rede, o catálogo usa o que houver e diz de quando é ([`Freshness`]).
//!
//! Limites: a rede passa pela `warden-http`; esta crate não instala loaders (é do launcher,
//! L-02) nem escreve no pack (o `pack.toml` é da P1-07). A normalização das versões do
//! Minecraft que o Fabric Loader faz (`McVersionLookup`) não está aqui: a D-01 decide onde
//! fica (ROADMAP D-01).

mod cache;
mod catalog;
mod error;
mod fabric;
mod forge;
mod model;
mod mojang;
mod neoforge;
mod order;

pub use cache::{CachedDocument, CatalogCache, LIST_TTL, WallClock};
pub use catalog::{
    BASE_URL_ENVS, Catalog, Endpoints, FABRIC_BASE_URL, FORGE_FILES_BASE_URL, FORGE_MAVEN_BASE_URL,
    MOJANG_BASE_URL, NEOFORGE_BASE_URL,
};
pub use error::{CatalogErrorCode, Error, Result};
pub use forge::{DENYLIST, denied};
pub use model::{
    Freshness, Loader, LoaderVersion, LoaderVersions, MinecraftVersion, MinecraftVersionKind,
    MinecraftVersions, Refresh, Source, VersionJson,
};
pub use mojang::SUPPORTED_SINCE;
