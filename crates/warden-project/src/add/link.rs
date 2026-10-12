//! Links do Modrinth e links diretos colados no campo único de "Adicionar" (SPEC T08 "Link";
//! P1-11). A classificação está em [`super::url`]; o link da CurseForge, em
//! [`super::link_curseforge`].
//!
//! - **Modrinth:** [`resolve_modrinth_link`] acha o projeto (e, no link de versão, a versão
//!   exata) pela API. Link de projeto abre a pré-visualização com o seletor de versão; link de
//!   versão vai direto ao diálogo de dependências com aquela versão, que avisa se ela não serve
//!   para o pack ou não é do canal configurado (`add_plan`).
//! - **Direto:** [`download_direct`] baixa o arquivo **uma vez**, numa pasta temporária, com
//!   limite de tamanho ([`MAX_DIRECT_BYTES`]) e de tempo ([`DIRECT_TIMEOUT`]) e com
//!   cancelamento. Falhou ou foi cancelado: a pasta temporária é apagada e nada chega ao pack.
//!   O arquivo baixado segue o mesmo caminho do arquivo do computador ([`super::local`]):
//!   metadados, identificação pelo hash e gravação da referência (`[download]` com o link e o
//!   `sha256`, como o `packwiz url add`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use url::Url;
use warden_core::{CancellationToken, NoProgress};
use warden_http::{DownloadRequest, FileHashes, HttpClient};
use warden_modrinth::{ModrinthClient, Project, Version, VersionFilter};

use super::AddChoice;
use super::modrinth::kind_of;
use super::url::{DirectLink, ModrinthLink};
use crate::search::{ProjectKind, SourceId};
use crate::{Error, ProjectErrorCode as Code, Result};

/// Maior arquivo aceito num link direto (e no arquivo do computador): 256 MiB.
pub const MAX_DIRECT_BYTES: u64 = 256 * 1024 * 1024;

/// Tempo máximo do download de um link direto, do começo ao fim.
pub const DIRECT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// O que um link do Modrinth aponta, para a tela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthLinkTarget {
    /// ID do projeto no Modrinth.
    pub project_id: String,
    /// Nome.
    pub title: String,
    /// Tipo.
    pub kind: ProjectKind,
    /// ID da versão, quando o link era de versão.
    pub version_id: Option<String>,
}

impl ModrinthLinkTarget {
    /// A escolha para o caminho normal (`add_plan`): a versão do link, ou a padrão do canal.
    #[must_use]
    pub fn choice(&self) -> AddChoice {
        AddChoice {
            source: SourceId::Modrinth,
            project_id: self.project_id.clone(),
            version_id: self.version_id.clone(),
        }
    }
}

fn modrinth_error(error: &warden_modrinth::Error, what: &str) -> Error {
    match error {
        warden_modrinth::Error::ProjectNotFound { .. }
        | warden_modrinth::Error::VersionNotFound { .. } => Error::new(
            Code::ItemNotFound,
            format!("{what} não encontrado no Modrinth"),
        )
        .param("source", "Modrinth"),
        other => super::modrinth::unavailable(other),
    }
}

/// Parece um ID do Modrinth (8 letras e números), e não um número de versão.
fn looks_like_id(text: &str) -> bool {
    text.len() == 8 && text.chars().all(|c| c.is_ascii_alphanumeric())
}

async fn find_version(
    client: &ModrinthClient,
    project: &Project,
    wanted: &str,
    cancel: &CancellationToken,
) -> Result<Version> {
    if looks_like_id(wanted) {
        match client.version(wanted, Some(cancel)).await {
            Ok(version) if version.project_id == project.id => return Ok(version),
            // De outro projeto, ou um número de versão com cara de ID: procura pelo número.
            Ok(_) | Err(warden_modrinth::Error::VersionNotFound { .. }) => {}
            Err(error) => return Err(modrinth_error(&error, "versão")),
        }
    }
    let versions = client
        .project_versions(&project.id, &VersionFilter::default(), Some(cancel))
        .await
        .map_err(|error| modrinth_error(&error, "projeto"))?;
    versions
        .into_iter()
        .find(|version| version.id == wanted || version.version_number == wanted)
        .ok_or_else(|| {
            Error::new(
                Code::ItemNotFound,
                format!(
                    "versão {wanted} de {} não encontrada no Modrinth",
                    project.title
                ),
            )
            .param("source", "Modrinth")
        })
}

/// Acha pela API o projeto (e a versão) de um link do Modrinth.
///
/// Erros: [`Code::InvalidInput`] para tipos que a página Adicionar não aceita (modpack,
/// datapack, plugin); [`Code::ItemNotFound`] se o projeto ou a versão não existem;
/// [`Code::SearchSourceUnavailable`] se o Modrinth não responder.
pub async fn resolve_modrinth_link(
    client: &ModrinthClient,
    link: &ModrinthLink,
    cancel: &CancellationToken,
) -> Result<ModrinthLinkTarget> {
    let project = client
        .project(link.project(), Some(cancel))
        .await
        .map_err(|error| modrinth_error(&error, "projeto"))?;
    let kind = kind_of(project.project_type).ok_or_else(|| {
        Error::new(
            Code::InvalidInput,
            format!(
                "o Warden só adiciona mods, resource packs e shaders do Modrinth ({})",
                project.title
            ),
        )
        .param("field", "link")
        .param("reason", "unsupportedType")
    })?;
    let version_id = match link {
        ModrinthLink::Project { .. } => None,
        ModrinthLink::Version { version, .. } => {
            Some(find_version(client, &project, version, cancel).await?.id)
        }
    };
    Ok(ModrinthLinkTarget {
        project_id: project.id,
        title: project.title,
        kind,
        version_id,
    })
}

/// Um link direto baixado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectDownload {
    /// O arquivo, na pasta temporária.
    pub path: PathBuf,
    /// Tamanho em bytes.
    pub size: u64,
    /// sha1, sha256, sha512 e a impressão da CurseForge.
    pub hashes: FileHashes,
}

/// Falha do download de um link direto.
#[derive(Debug, thiserror::Error)]
pub enum DirectError {
    /// Erro do domínio de packs.
    #[error(transparent)]
    Project(#[from] Error),
    /// Erro da rede (não encontrado, tempo esgotado, arquivo grande demais, redirecionamento
    /// para `http://`, cancelado…). Nunca é falha de disco, que leva o caminho da pasta.
    #[error(transparent)]
    Http(warden_http::Error),
}

/// Para onde o pedido vai: o próprio link ou, nos testes, o mesmo caminho num servidor
/// simulado (`via`; os E2E não têm `https`). A validação do link é sempre a do link original.
fn fetch_url(link: &DirectLink, via: Option<&Url>) -> Result<Url> {
    let Some(base) = via else {
        return Ok(link.url().clone());
    };
    let url = link.url();
    let mut relative = format!("{}{}", url.host_str().unwrap_or_default(), url.path());
    if let Some(query) = url.query() {
        relative.push('?');
        relative.push_str(query);
    }
    base.join(&relative)
        .map_err(|e| Error::new(Code::Internal, format!("servidor simulado inválido: {e}")))
}

/// Baixa um link direto para `dir/<nome do arquivo>`. `dir` não pode ter conteúdo; em qualquer
/// falha (inclusive cancelamento e tempo esgotado) a pasta é apagada.
///
/// `via`: só para testes, manda o pedido para o mesmo caminho num servidor simulado.
///
/// Erros: [`DirectError::Http`] (não encontrado, tempo esgotado, maior que
/// [`MAX_DIRECT_BYTES`], redirecionamento para `http://`, cancelado);
/// [`Code::InvalidInput`] se `dir` já tem arquivos; [`Code::Internal`] em falha de disco (sem o
/// caminho na mensagem).
pub async fn download_direct(
    http: &HttpClient,
    link: &DirectLink,
    dir: &Path,
    via: Option<&Url>,
    cancel: &CancellationToken,
) -> std::result::Result<DirectDownload, DirectError> {
    if dir
        .read_dir()
        .is_ok_and(|mut entries| entries.next().is_some())
    {
        return Err(Error::new(Code::InvalidInput, "a pasta do download não está vazia").into());
    }
    let request = DownloadRequest::new(fetch_url(link, via)?, dir.join(link.file_name()))
        .max_size(MAX_DIRECT_BYTES);
    let result = tokio::select! {
        result = http.download(&request, &NoProgress, Some(cancel)) => result.map_err(http_error),
        () = tokio::time::sleep(DIRECT_TIMEOUT) => Err(DirectError::Http(warden_http::Error::Timeout {
            host: link.url().host_str().unwrap_or_default().to_owned(),
            url: link.url().to_string(),
            seconds: DIRECT_TIMEOUT.as_secs(),
        })),
    };
    match result {
        Ok(downloaded) => Ok(DirectDownload {
            path: downloaded.path,
            size: downloaded.size,
            hashes: downloaded.hashes,
        }),
        Err(error) => {
            // Nada fica pela metade: nem o `.part` que a `warden-http` guardaria para retomar.
            if let Err(cleanup) = std::fs::remove_dir_all(dir)
                && cleanup.kind() != std::io::ErrorKind::NotFound
            {
                tracing::warn!(error = %cleanup.kind(), "pasta do download não apagada");
            }
            Err(error)
        }
    }
}

fn http_error(error: warden_http::Error) -> DirectError {
    match error {
        warden_http::Error::Io { action, source, .. } => DirectError::Project(Error::new(
            Code::Internal,
            format!("falha de disco ao {action} o download: {}", source.kind()),
        )),
        other => DirectError::Http(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::add::url::{LinkClass, classify_link};

    fn direct(text: &str) -> DirectLink {
        match classify_link(text) {
            LinkClass::Direct(link) => link,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn servidor_simulado_recebe_o_mesmo_caminho() {
        let link = direct("https://files.example.com/a/Meu%20Mod.jar?x=1");
        assert_eq!(fetch_url(&link, None).unwrap(), *link.url());
        let base = Url::parse("http://127.0.0.1:9/direto/").unwrap();
        assert_eq!(
            fetch_url(&link, Some(&base)).unwrap().as_str(),
            "http://127.0.0.1:9/direto/files.example.com/a/Meu%20Mod.jar?x=1"
        );
    }

    #[test]
    fn ids_do_modrinth() {
        assert!(looks_like_id("SMxNOGZ6"));
        assert!(!looks_like_id("mc1.21.1-0.6.0-fabric"));
        assert!(!looks_like_id("0.6.0"));
    }
}
