//! Fonte Modrinth de "Adicionar": projetos e versões pela API (com o cache de metadados), e o
//! `.pw.toml` como o `packwiz modrinth add` escreve (arquivo `primary`, hash `sha512`,
//! `[update.modrinth]`; ARCHITECTURE §6.1 e §6.2).
//!
//! Lado: o `environment` da versão (tabela da ARCHITECTURE §6.2); sem ele, o do projeto
//! (`environment` ou `client_side`/`server_side`).

use std::collections::HashMap;

use warden_core::CancellationToken;
use warden_modrinth::{
    DependencyType, Environment, HashAlgorithm, ModrinthClient, Project, ProjectType, SideSupport,
    Version, VersionFilter, VersionType,
};
use warden_packwiz::{HashFormat, Metafile, ModrinthFile, Side, side_from_modrinth};

use super::{
    AddSource, DependencyKind, SideNote, SourceDependency, SourceFile, SourceProject,
    SourceVersion, VersionChannel,
};
use crate::search::{PackTarget, ProjectKind, SourceId};
use crate::side::SideChoice;
use crate::{Error, ProjectErrorCode as Code, Result};

/// O Modrinth como fonte de "Adicionar".
#[derive(Debug, Clone)]
pub struct ModrinthAdd {
    client: ModrinthClient,
}

impl ModrinthAdd {
    /// Fonte com o cliente do app.
    #[must_use]
    pub fn new(client: ModrinthClient) -> Self {
        Self { client }
    }
}

/// Erro da API virando erro do domínio, com o nome da fonte para a frase.
pub(crate) fn unavailable(error: &warden_modrinth::Error) -> Error {
    Error::new(Code::SearchSourceUnavailable, error.to_string()).param("source", "Modrinth")
}

/// Tipo da página para o tipo do Modrinth (nenhum: não entra pela página Adicionar).
#[must_use]
pub fn kind_of(project_type: ProjectType) -> Option<ProjectKind> {
    match project_type {
        ProjectType::Mod => Some(ProjectKind::Mod),
        ProjectType::Resourcepack => Some(ProjectKind::ResourcePack),
        ProjectType::Shader => Some(ProjectKind::Shader),
        _ => None,
    }
}

fn side_choice(side: &Side) -> SideChoice {
    match side {
        Side::Client => SideChoice::Client,
        Side::Server => SideChoice::Server,
        _ => SideChoice::Both,
    }
}

fn side_note(note: Option<warden_packwiz::SideNote>) -> Option<SideNote> {
    note.map(|note| match note {
        warden_packwiz::SideNote::EitherSide => SideNote::EitherSide,
        warden_packwiz::SideNote::Unknown => SideNote::Unknown,
    })
}

/// Lado de um `environment` do Modrinth (ARCHITECTURE §6.2).
#[must_use]
pub fn side_of(environment: Option<Environment>) -> (SideChoice, Option<SideNote>) {
    let (side, note) = side_from_modrinth(environment.map(Environment::as_str));
    (side_choice(&side), side_note(note))
}

/// Lado informado pelo projeto: o `environment` (quando é um só) ou, na falta dele,
/// `client_side`/`server_side`.
#[must_use]
pub fn project_side(project: &Project) -> Option<(SideChoice, Option<SideNote>)> {
    if let [environment] = project.environment.as_slice() {
        return Some(side_of(Some(*environment)));
    }
    let client = project.client_side;
    let server = project.server_side;
    match (client, server) {
        (SideSupport::Unknown, _) | (_, SideSupport::Unknown) => None,
        (SideSupport::Required | SideSupport::Optional, SideSupport::Unsupported) => {
            Some((SideChoice::Client, None))
        }
        (SideSupport::Unsupported, SideSupport::Required | SideSupport::Optional) => {
            Some((SideChoice::Server, None))
        }
        _ => Some((SideChoice::Both, None)),
    }
}

fn source_project(project: Project) -> SourceProject {
    let side = project_side(&project);
    SourceProject {
        source: SourceId::Modrinth,
        kind: kind_of(project.project_type),
        icon_url: project.icon_url.clone().filter(|url| !url.is_empty()),
        id: project.id,
        slug: project.slug,
        title: project.title,
        side,
    }
}

/// Versão do Modrinth no formato comum.
#[must_use]
pub fn source_version(version: &Version) -> SourceVersion {
    let (side, side_note) = side_of(version.environment);
    let file = version.primary_file().map(|file| SourceFile {
        name: file.filename.clone(),
        url: file.url.clone(),
        sha1: Some(file.hashes.sha1.to_ascii_lowercase()).filter(|hash| !hash.is_empty()),
        sha512: Some(file.hashes.sha512.to_ascii_lowercase()).filter(|hash| !hash.is_empty()),
    });
    let dependencies = version
        .dependencies
        .iter()
        .filter_map(|dependency| {
            let kind = match dependency.dependency_type {
                DependencyType::Required => DependencyKind::Required,
                DependencyType::Optional => DependencyKind::Optional,
                DependencyType::Incompatible => DependencyKind::Incompatible,
                DependencyType::Embedded | DependencyType::Unknown => return None,
            };
            Some(SourceDependency {
                project_id: dependency.project_id.clone().filter(|id| !id.is_empty()),
                version_id: dependency.version_id.clone().filter(|id| !id.is_empty()),
                kind,
            })
        })
        .collect();
    SourceVersion {
        source: SourceId::Modrinth,
        id: version.id.clone(),
        project_id: version.project_id.clone(),
        number: version.version_number.clone(),
        channel: match version.version_type {
            VersionType::Release => VersionChannel::Release,
            VersionType::Beta => VersionChannel::Beta,
            // Desconhecido conta como o canal mais instável.
            VersionType::Alpha | VersionType::Unknown => VersionChannel::Alpha,
        },
        published: version.date_published.clone(),
        game_versions: version.game_versions.clone(),
        loaders: version.loaders.clone(),
        file,
        side,
        side_note,
        dependencies,
    }
}

#[async_trait::async_trait]
impl AddSource for ModrinthAdd {
    fn id(&self) -> SourceId {
        SourceId::Modrinth
    }

    async fn projects(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<Vec<SourceProject>> {
        let projects = self
            .client
            .projects(ids, Some(cancel))
            .await
            .map_err(|e| unavailable(&e))?;
        Ok(projects.into_iter().map(source_project).collect())
    }

    async fn compatible_versions(
        &self,
        project: &SourceProject,
        target: &PackTarget,
        cancel: &CancellationToken,
    ) -> Result<Vec<SourceVersion>> {
        let kind = project.kind.unwrap_or(ProjectKind::Mod);
        let filter = VersionFilter {
            loaders: target.loader_names(kind),
            game_versions: target.game_versions.clone(),
            featured: None,
            include_changelog: false,
        };
        let versions = match self
            .client
            .project_versions(&project.id, &filter, Some(cancel))
            .await
        {
            Ok(versions) => versions,
            Err(warden_modrinth::Error::ProjectNotFound { .. }) => Vec::new(),
            Err(error) => return Err(unavailable(&error)),
        };
        let mut versions: Vec<SourceVersion> = versions
            .iter()
            .map(source_version)
            // O servidor já filtra; conferir de novo protege de uma resposta mais larga.
            .filter(|version| version.file.is_some() && version.compatible(project.kind, target))
            .collect();
        // A API já devolve a mais nova primeiro; a ordem estável garante isso.
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(versions)
    }

    async fn versions(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<Vec<SourceVersion>> {
        let versions = self
            .client
            .versions(ids, Some(cancel))
            .await
            .map_err(|e| unavailable(&e))?;
        Ok(versions.iter().map(source_version).collect())
    }

    async fn projects_by_sha1(
        &self,
        hashes: &[String],
        cancel: &CancellationToken,
    ) -> Result<HashMap<String, String>> {
        let found = self
            .client
            .version_files(hashes, HashAlgorithm::Sha1, Some(cancel))
            .await
            .map_err(|e| unavailable(&e))?;
        Ok(found
            .into_iter()
            .map(|(hash, version)| (hash, version.project_id))
            .collect())
    }

    fn metafile(
        &self,
        project: &SourceProject,
        version: &SourceVersion,
        side: SideChoice,
    ) -> Result<Metafile> {
        let file = version.file.as_ref().ok_or_else(|| {
            Error::new(
                Code::InvalidInput,
                format!("a versão {} não tem arquivo", version.number),
            )
            .param("name", &project.title)
        })?;
        let hash = file.sha512.clone().ok_or_else(|| {
            Error::new(
                Code::Internal,
                format!("o Modrinth não informou o sha512 de {}", file.name),
            )
        })?;
        let modrinth = ModrinthFile {
            title: project.title.clone(),
            project_id: project.id.clone(),
            version_id: version.id.clone(),
            filename: file.name.clone(),
            url: file.url.clone(),
            hash_format: HashFormat::Sha512,
            hash,
        };
        Ok(Metafile::modrinth(&modrinth, side.side()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lado_pela_versao_e_pelo_projeto() {
        assert_eq!(
            side_of(Some(Environment::ClientOnly)),
            (SideChoice::Client, None)
        );
        assert_eq!(
            side_of(Some(Environment::ClientOrServer)),
            (SideChoice::Both, Some(SideNote::EitherSide))
        );
        assert_eq!(side_of(None), (SideChoice::Both, Some(SideNote::Unknown)));
        let mut project: Project = serde_json::from_value(serde_json::json!({
            "id": "p", "slug": "s", "project_type": "mod", "title": "T", "status": "approved",
            "client_side": "required", "server_side": "unsupported"
        }))
        .unwrap();
        assert_eq!(project_side(&project), Some((SideChoice::Client, None)));
        project.environment = vec![Environment::DedicatedServerOnly];
        assert_eq!(project_side(&project), Some((SideChoice::Server, None)));
        project.environment.clear();
        project.client_side = SideSupport::Unknown;
        assert_eq!(project_side(&project), None);
    }

    #[test]
    fn versao_do_modrinth_no_formato_comum() {
        let version: Version = serde_json::from_str(include_str!(
            "../../../warden-modrinth/tests/fixtures/http/2026-10-04-version-SMxNOGZ6.json"
        ))
        .unwrap();
        let common = source_version(&version);
        assert_eq!(common.channel, VersionChannel::Release);
        assert_eq!(common.side, SideChoice::Client);
        let file = common.file.unwrap();
        assert_eq!(
            std::path::Path::new(&file.name)
                .extension()
                .and_then(|e| e.to_str()),
            Some("jar")
        );
        assert_eq!(file.sha512.unwrap().len(), 128);
        assert_eq!(file.sha1.unwrap().len(), 40);
    }
}
