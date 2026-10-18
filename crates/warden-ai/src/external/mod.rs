//! As consultas da D-14 (ARCHITECTURE §9.5, tabela "Leitura"; ADR-0030; CA-T14-03 e CA-T14-13):
//! as declarações de `search_mod_issues`, `get_mod_changelog`, `get_mixin_report` e
//! `get_bisect_result` ([`tools`]), a busca de issues no GitHub ([`GithubIssues`]) e a parte
//! pura dos changelogs ([`changelog_range`], [`html_to_text`]).
//!
//! - **GitHub:** o repositório vem do `issues_url` ou do `source_url` do mod (Modrinth ou
//!   CurseForge) e só vale `https://github.com/<dono>/<repo>`. A consulta enviada é só
//!   `repo:<dono>/<repo> is:issue <termos>`: os termos passam por [`search_terms`] (palavras
//!   simples, já redigidas; nenhuma linha de log, caminho, endereço ou dado pessoal). O token do
//!   GitHub entra quando existe (30 buscas por minuto; sem ele, 10); a mesma consulta dentro de
//!   1 hora vem do cache, sem rede. O pedido exato (`GET <url>`) volta em
//!   [`ExternalRequest`], para o bloco "Enviado ao GitHub".
//! - **Changelogs:** quem busca as versões é a `warden-app`, pelos clientes do Modrinth e da
//!   CurseForge que o app já tem (cache, chave e limites deles); aqui fica só escolher as versões
//!   entre a instalada e a pedida e montar os itens citáveis, redigidos.
//! - `get_mixin_report` (D-10) e `get_bisect_result` (D-12) leem dados locais; as declarações
//!   ficam aqui com as outras consultas da D-14, e quem lê é a `warden-app`.
//!
//! Tudo o que vem da internet (títulos de issues, changelogs) é texto de terceiros: o resultado
//! vai marcado como dado, não instrução ([`crate::tools::ToolOutput::third_party`]).

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::json;
use warden_core::CancellationToken;
use warden_diagnostics::Redactor;
use warden_http::{HeaderName, HeaderValue, HostPolicy, HttpClient, HttpConfig, Url, nonzero};

use crate::tools::{Needs, Stage, ToolItem, ToolSpec};

#[cfg(test)]
mod tests;

/// `search_mod_issues`.
pub const SEARCH_MOD_ISSUES: &str = "search_mod_issues";
/// `get_mod_changelog`.
pub const GET_MOD_CHANGELOG: &str = "get_mod_changelog";
/// `get_mixin_report`.
pub const GET_MIXIN_REPORT: &str = "get_mixin_report";
/// `get_bisect_result`.
pub const GET_BISECT_RESULT: &str = "get_bisect_result";

/// API pública do GitHub.
pub const GITHUB_API: &str = "https://api.github.com/";
/// Variável (só em build de debug) que troca a API do GitHub pelos servidores simulados (a
/// mesma da publicação, V-03).
pub const GITHUB_API_ENV: &str = "WARDEN_API_BASE_GITHUB";

/// Issues devolvidas por busca.
pub const MAX_ISSUES: usize = 5;
/// Quanto tempo uma busca fica no cache.
pub const CACHE_TTL: Duration = Duration::from_secs(3600);
/// Buscas por minuto com o token do GitHub (limite da API de busca autenticada).
const AUTHED_PER_MINUTE: u32 = 30;
/// Buscas por minuto sem token.
const ANON_PER_MINUTE: u32 = 10;
/// Palavras numa busca.
const MAX_TERMS: usize = 8;
/// Versões num changelog (as mais próximas da versão pedida).
pub const MAX_CHANGELOG_VERSIONS: usize = 12;
/// Caracteres de cada entrada de changelog.
const MAX_CHANGELOG_CHARS: usize = 4000;
/// Caracteres do título de uma issue.
const MAX_TITLE_CHARS: usize = 300;

/// As declarações das consultas da D-14, no mesmo esquema da D-04 (declaradas desde o começo,
/// liberadas por `allowedFunctionNames`): o resultado da busca do culpado desde a primeira
/// rodada; changelog, issues e raio-x quando a IA já abriu um suspeito.
#[must_use]
pub fn tools() -> Vec<ToolSpec> {
    let mod_id = json!({ "type": "string", "description": "Id do mod no pack, como devolvido por list_mods (o id depois de 'mod:')." });
    vec![
        ToolSpec {
            name: GET_BISECT_RESULT,
            description: "Resultado da busca do culpado do pack (rodadas que o Warden já testou ligando e desligando mods): situação, mods culpados encontrados e o que cada rodada mostrou.",
            parameters: json!({ "type": "object", "properties": {} }),
            stage: Stage::Initial,
            needs: Needs::Nothing,
            opens_suspects: false,
        },
        ToolSpec {
            name: GET_MIXIN_REPORT,
            description: "Raio-x de mixins: o que o mod altera no jogo e os pontos que outros mods do pack também alteram (sobreposições, com o risco). Sem mod_id, as sobreposições de risco alto e médio do pack inteiro.",
            parameters: json!({ "type": "object", "properties": { "mod_id": mod_id }, "required": [] }),
            stage: Stage::AfterSuspect,
            needs: Needs::Nothing,
            opens_suspects: false,
        },
        ToolSpec {
            name: GET_MOD_CHANGELOG,
            description: "Changelog do mod entre duas versões (Modrinth ou CurseForge), para ver se uma versão mais nova corrige o erro. Sem from, a versão instalada; sem to, a mais nova compatível com o pack.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "mod_id": mod_id,
                    "from": { "type": "string", "description": "Versão de partida (número ou id); sem ela, a instalada." },
                    "to": { "type": "string", "description": "Versão de chegada (número ou id); sem ela, a mais nova compatível." }
                },
                "required": ["mod_id"]
            }),
            stage: Stage::AfterSuspect,
            needs: Needs::Nothing,
            opens_suspects: false,
        },
        ToolSpec {
            name: SEARCH_MOD_ISSUES,
            description: "Procura até 5 issues parecidas no repositório do mod no GitHub. Envie só palavras do erro (nome da exceção, classe, método), nunca linhas de log, caminhos ou nomes de jogador.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "mod_id": mod_id,
                    "query": { "type": "string", "description": "Até 8 palavras do erro, separadas por espaço, como NullPointerException travel." }
                },
                "required": ["mod_id", "query"]
            }),
            stage: Stage::AfterSuspect,
            needs: Needs::Github,
            opens_suspects: false,
        },
    ]
}

/// Para qual serviço foi um pedido feito por uma ferramenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ExternalService {
    /// API do GitHub.
    Github,
}

/// Um pedido feito a outro serviço durante uma consulta da IA ("Enviado ao GitHub").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRequest {
    /// O serviço.
    pub service: ExternalService,
    /// O pedido exatamente como saiu (`GET https://api.github.com/search/issues?q=…`), sem
    /// cabeçalhos (o token nunca aparece).
    pub request: String,
    /// O repositório (`dono/repo`).
    pub repository: String,
    /// Os termos enviados.
    pub terms: String,
    /// A resposta veio do cache desta execução: nada foi enviado de novo.
    pub cached: bool,
}

/// Repositório validado: sempre `github.com/<dono>/<repo>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GithubRepo {
    /// Dono.
    pub owner: String,
    /// Nome.
    pub name: String,
}

impl GithubRepo {
    /// O repositório pelo `issues_url` (primeiro) ou pelo `source_url`; outros servidores não
    /// contam.
    #[must_use]
    pub fn from_links(issues_url: Option<&str>, source_url: Option<&str>) -> Option<Self> {
        issues_url
            .and_then(Self::from_url)
            .or_else(|| source_url.and_then(Self::from_url))
    }

    /// Só HTTPS no domínio exato `github.com`, sem usuário, senha nem porta, com dono e nome
    /// válidos (`/issues`, `/tree/…` e `.git` no fim são aceitos).
    #[must_use]
    pub fn from_url(value: &str) -> Option<Self> {
        let url = Url::parse(value.trim()).ok()?;
        if url.scheme() != "https"
            || url.host_str() != Some("github.com")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            return None;
        }
        let mut parts = url.path_segments()?;
        let owner = parts.next()?.to_owned();
        let name = parts.next()?;
        let name = name.strip_suffix(".git").unwrap_or(name).to_owned();
        (valid_repo_part(&owner) && valid_repo_part(&name)).then_some(Self { owner, name })
    }

    /// `dono/repo`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

fn valid_repo_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// Erros das consultas, devolvidos à IA como texto (nunca com o token).
#[derive(Debug, thiserror::Error)]
pub enum ExternalError {
    /// O mod não tem repositório no GitHub.
    #[error("o mod não tem repositório no GitHub (nem issues_url nem source_url do GitHub)")]
    NoRepository,
    /// O GitHub não achou o repositório (apagado, renomeado ou privado).
    #[error("o GitHub não achou o repositório {repository} (apagado, renomeado ou privado)")]
    RepositoryNotFound {
        /// `dono/repo`.
        repository: String,
    },
    /// A consulta tem algo além de palavras do erro.
    #[error(
        "a consulta precisa ser só palavras do erro (nome da exceção, classe, método), sem linhas de log, caminhos, endereços ou dados pessoais"
    )]
    UnsafeQuery,
    /// O limite de buscas do GitHub acabou.
    #[error(
        "o limite de buscas do GitHub acabou por agora; responda sem a busca ou tente mais tarde"
    )]
    RateLimited,
    /// As versões pedidas não estão na lista.
    #[error("a versão {version} não está entre as versões compatíveis do mod")]
    VersionNotFound {
        /// A versão pedida.
        version: String,
    },
    /// `from` não é anterior a `to`.
    #[error("a versão de partida precisa ser anterior à de chegada")]
    WrongOrder,
    /// Erro de rede ou do servidor.
    #[error("consulta ao GitHub: {0}")]
    Http(#[from] warden_http::Error),
}

/// Uma issue resumida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    /// Número.
    pub number: u64,
    /// Título (redigido, até 300 caracteres).
    pub title: String,
    /// Aberta.
    pub open: bool,
    /// Endereço público.
    pub url: String,
}

/// O resultado de uma busca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueSearch {
    /// O repositório.
    pub repository: GithubRepo,
    /// Até cinco issues (sem pull requests).
    pub issues: Vec<Issue>,
    /// O pedido, para o bloco "Enviado ao GitHub".
    pub request: ExternalRequest,
}

impl IssueSearch {
    /// Os itens citáveis (`issue:<dono>/<repo>#<n>`).
    #[must_use]
    pub fn items(&self) -> Vec<ToolItem> {
        let repository = self.repository.label();
        if self.issues.is_empty() {
            return vec![ToolItem::new(
                format!("issue:{repository}"),
                format!(
                    "Nenhuma issue encontrada em {repository} com: {}",
                    self.request.terms
                ),
            )];
        }
        self.issues
            .iter()
            .map(|issue| {
                let state = if issue.open { "aberta" } else { "fechada" };
                ToolItem::new(
                    format!("issue:{repository}#{}", issue.number),
                    format!("#{} ({state}) {} {}", issue.number, issue.title, issue.url),
                )
            })
            .collect()
    }
}

/// Os termos que vão ao GitHub: até 8 palavras simples (letras, números, `_`, `-`, `.`, `$`),
/// sem repetição; palavras de 1 ou 2 letras saem. Recusa ([`ExternalError::UnsafeQuery`]) o
/// que tem cara de log, caminho, endereço ou dado pessoal: quebra de linha, `/`, `\`, `:`,
/// `@`, `<`, `>`, `=`, ou qualquer trecho que a redação trocaria por marcador.
pub fn search_terms(input: &str, redactor: &Redactor) -> Result<String, ExternalError> {
    if input.contains(['\n', '\r', '/', '\\', ':', '@', '<', '>', '=', '"', '\'']) {
        return Err(ExternalError::UnsafeQuery);
    }
    if redactor.redact(input).text != input {
        return Err(ExternalError::UnsafeQuery);
    }
    let mut words: Vec<&str> = Vec::new();
    for word in input.split_whitespace() {
        if !word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'$'))
            || word.len() > 80
        {
            return Err(ExternalError::UnsafeQuery);
        }
        let word = word.trim_matches('.');
        if word.chars().count() >= 3 && !words.contains(&word) {
            words.push(word);
        }
    }
    if words.is_empty() || words.len() > MAX_TERMS {
        return Err(ExternalError::UnsafeQuery);
    }
    Ok(words.join(" "))
}

/// A busca de issues no GitHub, com cache e limites. Fica no estado do app (o cache vale para
/// todas as conversas da execução).
pub struct GithubIssues {
    api: Url,
    anon: HttpClient,
    authed: HttpClient,
    cache: Mutex<HashMap<String, (Instant, Vec<Issue>)>>,
    ttl: Duration,
}

impl std::fmt::Debug for GithubIssues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubIssues")
            .field("api", &self.api.as_str())
            .finish_non_exhaustive()
    }
}

impl GithubIssues {
    /// A API oficial (ou a de [`GITHUB_API_ENV`], em build de debug).
    pub fn new(config: &HttpConfig) -> Result<Self, ExternalError> {
        #[cfg(debug_assertions)]
        if let Some(base) = std::env::var(GITHUB_API_ENV)
            .ok()
            .filter(|base| !base.trim().is_empty())
        {
            return Self::with_api(config, &base);
        }
        Self::with_api(config, GITHUB_API)
    }

    /// Outra API (servidores simulados nos testes).
    pub fn with_api(config: &HttpConfig, api: &str) -> Result<Self, ExternalError> {
        let mut api = warden_http::parse_url(api)?;
        if !api.path().ends_with('/') {
            let path = format!("{}/", api.path());
            api.set_path(&path);
        }
        let host = api.host_str().unwrap_or("api.github.com").to_owned();
        let client = |per_minute: u32| {
            HttpClient::new(config.clone().with_host(
                &host,
                HostPolicy::rate(nonzero(per_minute), nonzero(2)).with_max_concurrent(2),
            ))
        };
        Ok(Self {
            api,
            anon: client(ANON_PER_MINUTE)?,
            authed: client(AUTHED_PER_MINUTE)?,
            cache: Mutex::new(HashMap::new()),
            ttl: CACHE_TTL,
        })
    }

    /// Outro tempo de cache (testes).
    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// O endereço exato da busca.
    fn url(&self, repository: &GithubRepo, terms: &str) -> Result<Url, ExternalError> {
        let mut url =
            self.api
                .join("search/issues")
                .map_err(|error| warden_http::Error::InvalidUrl {
                    url: self.api.to_string(),
                    reason: error.to_string(),
                })?;
        url.query_pairs_mut()
            .append_pair(
                "q",
                &format!("repo:{} is:issue {terms}", repository.label()),
            )
            .append_pair("per_page", &MAX_ISSUES.to_string());
        Ok(url)
    }

    /// O pedido exato que [`GithubIssues::search`] manda, para o bloco "Enviado ao GitHub"
    /// também quando a busca falha depois de sair (limite, repositório renomeado, rede).
    pub fn request(
        &self,
        repository: &GithubRepo,
        terms: &str,
    ) -> Result<ExternalRequest, ExternalError> {
        let url = self.url(repository, terms)?;
        Ok(ExternalRequest {
            service: ExternalService::Github,
            request: format!("GET {url}"),
            repository: repository.label(),
            terms: terms.to_owned(),
            cached: false,
        })
    }

    /// Até cinco issues de `repository` com `terms` (já passados por [`search_terms`]). Títulos
    /// redigidos com `redactor`.
    pub async fn search(
        &self,
        repository: &GithubRepo,
        terms: &str,
        token: Option<&SecretString>,
        redactor: &Redactor,
        cancel: Option<&CancellationToken>,
    ) -> Result<IssueSearch, ExternalError> {
        let url = self.url(repository, terms)?;
        let key = url.to_string();
        let request = self.request(repository, terms)?;
        if let Some(issues) = self.cached(&key) {
            return Ok(IssueSearch {
                repository: repository.clone(),
                issues,
                request: ExternalRequest {
                    cached: true,
                    ..request
                },
            });
        }
        let client = if token.is_some() {
            &self.authed
        } else {
            &self.anon
        };
        let mut builder = client
            .get(url)
            .header(
                HeaderName::from_static("accept"),
                HeaderValue::from_static("application/vnd.github+json"),
            )
            .header(
                HeaderName::from_static("x-github-api-version"),
                HeaderValue::from_static("2022-11-28"),
            )
            .max_body(512 * 1024);
        if let Some(token) = token {
            let bearer = SecretString::from(format!("Bearer {}", token.expose_secret()));
            builder = builder.secret_header(HeaderName::from_static("authorization"), &bearer)?;
        }
        if let Some(cancel) = cancel {
            builder = builder.cancel(cancel);
        }
        let response: SearchResponse = builder.json().await.map_err(|error| match error {
            // O GitHub responde 403 quando o limite de buscas acaba (e 429 no secundário).
            warden_http::Error::RateLimited { .. }
            | warden_http::Error::UnexpectedStatus { status: 403, .. } => {
                ExternalError::RateLimited
            }
            warden_http::Error::UnexpectedStatus { status: 422, .. }
            | warden_http::Error::NotFound { .. } => ExternalError::RepositoryNotFound {
                repository: repository.label(),
            },
            other => ExternalError::Http(other),
        })?;
        let issues: Vec<Issue> = response
            .items
            .into_iter()
            .filter(|item| item.pull_request.is_none())
            .take(MAX_ISSUES)
            .map(|item| Issue {
                number: item.number,
                title: truncate(&redactor.redact(&item.title).text, MAX_TITLE_CHARS),
                open: item.state == "open",
                url: item.html_url,
            })
            .collect();
        self.store(key, &issues);
        Ok(IssueSearch {
            repository: repository.clone(),
            issues,
            request,
        })
    }

    fn cached(&self, key: &str) -> Option<Vec<Issue>> {
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        cache
            .get(key)
            .filter(|(at, _)| at.elapsed() < self.ttl)
            .map(|(_, issues)| issues.clone())
    }

    fn store(&self, key: String, issues: &[Issue]) {
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        cache.retain(|_, (at, _)| at.elapsed() < self.ttl);
        cache.insert(key, (Instant::now(), issues.to_vec()));
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    items: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    number: u64,
    title: String,
    html_url: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
}

/// Uma versão de um mod com as notas, como a fonte devolve (da mais nova para a mais antiga).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogVersion {
    /// Id na fonte (versão do Modrinth ou arquivo da CurseForge).
    pub id: String,
    /// Número legível.
    pub number: String,
    /// Data de publicação (RFC 3339).
    pub published: String,
    /// As notas, em texto (o HTML da CurseForge já convertido por [`html_to_text`]).
    pub text: String,
}

/// As versões depois de `from` até `to` (inclusive), da mais nova para a mais antiga, numa
/// lista da mais nova para a mais antiga. `from` e `to` valem pelo id ou pelo número. Quando
/// passam de [`MAX_CHANGELOG_VERSIONS`], ficam as mais próximas de `to` e o resto é contado.
pub fn changelog_range<'a>(
    versions: &'a [ChangelogVersion],
    from: &str,
    to: &str,
) -> Result<(Vec<&'a ChangelogVersion>, usize), ExternalError> {
    let find = |wanted: &str| {
        let wanted = wanted.trim();
        versions
            .iter()
            .position(|version| version.id == wanted || version.number == wanted)
            .ok_or_else(|| ExternalError::VersionNotFound {
                version: wanted.to_owned(),
            })
    };
    let newest = find(to)?;
    let oldest = find(from)?;
    if newest >= oldest {
        return Err(ExternalError::WrongOrder);
    }
    let range: Vec<&ChangelogVersion> = versions[newest..oldest].iter().collect();
    let left_out = range.len().saturating_sub(MAX_CHANGELOG_VERSIONS);
    Ok((
        range.into_iter().take(MAX_CHANGELOG_VERSIONS).collect(),
        left_out,
    ))
}

/// Os itens citáveis de um changelog (`changelog:<mod>@<versão>`), redigidos e limitados.
#[must_use]
pub fn changelog_items(
    mod_id: &str,
    versions: &[&ChangelogVersion],
    left_out: usize,
    redactor: &Redactor,
) -> Vec<ToolItem> {
    let mut items: Vec<ToolItem> = versions
        .iter()
        .map(|version| {
            let date = version.published.get(..10).unwrap_or("");
            let notes = version.text.trim();
            let notes = if notes.is_empty() {
                "(sem notas)".to_owned()
            } else {
                truncate(&redactor.redact(notes).text, MAX_CHANGELOG_CHARS)
            };
            ToolItem::new(
                format!("changelog:{mod_id}@{}", version.number),
                format!("Versão {} ({date}):\n{notes}", version.number),
            )
        })
        .collect();
    if left_out > 0 {
        items.push(ToolItem::new(
            format!("changelog:{mod_id}"),
            format!("Mais {left_out} versões antigas no intervalo ficaram de fora."),
        ));
    }
    items
}

/// Texto simples de um changelog em HTML (CurseForge): tags viram espaço (`<br>`, `<p>` e
/// `<li>` viram quebra de linha) e as entidades comuns voltam a ser caracteres.
#[must_use]
pub fn html_to_text(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        output.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + end]
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        output.push(
            if matches!(
                tag.as_str(),
                "br" | "p" | "li" | "div" | "h1" | "h2" | "h3" | "h4" | "tr"
            ) {
                '\n'
            } else {
                ' '
            },
        );
        rest = &rest[start + end + 1..];
    }
    output.push_str(rest);
    let decoded = output
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    decoded
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut short: String = value.chars().take(limit).collect();
    short.push('…');
    short
}
