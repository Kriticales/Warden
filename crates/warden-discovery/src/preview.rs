//! Partes da pré-visualização completa (SPEC T08, P1; ARCHITECTURE §17.1): galeria, notas de
//! versão (changelog) sob demanda e dependências com "Já no pack" / "Será adicionada".
//!
//! - **Galeria e notas** vêm de uma [`DetailSource`] (Modrinth aqui; a CurseForge entra com a
//!   P1-10 sem mudar o resto). No Modrinth a galeria sai do projeto, que o cache de 24 h do
//!   `warden-modrinth` já guarda junto com a descrição (zero requisição a mais depois da
//!   pré-visualização). As notas são pedidas **só quando a linha da versão é aberta**.
//! - **Dependências** usam as mesmas fontes de "Adicionar" (`AddSources`): lê as declaradas na
//!   versão e confere o inventário do pack.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;
use warden_core::CancellationToken;
use warden_modrinth::ModrinthClient;
use warden_project::add::{AddSources, DependencyKind};
use warden_project::details::DescriptionFormat;
use warden_project::search::{InstalledKeys, SourceId};
use warden_project::{Error, ProjectErrorCode as Code, Result};

use crate::images;

/// Uma imagem da galeria do projeto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GalleryItem {
    /// Miniatura para a tira da galeria (o endereço do Modrinth direto; o da CurseForge pelo
    /// protocolo `warden-img://`, ARCHITECTURE §17.1).
    pub thumb_url: String,
    /// A imagem inteira, mostrada grande sobre a página ao clicar na miniatura.
    pub url: String,
    /// Título, se o autor deu um.
    pub title: Option<String>,
    /// Descrição, se houver.
    pub description: Option<String>,
    /// A imagem de destaque do projeto.
    pub featured: bool,
}

/// As notas de uma versão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionNotes {
    /// O texto (a interface higieniza como a descrição; ARCHITECTURE §18).
    pub body: String,
    /// Formato do texto.
    pub format: DescriptionFormat,
}

/// Uma dependência declarada pela versão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DependencyInfo {
    /// Obrigatória, opcional ou incompatível.
    pub kind: DependencyKind,
    /// Fonte do projeto da dependência.
    pub source: SourceId,
    /// ID do projeto.
    pub project_id: String,
    /// Nome (o ID, se a fonte não souber o projeto).
    pub title: String,
    /// Ícone.
    pub icon_url: Option<String>,
    /// Já está no pack.
    pub in_pack: bool,
}

/// As dependências de uma versão.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionDependencies {
    /// Obrigatórias, depois opcionais, depois incompatíveis.
    pub items: Vec<DependencyInfo>,
}

/// O que uma fonte sabe dar para a pré-visualização completa.
#[async_trait::async_trait]
pub trait DetailSource: Send + Sync {
    /// Qual fonte é.
    fn id(&self) -> SourceId;

    /// A galeria do projeto, com os endereços já prontos para a interface.
    async fn gallery(
        &self,
        project_id: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<GalleryItem>>;

    /// As notas de uma versão (nenhum texto: `None`). Pedidas só ao abrir a linha.
    async fn notes(
        &self,
        project_id: &str,
        version_id: &str,
        cancel: &CancellationToken,
    ) -> Result<Option<VersionNotes>>;
}

/// As fontes de detalhe ligadas no app.
#[derive(Clone, Default)]
pub struct DetailSources {
    sources: Vec<Arc<dyn DetailSource>>,
}

impl DetailSources {
    /// Sem fontes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acrescenta uma fonte.
    #[must_use]
    pub fn with(mut self, source: Arc<dyn DetailSource>) -> Self {
        self.sources.push(source);
        self
    }

    fn get(&self, id: SourceId) -> Result<&dyn DetailSource> {
        self.sources
            .iter()
            .find(|source| source.id() == id)
            .map(AsRef::as_ref)
            .ok_or_else(|| {
                Error::new(
                    Code::SearchSourceUnavailable,
                    format!("fonte {} não está ligada", id.label()),
                )
                .param("source", id.label())
            })
    }
}

/// A galeria de um projeto.
pub async fn gallery(
    sources: &DetailSources,
    source: SourceId,
    project_id: &str,
    cancel: &CancellationToken,
) -> Result<Vec<GalleryItem>> {
    sources.get(source)?.gallery(project_id, cancel).await
}

/// As notas de uma versão.
pub async fn version_notes(
    sources: &DetailSources,
    source: SourceId,
    project_id: &str,
    version_id: &str,
    cancel: &CancellationToken,
) -> Result<Option<VersionNotes>> {
    sources
        .get(source)?
        .notes(project_id, version_id, cancel)
        .await
}

/// As dependências declaradas pela versão, com "Já no pack" pelo inventário (`installed`).
///
/// Um projeto que a fonte não conhece aparece pelo ID. Duas requisições no máximo: a versão e
/// o lote dos projetos (mais um lote de versões quando alguma dependência só traz a versão).
pub async fn version_dependencies(
    sources: &AddSources,
    installed: &InstalledKeys,
    source: SourceId,
    version_id: &str,
    cancel: &CancellationToken,
) -> Result<VersionDependencies> {
    let add = sources.get(source).ok_or_else(|| {
        Error::new(
            Code::SearchSourceUnavailable,
            format!("fonte {} não está ligada", source.label()),
        )
        .param("source", source.label())
    })?;
    let Some(version) = add
        .versions(&[version_id.to_owned()], cancel)
        .await?
        .into_iter()
        .next()
    else {
        return Ok(VersionDependencies { items: Vec::new() });
    };
    // Dependência só com a versão: o projeto vem da versão (um lote).
    let only_version: Vec<String> = version
        .dependencies
        .iter()
        .filter(|dependency| dependency.project_id.is_none())
        .filter_map(|dependency| dependency.version_id.clone())
        .collect();
    let mut project_of_version: HashMap<String, String> = HashMap::new();
    if !only_version.is_empty() {
        for found in add.versions(&only_version, cancel).await? {
            project_of_version.insert(found.id, found.project_id);
        }
    }
    let mut declared: Vec<(DependencyKind, String)> = Vec::new();
    let mut seen = HashSet::new();
    for dependency in &version.dependencies {
        let project = dependency.project_id.clone().or_else(|| {
            dependency
                .version_id
                .as_ref()
                .and_then(|id| project_of_version.get(id).cloned())
        });
        if let Some(project) = project
            && seen.insert((dependency.kind, project.clone()))
        {
            declared.push((dependency.kind, project));
        }
    }
    let ids: Vec<String> = declared.iter().map(|(_, id)| id.clone()).collect();
    let projects: HashMap<String, _> = if ids.is_empty() {
        HashMap::new()
    } else {
        add.projects(&ids, cancel)
            .await?
            .into_iter()
            .map(|project| (project.id.clone(), project))
            .collect()
    };
    let rank = |kind: DependencyKind| match kind {
        DependencyKind::Required => 0,
        DependencyKind::Optional => 1,
        DependencyKind::Incompatible => 2,
    };
    declared.sort_by_key(|(kind, _)| rank(*kind));
    let items = declared
        .into_iter()
        .map(|(kind, project_id)| {
            let known = projects.get(&project_id);
            DependencyInfo {
                kind,
                source,
                title: known.map_or_else(|| project_id.clone(), |project| project.title.clone()),
                icon_url: known.and_then(|project| project.icon_url.clone()),
                in_pack: installed.contains(&source.key(&project_id)),
                project_id,
            }
        })
        .collect();
    Ok(VersionDependencies { items })
}

/// O Modrinth como fonte de detalhes.
#[derive(Debug, Clone)]
pub struct ModrinthDetails {
    client: ModrinthClient,
}

impl ModrinthDetails {
    /// Fonte com o cliente do app.
    #[must_use]
    pub fn new(client: ModrinthClient) -> Self {
        Self { client }
    }
}

fn unavailable(error: &warden_modrinth::Error) -> Error {
    Error::new(Code::SearchSourceUnavailable, error.to_string()).param("source", "Modrinth")
}

#[async_trait::async_trait]
impl DetailSource for ModrinthDetails {
    fn id(&self) -> SourceId {
        SourceId::Modrinth
    }

    async fn gallery(
        &self,
        project_id: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<GalleryItem>> {
        let project = match self.client.project(project_id, Some(cancel)).await {
            Ok(project) => project,
            Err(warden_modrinth::Error::ProjectNotFound { .. }) => {
                return Err(Error::new(
                    Code::ItemNotFound,
                    format!("projeto {project_id} não encontrado no Modrinth"),
                )
                .param("path", project_id));
            }
            Err(error) => return Err(unavailable(&error)),
        };
        let mut items: Vec<GalleryItem> = project
            .gallery
            .into_iter()
            .filter(|image| images::is_https(&image.url))
            .map(|image| GalleryItem {
                url: image
                    .raw_url
                    .filter(|raw| images::is_https(raw))
                    .unwrap_or_else(|| image.url.clone()),
                thumb_url: image.url,
                title: image.title.filter(|title| !title.trim().is_empty()),
                description: image
                    .description
                    .filter(|description| !description.trim().is_empty()),
                featured: image.featured,
            })
            .collect();
        // A imagem de destaque vem primeiro; a ordem do autor vale no resto.
        items.sort_by_key(|item| !item.featured);
        Ok(items)
    }

    async fn notes(
        &self,
        _project_id: &str,
        version_id: &str,
        cancel: &CancellationToken,
    ) -> Result<Option<VersionNotes>> {
        let version = self
            .client
            .version(version_id, Some(cancel))
            .await
            .map_err(|error| unavailable(&error))?;
        Ok(version
            .changelog
            .filter(|text| !text.trim().is_empty())
            .map(|body| VersionNotes {
                body,
                format: DescriptionFormat::Markdown,
            }))
    }
}
