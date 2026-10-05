//! O catálogo: busca nas fontes primárias, guarda no cache e cai no cache sem rede.
//!
//! Cada lista vem de uma resposta (ou duas) de uma fonte, guardada como veio em
//! `metadata.sqlite`. Enquanto vale (6 h), o catálogo nem pergunta à fonte. Vencida, pergunta;
//! se a fonte não responder (sem internet, fora do ar, resposta estragada), devolve a lista
//! guardada com a data dela e `offline = true` (critério 3 da P1-05). Só sem nada guardado a
//! falha vira erro. Cancelamento nunca cai no cache.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use warden_core::CancellationToken;
use warden_http::{HttpClient, Url};

use crate::cache::CatalogCache;
use crate::error::{Error, Result};
use crate::model::{
    Freshness, Loader, LoaderVersion, LoaderVersions, MinecraftVersions, Refresh, Source,
    VersionJson,
};
use crate::mojang::{Manifest, parse_manifest, sha1_hex};
use crate::{fabric, forge, neoforge};

/// Variáveis que trocam o endereço de cada fonte, só em build de debug (ARCHITECTURE §17.1:
/// servidor de fixtures dos E2E). A do Forge vale para o Maven e para as promoções.
pub const BASE_URL_ENVS: [(&str, Source); 4] = [
    ("WARDEN_API_BASE_MOJANG", Source::Mojang),
    ("WARDEN_API_BASE_FABRIC", Source::Fabric),
    ("WARDEN_API_BASE_FORGE", Source::Forge),
    ("WARDEN_API_BASE_NEOFORGE", Source::NeoForge),
];

/// Endereço oficial dos JSON das versões e do manifesto.
pub const MOJANG_BASE_URL: &str = "https://piston-meta.mojang.com/";
/// Endereço oficial da meta do Fabric.
pub const FABRIC_BASE_URL: &str = "https://meta.fabricmc.net/";
/// Maven oficial do Forge.
pub const FORGE_MAVEN_BASE_URL: &str = "https://maven.minecraftforge.net/";
/// Servidor oficial das promoções do Forge.
pub const FORGE_FILES_BASE_URL: &str = "https://files.minecraftforge.net/";
/// Maven oficial do NeoForge.
pub const NEOFORGE_BASE_URL: &str = "https://maven.neoforged.net/";

/// Os endereços das fontes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// Manifesto da Mojang (o JSON das versões vem do endereço do manifesto; com outro
    /// endereço aqui, os JSON também são pedidos a ele, no mesmo caminho).
    pub mojang: Url,
    /// Meta do Fabric.
    pub fabric: Url,
    /// Maven do Forge (`maven-metadata.xml`).
    pub forge_maven: Url,
    /// Promoções do Forge (`promotions_slim.json`).
    pub forge_files: Url,
    /// Maven do NeoForge (API de versões).
    pub neoforge: Url,
}

impl Endpoints {
    /// Os endereços oficiais.
    pub fn official() -> Result<Self> {
        let url = |text: &str| {
            Url::parse(text).map_err(|e| Error::Internal(format!("endereço oficial {text:?}: {e}")))
        };
        Ok(Self {
            mojang: url(MOJANG_BASE_URL)?,
            fabric: url(FABRIC_BASE_URL)?,
            forge_maven: url(FORGE_MAVEN_BASE_URL)?,
            forge_files: url(FORGE_FILES_BASE_URL)?,
            neoforge: url(NEOFORGE_BASE_URL)?,
        })
    }

    /// Os oficiais, trocados pelos de `WARDEN_API_BASE_*` em build de debug. Valor inválido
    /// é erro.
    pub fn from_env() -> Result<Self> {
        let mut endpoints = Self::official()?;
        #[cfg(debug_assertions)]
        for (name, source) in BASE_URL_ENVS {
            if let Some(value) = std::env::var(name).ok().filter(|value| !value.is_empty()) {
                endpoints.set(source, base_url(&value, source)?);
            }
        }
        Ok(endpoints)
    }

    /// Todas as fontes num servidor só (testes e E2E).
    pub fn single(base: &str) -> Result<Self> {
        let base = base_url(base, Source::Mojang)?;
        Ok(Self {
            mojang: base.clone(),
            fabric: base.clone(),
            forge_maven: base.clone(),
            forge_files: base.clone(),
            neoforge: base,
        })
    }

    fn set(&mut self, source: Source, url: Url) {
        match source {
            Source::Mojang => self.mojang = url,
            Source::Fabric => self.fabric = url,
            Source::Forge => {
                self.forge_maven = url.clone();
                self.forge_files = url;
            }
            Source::NeoForge => self.neoforge = url,
        }
    }

    fn mojang_is_official(&self) -> bool {
        self.mojang.as_str() == MOJANG_BASE_URL
    }
}

/// Endereço base com `/` no fim, para `join` manter o caminho.
fn base_url(text: &str, origin: Source) -> Result<Url> {
    let mut url = warden_http::parse_url(text).map_err(Error::http(origin))?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

/// As respostas que o catálogo guarda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Document {
    MojangManifest,
    FabricGame,
    FabricLoader,
    ForgeMavenMetadata,
    ForgePromotions,
    NeoForgeVersions,
    NeoForgeLegacyVersions,
}

impl Document {
    fn key(self) -> &'static str {
        match self {
            Self::MojangManifest => "mojang/version_manifest_v2",
            Self::FabricGame => "fabric/versions/game",
            Self::FabricLoader => "fabric/versions/loader",
            Self::ForgeMavenMetadata => "forge/maven-metadata",
            Self::ForgePromotions => "forge/promotions_slim",
            Self::NeoForgeVersions => "neoforge/versions/neoforge",
            Self::NeoForgeLegacyVersions => "neoforge/versions/forge",
        }
    }

    fn source(self) -> Source {
        match self {
            Self::MojangManifest => Source::Mojang,
            Self::FabricGame | Self::FabricLoader => Source::Fabric,
            Self::ForgeMavenMetadata | Self::ForgePromotions => Source::Forge,
            Self::NeoForgeVersions | Self::NeoForgeLegacyVersions => Source::NeoForge,
        }
    }

    fn base(self, endpoints: &Endpoints) -> &Url {
        match self {
            Self::MojangManifest => &endpoints.mojang,
            Self::FabricGame | Self::FabricLoader => &endpoints.fabric,
            Self::ForgeMavenMetadata => &endpoints.forge_maven,
            Self::ForgePromotions => &endpoints.forge_files,
            Self::NeoForgeVersions | Self::NeoForgeLegacyVersions => &endpoints.neoforge,
        }
    }

    fn path(self) -> &'static str {
        match self {
            Self::MojangManifest => "mc/game/version_manifest_v2.json",
            Self::FabricGame => "v2/versions/game",
            Self::FabricLoader => "v2/versions/loader",
            Self::ForgeMavenMetadata => "net/minecraftforge/forge/maven-metadata.xml",
            Self::ForgePromotions => "net/minecraftforge/forge/promotions_slim.json",
            Self::NeoForgeVersions => "api/maven/versions/releases/net/neoforged/neoforge",
            Self::NeoForgeLegacyVersions => "api/maven/versions/releases/net/neoforged/forge",
        }
    }
}

/// Escolhe as versões de uma versão do Minecraft numa lista do NeoForge.
type SelectVersions = fn(&[String], &str) -> Vec<LoaderVersion>;

/// Uma lista lida, de onde veio e de quando é.
struct Loaded<T> {
    value: T,
    freshness: Freshness,
    from_source: bool,
}

/// Catálogo de versões do Minecraft e dos loaders. Barato de clonar.
#[derive(Debug, Clone)]
pub struct Catalog {
    http: HttpClient,
    endpoints: Arc<Endpoints>,
    cache: CatalogCache,
    requests: Arc<AtomicU64>,
}

impl Catalog {
    /// Catálogo com os endereços oficiais (ou os de `WARDEN_API_BASE_*`, em build de debug).
    pub fn new(http: HttpClient, cache: CatalogCache) -> Result<Self> {
        Ok(Self::with_endpoints(http, Endpoints::from_env()?, cache))
    }

    /// Catálogo com outros endereços (testes).
    #[must_use]
    pub fn with_endpoints(http: HttpClient, endpoints: Endpoints, cache: CatalogCache) -> Self {
        Self {
            http,
            endpoints: Arc::new(endpoints),
            cache,
            requests: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Os endereços em uso.
    #[must_use]
    pub fn endpoints(&self) -> &Endpoints {
        &self.endpoints
    }

    /// O cache em uso.
    #[must_use]
    pub fn cache(&self) -> &CatalogCache {
        &self.cache
    }

    /// Requisições mandadas por este catálogo e seus clones (sem contar novas tentativas).
    #[must_use]
    pub fn request_count(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }

    /// As versões do Minecraft, na ordem oficial do manifesto da Mojang.
    ///
    /// Erros: sem internet e sem cache, `core.NETWORK_UNAVAILABLE`; a Mojang fora do ar,
    /// [`Error::Http`]; resposta estragada, [`Error::InvalidResponse`].
    pub async fn minecraft_versions(
        &self,
        refresh: Refresh,
        cancel: Option<&CancellationToken>,
    ) -> Result<MinecraftVersions> {
        let loaded = self.manifest(refresh, cancel).await?;
        Ok(loaded.value.to_versions(loaded.freshness))
    }

    /// As versões de um loader para uma versão do Minecraft, da mais nova para a mais antiga,
    /// sem as quebradas ([`crate::DENYLIST`]). Lista vazia: o loader não existe para essa
    /// versão (NeoForge antes da 1.20.1, Fabric antes da 1.14).
    ///
    /// Erros: como em [`Self::minecraft_versions`], da fonte do loader.
    pub async fn loader_versions(
        &self,
        loader: Loader,
        minecraft: &str,
        refresh: Refresh,
        cancel: Option<&CancellationToken>,
    ) -> Result<LoaderVersions> {
        let (versions, preselected, freshness) = match loader {
            Loader::Fabric => self.fabric(minecraft, refresh, cancel).await?,
            Loader::Forge => self.forge(minecraft, refresh, cancel).await?,
            Loader::NeoForge => self.neoforge(minecraft, refresh, cancel).await?,
        };
        Ok(LoaderVersions {
            loader,
            minecraft: minecraft.to_owned(),
            versions,
            preselected,
            freshness,
        })
    }

    /// O JSON de uma versão do Minecraft, conferido pelo `sha1` do manifesto e guardado por
    /// ele (o conteúdo é imutável; a mesma versão republicada tem outro `sha1`).
    ///
    /// Erros: [`Error::VersionNotFound`] se a versão não está no manifesto (mesmo depois de
    /// pedir o manifesto de novo); [`Error::VersionJsonCorrupted`] se o conteúdo não bate com
    /// o `sha1`; os de rede, se não estiver no cache.
    pub async fn version_json(
        &self,
        id: &str,
        cancel: Option<&CancellationToken>,
    ) -> Result<VersionJson> {
        let mut loaded = self.manifest(Refresh::IfStale, cancel).await?;
        if loaded.value.entry(id).is_none() && !loaded.from_source {
            // Versão lançada depois do manifesto guardado.
            loaded = self.manifest(Refresh::Always, cancel).await?;
        }
        let entry = loaded
            .value
            .entry(id)
            .cloned()
            .ok_or_else(|| Error::VersionNotFound { id: id.to_owned() })?;
        match self.cache.version_json(&entry.sha1).await {
            Ok(Some(body)) if sha1_hex(&body) == entry.sha1 => {
                return Ok(VersionJson {
                    id: entry.id,
                    sha1: entry.sha1,
                    body,
                });
            }
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "cache do catálogo ilegível; pedindo à Mojang"),
        }
        let url = self.version_url(&entry.url)?;
        let body = self.get(url, Source::Mojang, cancel).await?;
        let actual = sha1_hex(&body);
        if actual != entry.sha1 {
            return Err(Error::VersionJsonCorrupted {
                id: entry.id,
                expected: entry.sha1,
                actual,
            });
        }
        if let Err(error) = self
            .cache
            .put_version_json(&entry.sha1, &entry.id, body.clone())
            .await
        {
            tracing::warn!(%error, "não foi possível guardar o JSON da versão no cache");
        }
        Ok(VersionJson {
            id: entry.id,
            sha1: entry.sha1,
            body,
        })
    }

    async fn manifest(
        &self,
        refresh: Refresh,
        cancel: Option<&CancellationToken>,
    ) -> Result<Loaded<Manifest>> {
        self.load(Document::MojangManifest, refresh, cancel, parse_manifest)
            .await
    }

    async fn fabric(
        &self,
        minecraft: &str,
        refresh: Refresh,
        cancel: Option<&CancellationToken>,
    ) -> Result<(Vec<LoaderVersion>, Option<String>, Freshness)> {
        let (games, loaders) = tokio::join!(
            self.load(
                Document::FabricGame,
                refresh,
                cancel,
                fabric::parse_game_versions
            ),
            self.load(
                Document::FabricLoader,
                refresh,
                cancel,
                fabric::parse_loader_versions
            ),
        );
        let (games, loaders) = (games?, loaders?);
        let versions = fabric::for_minecraft(&games.value, &loaders.value, minecraft);
        let preselected = neoforge::preselect(&versions);
        Ok((
            versions,
            preselected,
            games.freshness.combine(loaders.freshness),
        ))
    }

    async fn forge(
        &self,
        minecraft: &str,
        refresh: Refresh,
        cancel: Option<&CancellationToken>,
    ) -> Result<(Vec<LoaderVersion>, Option<String>, Freshness)> {
        let (metadata, promotions) = tokio::join!(
            self.load(
                Document::ForgeMavenMetadata,
                refresh,
                cancel,
                forge::parse_maven_metadata
            ),
            self.load(
                Document::ForgePromotions,
                refresh,
                cancel,
                forge::parse_promotions
            ),
        );
        let metadata = metadata?;
        // Sem as promoções a lista continua útil (sem a marca de recomendada); a interface vê
        // `offline` e a data.
        let (promotions, freshness) = match promotions {
            Ok(loaded) => (loaded.value, metadata.freshness.combine(loaded.freshness)),
            Err(error) if !error.is_cancelled() => {
                tracing::warn!(%error, "promoções do Forge indisponíveis; lista sem recomendada");
                (
                    forge::Promotions::new(),
                    Freshness {
                        offline: true,
                        ..metadata.freshness
                    },
                )
            }
            Err(error) => return Err(error),
        };
        let (versions, preselected) = forge::for_minecraft(&metadata.value, &promotions, minecraft);
        Ok((versions, preselected, freshness))
    }

    async fn neoforge(
        &self,
        minecraft: &str,
        refresh: Refresh,
        cancel: Option<&CancellationToken>,
    ) -> Result<(Vec<LoaderVersion>, Option<String>, Freshness)> {
        let (document, select): (Document, SelectVersions) =
            if minecraft == neoforge::LEGACY_MINECRAFT {
                (
                    Document::NeoForgeLegacyVersions,
                    neoforge::legacy_for_minecraft,
                )
            } else {
                (Document::NeoForgeVersions, neoforge::for_minecraft)
            };
        let loaded = self
            .load(document, refresh, cancel, neoforge::parse_api_versions)
            .await?;
        let versions = select(&loaded.value, minecraft);
        let preselected = neoforge::preselect(&versions);
        Ok((versions, preselected, loaded.freshness))
    }

    /// Lê uma resposta: do cache enquanto vale; senão da fonte (e guarda); se a fonte falhar,
    /// do cache vencido, com `offline = true`.
    async fn load<T>(
        &self,
        document: Document,
        refresh: Refresh,
        cancel: Option<&CancellationToken>,
        parse: fn(&[u8]) -> std::result::Result<T, String>,
    ) -> Result<Loaded<T>> {
        let cached = match self.cache.document(document.key()).await {
            Ok(cached) => cached,
            Err(error) => {
                tracing::warn!(%error, chave = document.key(), "cache do catálogo ilegível");
                None
            }
        };
        let from_cache = |offline: bool| {
            let cached = cached.as_ref()?;
            match parse(&cached.body) {
                Ok(value) => Some(Loaded {
                    value,
                    freshness: Freshness {
                        fetched_at_ms: cached.fetched_at_ms,
                        offline,
                    },
                    from_source: false,
                }),
                Err(reason) => {
                    tracing::warn!(chave = document.key(), %reason, "lista do cache ilegível; ignorada");
                    None
                }
            }
        };
        if refresh == Refresh::IfStale
            && cached.as_ref().is_some_and(|cached| cached.fresh)
            && let Some(loaded) = from_cache(false)
        {
            return Ok(loaded);
        }
        let error = match self.fetch(document, cancel, parse).await {
            Ok(loaded) => return Ok(loaded),
            Err(error) if error.is_cancelled() => return Err(error),
            Err(error) => error,
        };
        match from_cache(true) {
            Some(loaded) => {
                tracing::warn!(%error, chave = document.key(), "fonte indisponível; usando a lista guardada");
                Ok(loaded)
            }
            None => Err(error),
        }
    }

    async fn fetch<T>(
        &self,
        document: Document,
        cancel: Option<&CancellationToken>,
        parse: fn(&[u8]) -> std::result::Result<T, String>,
    ) -> Result<Loaded<T>> {
        let url = document
            .base(&self.endpoints)
            .join(document.path())
            .map_err(|e| Error::Internal(format!("endereço {:?}: {e}", document.path())))?;
        let body = self.get(url, document.source(), cancel).await?;
        let value = parse(&body).map_err(|reason| Error::InvalidResponse {
            origin: document.source(),
            reason,
        })?;
        let fetched_at_ms = match self.cache.put_document(document.key(), body).await {
            Ok(at) => at,
            Err(error) => {
                tracing::warn!(%error, chave = document.key(), "não foi possível guardar a lista no cache");
                self.cache.now_ms()
            }
        };
        Ok(Loaded {
            value,
            freshness: Freshness {
                fetched_at_ms,
                offline: false,
            },
            from_source: true,
        })
    }

    async fn get(
        &self,
        url: Url,
        source: Source,
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<u8>> {
        let mut request = self.http.get(url);
        if let Some(cancel) = cancel {
            request = request.cancel(cancel);
        }
        self.requests.fetch_add(1, Ordering::Relaxed);
        request.bytes().await.map_err(Error::http(source))
    }

    /// O endereço do JSON de uma versão: o do manifesto ou, com outro endereço da Mojang
    /// (testes, E2E), o mesmo caminho nele.
    fn version_url(&self, raw: &str) -> Result<Url> {
        let url = warden_http::parse_url(raw).map_err(Error::http(Source::Mojang))?;
        if self.endpoints.mojang_is_official() {
            return Ok(url);
        }
        self.endpoints
            .mojang
            .join(url.path().trim_start_matches('/'))
            .map_err(|e| Error::Internal(format!("endereço do JSON da versão {raw:?}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enderecos_oficiais_validos() {
        let official = Endpoints::official().unwrap();
        assert_eq!(official.mojang.as_str(), MOJANG_BASE_URL);
        assert_eq!(official.fabric.as_str(), FABRIC_BASE_URL);
        assert_eq!(official.forge_maven.as_str(), FORGE_MAVEN_BASE_URL);
        assert_eq!(official.forge_files.as_str(), FORGE_FILES_BASE_URL);
        assert_eq!(official.neoforge.as_str(), NEOFORGE_BASE_URL);
        assert!(official.mojang_is_official());
    }

    #[test]
    fn um_servidor_para_todas_as_fontes() {
        let single = Endpoints::single("http://127.0.0.1:9/base").unwrap();
        assert_eq!(single.mojang.as_str(), "http://127.0.0.1:9/base/");
        assert_eq!(single.forge_files, single.neoforge);
        assert!(!single.mojang_is_official());
        assert!(Endpoints::single("ftp://x").is_err());
        let mut endpoints = Endpoints::official().unwrap();
        endpoints.set(Source::Forge, single.mojang.clone());
        assert_eq!(endpoints.forge_maven, single.mojang);
        assert_eq!(endpoints.forge_files, single.mojang);
        endpoints.set(Source::Fabric, single.mojang.clone());
        endpoints.set(Source::NeoForge, single.mojang.clone());
        endpoints.set(Source::Mojang, single.mojang.clone());
        assert_eq!(endpoints, single);
    }

    #[test]
    fn cada_documento_tem_chave_fonte_e_caminho() {
        let all = [
            Document::MojangManifest,
            Document::FabricGame,
            Document::FabricLoader,
            Document::ForgeMavenMetadata,
            Document::ForgePromotions,
            Document::NeoForgeVersions,
            Document::NeoForgeLegacyVersions,
        ];
        let keys: std::collections::HashSet<_> = all.iter().map(|d| d.key()).collect();
        assert_eq!(keys.len(), all.len());
        let official = Endpoints::official().unwrap();
        let urls: Vec<String> = all
            .iter()
            .map(|d| d.base(&official).join(d.path()).unwrap().to_string())
            .collect();
        assert_eq!(
            urls,
            [
                "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
                "https://meta.fabricmc.net/v2/versions/game",
                "https://meta.fabricmc.net/v2/versions/loader",
                "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml",
                "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/forge",
            ]
        );
        assert_eq!(Document::ForgePromotions.source(), Source::Forge);
        assert_eq!(Document::NeoForgeLegacyVersions.source(), Source::NeoForge);
    }
}
