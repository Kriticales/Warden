//! A API REST do GitHub usada pela publicação (ARCHITECTURE §11.1 e §17; ADR-0028).
//!
//! - `GET /user`: o login do dono do token (identidade dos commits e dono do repositório);
//! - `POST /user/repos`: criar o repositório do pack, público por padrão;
//! - `GET /repos/{dono}/{repo}`: vincular um repositório que já existe (mostra a visibilidade);
//! - `POST /repos/{dono}/{repo}/releases` e `GET …/releases/tags/{tag}`: a Release da versão.
//!
//! Tudo pela `warden-http` (rustls, limitador, User-Agent do Warden). O token vai só no
//! cabeçalho `Authorization`, marcado como sensível: nunca entra em URL, registro, erro ou
//! `Debug` (CA-T18-02). Os endereços vêm de [`GithubEndpoints`]: os oficiais no app e um
//! servidor local nos testes.

use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize};
use url::Url;
use warden_core::{CancellationToken, CoreError};
use warden_http::{HeaderName, HeaderValue, HttpClient};

use crate::error::{Error, Result};

/// Variável (só em build de debug) que troca o endereço da API do GitHub.
pub const API_BASE_ENV: &str = "WARDEN_API_BASE_GITHUB";
/// Variável (só em build de debug) que troca o servidor git (`https://github.com/`).
pub const GIT_BASE_ENV: &str = "WARDEN_GIT_BASE_GITHUB";
/// Variável (só em build de debug) que troca o `raw.githubusercontent.com` do link do pack.
pub const RAW_BASE_ENV: &str = "WARDEN_RAW_BASE_GITHUB";

/// Versão da API pedida em todas as chamadas.
const API_VERSION: &str = "2022-11-28";

/// Permissão que o token precisa ter para cada etapa (a mensagem diz qual falta, SPEC T18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum GithubPermission {
    /// Criar repositórios (fine-grained: "Administration", leitura e escrita).
    CreateRepo,
    /// Enviar arquivos (fine-grained: "Contents", leitura e escrita).
    Push,
    /// Criar Releases (fine-grained: "Contents", leitura e escrita).
    Release,
}

impl GithubPermission {
    /// Nome estável, usado no parâmetro da frase do erro.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreateRepo => "createRepo",
            Self::Push => "push",
            Self::Release => "release",
        }
    }
}

/// Endereços do GitHub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubEndpoints {
    /// API REST (`https://api.github.com/`).
    pub api: Url,
    /// Servidor git (`https://github.com/`): o envio vai para `<git>/<dono>/<repo>.git`.
    pub git: Url,
    /// Arquivos crus (`https://raw.githubusercontent.com/`): o link do pack.
    pub raw: Url,
}

fn base(text: &str) -> Result<Url> {
    let mut url = Url::parse(text).map_err(|error| Error::Internal(format!("{text}: {error}")))?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

impl GithubEndpoints {
    /// Os endereços oficiais.
    ///
    /// # Panics
    ///
    /// Nunca: os endereços são constantes válidas.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn official() -> Self {
        Self::new(
            "https://api.github.com/",
            "https://github.com/",
            "https://raw.githubusercontent.com/",
        )
        .unwrap_or_else(|_| unreachable!("endereços oficiais válidos"))
    }

    /// Endereços dados (servidor local dos testes). Cada um ganha a barra final.
    ///
    /// Erros: [`Error::Internal`] se algum não for URL.
    pub fn new(api: &str, git: &str, raw: &str) -> Result<Self> {
        Ok(Self {
            api: base(api)?,
            git: base(git)?,
            raw: base(raw)?,
        })
    }

    /// Os oficiais; em build de debug, trocados pelos de [`API_BASE_ENV`], [`GIT_BASE_ENV`] e
    /// [`RAW_BASE_ENV`] (servidor de fixtures dos E2E). Valor inválido é ignorado.
    #[must_use]
    pub fn from_env() -> Self {
        let mut endpoints = Self::official();
        if cfg!(debug_assertions) {
            let read = |name: &str| std::env::var(name).ok().and_then(|value| base(&value).ok());
            if let Some(api) = read(API_BASE_ENV) {
                endpoints.api = api;
            }
            if let Some(git) = read(GIT_BASE_ENV) {
                endpoints.git = git;
            }
            if let Some(raw) = read(RAW_BASE_ENV) {
                endpoints.raw = raw;
            }
        }
        endpoints
    }

    /// Endereço git do repositório (`https://github.com/<dono>/<repo>.git`).
    ///
    /// Erros: [`Error::Internal`] (nome que não cabe numa URL; não acontece com nomes válidos).
    pub fn remote(&self, repo: &RepoRef) -> Result<Url> {
        self.git
            .join(&format!("{}/{}.git", repo.owner, repo.name))
            .map_err(|error| Error::Internal(format!("endereço do repositório: {error}")))
    }

    /// Link do pack: `https://raw.githubusercontent.com/<dono>/<repo>/main/pack.toml`.
    #[must_use]
    pub fn pack_link(&self, repo: &RepoRef) -> String {
        format!(
            "{}{}/{}/main/pack.toml",
            self.raw.as_str(),
            repo.owner,
            repo.name
        )
    }

    /// Página do repositório no GitHub.
    #[must_use]
    pub fn repo_page(&self, repo: &RepoRef) -> String {
        format!("{}{}/{}", self.git.as_str(), repo.owner, repo.name)
    }
}

/// Um repositório, `dono/nome`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoRef {
    /// Dono (usuário ou organização).
    pub owner: String,
    /// Nome.
    pub name: String,
}

impl RepoRef {
    /// Lê `dono/nome` (também aceita o endereço `https://github.com/dono/nome`).
    ///
    /// Erros: [`Error::GithubInvalidRepoName`].
    pub fn parse(text: &str) -> Result<Self> {
        let trimmed = text.trim();
        let path = trimmed
            .strip_prefix("https://github.com/")
            .or_else(|| trimmed.strip_prefix("github.com/"))
            .unwrap_or(trimmed)
            .trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        let invalid = || Error::GithubInvalidRepoName {
            name: trimmed.chars().take(200).collect(),
        };
        let (owner, name) = path.split_once('/').ok_or_else(invalid)?;
        let owner_ok = !owner.is_empty()
            && owner.len() <= 39
            && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !owner_ok || validate_repo_name(name).is_err() {
            return Err(invalid());
        }
        Ok(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }

    /// `dono/nome`.
    #[must_use]
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// Confere um nome de repositório como o GitHub aceita: letras, números, `.`, `_` e `-`,
/// até 100 caracteres, sem ser `.` nem `..`.
///
/// Erros: [`Error::GithubInvalidRepoName`].
pub fn validate_repo_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 100
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(())
    } else {
        Err(Error::GithubInvalidRepoName {
            name: name.chars().take(200).collect(),
        })
    }
}

/// Nome padrão do repositório: o nome do pack em minúsculas, sem acentos, com hífens
/// ("Vale Sereno" → `vale-sereno`). Sem nada aproveitável, `modpack`.
#[must_use]
pub fn default_repo_name(pack_name: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    use unicode_normalization::char::is_combining_mark;
    let mut out = String::new();
    let mut dash = false;
    for c in pack_name.nfd().filter(|c| !is_combining_mark(*c)) {
        if c.is_ascii_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c.to_ascii_lowercase());
        } else {
            dash = true;
        }
        if out.len() >= 100 {
            break;
        }
    }
    out.truncate(100);
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "modpack".to_owned()
    } else {
        out
    }
}

/// Um repositório como o GitHub informa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    /// `dono/nome`.
    pub repo: RepoRef,
    /// Privado (o link não funciona para os jogadores).
    pub private: bool,
    /// Página no GitHub.
    pub html_url: String,
}

/// Uma Release criada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    /// Id (a E-04 anexa a instância pronta para o Prism por ele).
    #[specta(type = specta_typescript::Number)]
    pub id: u64,
    /// Página da Release.
    pub html_url: String,
}

#[derive(Deserialize)]
struct UserJson {
    login: String,
}

#[derive(Deserialize)]
struct OwnerJson {
    login: String,
}

#[derive(Deserialize)]
struct RepoJson {
    name: String,
    owner: OwnerJson,
    private: bool,
    html_url: String,
}

impl From<RepoJson> for RepoInfo {
    fn from(json: RepoJson) -> Self {
        Self {
            repo: RepoRef {
                owner: json.owner.login,
                name: json.name,
            },
            private: json.private,
            html_url: json.html_url,
        }
    }
}

#[derive(Deserialize)]
struct ReleaseJson {
    id: u64,
    html_url: String,
}

impl From<ReleaseJson> for ReleaseInfo {
    fn from(json: ReleaseJson) -> Self {
        Self {
            id: json.id,
            html_url: json.html_url,
        }
    }
}

/// Converte um erro HTTP do GitHub no erro do domínio: sem conexão, token recusado (401),
/// sem permissão (403), não encontrado (404) ou falha genérica.
pub(crate) fn http_error(
    error: warden_http::Error,
    action: &'static str,
    permission: GithubPermission,
    repo: Option<&RepoRef>,
) -> Error {
    match error {
        warden_http::Error::Cancelled => Error::Core(CoreError::Cancelled),
        warden_http::Error::Network { .. } | warden_http::Error::Timeout { .. } => {
            Error::GithubOffline {
                action,
                source: Box::new(error),
            }
        }
        _ => match (error.status(), repo) {
            (Some(401), _) => Error::GithubTokenRejected { action },
            (Some(403), _) => Error::GithubPermissionMissing { permission, action },
            (Some(404), Some(repo)) => Error::GithubRepoNotFound {
                repo: repo.full_name(),
            },
            _ => Error::GithubFailed {
                action,
                source: Box::new(error),
            },
        },
    }
}

/// Cliente da API do GitHub com o token do usuário (lido do cofre na hora do uso).
#[derive(Clone)]
pub struct GithubClient {
    http: HttpClient,
    endpoints: GithubEndpoints,
    token: SecretString,
}

impl std::fmt::Debug for GithubClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubClient")
            .field("api", &self.endpoints.api.as_str())
            .finish_non_exhaustive()
    }
}

impl GithubClient {
    /// Cliente com o token dado.
    #[must_use]
    pub fn new(http: HttpClient, endpoints: GithubEndpoints, token: SecretString) -> Self {
        Self {
            http,
            endpoints,
            token,
        }
    }

    /// Os endereços em uso.
    #[must_use]
    pub fn endpoints(&self) -> &GithubEndpoints {
        &self.endpoints
    }

    pub(crate) fn http(&self) -> &HttpClient {
        &self.http
    }

    /// `Authorization: Basic base64("x-access-token:<token>")`, o formato que o servidor git
    /// do GitHub aceita para tokens (clássicos e fine-grained).
    pub(crate) fn basic_auth(&self) -> SecretString {
        use base64::Engine as _;
        let raw = format!("x-access-token:{}", self.token.expose_secret());
        SecretString::from(format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        ))
    }

    fn bearer(&self) -> SecretString {
        SecretString::from(format!("Bearer {}", self.token.expose_secret()))
    }

    fn url(&self, path: &str) -> Result<Url> {
        self.endpoints
            .api
            .join(path)
            .map_err(|error| Error::Internal(format!("endereço da API: {error}")))
    }

    fn api_headers(&self, request: warden_http::Request) -> Result<warden_http::Request> {
        request
            .header(
                HeaderName::from_static("accept"),
                HeaderValue::from_static("application/vnd.github+json"),
            )
            .header(
                HeaderName::from_static("x-github-api-version"),
                HeaderValue::from_static(API_VERSION),
            )
            .secret_header(HeaderName::from_static("authorization"), &self.bearer())
            .map_err(|error| Error::GithubFailed {
                action: "montar o pedido",
                source: Box::new(error),
            })
    }

    /// O login do dono do token. Serve também para "Testar" o token em Configurações.
    ///
    /// Erros: [`Error::GithubTokenRejected`], [`Error::GithubOffline`], [`Error::GithubFailed`].
    pub async fn user(&self, cancel: Option<&CancellationToken>) -> Result<String> {
        const ACTION: &str = "conferir o token";
        let mut request = self.api_headers(self.http.get(self.url("user")?))?;
        if let Some(cancel) = cancel {
            request = request.cancel(cancel);
        }
        let user: UserJson = request
            .json()
            .await
            .map_err(|error| http_error(error, ACTION, GithubPermission::Push, None))?;
        Ok(user.login)
    }

    /// Um repositório existente (vincular). Erros: [`Error::GithubRepoNotFound`] e os de
    /// [`GithubClient::user`].
    pub async fn repo(&self, repo: &RepoRef) -> Result<RepoInfo> {
        const ACTION: &str = "ler o repositório";
        let url = self.url(&format!("repos/{}/{}", repo.owner, repo.name))?;
        let json: RepoJson = self
            .api_headers(self.http.get(url))?
            .json()
            .await
            .map_err(|error| http_error(error, ACTION, GithubPermission::Push, Some(repo)))?;
        Ok(json.into())
    }

    /// Cria o repositório na conta do token, público (`private = false`, o recomendado) ou
    /// privado ("só backup").
    ///
    /// Erros: [`Error::GithubRepoNameTaken`] (422), [`Error::GithubPermissionMissing`] com
    /// [`GithubPermission::CreateRepo`], [`Error::GithubInvalidRepoName`] e os de
    /// [`GithubClient::user`].
    pub async fn create_repo(
        &self,
        owner: &str,
        name: &str,
        private: bool,
        description: &str,
    ) -> Result<RepoInfo> {
        const ACTION: &str = "criar o repositório";
        validate_repo_name(name)?;
        let body = serde_json::json!({
            "name": name,
            "private": private,
            "description": description,
            "has_issues": false,
            "has_wiki": false,
            "has_projects": false,
            "auto_init": false,
        });
        let request = self
            .http
            .post_json(self.url("user/repos")?, &body)
            .map_err(|error| Error::GithubFailed {
                action: ACTION,
                source: Box::new(error),
            })?;
        let repo = RepoRef {
            owner: owner.to_owned(),
            name: name.to_owned(),
        };
        let json: RepoJson =
            self.api_headers(request)?
                .json()
                .await
                .map_err(|error| match error.status() {
                    Some(422) => Error::GithubRepoNameTaken {
                        repo: repo.full_name(),
                    },
                    // Token fine-grained sem "Administration" recebe 403 ou 404 aqui.
                    Some(404) => Error::GithubPermissionMissing {
                        permission: GithubPermission::CreateRepo,
                        action: ACTION,
                    },
                    _ => http_error(error, ACTION, GithubPermission::CreateRepo, None),
                })?;
        Ok(json.into())
    }

    /// A Release de uma tag, se existir.
    ///
    /// Erros: os de [`GithubClient::user`].
    pub async fn release_by_tag(&self, repo: &RepoRef, tag: &str) -> Result<Option<ReleaseInfo>> {
        const ACTION: &str = "ler a Release";
        let url = self.url(&format!(
            "repos/{}/{}/releases/tags/{tag}",
            repo.owner, repo.name
        ))?;
        match self
            .api_headers(self.http.get(url))?
            .json::<ReleaseJson>()
            .await
        {
            Ok(json) => Ok(Some(json.into())),
            Err(error) if error.status() == Some(404) => Ok(None),
            Err(error) => Err(http_error(error, ACTION, GithubPermission::Release, None)),
        }
    }

    /// Cria a Release da tag (já enviada). Se ela já existir (uma tentativa anterior cuja
    /// resposta se perdeu), devolve a existente.
    ///
    /// Erros: [`Error::GithubPermissionMissing`] com [`GithubPermission::Release`] e os de
    /// [`GithubClient::user`].
    pub async fn create_release(
        &self,
        repo: &RepoRef,
        tag: &str,
        name: &str,
        body: &str,
    ) -> Result<ReleaseInfo> {
        const ACTION: &str = "criar a Release";
        if let Some(existing) = self.release_by_tag(repo, tag).await? {
            return Ok(existing);
        }
        let json = serde_json::json!({
            "tag_name": tag,
            "name": name,
            "body": body,
            "draft": false,
            "prerelease": false,
        });
        let url = self.url(&format!("repos/{}/{}/releases", repo.owner, repo.name))?;
        let request = self
            .http
            .post_json(url, &json)
            .map_err(|error| Error::GithubFailed {
                action: ACTION,
                source: Box::new(error),
            })?;
        let created: ReleaseJson =
            self.api_headers(request)?.json().await.map_err(|error| {
                http_error(error, ACTION, GithubPermission::Release, Some(repo))
            })?;
        Ok(created.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_padrao_do_repositorio() {
        assert_eq!(default_repo_name("Vale Sereno"), "vale-sereno");
        assert_eq!(default_repo_name("  Ação & Magia 2!  "), "acao-magia-2");
        assert_eq!(default_repo_name("já"), "ja");
        assert_eq!(default_repo_name("✦✦"), "modpack");
        assert_eq!(default_repo_name(&"a".repeat(300)).len(), 100);
    }

    #[test]
    fn dono_e_nome() {
        let repo = RepoRef::parse(" kriticales/vale-sereno ").unwrap();
        assert_eq!(repo.full_name(), "kriticales/vale-sereno");
        assert_eq!(
            RepoRef::parse("https://github.com/a-b/c.d_e.git")
                .unwrap()
                .full_name(),
            "a-b/c.d_e"
        );
        for bad in [
            "",
            "sem-barra",
            "a/",
            "/b",
            "a/b/c",
            "a b/c",
            "a/..",
            "a/ção",
        ] {
            assert!(RepoRef::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn link_do_pack_e_endereco_git() {
        let endpoints = GithubEndpoints::official();
        let repo = RepoRef::parse("dono/pack").unwrap();
        assert_eq!(
            endpoints.pack_link(&repo),
            "https://raw.githubusercontent.com/dono/pack/main/pack.toml"
        );
        assert_eq!(
            endpoints.remote(&repo).unwrap().as_str(),
            "https://github.com/dono/pack.git"
        );
        let local = GithubEndpoints::new(
            "http://127.0.0.1:1/api",
            "http://127.0.0.1:1/git",
            "http://127.0.0.1:1/raw",
        )
        .unwrap();
        assert_eq!(
            local.pack_link(&repo),
            "http://127.0.0.1:1/raw/dono/pack/main/pack.toml"
        );
    }

    #[test]
    fn debug_nao_mostra_o_token() {
        let client = GithubClient::new(
            HttpClient::new(warden_http::HttpConfig::default()).unwrap(),
            GithubEndpoints::official(),
            SecretString::from("ghp_segredoQueNaoPodeAparecer000000".to_owned()),
        );
        let text = format!("{client:?}");
        assert!(!text.contains("ghp_"), "{text}");
    }
}
