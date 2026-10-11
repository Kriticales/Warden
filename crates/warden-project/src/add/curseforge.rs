//! Fonte CurseForge de "Adicionar": projetos e arquivos pela API, e o `.pw.toml` como o
//! `packwiz curseforge add` escreve (`mode = "metadata:curseforge"`, hash `sha1`,
//! `[update.curseforge]`; ARCHITECTURE §6.1 e §6.2).
//!
//! - **Versão:** o "ID da versão" desta fonte é o ID do arquivo.
//! - **Lado:** a CurseForge não informa o lado: o padrão é "Cliente e servidor", com a
//!   observação "Lado desconhecido — confira se é só de cliente" (SPEC T08, regras comuns).
//! - **SHA-1:** a API não acha arquivo por SHA-1 (só pela impressão digital murmur2), então
//!   [`AddSource::projects_by_sha1`] não acha nada aqui; a conferência entre fontes acontece
//!   pelo Modrinth, que acha o SHA-1 que a CurseForge informou.
//! - **Nada vai para o disco:** a crate `warden-curseforge` guarda o cache só em memória; aqui
//!   o `.pw.toml` leva apenas os IDs, o nome e o hash (sem endereço de download).

use std::collections::{HashMap, HashSet};

use warden_core::CancellationToken;
use warden_curseforge::{
    CurseforgeClient, File, FilesQuery, Mod, ModLoaderType, ProjectClass, RelationType, ReleaseType,
};
use warden_packwiz::{CurseForgeFile, HashFormat, Loader, Metafile};

use super::{
    AddSource, DependencyKind, SideNote, SourceDependency, SourceFile, SourceProject,
    SourceVersion, VersionChannel,
};
use crate::search::{PackTarget, ProjectKind, SourceId};
use crate::side::SideChoice;
use crate::{Error, ProjectErrorCode as Code, Result};

/// Arquivos pedidos por consulta (o máximo da API).
const FILES_PAGE: u32 = warden_curseforge::MAX_PAGE_SIZE;

/// A CurseForge como fonte de "Adicionar".
#[derive(Debug, Clone)]
pub struct CurseforgeAdd {
    client: CurseforgeClient,
}

impl CurseforgeAdd {
    /// Fonte com o cliente do app (com a chave do usuário).
    #[must_use]
    pub fn new(client: CurseforgeClient) -> Self {
        Self { client }
    }
}

/// Erro da API virando erro do domínio, com o nome da fonte para a frase.
pub(crate) fn unavailable(error: &warden_curseforge::Error) -> Error {
    Error::new(Code::SearchSourceUnavailable, error.to_string()).param("source", "CurseForge")
}

/// ID numérico da CurseForge (os de outras fontes são ignorados).
fn numeric(id: &str) -> Option<u64> {
    id.trim().parse().ok()
}

/// Tipo da página para a classe da CurseForge (nenhum: não entra pela página Adicionar).
#[must_use]
pub fn kind_of(class: Option<ProjectClass>) -> Option<ProjectKind> {
    match class? {
        ProjectClass::Mod => Some(ProjectKind::Mod),
        ProjectClass::ResourcePack => Some(ProjectKind::ResourcePack),
        ProjectClass::Shader => Some(ProjectKind::Shader),
        _ => None,
    }
}

/// Classe da CurseForge para o tipo da página.
#[must_use]
pub fn class_of(kind: ProjectKind) -> ProjectClass {
    match kind {
        ProjectKind::Mod => ProjectClass::Mod,
        ProjectKind::ResourcePack => ProjectClass::ResourcePack,
        ProjectKind::Shader => ProjectClass::Shader,
    }
}

/// O loader da CurseForge para o loader do pack.
#[must_use]
pub fn loader_type(loader: Loader) -> ModLoaderType {
    match loader {
        Loader::Fabric => ModLoaderType::Fabric,
        Loader::Forge => ModLoaderType::Forge,
        Loader::NeoForge => ModLoaderType::NeoForge,
        Loader::Quilt => ModLoaderType::Quilt,
        Loader::LiteLoader => ModLoaderType::LiteLoader,
    }
}

/// Os loaders da CurseForge que o pack aceita para um tipo (vazio: sem filtro de loader).
#[must_use]
pub fn loader_types(target: &PackTarget, kind: ProjectKind) -> Vec<ModLoaderType> {
    match kind {
        ProjectKind::Mod => target.loaders.iter().copied().map(loader_type).collect(),
        ProjectKind::ResourcePack | ProjectKind::Shader => Vec::new(),
    }
}

/// O melhor ícone do projeto.
#[must_use]
pub fn icon_of(project: &Mod) -> Option<String> {
    let logo = project.logo.as_ref()?;
    [&logo.thumbnail_url, &logo.url]
        .into_iter()
        .flatten()
        .find(|url| !url.trim().is_empty())
        .cloned()
}

fn source_project(project: Mod) -> SourceProject {
    SourceProject {
        source: SourceId::Curseforge,
        kind: kind_of(project.class_id),
        icon_url: icon_of(&project),
        id: project.id.to_string(),
        slug: project.slug,
        title: project.name,
        side: None,
    }
}

fn channel_of(release: Option<ReleaseType>) -> VersionChannel {
    match release {
        Some(ReleaseType::Release) => VersionChannel::Release,
        Some(ReleaseType::Beta) => VersionChannel::Beta,
        // Desconhecido conta como o canal mais instável.
        Some(ReleaseType::Alpha | ReleaseType::Other(_)) | None => VersionChannel::Alpha,
    }
}

/// Arquivo da CurseForge no formato comum.
#[must_use]
pub fn source_version(file: &File) -> SourceVersion {
    let dependencies = file
        .dependencies
        .iter()
        .filter_map(|dependency| {
            let kind = match dependency.relation_type {
                RelationType::RequiredDependency => DependencyKind::Required,
                RelationType::OptionalDependency => DependencyKind::Optional,
                RelationType::Incompatible => DependencyKind::Incompatible,
                // Embutida, incluída e ferramenta não pedem nada ao usuário.
                _ => return None,
            };
            Some(SourceDependency {
                project_id: Some(dependency.mod_id.to_string()),
                version_id: None,
                kind,
            })
        })
        .collect();
    SourceVersion {
        source: SourceId::Curseforge,
        id: file.id.to_string(),
        project_id: file.mod_id.to_string(),
        number: [&file.display_name, &file.file_name]
            .into_iter()
            .find(|text| !text.trim().is_empty())
            .cloned()
            .unwrap_or_default(),
        channel: channel_of(file.release_type),
        published: file.file_date.clone().unwrap_or_default(),
        game_versions: file.minecraft_versions(),
        loaders: file
            .loaders()
            .into_iter()
            .filter_map(ModLoaderType::tag)
            .map(str::to_ascii_lowercase)
            .collect(),
        file: Some(SourceFile {
            name: file.file_name.clone(),
            // A CurseForge baixa pela API: o `.pw.toml` não leva endereço.
            url: String::new(),
            sha1: file.sha1(),
            sha512: None,
        }),
        side: SideChoice::Both,
        side_note: Some(SideNote::Unknown),
        dependencies,
    }
}

#[async_trait::async_trait]
impl AddSource for CurseforgeAdd {
    fn id(&self) -> SourceId {
        SourceId::Curseforge
    }

    async fn projects(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<Vec<SourceProject>> {
        let ids: Vec<u64> = ids.iter().filter_map(|id| numeric(id)).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let projects = self
            .client
            .projects(&ids, Some(cancel))
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
        let Some(mod_id) = numeric(&project.id) else {
            return Ok(Vec::new());
        };
        let kind = project.kind.unwrap_or(ProjectKind::Mod);
        let loaders = loader_types(target, kind);
        // A API filtra por uma versão e um loader por consulta: uma consulta por combinação.
        let mut queries = Vec::new();
        for game_version in &target.game_versions {
            let base = FilesQuery::default()
                .game_version(game_version.clone())
                .page(0, FILES_PAGE);
            if loaders.is_empty() {
                queries.push(base);
            } else {
                queries.extend(loaders.iter().map(|loader| base.clone().loader(*loader)));
            }
        }
        let mut seen = HashSet::new();
        let mut versions = Vec::new();
        for query in queries {
            let page = match self.client.files(mod_id, &query, Some(cancel)).await {
                Ok(page) => page,
                Err(warden_curseforge::Error::ModNotFound { .. }) => return Ok(Vec::new()),
                Err(error) => return Err(unavailable(&error)),
            };
            for file in page.files {
                if !file.is_available
                    || file.is_server_pack == Some(true)
                    || file.sha1().is_none()
                    || !seen.insert(file.id)
                {
                    continue;
                }
                let version = source_version(&file);
                // O servidor já filtra; conferir de novo protege de uma resposta mais larga.
                if version.compatible(project.kind, target) {
                    versions.push(version);
                }
            }
        }
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(versions)
    }

    async fn versions(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<Vec<SourceVersion>> {
        let ids: Vec<u64> = ids.iter().filter_map(|id| numeric(id)).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let files = self
            .client
            .files_by_id(&ids, Some(cancel))
            .await
            .map_err(|e| unavailable(&e))?;
        Ok(files.iter().map(source_version).collect())
    }

    async fn projects_by_sha1(
        &self,
        _hashes: &[String],
        _cancel: &CancellationToken,
    ) -> Result<HashMap<String, String>> {
        // A API só reconhece arquivos pela impressão digital (murmur2), não pelo SHA-1.
        Ok(HashMap::new())
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
        let hash = file.sha1.clone().ok_or_else(|| {
            Error::new(
                Code::Internal,
                format!("a CurseForge não informou o sha1 de {}", file.name),
            )
        })?;
        let id = |text: &str, what: &str| {
            text.parse::<u32>().map_err(|_| {
                Error::new(
                    Code::Internal,
                    format!("ID de {what} da CurseForge fora do intervalo: {text}"),
                )
            })
        };
        let curseforge = CurseForgeFile {
            name: project.title.clone(),
            project_id: id(&project.id, "projeto")?,
            file_id: id(&version.id, "arquivo")?,
            filename: file.name.clone(),
            hash_format: HashFormat::Sha1,
            hash,
        };
        Ok(Metafile::curseforge(&curseforge, side.side()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(json: serde_json::Value) -> File {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn arquivo_no_formato_comum() {
        let file = file(serde_json::json!({
            "id": 4_000_001, "modId": 238_222, "isAvailable": true,
            "displayName": "jei-1.20.1-forge-15.20.0.106.jar",
            "fileName": "jei-1.20.1-forge-15.20.0.106.jar",
            "releaseType": 1, "fileDate": "2024-05-01T10:00:00Z",
            "hashes": [{"value": "ABCDEF0123456789ABCDEF0123456789ABCDEF01", "algo": 1}],
            "downloadUrl": "https://edge.forgecdn.net/files/4000/001/jei.jar",
            "gameVersions": ["1.20.1", "Forge", "Client"],
            "sortableGameVersions": [
                {"gameVersionName": "1.20.1", "gameVersion": "1.20.1"},
                {"gameVersionName": "Forge", "gameVersion": ""}
            ],
            "dependencies": [
                {"modId": 1, "relationType": 3},
                {"modId": 2, "relationType": 2},
                {"modId": 3, "relationType": 5},
                {"modId": 4, "relationType": 1}
            ]
        }));
        let version = source_version(&file);
        assert_eq!(version.id, "4000001");
        assert_eq!(version.project_id, "238222");
        assert_eq!(version.channel, VersionChannel::Release);
        assert_eq!(version.game_versions, ["1.20.1"]);
        assert_eq!(version.loaders, ["forge"]);
        let source = version.file.as_ref().unwrap();
        assert_eq!(
            source.sha1.as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef01")
        );
        assert!(source.url.is_empty(), "o endereço nunca vai para o pack");
        let kinds: Vec<_> = version.dependencies.iter().map(|d| d.kind).collect();
        assert_eq!(
            kinds,
            [
                DependencyKind::Required,
                DependencyKind::Optional,
                DependencyKind::Incompatible
            ]
        );
        assert_eq!(version.side_note, Some(SideNote::Unknown));
    }

    #[test]
    fn metafile_no_formato_do_packwiz() {
        let client = CurseforgeClient::new(
            warden_http::HttpClient::new(warden_http::HttpConfig::for_version("0.0.0")).unwrap(),
            None,
        )
        .unwrap();
        let add = CurseforgeAdd::new(client);
        let project = SourceProject {
            source: SourceId::Curseforge,
            id: "238222".into(),
            slug: "jei".into(),
            title: "Just Enough Items (JEI)".into(),
            kind: Some(ProjectKind::Mod),
            icon_url: None,
            side: None,
        };
        let mut version = source_version(&file(serde_json::json!({
            "id": 4_000_001, "modId": 238_222, "fileName": "jei.jar", "isAvailable": true,
            "hashes": [{"value": "abcdef0123456789abcdef0123456789abcdef01", "algo": 1}],
        })));
        let metafile = add.metafile(&project, &version, SideChoice::Both).unwrap();
        let text = metafile.to_toml_string();
        assert!(text.contains("mode = \"metadata:curseforge\""), "{text}");
        assert!(text.contains("hash-format = \"sha1\""), "{text}");
        assert!(text.contains("project-id = 238222"), "{text}");
        assert!(text.contains("file-id = 4000001"), "{text}");
        assert!(!text.contains("url ="), "{text}");
        version.file.as_mut().unwrap().sha1 = None;
        assert!(add.metafile(&project, &version, SideChoice::Both).is_err());
    }
}
