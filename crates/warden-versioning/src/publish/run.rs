//! Publicar uma versão final (ARCHITECTURE §11.1 item 2): commit e tag na linha de
//! publicação, envio protegido para a `main` e a tag `vX.Y.Z` do GitHub, e a Release.
//!
//! Ordem e garantias:
//! 1. Lê as referências do GitHub. Se a `main` de lá não for a última publicação deste
//!    computador, para com [`Error::RemoteDiverged`] — a menos que a pessoa tenha confirmado
//!    "Substituir o GitHub pelo que está aqui" para exatamente o commit que viu
//!    ([`ReplaceRemote`]).
//! 2. Monta commit e tag (objetos soltos) e envia; o servidor só aceita se a `main` dele
//!    ainda for a esperada. Falhou aqui: nada mudou, nem lá nem aqui.
//! 3. Grava as referências locais (versão "publicada", com Release pendente).
//! 4. Cria a Release. Falhou aqui: [`Error::ReleasePending`]; [`retry_release`] cria só ela.
//!
//! O [`crate::PackRepo`] nunca atravessa um `await` (a `git2::Repository` não é `Sync`):
//! cada trecho com o histórico abre o repositório de novo.

use std::path::Path;

use git2::Oid;
use serde::{Deserialize, Serialize};
use warden_core::CancellationToken;

use super::line::{PreparedPublication, PublishRequest};
use super::transport::{GitRemote, RefUpdate};
use crate::error::{Error, Result};
use crate::github::{GithubClient, ReleaseInfo, RepoRef};
use crate::repo::PackRepo;
use crate::versions::{parse_version, tag_name};

/// A `main` do GitHub comparada com a última publicação deste computador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    /// Commit da `main` do GitHub (`None`: repositório vazio).
    pub remote_main: Option<String>,
    /// A `main` do GitHub é a última publicação daqui (ou os dois estão vazios).
    pub in_sync: bool,
}

/// Confirmação de "Substituir o GitHub pelo que está aqui": só vale se a `main` do GitHub
/// ainda for a que a pessoa viu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceRemote {
    /// Commit da `main` do GitHub quando a divergência foi mostrada (`None`: vazia).
    pub expected_main: Option<String>,
}

/// Uma publicação feita.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Published {
    /// Versão publicada.
    pub version: String,
    /// Link do pack (`…/main/pack.toml`).
    pub pack_link: String,
    /// A Release criada (o id serve à E-04).
    pub release: ReleaseInfo,
    /// Notas da versão (para "Copiar texto das notas da versão").
    pub notes: String,
}

fn parse_oid(text: &str) -> Result<Oid> {
    Oid::from_str(text).map_err(|_| Error::Internal(format!("commit inválido: {text}")))
}

/// Compara a `main` do GitHub com a última publicação daqui.
///
/// Erros: os de leitura do GitHub ([`Error::GithubTokenRejected`],
/// [`Error::GithubRepoNotFound`], [`Error::GithubOffline`]…) e do histórico.
pub async fn remote_status(
    client: &GithubClient,
    root: &Path,
    repo: &RepoRef,
    cancel: &CancellationToken,
) -> Result<RemoteStatus> {
    let refs = GitRemote::new(client, repo)?.refs(cancel).await?;
    let remote_main = refs.get("refs/heads/main").copied();
    let tip = PackRepo::open(root)?.publish_line_tip()?;
    Ok(RemoteStatus {
        remote_main: remote_main.map(|oid| oid.to_string()),
        in_sync: remote_main == tip,
    })
}

/// Publica a versão final `request.version` no repositório `repo`.
///
/// Erros: os de [`PackRepo::prepare_publication`]; [`Error::RemoteDiverged`];
/// [`Error::PushRejected`]; os do GitHub; [`Error::ReleasePending`] quando o envio deu certo
/// e só a Release falhou (a versão já conta como publicada).
pub async fn publish(
    client: &GithubClient,
    root: &Path,
    repo: &RepoRef,
    request: &PublishRequest<'_>,
    replace: Option<&ReplaceRemote>,
    cancel: &CancellationToken,
) -> Result<Published> {
    // Confere antes de qualquer pedido de rede (versão final, não publicada…).
    PackRepo::open(root)?.check_publishable(request.version)?;
    let remote = GitRemote::new(client, repo)?;
    let refs = remote.refs(cancel).await?;
    let remote_main = refs.get("refs/heads/main").copied();
    let version = parse_version(request.version)?;
    let tag_ref = format!("refs/tags/{}", tag_name(&version));
    let remote_tag = refs.get(&tag_ref).copied();
    let (prepared, pack) = {
        let pack_repo = PackRepo::open(root)?;
        let tip = pack_repo.publish_line_tip()?;
        let diverged = remote_main != tip || remote_tag.is_some();
        if diverged {
            let confirmed = replace.is_some_and(|replace| {
                replace
                    .expected_main
                    .as_deref()
                    .map(parse_oid)
                    .transpose()
                    .ok()
                    == Some(remote_main)
            });
            if !confirmed {
                return Err(Error::RemoteDiverged {
                    repo: repo.full_name(),
                });
            }
        }
        let prepared = pack_repo.prepare_publication(request)?;
        let pack = pack_repo.publication_pack(&prepared, remote_main)?;
        (prepared, pack)
    };
    if cancel.is_cancelled() {
        return Err(Error::Core(warden_core::CoreError::Cancelled));
    }
    let updates = [
        RefUpdate {
            name: "refs/heads/main".to_owned(),
            old: remote_main,
            new: prepared.commit,
        },
        RefUpdate {
            name: tag_ref,
            old: remote_tag,
            new: prepared.tag,
        },
    ];
    remote.push(&updates, &pack, cancel).await?;
    PackRepo::open(root)?.record_publication(&prepared)?;
    tracing::info!(versao = %prepared.version, repo = %repo.full_name(), "versão enviada ao GitHub");
    let release = create_release(client, root, repo, &prepared).await?;
    Ok(Published {
        version: prepared.version.to_string(),
        pack_link: client.endpoints().pack_link(repo),
        release,
        notes: prepared.notes,
    })
}

async fn create_release(
    client: &GithubClient,
    root: &Path,
    repo: &RepoRef,
    prepared: &PreparedPublication,
) -> Result<ReleaseInfo> {
    let tag = tag_name(&prepared.version);
    match client
        .create_release(repo, &tag, &prepared.release_name, &prepared.notes)
        .await
    {
        Ok(release) => {
            PackRepo::open(root)?.record_release(&prepared.version)?;
            Ok(release)
        }
        Err(error) => Err(Error::ReleasePending {
            version: prepared.version.to_string(),
            source: Box::new(error),
        }),
    }
}

/// "Tentar de novo" de uma versão "publicada, Release pendente": cria só a Release, com o
/// nome e as notas gravados na publicação.
///
/// Erros: [`Error::VersionNotFound`] se a versão não foi publicada; os do GitHub.
pub async fn retry_release(
    client: &GithubClient,
    root: &Path,
    repo: &RepoRef,
    version: &str,
) -> Result<Published> {
    let version = parse_version(version)?;
    let (name, notes) = PackRepo::open(root)?.published_release_text(&version)?;
    let release = client
        .create_release(repo, &tag_name(&version), &name, &notes)
        .await?;
    PackRepo::open(root)?.record_release(&version)?;
    Ok(Published {
        version: version.to_string(),
        pack_link: client.endpoints().pack_link(repo),
        release,
        notes,
    })
}
