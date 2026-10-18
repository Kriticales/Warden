//! Consultas externas da IA. Nenhum texto bruto de log sai para o GitHub.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use warden_diagnostics::Redactor;
use warden_http::{HeaderName, HostPolicy, HttpClient, HttpConfig, Url, nonzero};

/// Endereços das APIs; a substituição permite servidores simulados nos testes.
#[derive(Debug, Clone)]
pub struct Endpoints {
    /// API do GitHub.
    pub github: Url,
    /// API do Modrinth.
    pub modrinth: Url,
    /// API da CurseForge.
    pub curseforge: Url,
}

impl Endpoints {
    /// APIs públicas usadas em produção.
    pub fn production() -> Result<Self, ExternalError> {
        Ok(Self {
            github: Url::parse("https://api.github.com/")
                .map_err(|_| ExternalError::InvalidResponse)?,
            modrinth: Url::parse("https://api.modrinth.com/v2/")
                .map_err(|_| ExternalError::InvalidResponse)?,
            curseforge: Url::parse("https://api.curseforge.com/v1/")
                .map_err(|_| ExternalError::InvalidResponse)?,
        })
    }
}

/// URLs de origem vindas dos metadados de um mod.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModLinks {
    /// `source_url` do Modrinth ou `links.sourceUrl` da CurseForge.
    pub source_url: Option<String>,
    /// `issues_url` do Modrinth ou `links.issuesUrl` da CurseForge.
    pub issues_url: Option<String>,
}

/// Repositório validado, sempre `github.com/<owner>/<repo>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GithubRepo {
    /// Dono do repositório.
    pub owner: String,
    /// Nome do repositório.
    pub name: String,
}

impl GithubRepo {
    /// Extrai o repositório de `issues_url` ou `source_url`, ignorando hosts não GitHub.
    #[must_use]
    pub fn from_links(links: &ModLinks) -> Option<Self> {
        links
            .issues_url
            .as_deref()
            .and_then(Self::from_url)
            .or_else(|| links.source_url.as_deref().and_then(Self::from_url))
    }

    /// Aceita somente HTTPS no domínio exato e dois segmentos de repositório válidos.
    #[must_use]
    pub fn from_url(value: &str) -> Option<Self> {
        let url = Url::parse(value).ok()?;
        if url.scheme() != "https"
            || url.host_str() != Some("github.com")
            || url.username() != ""
            || url.password().is_some()
            || url.port().is_some()
        {
            return None;
        }
        let mut parts = url.path_segments()?;
        let owner = parts.next()?.to_owned();
        let name = parts.next()?.trim_end_matches(".git").to_owned();
        if !valid_repo_part(&owner) || !valid_repo_part(&name) {
            return None;
        }
        Some(Self { owner, name })
    }

    fn label(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

fn valid_repo_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        && value != "."
        && value != ".."
}

/// Issue resumida, sempre marcada como dado externo não confiável ao enviar à IA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    /// Id citável `issue:<owner>/<repo>#<n>`.
    pub evidence_id: String,
    /// Número da issue.
    pub number: u64,
    /// Título do GitHub, redigido.
    pub title: String,
    /// URL pública da issue.
    pub url: String,
}

/// Consulta exata enviada ao GitHub, para o bloco "Enviado ao GitHub".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSearch {
    /// Texto exato do parâmetro `q`.
    pub sent_query: String,
    /// Até cinco issues.
    pub issues: Vec<Issue>,
}

/// Fonte do histórico de mudanças.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangelogSource {
    /// Projeto Modrinth; `from` e `to` são ids ou números de versão.
    Modrinth {
        /// Id ou slug do projeto.
        project_id: String,
    },
    /// Mod CurseForge; `from` e `to` são ids dos arquivos.
    CurseForge {
        /// Id numérico do mod.
        mod_id: u64,
    },
    /// Releases GitHub; `from` e `to` são tags.
    Github {
        /// Repositório do mod.
        repository: GithubRepo,
    },
}

/// Uma versão situada estritamente depois de `from` até `to`, inclusive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangelogEntry {
    /// Id citável do changelog.
    pub evidence_id: String,
    /// Versão da entrada.
    pub version: String,
    /// Texto redigido e limitado; dado externo não confiável.
    pub text: String,
}

/// Erros de consulta devolvidos ao registro da D-04 sem expor segredos.
#[derive(Debug, Error)]
pub enum ExternalError {
    /// A conversa não permitiu GitHub.
    #[error("busca no GitHub não permitida nesta conversa")]
    PermissionDenied,
    /// Os metadados não trazem um repositório GitHub válido.
    #[error("o mod não tem repositório GitHub válido")]
    NoRepository,
    /// A consulta não contém apenas termos seguros de erro.
    #[error("use somente palavras do erro, sem logs, caminhos ou dados pessoais")]
    UnsafeQuery,
    /// A versão não está na janela consultada da fonte.
    #[error("as versões pedidas não constam do histórico consultado")]
    VersionNotFound,
    /// Chave exigida pela API não disponível.
    #[error("a chave da CurseForge não está disponível")]
    MissingCurseForgeKey,
    /// Erro de HTTP.
    #[error("consulta externa: {0}")]
    Http(#[from] warden_http::Error),
    /// Resposta inválida.
    #[error("resposta externa inválida")]
    InvalidResponse,
}

/// Cliente de consultas externas. Cache e limites são compartilhados entre clones do HTTP.
pub struct ExternalClient {
    github: HttpClient,
    http: HttpClient,
    endpoints: Endpoints,
    github_token: Option<SecretString>,
    cache: Mutex<HashMap<String, (Instant, IssueSearch)>>,
    cache_ttl: Duration,
}

impl ExternalClient {
    /// Cria os clientes com 30 buscas/minuto autenticadas ou 10 sem token.
    pub fn new(
        config: HttpConfig,
        endpoints: Endpoints,
        github_token: Option<SecretString>,
    ) -> Result<Self, ExternalError> {
        let github_host = endpoints
            .github
            .host_str()
            .ok_or(ExternalError::InvalidResponse)?;
        let rate = if github_token.is_some() { 30 } else { 10 };
        let github_config = config.clone().with_host(
            github_host,
            HostPolicy::rate(nonzero(rate), nonzero(1)).with_max_concurrent(2),
        );
        Ok(Self {
            github: HttpClient::new(github_config)?,
            http: HttpClient::new(config)?,
            endpoints,
            github_token,
            cache: Mutex::new(HashMap::new()),
            cache_ttl: Duration::from_secs(3600),
        })
    }

    /// Consulta até cinco issues. A permissão é verificada antes do cache e da rede.
    pub async fn search_mod_issues(
        &self,
        links: &ModLinks,
        words: &str,
        redactor: &Redactor,
        github_allowed: bool,
    ) -> Result<IssueSearch, ExternalError> {
        if !github_allowed {
            return Err(ExternalError::PermissionDenied);
        }
        let repo = GithubRepo::from_links(links).ok_or(ExternalError::NoRepository)?;
        let terms = safe_error_terms(words, redactor)?;
        let sent_query = format!("repo:{} is:issue {terms}", repo.label());
        if let Some((at, value)) = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&sent_query)
            && at.elapsed() < self.cache_ttl
        {
            return Ok(value.clone());
        }
        let mut url = self
            .endpoints
            .github
            .join("search/issues")
            .map_err(|_| ExternalError::InvalidResponse)?;
        url.query_pairs_mut()
            .append_pair("q", &sent_query)
            .append_pair("per_page", "5");
        let request = self.github.get(url).header(
            HeaderName::from_static("accept"),
            warden_http::HeaderValue::from_static("application/vnd.github+json"),
        );
        let request = match &self.github_token {
            Some(token) => {
                request.secret_header(HeaderName::from_static("authorization"), &bearer(token))?
            }
            None => request,
        };
        let result: GithubIssueResponse = request.max_body(512 * 1024).json().await?;
        let issues = result
            .items
            .into_iter()
            .filter(|item| item.pull_request.is_none())
            .take(5)
            .map(|item| Issue {
                evidence_id: format!("issue:{}#{}", repo.label(), item.number),
                number: item.number,
                title: truncate(&redactor.redact(&item.title).text, 500),
                url: item.html_url,
            })
            .collect();
        let search = IssueSearch {
            sent_query: sent_query.clone(),
            issues,
        };
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(sent_query, (Instant::now(), search.clone()));
        Ok(search)
    }

    /// Lê as entradas entre duas versões, sem incluir a versão inicial.
    pub async fn get_mod_changelog(
        &self,
        source: &ChangelogSource,
        from: &str,
        to: &str,
        redactor: &Redactor,
        curseforge_key: Option<&SecretString>,
    ) -> Result<Vec<ChangelogEntry>, ExternalError> {
        if from.is_empty() || to.is_empty() || from == to {
            return Err(ExternalError::VersionNotFound);
        }
        let entries = match source {
            ChangelogSource::Modrinth { project_id } => fetch_modrinth(self, project_id).await?,
            ChangelogSource::CurseForge { mod_id } => {
                let key = curseforge_key.ok_or(ExternalError::MissingCurseForgeKey)?;
                return Ok(finalize(
                    fetch_curseforge(self, *mod_id, from, to, key).await?,
                    redactor,
                ));
            }
            ChangelogSource::Github { repository } => fetch_github(self, repository).await?,
        };
        Ok(finalize(between(entries, from, to)?, redactor))
    }
}

async fn fetch_modrinth(
    client: &ExternalClient,
    project_id: &str,
) -> Result<Vec<RawEntry>, ExternalError> {
    let mut url = client
        .endpoints
        .modrinth
        .join("project")
        .map_err(|_| ExternalError::InvalidResponse)?;
    url.path_segments_mut()
        .map_err(|()| ExternalError::InvalidResponse)?
        .push(project_id)
        .push("version");
    url.query_pairs_mut()
        .append_pair("include_changelog", "true");
    let versions: Vec<ModrinthVersion> = client
        .http
        .get(url)
        .max_body(2 * 1024 * 1024)
        .json()
        .await?;
    Ok(versions
        .into_iter()
        .map(|v| RawEntry {
            id: v.id,
            version: v.version_number,
            text: v.changelog.unwrap_or_default(),
        })
        .collect())
}

async fn fetch_curseforge(
    client: &ExternalClient,
    mod_id: u64,
    from: &str,
    to: &str,
    key: &SecretString,
) -> Result<Vec<RawEntry>, ExternalError> {
    let mut files = Vec::new();
    // A API limita a página a 50; até quatro páginas mantêm a consulta curta.
    for page in 0..4 {
        let mut url = client
            .endpoints
            .curseforge
            .join("mods")
            .map_err(|_| ExternalError::InvalidResponse)?;
        url.path_segments_mut()
            .map_err(|()| ExternalError::InvalidResponse)?
            .push(&mod_id.to_string())
            .push("files");
        url.query_pairs_mut()
            .append_pair("pageSize", "50")
            .append_pair("index", &(page * 50).to_string());
        let request = client
            .http
            .get(url)
            .secret_header(HeaderName::from_static("x-api-key"), key)?;
        let batch: CurseForgeFiles = request.max_body(2 * 1024 * 1024).json().await?;
        let count = batch.data.len();
        files.extend(batch.data);
        if count < 50
            || (files.iter().any(|v| v.id.to_string() == from)
                && files.iter().any(|v| v.id.to_string() == to))
        {
            break;
        }
    }
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date).then_with(|| b.id.cmp(&a.id)));
    let raw = files
        .into_iter()
        .map(|v| RawEntry {
            id: v.id.to_string(),
            version: v.display_name,
            text: String::new(),
        })
        .collect();
    let mut range = between(raw, from, to)?;
    for entry in &mut range {
        let mut url = client
            .endpoints
            .curseforge
            .join("mods")
            .map_err(|_| ExternalError::InvalidResponse)?;
        url.path_segments_mut()
            .map_err(|()| ExternalError::InvalidResponse)?
            .push(&mod_id.to_string())
            .push("files")
            .push(&entry.id)
            .push("changelog");
        let request = client
            .http
            .get(url)
            .secret_header(HeaderName::from_static("x-api-key"), key)?;
        let response: CurseForgeChangelog = request.max_body(256 * 1024).json().await?;
        entry.text = html_to_text(&response.data);
    }
    Ok(range)
}

async fn fetch_github(
    client: &ExternalClient,
    repository: &GithubRepo,
) -> Result<Vec<RawEntry>, ExternalError> {
    let mut url = client
        .endpoints
        .github
        .join("repos")
        .map_err(|_| ExternalError::InvalidResponse)?;
    url.path_segments_mut()
        .map_err(|()| ExternalError::InvalidResponse)?
        .push(&repository.owner)
        .push(&repository.name)
        .push("releases");
    url.query_pairs_mut().append_pair("per_page", "100");
    let request = client.github.get(url);
    let request = match &client.github_token {
        Some(token) => {
            request.secret_header(HeaderName::from_static("authorization"), &bearer(token))?
        }
        None => request,
    };
    let versions: Vec<GithubRelease> = request.max_body(2 * 1024 * 1024).json().await?;
    Ok(versions
        .into_iter()
        .map(|v| RawEntry {
            id: v.tag_name.clone(),
            version: v.tag_name,
            text: v.body.unwrap_or_default(),
        })
        .collect())
}

/// Aceita no máximo oito palavras simples. Logs, caminhos, URLs e placeholders são recusados.
fn safe_error_terms(input: &str, redactor: &Redactor) -> Result<String, ExternalError> {
    if input.contains(['\n', '\r', '/', '\\', ':', '@', '<', '>']) {
        return Err(ExternalError::UnsafeQuery);
    }
    let cleaned = redactor.redact(input).text;
    if cleaned != input {
        return Err(ExternalError::UnsafeQuery);
    }
    let words = cleaned.split_whitespace().collect::<Vec<_>>();
    if words.is_empty()
        || words.len() > 8
        || words.iter().any(|word| {
            word.len() < 3
                || word.len() > 50
                || !word
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
    {
        return Err(ExternalError::UnsafeQuery);
    }
    Ok(words.join(" "))
}

fn bearer(token: &SecretString) -> SecretString {
    SecretString::new(format!("Bearer {}", token.expose_secret()).into())
}

fn truncate(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

#[derive(Debug, Deserialize)]
struct GithubIssueResponse {
    items: Vec<GithubIssue>,
}
#[derive(Debug, Deserialize)]
struct GithubIssue {
    number: u64,
    title: String,
    html_url: String,
    pull_request: Option<serde_json::Value>,
}
#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    body: Option<String>,
}
#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    id: String,
    version_number: String,
    changelog: Option<String>,
}
#[derive(Debug, Deserialize)]
struct CurseForgeFiles {
    data: Vec<CurseForgeFile>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeFile {
    id: u64,
    display_name: String,
    #[serde(default)]
    file_date: Option<String>,
}
#[derive(Debug, Deserialize)]
struct CurseForgeChangelog {
    data: String,
}

struct RawEntry {
    id: String,
    version: String,
    text: String,
}

fn between(entries: Vec<RawEntry>, from: &str, to: &str) -> Result<Vec<RawEntry>, ExternalError> {
    let start = entries
        .iter()
        .position(|v| v.id == to || v.version == to)
        .ok_or(ExternalError::VersionNotFound)?;
    let end = entries
        .iter()
        .position(|v| v.id == from || v.version == from)
        .ok_or(ExternalError::VersionNotFound)?;
    if start >= end || end - start > 20 {
        return Err(ExternalError::VersionNotFound);
    }
    Ok(entries.into_iter().skip(start).take(end - start).collect())
}

fn finalize(entries: Vec<RawEntry>, redactor: &Redactor) -> Vec<ChangelogEntry> {
    entries
        .into_iter()
        .map(|v| ChangelogEntry {
            evidence_id: format!("changelog:{}@{}", v.id, v.version),
            version: v.version,
            text: truncate(&redactor.redact(&v.text).text, 8000),
        })
        .collect()
}

fn html_to_text(html: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests;
