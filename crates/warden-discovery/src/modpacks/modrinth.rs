//! Mods de um modpack do Modrinth (R5B §6.1).
//!
//! 1. As versões do modpack para o pack (`GET /project/{id}/version` com os loaders e as versões
//!    do Minecraft do pack; nenhuma: todas). A mais nova é a lida.
//! 2. As dependências `embedded` dessa versão: cada mod com `project_id` e `version_id`; os
//!    arquivos fora do Modrinth só com `file_name`. Versão sem nenhuma (modpacks antigos): o
//!    `modrinth.index.json` do `.mrpack`, lido por partes ([`super::archive`]).
//! 3. Nomes, ícones, tipo, licença e as versões do Minecraft e loaders de cada mod em lote
//!    (`GET /projects?ids=`, com o próprio modpack junto).
//!
//! Um modpack de até 199 mods custa 3 requisições (CA-T08-14: no máximo 4).

use std::collections::HashSet;

use serde::Deserialize;
use warden_core::CancellationToken;
use warden_modrinth::{
    DependencyType, ModrinthClient, Project, ProjectType, Version, VersionFilter,
};
use warden_project::dedup::Identity;
use warden_project::search::modrinth::page_url;
use warden_project::search::{PackTarget, ProjectKind, SourceId};

use super::archive::read_entry;
use super::{
    ModpackContents, ModpackError, ModpackFailure, ModpackLicense, ModpackMod, PackView, Result,
    external, is_free_license, override_jars, sort_mods, store_state, unavailable,
};

/// O índice de um `.mrpack`.
const INDEX_FILE: &str = "modrinth.index.json";

/// Um mod que o modpack traz de uma loja.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StoreMod {
    project_id: String,
    version_id: Option<String>,
}

/// O que a versão do modpack lista.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Listing {
    store: Vec<StoreMod>,
    external: Vec<String>,
    whole_download: bool,
}

/// A lista de mods de um modpack do Modrinth.
pub(super) async fn contents(
    client: &ModrinthClient,
    http: &warden_http::HttpClient,
    pack: &PackView<'_>,
    project_id: &str,
    cancel: &CancellationToken,
) -> Result<ModpackContents> {
    let version = modpack_version(client, pack.target, project_id, cancel).await?;
    let mut listing = embedded(&version);
    if listing.store.is_empty() && listing.external.is_empty() {
        listing = from_index(http, &version, cancel).await?;
    }
    let mut ids = vec![project_id.to_owned()];
    ids.extend(listing.store.iter().map(|item| item.project_id.clone()));
    let projects = client
        .projects(&ids, Some(cancel))
        .await
        .map_err(|error| unavailable(SourceId::Modrinth, error))?;
    let find = |id: &str| projects.iter().find(|project| project.id == id);
    let modpack = find(project_id);
    let mut mods: Vec<ModpackMod> = Vec::new();
    let mut seen = HashSet::new();
    for item in &listing.store {
        if !seen.insert(item.project_id.clone()) {
            continue;
        }
        let project = find(&item.project_id);
        // Resource packs e shaders do modpack ficam fora: a lista é de mods.
        if project.is_some_and(|project| !is_mod(project)) {
            continue;
        }
        mods.push(store_mod(pack, item, project));
    }
    mods.extend(listing.external.iter().map(|name| external(name)));
    sort_mods(&mut mods);
    let license = modpack.and_then(|project| project.license.as_ref()).map_or(
        ModpackLicense {
            name: None,
            free: false,
        },
        |license| {
            let name = [&license.id, &license.name]
                .into_iter()
                .find(|value| !value.trim().is_empty())
                .cloned();
            ModpackLicense {
                free: is_free_license(&license.id),
                name,
            }
        },
    );
    Ok(ModpackContents {
        source: SourceId::Modrinth,
        project_id: project_id.to_owned(),
        title: modpack.map_or_else(|| version.name.clone(), |project| project.title.clone()),
        version_number: version.version_number.clone(),
        game_versions: version.game_versions.clone(),
        loaders: version.loaders.clone(),
        same_target: same_target(&version, pack.target),
        license,
        page_url: modpack.map(page_url),
        mods,
        whole_download: listing.whole_download,
    })
}

/// A versão do modpack lida: a mais nova para o pack; sem nenhuma, a mais nova de todas.
async fn modpack_version(
    client: &ModrinthClient,
    target: &PackTarget,
    project_id: &str,
    cancel: &CancellationToken,
) -> Result<Version> {
    let for_pack = VersionFilter {
        loaders: target.loader_names(ProjectKind::Modpack),
        game_versions: target.game_versions.clone(),
        featured: None,
        include_changelog: false,
    };
    let mut versions = fetch_versions(client, project_id, &for_pack, cancel).await?;
    if versions.is_empty() {
        let all = VersionFilter {
            include_changelog: false,
            ..VersionFilter::default()
        };
        versions = fetch_versions(client, project_id, &all, cancel).await?;
    }
    versions
        .into_iter()
        .max_by(|a, b| a.date_published.cmp(&b.date_published))
        .ok_or_else(|| ModpackError::unreadable("o modpack não tem versões").into())
}

async fn fetch_versions(
    client: &ModrinthClient,
    project_id: &str,
    filter: &VersionFilter,
    cancel: &CancellationToken,
) -> Result<Vec<Version>> {
    match client
        .project_versions(project_id, filter, Some(cancel))
        .await
    {
        Ok(versions) => Ok(versions),
        Err(warden_modrinth::Error::ProjectNotFound { .. }) => Err(ModpackFailure::Source(
            warden_project::Error::new(
                warden_project::ProjectErrorCode::ItemNotFound,
                format!("modpack {project_id} não encontrado no Modrinth"),
            )
            .param("path", project_id),
        )),
        Err(error) => Err(unavailable(SourceId::Modrinth, error)),
    }
}

/// A versão é da versão do Minecraft e do loader do pack.
fn same_target(version: &Version, target: &PackTarget) -> bool {
    let loaders = target.loader_names(ProjectKind::Modpack);
    version
        .game_versions
        .iter()
        .any(|game| game == &target.minecraft)
        && version
            .loaders
            .iter()
            .any(|loader| loaders.contains(loader))
}

/// As dependências `embedded` da versão do modpack.
fn embedded(version: &Version) -> Listing {
    let mut listing = Listing::default();
    for dependency in &version.dependencies {
        if dependency.dependency_type != DependencyType::Embedded {
            continue;
        }
        match (&dependency.project_id, &dependency.file_name) {
            (Some(project_id), _) if !project_id.is_empty() => listing.store.push(StoreMod {
                project_id: project_id.clone(),
                version_id: dependency.version_id.clone().filter(|id| !id.is_empty()),
            }),
            (_, Some(file_name)) if !file_name.is_empty() => {
                listing.external.push(file_name.clone());
            }
            _ => {}
        }
    }
    listing
}

#[derive(Debug, Deserialize)]
struct Index {
    #[serde(default)]
    files: Vec<IndexFile>,
}

#[derive(Debug, Deserialize)]
struct IndexFile {
    path: String,
    #[serde(default)]
    downloads: Vec<String>,
}

/// O `modrinth.index.json` do `.mrpack` principal, lido por partes.
async fn from_index(
    http: &warden_http::HttpClient,
    version: &Version,
    cancel: &CancellationToken,
) -> Result<Listing> {
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| ModpackError::unreadable("a versão do modpack não tem arquivo"))?;
    let entry = read_entry(
        http,
        SourceId::Modrinth,
        &file.url,
        INDEX_FILE,
        warden_http::RangeOptions::default(),
        cancel,
    )
    .await?;
    let index: Index = serde_json::from_slice(&entry.bytes)
        .map_err(|e| ModpackError::unreadable(format!("{INDEX_FILE} inválido: {e}")))?;
    let mut listing = Listing {
        whole_download: entry.whole_download,
        ..Listing::default()
    };
    for file in &index.files {
        if !file.path.starts_with("mods/") {
            continue;
        }
        match file.downloads.iter().find_map(|url| cdn_ids(url)) {
            Some((project_id, version_id)) => listing.store.push(StoreMod {
                project_id,
                version_id: Some(version_id),
            }),
            None => listing.external.push(file.path.clone()),
        }
    }
    listing.external.extend(override_jars(&entry.names));
    Ok(listing)
}

/// O projeto e a versão num endereço do CDN do Modrinth
/// (`https://cdn.modrinth.com/data/<projeto>/versions/<versão>/<arquivo>`).
fn cdn_ids(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://cdn.modrinth.com/data/")?;
    let mut parts = rest.split('/');
    let project = parts.next()?;
    (parts.next()? == "versions").then_some(())?;
    let version = parts.next()?;
    (!project.is_empty() && !version.is_empty()).then(|| (project.to_owned(), version.to_owned()))
}

/// Um mod da loja com o estado para o pack.
fn store_mod(pack: &PackView<'_>, item: &StoreMod, project: Option<&Project>) -> ModpackMod {
    let Some(project) = project else {
        // O Modrinth não devolveu o projeto (removido ou escondido): sem versão para o pack.
        return ModpackMod {
            key: SourceId::Modrinth.key(&item.project_id),
            source: Some(SourceId::Modrinth),
            project_id: Some(item.project_id.clone()),
            version_id: item.version_id.clone(),
            title: item.project_id.clone(),
            summary: String::new(),
            icon_url: None,
            file_name: None,
            state: super::ModpackModState::NoVersion,
            pack_path: None,
        };
    };
    let identity = Identity {
        author: "",
        slug: &project.slug,
        title: &project.title,
    };
    let (state, pack_path) = store_state(
        pack,
        SourceId::Modrinth,
        &project.id,
        identity,
        None,
        project_compatible(project, pack.target),
    );
    ModpackMod {
        key: SourceId::Modrinth.key(&project.id),
        source: Some(SourceId::Modrinth),
        project_id: Some(project.id.clone()),
        version_id: item.version_id.clone(),
        title: project.title.clone(),
        summary: project.description.clone(),
        icon_url: project.icon_url.clone().filter(|url| !url.is_empty()),
        file_name: None,
        state,
        pack_path,
    }
}

/// Projeto de mod (o Modrinth lista também os resource packs e shaders do modpack).
fn is_mod(project: &Project) -> bool {
    !matches!(
        project.project_type,
        ProjectType::Resourcepack
            | ProjectType::Shader
            | ProjectType::Datapack
            | ProjectType::Modpack
    )
}

/// O projeto tem alguma versão para o Minecraft e o loader do pack (pelas listas do próprio
/// projeto, sem requisição a mais).
fn project_compatible(project: &Project, target: &PackTarget) -> bool {
    let loaders = target.loader_names(ProjectKind::Mod);
    project
        .game_versions
        .iter()
        .any(|game| target.game_versions.contains(game))
        && (project.loaders.is_empty()
            || project
                .loaders
                .iter()
                .any(|loader| loaders.contains(loader)))
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn le_os_ids_do_cdn_do_modrinth() {
        assert_eq!(
            cdn_ids("https://cdn.modrinth.com/data/2RKFTmiB/versions/B2L4LeMV/tooltipfix.jar"),
            Some(("2RKFTmiB".into(), "B2L4LeMV".into()))
        );
        assert_eq!(cdn_ids("https://github.com/x/y/releases/a.jar"), None);
        assert_eq!(cdn_ids("https://cdn.modrinth.com/data/abc/other/x"), None);
    }
}
