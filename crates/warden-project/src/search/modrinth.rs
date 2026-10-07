//! Fonte Modrinth da busca combinada (`GET /v2/search`).
//!
//! Filtros travados pelo pack: tipo de projeto, versões do Minecraft aceitas e, para mods, os
//! loaders compatíveis (`categories:<loader>`). Com "Mostrar também os sem versão compatível",
//! só o tipo continua travado e cada resultado diz se tem versão para o pack (pelas versões e
//! categorias que a própria busca devolve, sem requisição a mais).

use warden_core::CancellationToken;
use warden_modrinth::{
    Facets, ModrinthClient, Project, ProjectType, SearchHit, SearchIndex, SearchQuery,
};

use super::{
    EnvironmentFilter, PackTarget, ProjectKind, ProjectLinks, ProjectPreview, SearchSort,
    SearchSource, SourceFailure, SourceHit, SourceId, SourcePage, SourceQuery, SourceRef,
};
use crate::details::DescriptionFormat;
use crate::{Error, ProjectErrorCode as Code};

/// A busca do Modrinth.
#[derive(Debug, Clone)]
pub struct ModrinthSearch {
    client: ModrinthClient,
}

impl ModrinthSearch {
    /// Fonte com o cliente do app.
    #[must_use]
    pub fn new(client: ModrinthClient) -> Self {
        Self { client }
    }
}

/// Tipo do Modrinth para o tipo da página.
#[must_use]
pub fn project_type(kind: ProjectKind) -> ProjectType {
    match kind {
        ProjectKind::Mod => ProjectType::Mod,
        ProjectKind::ResourcePack => ProjectType::Resourcepack,
        ProjectKind::Shader => ProjectType::Shader,
    }
}

/// Os parâmetros da busca no Modrinth.
#[must_use]
pub fn modrinth_query(query: &SourceQuery) -> SearchQuery {
    let mut facets = Facets::new().project_type(project_type(query.kind));
    if !query.include_incompatible {
        facets = facets.game_versions(&query.target.game_versions);
        let loaders = query.target.loader_names(query.kind);
        if !loaders.is_empty() {
            facets = facets.loaders(&loaders);
        }
    }
    facets = match query.environment {
        None => facets,
        Some(EnvironmentFilter::Client) => {
            facets.any_of(["client_side:required", "client_side:optional"])
        }
        Some(EnvironmentFilter::Server) => {
            facets.any_of(["server_side:required", "server_side:optional"])
        }
    };
    let index = match query.sort {
        SearchSort::Relevance => SearchIndex::Relevance,
        SearchSort::Downloads => SearchIndex::Downloads,
        SearchSort::Updated => SearchIndex::Updated,
        SearchSort::Newest => SearchIndex::Newest,
    };
    SearchQuery::new(query.query.clone())
        .facets(facets)
        .index(index)
        .page(query.offset, query.limit)
}

/// Se o resultado tem versão para o pack, pelas versões e categorias (loaders) da busca.
#[must_use]
pub fn hit_compatible(hit: &SearchHit, kind: ProjectKind, target: &PackTarget) -> bool {
    let version = hit
        .versions
        .iter()
        .any(|version| target.game_versions.contains(version));
    let loaders = target.loader_names(kind);
    let loader = loaders.is_empty()
        || hit
            .categories
            .iter()
            .any(|category| loaders.contains(category));
    version && loader
}

/// Endereço da página do projeto no Modrinth.
#[must_use]
pub fn page_url(project: &Project) -> String {
    let kind = match project.project_type {
        ProjectType::Mod => "mod",
        ProjectType::Modpack => "modpack",
        ProjectType::Resourcepack => "resourcepack",
        ProjectType::Shader => "shader",
        ProjectType::Datapack => "datapack",
        ProjectType::Plugin => "plugin",
        ProjectType::Unknown => "project",
    };
    let slug = if project.slug.is_empty() {
        &project.id
    } else {
        &project.slug
    };
    format!("https://modrinth.com/{kind}/{slug}")
}

/// A pré-visualização a partir do projeto do Modrinth.
#[must_use]
pub fn preview_of(project: &Project) -> ProjectPreview {
    let non_empty = |value: &Option<String>| value.clone().filter(|v| !v.trim().is_empty());
    ProjectPreview {
        source: SourceId::Modrinth,
        project_id: project.id.clone(),
        slug: project.slug.clone(),
        title: project.title.clone(),
        summary: project.description.clone(),
        body: project.body.clone(),
        body_format: DescriptionFormat::Markdown,
        icon_url: non_empty(&project.icon_url),
        downloads: super::count(project.downloads),
        updated: project.updated.clone(),
        license: project.license.as_ref().and_then(|license| {
            [&license.name, &license.id]
                .into_iter()
                .find(|value| !value.trim().is_empty())
                .cloned()
        }),
        side: crate::add::modrinth::project_side(project).map(|(side, _)| side),
        links: ProjectLinks {
            page: Some(page_url(project)),
            issues: non_empty(&project.issues_url),
            source: non_empty(&project.source_url),
            wiki: non_empty(&project.wiki_url),
            discord: non_empty(&project.discord_url),
        },
    }
}

#[async_trait::async_trait]
impl SearchSource for ModrinthSearch {
    fn id(&self) -> SourceId {
        SourceId::Modrinth
    }

    async fn preview(
        &self,
        project_id: &str,
        cancel: &CancellationToken,
    ) -> crate::Result<ProjectPreview> {
        match self.client.project(project_id, Some(cancel)).await {
            Ok(project) => Ok(preview_of(&project)),
            Err(warden_modrinth::Error::ProjectNotFound { .. }) => Err(Error::new(
                Code::ItemNotFound,
                format!("projeto {project_id} não encontrado no Modrinth"),
            )
            .param("path", project_id)),
            Err(error) => Err(crate::add::modrinth::unavailable(&error)),
        }
    }

    async fn search(
        &self,
        query: &SourceQuery,
        cancel: &CancellationToken,
    ) -> Result<SourcePage, SourceFailure> {
        let results = self
            .client
            .search(&modrinth_query(query), Some(cancel))
            .await
            .map_err(|error| SourceFailure::unavailable(error.to_string()))?;
        let hits = results
            .hits
            .into_iter()
            .map(|hit| {
                let compatible = hit_compatible(&hit, query.kind, &query.target);
                SourceHit {
                    reference: SourceRef {
                        source: SourceId::Modrinth,
                        project_id: hit.project_id,
                        slug: hit.slug,
                        downloads: super::count(hit.downloads),
                    },
                    title: hit.title,
                    author: hit.author,
                    summary: hit.description,
                    icon_url: hit.icon_url.filter(|url| !url.is_empty()),
                    updated: hit.date_modified,
                    created: hit.date_created,
                    compatible,
                    manual_download: false,
                }
            })
            .collect();
        Ok(SourcePage {
            hits,
            total: results.total_hits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warden_packwiz::Loader;

    fn target() -> PackTarget {
        PackTarget {
            minecraft: "1.21.1".into(),
            game_versions: vec!["1.21.1".into(), "1.21".into()],
            loaders: vec![Loader::Fabric],
        }
    }

    fn query(kind: ProjectKind, include_incompatible: bool) -> SourceQuery {
        SourceQuery {
            query: "sodium".into(),
            kind,
            sort: SearchSort::Downloads,
            environment: Some(EnvironmentFilter::Client),
            include_incompatible,
            target: target(),
            offset: 40,
            limit: 20,
        }
    }

    fn param(query: &SearchQuery, name: &str) -> String {
        query
            .to_params()
            .into_iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
            .unwrap_or_default()
    }

    #[test]
    fn filtros_travados_do_pack() {
        let mods = modrinth_query(&query(ProjectKind::Mod, false));
        assert_eq!(
            param(&mods, "facets"),
            r#"[["project_type:mod"],["versions:1.21.1","versions:1.21"],["categories:fabric"],["client_side:required","client_side:optional"]]"#
        );
        assert_eq!(param(&mods, "index"), "downloads");
        assert_eq!(param(&mods, "offset"), "40");
        // Resource packs e shaders não filtram pelo loader do pack.
        let shaders = modrinth_query(&query(ProjectKind::Shader, false));
        assert!(!param(&shaders, "facets").contains("categories:"));
        assert!(param(&shaders, "facets").contains("project_type:shader"));
        // Sem versão compatível: só o tipo (e o ambiente) continuam.
        let all = modrinth_query(&query(ProjectKind::Mod, true));
        assert_eq!(
            param(&all, "facets"),
            r#"[["project_type:mod"],["client_side:required","client_side:optional"]]"#
        );
    }
}
