//! Fonte CurseForge da busca combinada (`GET /v1/mods/search`).
//!
//! Filtros travados pelo pack: classe (mods, resource packs, shaders), versões do Minecraft
//! aceitas e, para mods, os loaders compatíveis. Com "Mostrar também os sem versão
//! compatível", só a classe continua travada e cada resultado diz se tem arquivo para o pack
//! (pelos `latestFilesIndexes` que a própria busca devolve, sem requisição a mais).
//!
//! A chave é do usuário: sem chave a fonte nem entra na busca (o app a deixa de fora com o
//! aviso, sem nenhuma requisição); com a chave recusada, a falha vira o aviso "A CurseForge
//! recusou a chave" ([`SourceWarningReason::KeyRejected`]).
//!
//! A CurseForge não tem filtro de ambiente (cliente ou servidor): o filtro **Ambiente** só vale
//! para o Modrinth.

use warden_core::CancellationToken;
use warden_curseforge::{
    CurseforgeClient, MAX_GAME_VERSIONS, MAX_SEARCH_WINDOW, Mod, ModLoaderType, SearchQuery,
    SortField, SortOrder,
};

use super::{
    PackTarget, ProjectKind, ProjectLinks, ProjectPreview, SearchSort, SearchSource, SourceFailure,
    SourceHit, SourceId, SourcePage, SourceQuery, SourceRef, SourceWarningReason,
};
use crate::add::curseforge::{class_of, icon_of, loader_types};
use crate::details::DescriptionFormat;
use crate::{Error, ProjectErrorCode as Code};

/// A busca da CurseForge.
#[derive(Debug, Clone)]
pub struct CurseforgeSearch {
    client: CurseforgeClient,
}

impl CurseforgeSearch {
    /// Fonte com o cliente do app (com a chave do usuário).
    #[must_use]
    pub fn new(client: CurseforgeClient) -> Self {
        Self { client }
    }
}

/// Por que uma falha da API deixa a fonte de fora.
#[must_use]
pub fn failure(error: &warden_curseforge::Error) -> SourceFailure {
    let reason = match error {
        warden_curseforge::Error::KeyMissing => SourceWarningReason::KeyMissing,
        warden_curseforge::Error::KeyInvalid { .. } => SourceWarningReason::KeyRejected,
        _ => SourceWarningReason::Unavailable,
    };
    SourceFailure {
        reason,
        detail: error.to_string(),
    }
}

/// Os parâmetros da busca na CurseForge.
#[must_use]
pub fn curseforge_query(query: &SourceQuery) -> SearchQuery {
    let mut search = SearchQuery::new(query.query.clone()).class(class_of(query.kind));
    if !query.include_incompatible {
        for version in query.target.game_versions.iter().take(MAX_GAME_VERSIONS) {
            search = search.game_version(version.clone());
        }
        for loader in loader_types(&query.target, query.kind) {
            search = search.loader(loader);
        }
    }
    let (field, order) = match query.sort {
        SearchSort::Relevance => (SortField::Popularity, SortOrder::Descending),
        SearchSort::Downloads => (SortField::TotalDownloads, SortOrder::Descending),
        SearchSort::Updated => (SortField::LastUpdated, SortOrder::Descending),
        SearchSort::Newest => (SortField::ReleasedDate, SortOrder::Descending),
    };
    // A busca da CurseForge só chega aos primeiros 10 mil resultados.
    let limit = query
        .limit
        .min(MAX_SEARCH_WINDOW.saturating_sub(query.offset));
    search.sort(field, order).page(query.offset, limit.max(1))
}

/// Se o resultado tem arquivo para o pack, pelos arquivos mais recentes por versão e loader.
#[must_use]
pub fn hit_compatible(project: &Mod, kind: ProjectKind, target: &PackTarget) -> bool {
    let loaders: Vec<ModLoaderType> = loader_types(target, kind);
    project.latest_files_indexes.iter().any(|index| {
        target.game_versions.contains(&index.game_version)
            && (loaders.is_empty()
                || index
                    .mod_loader
                    .is_none_or(|loader| loader == ModLoaderType::Any || loaders.contains(&loader)))
    })
}

fn non_empty(value: Option<&String>) -> Option<String> {
    value.cloned().filter(|v| !v.trim().is_empty())
}

/// A pré-visualização a partir do projeto da CurseForge e da descrição (HTML).
#[must_use]
pub fn preview_of(project: &Mod, body: String) -> ProjectPreview {
    let updated = [&project.date_modified, &project.date_released]
        .into_iter()
        .find_map(|date| non_empty(date.as_ref()))
        .unwrap_or_default();
    ProjectPreview {
        source: SourceId::Curseforge,
        project_id: project.id.to_string(),
        slug: project.slug.clone(),
        title: project.name.clone(),
        summary: project.summary.clone(),
        body,
        body_format: DescriptionFormat::Html,
        icon_url: icon_of(project),
        downloads: super::count(project.download_count),
        updated,
        // A API da CurseForge não informa licença nem lado.
        license: None,
        side: None,
        links: ProjectLinks {
            page: non_empty(project.links.website_url.as_ref()),
            issues: non_empty(project.links.issues_url.as_ref()),
            source: non_empty(project.links.source_url.as_ref()),
            wiki: non_empty(project.links.wiki_url.as_ref()),
            discord: None,
        },
    }
}

#[async_trait::async_trait]
impl SearchSource for CurseforgeSearch {
    fn id(&self) -> SourceId {
        SourceId::Curseforge
    }

    async fn preview(
        &self,
        project_id: &str,
        cancel: &CancellationToken,
    ) -> crate::Result<ProjectPreview> {
        let not_found = || {
            Error::new(
                Code::ItemNotFound,
                format!("projeto {project_id} não encontrado na CurseForge"),
            )
            .param("path", project_id)
        };
        let id: u64 = project_id.trim().parse().map_err(|_| not_found())?;
        let project = match self.client.project(id, Some(cancel)).await {
            Ok(project) => project,
            Err(warden_curseforge::Error::ModNotFound { .. }) => return Err(not_found()),
            Err(error) => return Err(crate::add::curseforge::unavailable(&error)),
        };
        // A descrição é um complemento: sem ela a pré-visualização ainda mostra o resumo.
        let body = match self.client.description(id, Some(cancel)).await {
            Ok(text) => text,
            Err(
                error @ (warden_curseforge::Error::KeyInvalid { .. }
                | warden_curseforge::Error::KeyMissing),
            ) => {
                return Err(crate::add::curseforge::unavailable(&error));
            }
            Err(error) => {
                tracing::debug!(%error, id, "descrição da CurseForge indisponível");
                String::new()
            }
        };
        Ok(preview_of(&project, body))
    }

    async fn search(
        &self,
        query: &SourceQuery,
        cancel: &CancellationToken,
    ) -> Result<SourcePage, SourceFailure> {
        if query.offset >= MAX_SEARCH_WINDOW {
            // Além da janela da API não há mais resultados para mostrar.
            return Ok(SourcePage::default());
        }
        let results = self
            .client
            .search(&curseforge_query(query), Some(cancel))
            .await
            .map_err(|error| failure(&error))?;
        let hits = results
            .mods
            .into_iter()
            .map(|project| {
                // Com os filtros travados o servidor já só devolve o que serve para o pack.
                let compatible = !query.include_incompatible
                    || hit_compatible(&project, query.kind, &query.target);
                SourceHit {
                    reference: SourceRef {
                        source: SourceId::Curseforge,
                        project_id: project.id.to_string(),
                        slug: project.slug.clone(),
                        downloads: super::count(project.download_count),
                    },
                    author: project
                        .authors
                        .first()
                        .map(|author| author.name.clone())
                        .unwrap_or_default(),
                    icon_url: icon_of(&project),
                    updated: [&project.date_modified, &project.date_released]
                        .into_iter()
                        .find_map(|date| non_empty(date.as_ref()))
                        .unwrap_or_default(),
                    created: non_empty(project.date_created.as_ref()).unwrap_or_default(),
                    manual_download: project.is_distribution_blocked(),
                    summary: project.summary,
                    title: project.name,
                    compatible,
                }
            })
            .collect();
        Ok(SourcePage {
            hits,
            total: results
                .pagination
                .total_count
                .min(u64::from(MAX_SEARCH_WINDOW)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warden_packwiz::Loader;

    fn target() -> PackTarget {
        PackTarget {
            minecraft: "1.20.1".into(),
            game_versions: vec!["1.20.1".into()],
            loaders: vec![Loader::Forge],
        }
    }

    fn query(kind: ProjectKind, include_incompatible: bool) -> SourceQuery {
        SourceQuery {
            query: "jei".into(),
            kind,
            sort: SearchSort::Downloads,
            environment: None,
            include_incompatible,
            target: target(),
            offset: 40,
            limit: 20,
        }
    }

    fn param(query: &SearchQuery, name: &str) -> Option<String> {
        query
            .to_params()
            .into_iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }

    #[test]
    fn filtros_travados_do_pack() {
        let mods = curseforge_query(&query(ProjectKind::Mod, false));
        assert_eq!(param(&mods, "classId").as_deref(), Some("6"));
        assert_eq!(param(&mods, "gameVersion").as_deref(), Some("1.20.1"));
        assert_eq!(param(&mods, "modLoaderType").as_deref(), Some("1"));
        assert_eq!(param(&mods, "index").as_deref(), Some("40"));
        assert_eq!(param(&mods, "sortField").as_deref(), Some("6"));
        // Shaders não filtram pelo loader do pack e usam a classe dos shaders.
        let shaders = curseforge_query(&query(ProjectKind::Shader, false));
        assert_eq!(param(&shaders, "classId").as_deref(), Some("6552"));
        assert_eq!(param(&shaders, "modLoaderType"), None);
        // Sem versão compatível: só a classe continua.
        let all = curseforge_query(&query(ProjectKind::Mod, true));
        assert_eq!(param(&all, "gameVersion"), None);
        assert_eq!(param(&all, "modLoaderType"), None);
        assert_eq!(param(&all, "classId").as_deref(), Some("6"));
    }

    #[test]
    fn a_janela_da_api_nao_estoura() {
        let mut near_end = query(ProjectKind::Mod, false);
        near_end.offset = 9_990;
        let search = curseforge_query(&near_end);
        search.validate().unwrap();
        assert_eq!(param(&search, "pageSize").as_deref(), Some("10"));
    }

    #[test]
    fn falha_de_chave_vira_aviso_de_chave() {
        let missing = failure(&warden_curseforge::Error::KeyMissing);
        assert_eq!(missing.reason, SourceWarningReason::KeyMissing);
        let rejected = failure(&warden_curseforge::Error::KeyInvalid { status: Some(403) });
        assert_eq!(rejected.reason, SourceWarningReason::KeyRejected);
        let down = failure(&warden_curseforge::Error::InvalidResponse("x".into()));
        assert_eq!(down.reason, SourceWarningReason::Unavailable);
    }
}
