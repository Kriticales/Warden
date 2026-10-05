//! Cliente da API v2 do Modrinth (R3 §3.1; ARCHITECTURE §17).
//!
//! Usa os endpoints em lote sempre que possível: `/projects?ids=`, `/versions?ids=`,
//! `POST /version_files` e `POST /version_files/update` (uma requisição para o pack inteiro, em
//! vez de uma por mod como o packwiz). Os `POST` de consulta são marcados como idempotentes,
//! para o cliente HTTP poder repeti-los num 429 ou 5xx. As listas grandes são divididas em
//! lotes ([`ID_BATCH`], [`HASH_BATCH`]).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::DeserializeOwned;
use warden_core::CancellationToken;
use warden_http::{HttpClient, Url};

use crate::cache::{Cached, MetadataCache};
use crate::error::{Error, Result};
use crate::model::{
    CategoryTag, GameVersionTag, HashAlgorithm, LoaderTag, Project, SearchResults, Version,
};
use crate::query::{HashesBody, SearchQuery, UpdateBody, UpdateFilter, VersionFilter, json_list};

/// Endereço da API.
pub const DEFAULT_BASE_URL: &str = "https://api.modrinth.com/v2/";

/// Variável que troca o endereço da API, só em build de debug (ARCHITECTURE §17.1: servidor de
/// fixtures dos E2E).
pub const BASE_URL_ENV: &str = "WARDEN_API_BASE_MODRINTH";

/// IDs por requisição em `/projects?ids=` e `/versions?ids=` (a URL fica com menos de 2 KB).
pub const ID_BATCH: usize = 100;

/// Hashes por requisição em `/version_files` e `/version_files/update`. A API não documenta
/// limite; 1000 cobre um pack grande numa requisição só.
pub const HASH_BATCH: usize = 1000;

/// Cliente do Modrinth. Barato de clonar.
#[derive(Debug, Clone)]
pub struct ModrinthClient {
    http: HttpClient,
    base: Url,
    cache: Option<MetadataCache>,
    requests: Arc<AtomicU64>,
}

impl ModrinthClient {
    /// Cliente com o endereço oficial (ou o de `WARDEN_API_BASE_MODRINTH`, em build de debug)
    /// e o cache dado (`None`: sem cache).
    pub fn new(http: HttpClient, cache: Option<MetadataCache>) -> Result<Self> {
        let base = base_url_from_env().unwrap_or_else(|| DEFAULT_BASE_URL.to_owned());
        Self::with_base_url(http, &base, cache)
    }

    /// Cliente com outro endereço da API (testes).
    pub fn with_base_url(
        http: HttpClient,
        base: &str,
        cache: Option<MetadataCache>,
    ) -> Result<Self> {
        let mut base = warden_http::parse_url(base)?;
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        Ok(Self {
            http,
            base,
            cache,
            requests: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Endereço da API em uso.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base
    }

    /// O cache em uso.
    #[must_use]
    pub fn cache(&self) -> Option<&MetadataCache> {
        self.cache.as_ref()
    }

    /// Requisições mandadas por este cliente e seus clones (sem contar novas tentativas).
    #[must_use]
    pub fn request_count(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }

    fn endpoint(&self, path: &str, params: &[(&str, String)]) -> Result<Url> {
        let mut url = self
            .base
            .join(path)
            .map_err(|e| Error::Internal(format!("endereço {path:?}: {e}")))?;
        if !params.is_empty() {
            let mut query = url.query_pairs_mut();
            for (name, value) in params {
                query.append_pair(name, value);
            }
        }
        Ok(url)
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        url: Url,
        cancel: Option<&CancellationToken>,
    ) -> Result<(T, serde_json::Value)> {
        let mut request = self.http.get(url);
        if let Some(cancel) = cancel {
            request = request.cancel(cancel);
        }
        self.requests.fetch_add(1, Ordering::Relaxed);
        let raw: serde_json::Value = request.json().await?;
        Ok((decode(raw.clone())?, raw))
    }

    async fn post_json<T: DeserializeOwned, B: serde::Serialize + ?Sized>(
        &self,
        url: Url,
        body: &B,
        cancel: Option<&CancellationToken>,
    ) -> Result<(T, serde_json::Value)> {
        let mut request = self.http.post_json(url, body)?.idempotent(true);
        if let Some(cancel) = cancel {
            request = request.cancel(cancel);
        }
        self.requests.fetch_add(1, Ordering::Relaxed);
        let raw: serde_json::Value = request.json().await?;
        Ok((decode(raw.clone())?, raw))
    }

    /// Busca (`GET /search`). Não usa o cache persistente.
    pub async fn search(
        &self,
        query: &SearchQuery,
        cancel: Option<&CancellationToken>,
    ) -> Result<SearchResults> {
        let url = self.endpoint("search", &query.to_params())?;
        Ok(self.get_json(url, cancel).await?.0)
    }

    /// Projeto pelo ID ou slug (`GET /project/{id}`), com cache de 24 h. Sem rede, devolve o
    /// que houver no cache, mesmo vencido. Erro: [`Error::ProjectNotFound`].
    pub async fn project(
        &self,
        id_or_slug: &str,
        cancel: Option<&CancellationToken>,
    ) -> Result<Project> {
        let cached = self.cached_project(id_or_slug).await;
        if let Some(cached) = cached.as_ref().filter(|cached| cached.fresh) {
            return Ok(cached.value.clone());
        }
        let url = self.endpoint(&format!("project/{}", encode_segment(id_or_slug)), &[])?;
        match self.get_json::<Project>(url, cancel).await {
            Ok((project, raw)) => {
                self.store_projects(vec![(project.clone(), raw.to_string())])
                    .await;
                Ok(project)
            }
            Err(Error::Http(warden_http::Error::NotFound { .. })) => Err(Error::ProjectNotFound {
                id: id_or_slug.to_owned(),
            }),
            Err(error) => offline_fallback(error, cached),
        }
    }

    /// Projetos pelos IDs ou slugs (`GET /projects?ids=`), em lotes, com cache de 24 h. A API
    /// omite os que não existem: o resultado pode ter menos itens que o pedido (compare pelos
    /// IDs). Sem rede, devolve os do cache.
    pub async fn projects(
        &self,
        ids: &[String],
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<Project>> {
        let ids = unique(ids);
        let mut found: HashMap<String, Project> = HashMap::new();
        let mut stale: HashMap<String, Project> = HashMap::new();
        let mut missing = Vec::new();
        for id in &ids {
            match self.cached_project(id).await {
                Some(cached) if cached.fresh => {
                    found.insert(id.clone(), cached.value);
                }
                Some(cached) => {
                    stale.insert(id.clone(), cached.value);
                    missing.push(id.clone());
                }
                None => missing.push(id.clone()),
            }
        }
        for batch in missing.chunks(ID_BATCH) {
            let url = self.endpoint("projects", &[("ids", json_list(batch))])?;
            match self.get_json::<Vec<Project>>(url, cancel).await {
                Ok((projects, raw)) => {
                    let raw_jsons = raw_items(&raw);
                    let rows: Vec<(Project, String)> =
                        projects.iter().cloned().zip(raw_jsons).collect();
                    self.store_projects(rows).await;
                    for project in projects {
                        let key = batch
                            .iter()
                            .find(|id| **id == project.id || id.eq_ignore_ascii_case(&project.slug))
                            .cloned()
                            .unwrap_or_else(|| project.id.clone());
                        found.insert(key, project);
                    }
                }
                Err(error)
                    if error.is_offline() && batch.iter().all(|id| stale.contains_key(id)) =>
                {
                    tracing::warn!(%error, "Modrinth indisponível; usando projetos do cache");
                    for id in batch {
                        if let Some(project) = stale.remove(id) {
                            found.insert(id.clone(), project);
                        }
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Ok(ids.iter().filter_map(|id| found.remove(id)).collect())
    }

    /// Versões de um projeto (`GET /project/{id}/version`), filtradas no servidor. As versões
    /// recebidas vão para o cache (marcadas sem notas quando `include_changelog` é falso).
    pub async fn project_versions(
        &self,
        id_or_slug: &str,
        filter: &VersionFilter,
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<Version>> {
        let url = self.endpoint(
            &format!("project/{}/version", encode_segment(id_or_slug)),
            &filter.to_params(),
        )?;
        match self.get_json::<Vec<Version>>(url, cancel).await {
            Ok((versions, raw)) => {
                self.store_versions(&versions, &raw, filter.include_changelog)
                    .await;
                Ok(versions)
            }
            Err(Error::Http(warden_http::Error::NotFound { .. })) => Err(Error::ProjectNotFound {
                id: id_or_slug.to_owned(),
            }),
            Err(error) => Err(error),
        }
    }

    /// Versão pelo ID (`GET /version/{id}`), com as notas. Versões não vencem no cache. Erro:
    /// [`Error::VersionNotFound`].
    pub async fn version(&self, id: &str, cancel: Option<&CancellationToken>) -> Result<Version> {
        if let Some(cache) = &self.cache
            && let Some(version) = cache_read(cache.versions(&[id.to_owned()], true).await)
                .and_then(|mut versions| versions.pop())
        {
            return Ok(version);
        }
        let url = self.endpoint(&format!("version/{}", encode_segment(id)), &[])?;
        match self.get_json::<Version>(url, cancel).await {
            Ok((version, raw)) => {
                self.store_versions(
                    std::slice::from_ref(&version),
                    &serde_json::json!([raw]),
                    true,
                )
                .await;
                Ok(version)
            }
            Err(Error::Http(warden_http::Error::NotFound { .. })) => {
                Err(Error::VersionNotFound { id: id.to_owned() })
            }
            Err(error) => Err(error),
        }
    }

    /// Versões pelos IDs (`GET /versions?ids=`), em lotes, com cache. A API omite as que não
    /// existem.
    pub async fn versions(
        &self,
        ids: &[String],
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<Version>> {
        let ids = unique(ids);
        let mut found: HashMap<String, Version> = HashMap::new();
        if let Some(cache) = &self.cache
            && let Some(cached) = cache_read(cache.versions(&ids, true).await)
        {
            found.extend(
                cached
                    .into_iter()
                    .map(|version| (version.id.clone(), version)),
            );
        }
        let missing: Vec<String> = ids
            .iter()
            .filter(|id| !found.contains_key(*id))
            .cloned()
            .collect();
        for batch in missing.chunks(ID_BATCH) {
            let url = self.endpoint("versions", &[("ids", json_list(batch))])?;
            let (versions, raw) = self.get_json::<Vec<Version>>(url, cancel).await?;
            self.store_versions(&versions, &raw, true).await;
            found.extend(
                versions
                    .into_iter()
                    .map(|version| (version.id.clone(), version)),
            );
        }
        Ok(ids.iter().filter_map(|id| found.remove(id)).collect())
    }

    /// Identifica arquivos pelo hash (`POST /version_files`): hash → versão. Hashes que o
    /// Modrinth não conhece ficam de fora do mapa.
    pub async fn version_files(
        &self,
        hashes: &[String],
        algorithm: HashAlgorithm,
        cancel: Option<&CancellationToken>,
    ) -> Result<HashMap<String, Version>> {
        let hashes = unique_lowercase(hashes);
        let mut result = HashMap::new();
        for batch in hashes.chunks(HASH_BATCH) {
            let url = self.endpoint("version_files", &[])?;
            let body = HashesBody {
                hashes: batch,
                algorithm,
            };
            let (map, raw) = self
                .post_json::<HashMap<String, Version>, _>(url, &body, cancel)
                .await?;
            self.store_version_map(&map, &raw).await;
            result.extend(map);
        }
        Ok(result)
    }

    /// Versão mais nova compatível para cada arquivo (`POST /version_files/update`): hash →
    /// versão. Uma requisição para até [`HASH_BATCH`] hashes (base de CA-T10-01). Hashes sem
    /// versão compatível ficam de fora do mapa.
    pub async fn version_files_update(
        &self,
        hashes: &[String],
        algorithm: HashAlgorithm,
        filter: &UpdateFilter,
        cancel: Option<&CancellationToken>,
    ) -> Result<HashMap<String, Version>> {
        let hashes = unique_lowercase(hashes);
        let mut result = HashMap::new();
        for batch in hashes.chunks(HASH_BATCH) {
            let url = self.endpoint("version_files/update", &[])?;
            let body = UpdateBody {
                hashes: batch,
                algorithm,
                loaders: &filter.loaders,
                game_versions: &filter.game_versions,
                version_types: filter.version_types.iter().map(|t| t.as_str()).collect(),
            };
            let (map, raw) = self
                .post_json::<HashMap<String, Version>, _>(url, &body, cancel)
                .await?;
            self.store_version_map(&map, &raw).await;
            result.extend(map);
        }
        Ok(result)
    }

    /// Loaders (`GET /tag/loader`), com cache de 24 h.
    pub async fn loaders(&self, cancel: Option<&CancellationToken>) -> Result<Vec<LoaderTag>> {
        self.tag("loader", cancel).await
    }

    /// Versões do Minecraft (`GET /tag/game_version`), com cache de 24 h.
    pub async fn game_versions(
        &self,
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<GameVersionTag>> {
        self.tag("game_version", cancel).await
    }

    /// Categorias (`GET /tag/category`), com cache de 24 h.
    pub async fn categories(&self, cancel: Option<&CancellationToken>) -> Result<Vec<CategoryTag>> {
        self.tag("category", cancel).await
    }

    async fn tag<T: DeserializeOwned>(
        &self,
        kind: &str,
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<T>> {
        let cached = match &self.cache {
            Some(cache) => cache_read(cache.tag(kind).await).flatten(),
            None => None,
        };
        let parsed = cached.and_then(|cached| {
            serde_json::from_str::<Vec<T>>(&cached.value)
                .ok()
                .map(|value| Cached {
                    value,
                    fetched_at: cached.fetched_at,
                    fresh: cached.fresh,
                })
        });
        let stale = match parsed {
            Some(cached) if cached.fresh => return Ok(cached.value),
            other => other,
        };
        let url = self.endpoint(&format!("tag/{kind}"), &[])?;
        match self.get_json::<Vec<T>>(url, cancel).await {
            Ok((value, raw)) => {
                if let Some(cache) = &self.cache {
                    cache_write(cache.put_tag(kind, raw.to_string()).await);
                }
                Ok(value)
            }
            Err(error) => offline_fallback(error, stale),
        }
    }

    async fn cached_project(&self, id_or_slug: &str) -> Option<Cached<Project>> {
        let cache = self.cache.as_ref()?;
        cache_read(cache.project(id_or_slug).await).flatten()
    }

    async fn store_projects(&self, rows: Vec<(Project, String)>) {
        if let Some(cache) = &self.cache {
            cache_write(cache.put_projects(rows).await);
        }
    }

    async fn store_versions(&self, versions: &[Version], raw: &serde_json::Value, complete: bool) {
        if let Some(cache) = &self.cache {
            let rows = versions.iter().cloned().zip(raw_items(raw)).collect();
            cache_write(cache.put_versions(rows, complete).await);
        }
    }

    async fn store_version_map(&self, map: &HashMap<String, Version>, raw: &serde_json::Value) {
        let Some(cache) = &self.cache else {
            return;
        };
        let Some(object) = raw.as_object() else {
            return;
        };
        let rows: Vec<(Version, String)> = map
            .iter()
            .filter_map(|(hash, version)| Some((version.clone(), object.get(hash)?.to_string())))
            .collect();
        // `version_files` traz as notas da versão.
        cache_write(cache.put_versions(rows, true).await);
    }
}

/// Endereço da API vindo do ambiente, só em build de debug.
fn base_url_from_env() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::env::var(BASE_URL_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

fn decode<T: DeserializeOwned>(raw: serde_json::Value) -> Result<T> {
    serde_json::from_value(raw).map_err(|e| Error::InvalidResponse(e.to_string()))
}

/// O JSON cru de cada item de uma lista.
fn raw_items(raw: &serde_json::Value) -> Vec<String> {
    raw.as_array()
        .map(|items| items.iter().map(serde_json::Value::to_string).collect())
        .unwrap_or_default()
}

/// Sem rede: o valor do cache, se houver; senão, o erro.
fn offline_fallback<T>(error: Error, cached: Option<Cached<T>>) -> Result<T> {
    match cached {
        Some(cached) if error.is_offline() => {
            tracing::warn!(%error, buscado_em = cached.fetched_at, "Modrinth indisponível; usando o cache");
            Ok(cached.value)
        }
        _ => Err(error),
    }
}

/// Falha do cache não impede a consulta: fica registrada e a API responde.
fn cache_read<T>(result: Result<T>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(%error, "falha ao ler o cache do Modrinth");
            None
        }
    }
}

fn cache_write(result: Result<()>) {
    if let Err(error) = result {
        tracing::warn!(%error, "falha ao gravar o cache do Modrinth");
    }
}

/// Sem repetidos, na ordem.
fn unique(items: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    items
        .iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty() && seen.insert(item.clone()))
        .collect()
}

/// Hashes em minúsculas, sem repetidos (a API devolve as chaves em minúsculas).
fn unique_lowercase(items: &[String]) -> Vec<String> {
    let lowered: Vec<String> = items.iter().map(|item| item.to_ascii_lowercase()).collect();
    unique(&lowered)
}

/// Um trecho de caminho da URL (IDs e slugs são alfanuméricos, mas não confiamos nisso).
fn encode_segment(segment: &str) -> String {
    let segment = segment.trim();
    if segment.chars().all(|c| c == '.') {
        // `.` e `..` mudariam de pasta na URL.
        return segment.replace('.', "%2E");
    }
    url::form_urlencoded::byte_serialize(segment.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repetidos_e_maiusculas() {
        let items = vec![
            "AA".to_owned(),
            " aa ".to_owned(),
            "AA".to_owned(),
            String::new(),
        ];
        assert_eq!(unique(&items), vec!["AA", "aa"]);
        assert_eq!(unique_lowercase(&items), vec!["aa"]);
    }

    #[test]
    fn segmento_codificado() {
        assert_eq!(encode_segment("sodium"), "sodium");
        assert_eq!(encode_segment("a/b c"), "a%2Fb%20c");
        assert_eq!(encode_segment("../x"), "..%2Fx");
        assert_eq!(encode_segment(".."), "%2E%2E");
    }
}
