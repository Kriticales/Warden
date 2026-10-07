//! Cliente da API da CurseForge (Core API v1; R3 §3.2; ARCHITECTURE §17).
//!
//! - A chave do usuário vai no cabeçalho `x-api-key`, marcado como sensível (nunca registrado,
//!   nunca no `Debug`). Sem chave, nenhum método manda requisição: todos devolvem
//!   [`Error::KeyMissing`] antes (CA-T08-06). 401 e 403 da API viram [`Error::KeyInvalid`].
//! - Lotes sempre que possível: `POST /mods`, `POST /mods/files` e `POST /fingerprints/432`,
//!   marcados como idempotentes para a `warden-http` poder repeti-los num 429 ou 5xx. A API omite
//!   os IDs que não existem, então o resultado é sempre comparado com o pedido.
//! - Cache só em memória ([`MemoryCache`]): nada da API vai para o disco (termos da CurseForge).
//! - Downloads da CDN: [`CurseforgeClient::download_headers`] devolve o `x-api-key` para a
//!   `warden-instance` (L-03) e a exportação (E-03, E-04), só para os servidores da CurseForge
//!   (R7 §9 item 2, ADR-0051). O download em si é da `warden-http`, que resolve o
//!   redirecionamento sem `Range` antes de pedir partes (o `edge.forgecdn.net` responde 404 a
//!   `Range`) e só repassa o cabeçalho sensível para o mesmo site.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use secrecy::{ExposeSecret as _, SecretString};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;
use warden_http::{
    DownloadRequest, HeaderMap, HeaderName, HeaderValue, HttpClient, Request, SystemTimer, Url,
};
use warden_packwiz::HashFormat;

use crate::cache::MemoryCache;
use crate::error::{Error, Result};
use crate::model::{
    Category, Distribution, Envelope, File, FileDistribution, FilePage, FileRef,
    FingerprintMatches, FingerprintResponse, MINECRAFT_GAME_ID, Mod, Paged, ProjectClass,
    SearchResults,
};
use crate::query::{FilesQuery, SearchQuery};

/// Endereço da API.
pub const DEFAULT_BASE_URL: &str = "https://api.curseforge.com/v1/";

/// Variável que troca o endereço da API, só em build de debug (ARCHITECTURE §17.1: servidor de
/// fixtures dos E2E).
pub const BASE_URL_ENV: &str = "WARDEN_API_BASE_CURSEFORGE";

/// Cabeçalho da chave.
pub const API_KEY_HEADER: &str = "x-api-key";

/// Sites da CurseForge que recebem a chave nos downloads (a CDN `edge.forgecdn.net`, o
/// redirecionamento para `mediafilez.forgecdn.net` e os endereços do próprio site).
pub const CDN_SITES: [&str; 2] = ["forgecdn.net", "curseforge.com"];

/// IDs por requisição em `POST /mods` e `POST /mods/files` (a API não documenta limite; o
/// packwiz manda o pack inteiro de uma vez).
pub const ID_BATCH: usize = 500;

/// Impressões digitais por requisição em `POST /fingerprints/432`.
pub const FINGERPRINT_BATCH: usize = 1000;

/// Cliente da CurseForge. Barato de clonar: os clones dividem conexões, cache e contador.
#[derive(Clone)]
pub struct CurseforgeClient {
    http: HttpClient,
    base: Url,
    key: Option<SecretString>,
    cache: MemoryCache,
    requests: Arc<AtomicU64>,
}

impl fmt::Debug for CurseforgeClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Nunca a chave: só se há uma.
        f.debug_struct("CurseforgeClient")
            .field("base", &self.base.as_str())
            .field("has_key", &self.key.is_some())
            .field("cache", &self.cache)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModIdsBody<'a> {
    mod_ids: &'a [u64],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileIdsBody<'a> {
    file_ids: &'a [u64],
}

#[derive(Serialize)]
struct FingerprintsBody<'a> {
    fingerprints: &'a [u32],
}

/// Resposta de `GET /games/432`, usada só para testar a chave.
#[derive(Deserialize)]
struct Game {
    #[serde(default)]
    id: u32,
}

impl CurseforgeClient {
    /// Cliente com o endereço oficial (ou o de `WARDEN_API_BASE_CURSEFORGE`, em build de debug),
    /// a chave dada (`None` ou vazia: sem chave) e um cache em memória novo.
    pub fn new(http: HttpClient, key: Option<SecretString>) -> Result<Self> {
        let base = base_url_from_env().unwrap_or_else(|| DEFAULT_BASE_URL.to_owned());
        Self::with_base_url(http, &base, key)
    }

    /// Cliente com outro endereço da API (testes e servidor de fixtures).
    pub fn with_base_url(http: HttpClient, base: &str, key: Option<SecretString>) -> Result<Self> {
        let mut base = warden_http::parse_url(base)?;
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        let key = key.filter(|key| !key.expose_secret().trim().is_empty());
        Ok(Self {
            http,
            base,
            key,
            cache: MemoryCache::new(Arc::new(SystemTimer::new())),
            requests: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Usa outro cache em memória (o app guarda um para a sessão inteira e cria clientes com a
    /// chave lida na hora; os testes passam um com [`warden_http::ManualTimer`]).
    #[must_use]
    pub fn with_cache(mut self, cache: MemoryCache) -> Self {
        self.cache = cache;
        self
    }

    /// Endereço da API em uso.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base
    }

    /// Se há chave. Sem ela, a fonte CurseForge fica de fora (SPEC T08).
    #[must_use]
    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    /// O cache em memória.
    #[must_use]
    pub fn cache(&self) -> &MemoryCache {
        &self.cache
    }

    /// Requisições à API mandadas por este cliente e seus clones (sem contar novas tentativas
    /// nem downloads).
    #[must_use]
    pub fn request_count(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }

    fn key(&self) -> Result<&SecretString> {
        self.key.as_ref().ok_or(Error::KeyMissing)
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

    fn authorize(&self, request: Request, cancel: Option<&CancellationToken>) -> Result<Request> {
        let key = self.key()?;
        let mut request = request
            .secret_header(HeaderName::from_static(API_KEY_HEADER), key)
            .map_err(|_| Error::KeyInvalid { status: None })?;
        if let Some(cancel) = cancel {
            request = request.cancel(cancel);
        }
        Ok(request)
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
        cancel: Option<&CancellationToken>,
    ) -> Result<T> {
        self.key()?;
        let url = self.endpoint(path, params)?;
        let request = self.authorize(self.http.get(url), cancel)?;
        self.send(request, path).await
    }

    async fn post<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        cancel: Option<&CancellationToken>,
    ) -> Result<T> {
        self.key()?;
        let url = self.endpoint(path, &[])?;
        let request = self.http.post_json(url, body)?.idempotent(true);
        let request = self.authorize(request, cancel)?;
        self.send(request, path).await
    }

    async fn send<T: DeserializeOwned>(&self, request: Request, path: &str) -> Result<T> {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let body = request.bytes().await.map_err(map_http_error)?;
        // Só o caminho e o tamanho: o corpo é da CurseForge (ARCHITECTURE §16).
        tracing::debug!(
            endpoint = path,
            bytes = body.len(),
            "resposta da CurseForge"
        );
        decode(&body)
    }

    /// Confere a chave na API (`GET /games/432`): o "Testar" de Configurações → Chaves e
    /// contas (CA-T21-02). Erros: [`Error::KeyMissing`] e [`Error::KeyInvalid`].
    pub async fn check_key(&self, cancel: Option<&CancellationToken>) -> Result<()> {
        let game: Envelope<Game> = self
            .get(&format!("games/{MINECRAFT_GAME_ID}"), &[], cancel)
            .await?;
        if game.data.id != MINECRAFT_GAME_ID {
            return Err(Error::InvalidResponse(format!(
                "o jogo {MINECRAFT_GAME_ID} voltou com o ID {}",
                game.data.id
            )));
        }
        Ok(())
    }

    /// Busca (`GET /mods/search`), com cache em memória de 5 minutos. Os limites da API são
    /// conferidos antes ([`Error::InvalidQuery`]).
    pub async fn search(
        &self,
        query: &SearchQuery,
        cancel: Option<&CancellationToken>,
    ) -> Result<SearchResults> {
        query.validate()?;
        self.key()?;
        if let Some(results) = self.cache.search(query) {
            return Ok(results);
        }
        let page: Paged<Mod> = self.get("mods/search", &query.to_params(), cancel).await?;
        let results = SearchResults {
            mods: page.data,
            pagination: page.pagination,
        };
        self.cache.put_search(query.clone(), results.clone());
        Ok(results)
    }

    /// Projeto pelo ID (`GET /mods/{id}`), com cache em memória. Erro: [`Error::ModNotFound`].
    pub async fn project(&self, id: u64, cancel: Option<&CancellationToken>) -> Result<Mod> {
        self.key()?;
        if let Some(project) = self.cache.project(id) {
            return Ok(project);
        }
        let project: Envelope<Mod> = self
            .get(&format!("mods/{id}"), &[], cancel)
            .await
            .map_err(|error| not_found(error, || Error::ModNotFound { id }))?;
        self.cache.put_projects(std::slice::from_ref(&project.data));
        Ok(project.data)
    }

    /// Projetos pelos IDs (`POST /mods`), em lotes, com cache em memória. A API omite os que não
    /// existem: o resultado pode ter menos itens que o pedido (compare pelos IDs). A ordem é a
    /// do pedido.
    pub async fn projects(
        &self,
        ids: &[u64],
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<Mod>> {
        self.key()?;
        let ids = unique(ids);
        let mut found: HashMap<u64, Mod> = HashMap::new();
        let mut missing = Vec::new();
        for id in &ids {
            match self.cache.project(*id) {
                Some(project) => {
                    found.insert(*id, project);
                }
                None => missing.push(*id),
            }
        }
        for batch in missing.chunks(ID_BATCH) {
            let projects: Envelope<Vec<Mod>> = none_found_is_empty(
                self.post("mods", &ModIdsBody { mod_ids: batch }, cancel)
                    .await,
            )?;
            self.cache.put_projects(&projects.data);
            found.extend(
                projects
                    .data
                    .into_iter()
                    .map(|project| (project.id, project)),
            );
        }
        Ok(ids.iter().filter_map(|id| found.remove(id)).collect())
    }

    /// Descrição em HTML de um projeto (`GET /mods/{id}/description`), só em memória.
    pub async fn description(&self, id: u64, cancel: Option<&CancellationToken>) -> Result<String> {
        self.key()?;
        if let Some(text) = self.cache.description(id) {
            return Ok(text);
        }
        let text: Envelope<String> = self
            .get(&format!("mods/{id}/description"), &[], cancel)
            .await
            .map_err(|error| not_found(error, || Error::ModNotFound { id }))?;
        self.cache.put_description(id, text.data.clone());
        Ok(text.data)
    }

    /// Arquivos de um projeto (`GET /mods/{id}/files`), filtrados no servidor, do mais novo para
    /// o mais antigo. Erro: [`Error::ModNotFound`].
    pub async fn files(
        &self,
        mod_id: u64,
        query: &FilesQuery,
        cancel: Option<&CancellationToken>,
    ) -> Result<FilePage> {
        query.validate()?;
        let page: Paged<File> = self
            .get(&format!("mods/{mod_id}/files"), &query.to_params(), cancel)
            .await
            .map_err(|error| not_found(error, || Error::ModNotFound { id: mod_id }))?;
        self.cache.put_files(&page.data);
        Ok(FilePage {
            files: page.data,
            pagination: page.pagination,
        })
    }

    /// Um arquivo (`GET /mods/{modId}/files/{fileId}`), com cache em memória. Erro:
    /// [`Error::FileNotFound`].
    pub async fn file(
        &self,
        mod_id: u64,
        file_id: u64,
        cancel: Option<&CancellationToken>,
    ) -> Result<File> {
        self.key()?;
        if let Some(file) = self
            .cache
            .file(file_id)
            .filter(|file| file.mod_id == mod_id)
        {
            return Ok(file);
        }
        let file: Envelope<File> = self
            .get(&format!("mods/{mod_id}/files/{file_id}"), &[], cancel)
            .await
            .map_err(|error| {
                not_found(error, || Error::FileNotFound {
                    mod_id: Some(mod_id),
                    file_id,
                })
            })?;
        self.cache.put_files(std::slice::from_ref(&file.data));
        Ok(file.data)
    }

    /// Notas de um arquivo em HTML (`GET /mods/{modId}/files/{fileId}/changelog`), só em
    /// memória por quem chama (termos da CurseForge). Erro: [`Error::FileNotFound`].
    pub async fn file_changelog(
        &self,
        mod_id: u64,
        file_id: u64,
        cancel: Option<&CancellationToken>,
    ) -> Result<String> {
        let text: Envelope<String> = self
            .get(
                &format!("mods/{mod_id}/files/{file_id}/changelog"),
                &[],
                cancel,
            )
            .await
            .map_err(|error| {
                not_found(error, || Error::FileNotFound {
                    mod_id: Some(mod_id),
                    file_id,
                })
            })?;
        Ok(text.data)
    }

    /// Arquivos pelos IDs (`POST /mods/files`), em lotes, com cache em memória. A API omite os
    /// que não existem; a ordem é a do pedido.
    pub async fn files_by_id(
        &self,
        ids: &[u64],
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<File>> {
        self.key()?;
        let ids = unique(ids);
        let mut found: HashMap<u64, File> = HashMap::new();
        let mut missing = Vec::new();
        for id in &ids {
            match self.cache.file(*id) {
                Some(file) => {
                    found.insert(*id, file);
                }
                None => missing.push(*id),
            }
        }
        for batch in missing.chunks(ID_BATCH) {
            let files: Envelope<Vec<File>> = none_found_is_empty(
                self.post("mods/files", &FileIdsBody { file_ids: batch }, cancel)
                    .await,
            )?;
            self.cache.put_files(&files.data);
            found.extend(files.data.into_iter().map(|file| (file.id, file)));
        }
        Ok(ids.iter().filter_map(|id| found.remove(id)).collect())
    }

    /// Endereço de download de um arquivo (`GET /mods/{modId}/files/{fileId}/download-url`),
    /// nunca guardado. A API responde 403 tanto para chave recusada quanto para arquivo com
    /// distribuição bloqueada; o Warden desfaz a dúvida pedindo o arquivo: se ele vem sem
    /// `downloadUrl`, o erro é [`Error::DistributionBlocked`] (com a página para o download
    /// manual); se o arquivo também é recusado, [`Error::KeyInvalid`].
    pub async fn download_url(
        &self,
        mod_id: u64,
        file_id: u64,
        cancel: Option<&CancellationToken>,
    ) -> Result<Url> {
        let result: Result<Envelope<Option<String>>> = self
            .get(
                &format!("mods/{mod_id}/files/{file_id}/download-url"),
                &[],
                cancel,
            )
            .await;
        let text = match result {
            Ok(Envelope { data: Some(url) }) if !url.trim().is_empty() => url,
            Ok(_) | Err(Error::KeyInvalid { status: Some(403) }) => {
                let file = self.file(mod_id, file_id, cancel).await?;
                match file.download_url() {
                    Some(url) => url.to_owned(),
                    None => return Err(self.blocked_error(&file, cancel).await),
                }
            }
            Err(error) => {
                return Err(not_found(error, || Error::FileNotFound {
                    mod_id: Some(mod_id),
                    file_id,
                }));
            }
        };
        parse_download_url(&text)
    }

    /// O erro de distribuição bloqueada de um arquivo, com a página do arquivo no site quando
    /// o projeto puder ser lido.
    async fn blocked_error(&self, file: &File, cancel: Option<&CancellationToken>) -> Error {
        let page_url = match self.project(file.mod_id, cancel).await {
            Ok(project) => project.file_page_url(file.id),
            Err(error) => {
                tracing::debug!(%error, mod_id = file.mod_id, "página do arquivo bloqueado indisponível");
                None
            }
        };
        Error::DistributionBlocked {
            mod_id: file.mod_id,
            file_id: file.id,
            file_name: (!file.file_name.is_empty()).then(|| file.file_name.clone()),
            page_url,
        }
    }

    /// Identifica arquivos pela impressão digital (murmur2 da CurseForge, calculado por
    /// `warden_packwiz::hash::curseforge_fingerprint` ou pelo download da `warden-http`):
    /// `POST /fingerprints/432`, em lotes.
    pub async fn fingerprints(
        &self,
        fingerprints: &[u32],
        cancel: Option<&CancellationToken>,
    ) -> Result<FingerprintMatches> {
        self.key()?;
        let fingerprints = unique(fingerprints);
        let mut matches = HashMap::new();
        for batch in fingerprints.chunks(FINGERPRINT_BATCH) {
            let response: Envelope<FingerprintResponse> = self
                .post(
                    &format!("fingerprints/{MINECRAFT_GAME_ID}"),
                    &FingerprintsBody {
                        fingerprints: batch,
                    },
                    cancel,
                )
                .await?;
            let wanted: HashSet<u32> = batch.iter().copied().collect();
            for found in response.data.exact_matches {
                let fingerprint = found.file.file_fingerprint;
                if wanted.contains(&fingerprint) {
                    self.cache.put_files(std::slice::from_ref(&found.file));
                    matches.insert(fingerprint, found);
                }
            }
        }
        let unmatched = fingerprints
            .into_iter()
            .filter(|fingerprint| !matches.contains_key(fingerprint))
            .collect();
        Ok(FingerprintMatches { matches, unmatched })
    }

    /// Situação de cada arquivo para o download (detecção de distribuição bloqueada): os
    /// arquivos em lote (`POST /mods/files`) e, só para os bloqueados, os projetos em lote
    /// (`POST /mods`) para montar a página do download manual, como faz o packwiz. A ordem é a
    /// do pedido; arquivos que a API não devolveu (ou de outro projeto) ficam
    /// [`Distribution::Missing`].
    pub async fn distribution(
        &self,
        files: &[FileRef],
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<FileDistribution>> {
        let ids: Vec<u64> = files.iter().map(|file| file.file_id).collect();
        let found: HashMap<u64, File> = self
            .files_by_id(&ids, cancel)
            .await?
            .into_iter()
            .map(|file| (file.id, file))
            .collect();
        let blocked_projects: Vec<u64> = files
            .iter()
            .filter(|wanted| {
                found.get(&wanted.file_id).is_some_and(|file| {
                    file.mod_id == wanted.mod_id && file.is_distribution_blocked()
                })
            })
            .map(|wanted| wanted.mod_id)
            .collect();
        let projects: HashMap<u64, Mod> = if blocked_projects.is_empty() {
            HashMap::new()
        } else {
            self.projects(&blocked_projects, cancel)
                .await?
                .into_iter()
                .map(|project| (project.id, project))
                .collect()
        };
        Ok(files
            .iter()
            .map(|wanted| {
                let file = found
                    .get(&wanted.file_id)
                    .filter(|file| file.mod_id == wanted.mod_id);
                let distribution = match file {
                    None => Distribution::Missing,
                    Some(file) => match file.download_url() {
                        Some(url) => Distribution::Allowed {
                            url: url.to_owned(),
                        },
                        None => Distribution::Blocked {
                            page_url: projects
                                .get(&wanted.mod_id)
                                .and_then(|project| project.file_page_url(wanted.file_id)),
                        },
                    },
                };
                FileDistribution {
                    file: *wanted,
                    file_name: file.map(|file| file.file_name.clone()),
                    distribution,
                }
            })
            .collect())
    }

    /// Categorias do Minecraft (`GET /categories?gameId=432`), de uma classe ou de todas.
    pub async fn categories(
        &self,
        class: Option<ProjectClass>,
        cancel: Option<&CancellationToken>,
    ) -> Result<Vec<Category>> {
        let mut params = vec![("gameId", MINECRAFT_GAME_ID.to_string())];
        if let Some(class) = class {
            params.push(("classId", class.id().to_string()));
        }
        let categories: Envelope<Vec<Category>> = self.get("categories", &params, cancel).await?;
        Ok(categories.data)
    }

    /// Classes do Minecraft (`GET /categories?gameId=432&classesOnly=true`).
    pub async fn classes(&self, cancel: Option<&CancellationToken>) -> Result<Vec<Category>> {
        let params = [
            ("gameId", MINECRAFT_GAME_ID.to_string()),
            ("classesOnly", "true".to_owned()),
        ];
        let classes: Envelope<Vec<Category>> = self.get("categories", &params, cancel).await?;
        Ok(classes.data)
    }

    /// Se `url` é de um servidor da CurseForge que deve receber a chave num download:
    /// `*.forgecdn.net` e `*.curseforge.com` por https, ou o servidor da própria API em uso
    /// (o servidor de fixtures dos testes e dos E2E).
    #[must_use]
    pub fn is_curseforge_download(&self, url: &Url) -> bool {
        let Some(host) = url
            .host_str()
            .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
        else {
            return false;
        };
        let official = url.scheme() == "https"
            && CDN_SITES
                .iter()
                .any(|site| host == *site || host.ends_with(&format!(".{site}")));
        let api_server = url.scheme() == self.base.scheme()
            && url.host() == self.base.host()
            && url.port_or_known_default() == self.base.port_or_known_default();
        official || api_server
    }

    /// Cabeçalhos de um download de `url`: o `x-api-key` (sensível) quando há chave e o
    /// endereço é da CurseForge ([`Self::is_curseforge_download`]); vazio nos demais casos, para
    /// a chave nunca ir para outro servidor. Sem chave, o download segue sem o cabeçalho (a CDN
    /// ainda não exige; ADR-0051). Usado pela `warden-instance` (L-03) e pela exportação (E-03,
    /// E-04) com `warden_http::DownloadRequest::headers`.
    pub fn download_headers(&self, url: &Url) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        if let Some(key) = &self.key
            && self.is_curseforge_download(url)
        {
            let mut value = HeaderValue::from_str(key.expose_secret().trim())
                .map_err(|_| Error::KeyInvalid { status: None })?;
            value.set_sensitive(true);
            headers.insert(HeaderName::from_static(API_KEY_HEADER), value);
        }
        Ok(headers)
    }

    /// Pedido de download de um arquivo, pronto para `warden_http::HttpClient::download`: o
    /// endereço da API (só em memória), o tamanho e o SHA-1 esperados (ou o murmur2, se a API
    /// não deu SHA-1) e os cabeçalhos de [`Self::download_headers`]. Arquivo sem `downloadUrl`
    /// → [`Error::DistributionBlocked`].
    pub async fn download_request(
        &self,
        file: &File,
        destination: impl Into<PathBuf>,
        cancel: Option<&CancellationToken>,
    ) -> Result<DownloadRequest> {
        let Some(text) = file.download_url() else {
            return Err(self.blocked_error(file, cancel).await);
        };
        let url = parse_download_url(text)?;
        let mut request = DownloadRequest::new(url.clone(), destination);
        request.headers = self.download_headers(&url)?;
        if file.file_length > 0 {
            request = request.expect_size(file.file_length);
        }
        if let Some(sha1) = file.sha1() {
            request = request.expect_hash(HashFormat::Sha1, sha1);
        } else if file.file_fingerprint != 0 {
            request = request.expect_hash(HashFormat::Murmur2, file.file_fingerprint.to_string());
        }
        Ok(request)
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

/// 401 e 403 da API viram chave recusada. O começo do corpo de outros 4xx sai do erro (a
/// `warden-http` já faz isso para `*.curseforge.com`; aqui vale também para o servidor de
/// fixtures e para qualquer endereço configurado).
fn map_http_error(error: warden_http::Error) -> Error {
    match error {
        warden_http::Error::UnexpectedStatus {
            status: status @ (401 | 403),
            ..
        } => Error::KeyInvalid {
            status: Some(status),
        },
        warden_http::Error::UnexpectedStatus {
            host, url, status, ..
        } => Error::Http(warden_http::Error::UnexpectedStatus {
            host,
            url,
            status,
            body: None,
        }),
        other => Error::Http(other),
    }
}

/// Nos lotes (`POST /mods` e `POST /mods/files`), a API responde 404 quando **nenhum** dos IDs
/// existe, em vez de uma lista vazia (conferido em 05/10/2026).
fn none_found_is_empty<T>(result: Result<Envelope<Vec<T>>>) -> Result<Envelope<Vec<T>>> {
    match result {
        Err(Error::Http(warden_http::Error::NotFound { .. })) => Ok(Envelope { data: Vec::new() }),
        other => other,
    }
}

/// 404 vira o "não encontrado" do recurso.
fn not_found(error: Error, specific: impl FnOnce() -> Error) -> Error {
    match error {
        Error::Http(warden_http::Error::NotFound { .. }) => specific(),
        other => other,
    }
}

/// Lê o JSON sem copiar trechos da resposta para o erro (o `Display` do `serde_json` cita o
/// valor recusado, que é dado da CurseForge).
fn decode<T: DeserializeOwned>(body: &[u8]) -> Result<T> {
    serde_json::from_slice(body).map_err(|e| {
        let kind = match e.classify() {
            serde_json::error::Category::Io => "falha de leitura",
            serde_json::error::Category::Syntax => "JSON malformado",
            serde_json::error::Category::Data => "campo com tipo inesperado",
            serde_json::error::Category::Eof => "JSON incompleto",
        };
        Error::InvalidResponse(format!(
            "{kind} na linha {}, coluna {}",
            e.line(),
            e.column()
        ))
    })
}

fn parse_download_url(text: &str) -> Result<Url> {
    warden_http::parse_url(text)
        .map_err(|_| Error::InvalidResponse("endereço de download inválido".to_owned()))
}

/// Sem repetidos, na ordem.
fn unique<T: Copy + Eq + std::hash::Hash>(items: &[T]) -> Vec<T> {
    let mut seen = HashSet::new();
    items
        .iter()
        .copied()
        .filter(|item| seen.insert(*item))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_repetidos() {
        assert_eq!(unique(&[3, 1, 3, 2, 1]), vec![3, 1, 2]);
    }

    #[test]
    fn erro_de_json_sem_trecho_da_resposta() {
        let error = decode::<Envelope<u32>>(br#"{"data": "texto-da-curseforge"}"#).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("tipo inesperado"), "{text}");
        assert!(!text.contains("texto-da-curseforge"), "{text}");
        assert!(decode::<Envelope<u32>>(b"{").is_err());
        assert!(decode::<Envelope<u32>>(b"x").is_err());
    }

    #[test]
    fn chave_recusada_pelo_codigo() {
        let status = |status| warden_http::Error::UnexpectedStatus {
            host: "api.curseforge.com".into(),
            url: "u".into(),
            status,
            body: None,
        };
        assert!(matches!(
            map_http_error(status(401)),
            Error::KeyInvalid { status: Some(401) }
        ));
        assert!(matches!(
            map_http_error(status(403)),
            Error::KeyInvalid { status: Some(403) }
        ));
        let with_body = warden_http::Error::UnexpectedStatus {
            host: "x".into(),
            url: "u".into(),
            status: 400,
            body: Some("corpo".into()),
        };
        assert!(matches!(
            map_http_error(with_body),
            Error::Http(warden_http::Error::UnexpectedStatus { body: None, .. })
        ));
    }
}
