//! A API REST do GitHub no repositório do Warden, com o token do login (UI-03).
//!
//! Cada resposta é classificada aqui, porque o mesmo código quer dizer coisas diferentes no
//! GitHub: 401 é token vencido ou revogado; 403 é falta de permissão, a menos que o limite de
//! pedidos tenha acabado (`x-ratelimit-remaining: 0` ou `retry-after`); 404 num endereço do
//! repositório é repositório não encontrado (ou sem acesso a ele).

use std::time::{Duration, SystemTime};

use base64::Engine as _;
use secrecy::{ExposeSecret as _, SecretString};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use warden_http::headers::{ratelimit_exhausted, requested_wait};
use warden_http::{
    HeaderName, HeaderValue, HttpClient, Method, Request, Response, StatusCode, Url,
};

use crate::endpoints::{ATTACHMENTS_BRANCH, ATTACHMENTS_DIR, Endpoints};
use crate::error::{Error, Result};

/// Versão da API pedida (`X-GitHub-Api-Version`).
const API_VERSION: &str = "2022-11-28";

/// Maior resposta aceita (as da Contents API repetem metadados, nunca o arquivo).
const MAX_RESPONSE: u64 = 4 * 1024 * 1024;

/// Tempo-limite do envio de um print (um arquivo de 10 MB numa conexão lenta).
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(300);

/// Arquivo do commit órfão que cria a branch dos prints.
const BRANCH_README: &str = "# Anexos dos relatos do Warden\n\n\
Esta branch guarda os prints enviados pelo botão \"Relatar problema\" do Warden. Ela não tem \
relação com o código (não sai do `main`) e pode ser apagada: o Warden a recria no próximo \
envio, e as issues antigas perdem as imagens.\n";

/// A conta do login.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct User {
    /// Nome de usuário ("Kriticales").
    pub login: String,
}

/// Uma label do repositório.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Label {
    /// Nome ("relato-do-dono", "categoria: erro").
    pub name: String,
    /// Cor em hexadecimal, sem `#`.
    pub color: String,
    /// Descrição.
    pub description: String,
}

/// A issue criada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedIssue {
    /// Número (`#42`).
    pub number: u64,
    /// Página da issue no GitHub.
    pub url: Url,
}

/// Cliente da API com o token do login. O token nunca aparece no `Debug`.
#[derive(Clone)]
pub struct GithubApi {
    http: HttpClient,
    endpoints: Endpoints,
    authorization: SecretString,
}

impl std::fmt::Debug for GithubApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubApi")
            .field("endpoints", &self.endpoints)
            .finish_non_exhaustive()
    }
}

/// Resposta de erro do GitHub (`{"message": …}`).
#[derive(Deserialize)]
struct ErrorBody {
    message: Option<String>,
}

#[derive(Deserialize)]
struct Sha {
    sha: String,
}

#[derive(Deserialize)]
struct RefObject {
    #[serde(rename = "object")]
    _object: Sha,
}

#[derive(Deserialize)]
struct IssueReply {
    number: u64,
}

impl GithubApi {
    /// Cliente com o token de acesso.
    #[must_use]
    pub fn new(http: HttpClient, endpoints: Endpoints, token: &SecretString) -> Self {
        Self {
            http,
            endpoints,
            authorization: SecretString::from(format!("Bearer {}", token.expose_secret())),
        }
    }

    /// Os endereços em uso.
    #[must_use]
    pub fn endpoints(&self) -> &Endpoints {
        &self.endpoints
    }

    /// A conta dona do token.
    pub async fn user(&self) -> Result<User> {
        let response = self
            .send(self.request(Method::GET, self.endpoints.user_url()?)?)
            .await?;
        read_json(response).await
    }

    /// Cria a label se ela não existir. Devolve se criou.
    pub async fn ensure_label(&self, label: &Label) -> Result<bool> {
        let url = self.endpoints.repo_url(&["labels", &label.name])?;
        match self.send(self.request(Method::GET, url)?).await {
            Ok(_) => return Ok(false),
            Err(Error::RepositoryNotFound { .. }) => {}
            Err(error) => return Err(error),
        }
        let url = self.endpoints.repo_url(&["labels"])?;
        let request = self.request(Method::POST, url)?.json_body(label)?;
        match self.send(request).await {
            Ok(_) => {
                tracing::info!(label = %label.name, "label criada no repositório do Warden");
                Ok(true)
            }
            // Criada por outro envio ao mesmo tempo (`already_exists`).
            Err(Error::Rejected { status: 422, .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Cria a branch dos prints a partir de um commit órfão, se ela não existir (o `main` nunca
    /// é tocado). Devolve se criou.
    pub async fn ensure_attachments_branch(&self) -> Result<bool> {
        let reference = format!("heads/{ATTACHMENTS_BRANCH}");
        let url = self
            .endpoints
            .repo_url(&["git", "ref", "heads", ATTACHMENTS_BRANCH])?;
        match self.send(self.request(Method::GET, url)?).await {
            Ok(response) => {
                // Confere que é mesmo a referência (e não outra coisa com o mesmo nome).
                let _: RefObject = read_json(response).await?;
                return Ok(false);
            }
            Err(Error::RepositoryNotFound { .. }) => {}
            Err(error) => return Err(error),
        }
        let tree: Sha = self
            .post_json(
                &["git", "trees"],
                &json!({
                    "tree": [{
                        "path": "LEIA-ME.md",
                        "mode": "100644",
                        "type": "blob",
                        "content": BRANCH_README,
                    }]
                }),
            )
            .await?;
        let commit: Sha = self
            .post_json(
                &["git", "commits"],
                &json!({
                    "message": "Anexos dos relatos do Warden",
                    "tree": tree.sha,
                    "parents": [],
                }),
            )
            .await?;
        let url = self.endpoints.repo_url(&["git", "refs"])?;
        let request = self
            .request(Method::POST, url)?
            .json_body(&json!({ "ref": format!("refs/{reference}"), "sha": commit.sha }))?;
        match self.send(request).await {
            Ok(_) => {
                tracing::info!(
                    branch = ATTACHMENTS_BRANCH,
                    "branch dos prints criada (commit órfão)"
                );
                Ok(true)
            }
            // Criada por outro envio ao mesmo tempo ("Reference already exists").
            Err(Error::Rejected { status: 422, .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Envia um arquivo para `anexos/<file_name>` na branch dos prints e devolve o endereço
    /// que mostra a imagem. `file_name` vem do Warden (`<hash do conteúdo>.<ext>`), nunca da
    /// interface; por isso um arquivo que já existe (422) é o mesmo print, e não é erro.
    pub async fn upload_attachment(&self, file_name: &str, bytes: &[u8]) -> Result<Url> {
        if file_name.is_empty()
            || !file_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
            || file_name.starts_with('.')
        {
            return Err(Error::Internal(format!(
                "nome de anexo inválido: {file_name:?}"
            )));
        }
        let path = format!("{ATTACHMENTS_DIR}/{file_name}");
        let url = self
            .endpoints
            .repo_url(&["contents", ATTACHMENTS_DIR, file_name])?;
        let content = base64::engine::general_purpose::STANDARD.encode(bytes);
        let request = self
            .request(Method::PUT, url)?
            .json_body(&json!({
                "message": format!("Print do relato ({file_name})"),
                "content": content,
                "branch": ATTACHMENTS_BRANCH,
            }))?
            .timeout(UPLOAD_TIMEOUT);
        match self.send(request).await {
            Ok(_) => tracing::info!(bytes = bytes.len(), arquivo = %path, "print enviado"),
            // O nome vem do conteúdo: o arquivo já está lá (envio anterior que falhou depois).
            Err(Error::Rejected { status: 422, .. }) => {
                tracing::info!(arquivo = %path, "print já estava na branch dos anexos");
            }
            Err(error) => return Err(error),
        }
        self.endpoints.attachment_page(&path)
    }

    /// Cria a issue.
    pub async fn create_issue(
        &self,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<CreatedIssue> {
        let reply: IssueReply = self
            .post_json(
                &["issues"],
                &json!({ "title": title, "body": body, "labels": labels }),
            )
            .await?;
        tracing::info!(numero = reply.number, "issue do relato criada");
        Ok(CreatedIssue {
            number: reply.number,
            url: self.endpoints.issue_page(reply.number)?,
        })
    }

    async fn post_json<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        body: &serde_json::Value,
    ) -> Result<T> {
        let url = self.endpoints.repo_url(segments)?;
        let request = self.request(Method::POST, url)?.json_body(body)?;
        read_json(self.send(request).await?).await
    }

    fn request(&self, method: Method, url: Url) -> Result<Request> {
        Ok(self
            .http
            .request(method, url)
            .header(
                HeaderName::from_static("accept"),
                HeaderValue::from_static("application/vnd.github+json"),
            )
            .header(
                HeaderName::from_static("x-github-api-version"),
                HeaderValue::from_static(API_VERSION),
            )
            .secret_header(
                HeaderName::from_static("authorization"),
                &self.authorization,
            )?
            .max_body(MAX_RESPONSE))
    }

    async fn send(&self, request: Request) -> Result<Response> {
        classify(request.send_unchecked().await?).await
    }
}

/// Transforma o código da resposta no erro do GitHub (2xx passa).
async fn classify(response: Response) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let url = response.request_url().to_string();
    let headers = response.headers();
    let limited = status == StatusCode::TOO_MANY_REQUESTS
        || (status == StatusCode::FORBIDDEN
            && (ratelimit_exhausted(headers) || headers.contains_key("retry-after")));
    if limited {
        let wait = requested_wait(headers, SystemTime::now());
        return Err(Error::RateLimited { url, wait });
    }
    match status.as_u16() {
        401 => Err(Error::SessionExpired {
            reason: "HTTP 401".to_owned(),
        }),
        403 => Err(Error::PermissionDenied { url }),
        404 | 410 => Err(Error::RepositoryNotFound { url }),
        code if status.is_server_error() => Err(Error::Http(warden_http::Error::ServerError {
            host: response.host().to_owned(),
            url,
            status: code,
        })),
        code => {
            let message = response
                .bytes_limited(64 * 1024)
                .await
                .ok()
                .and_then(|body| serde_json::from_slice::<ErrorBody>(&body).ok())
                .and_then(|body| body.message)
                .map(|text| text.chars().take(300).collect());
            Err(Error::Rejected {
                url,
                status: code,
                message,
            })
        }
    }
}

async fn read_json<T: DeserializeOwned>(response: Response) -> Result<T> {
    let url = response.request_url().to_string();
    let body = response.bytes_limited(MAX_RESPONSE).await?;
    serde_json::from_slice(&body)
        .map_err(|error| Error::InvalidResponse(format!("{url}: JSON inesperado ({error})")))
}
