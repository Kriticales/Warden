//! Mods de um modpack da CurseForge (R5B §6.2).
//!
//! A API não tem rota com o conteúdo de um modpack: a lista está no `manifest.json` de dentro
//! do zip. O Warden pede o projeto, escolhe o arquivo do modpack para o pack (pelos
//! `latestFilesIndexes`), pede o endereço de download e lê **só** o `manifest.json` por partes
//! ([`super::archive`]); os nomes, ícones e hashes vêm em lote (`POST /mods`, `POST
//! /mods/files`). Tudo fica em memória (termos da CurseForge; CA-T08-16): nem as respostas da
//! API nem o manifesto são gravados.
//!
//! Modpack cujo autor bloqueou apps de terceiros (`allowModDistribution` falso ou
//! `downloadUrl` nulo): [`crate::DiscoveryErrorCode::ModpackBlocked`], com a página do modpack.

use std::collections::HashSet;

use serde::Deserialize;
use warden_core::CancellationToken;
use warden_curseforge::{CurseforgeClient, File, FileIndex, Mod};
use warden_project::add::curseforge::loader_types;
use warden_project::dedup::Identity;
use warden_project::search::curseforge::hit_compatible;
use warden_project::search::{PackTarget, ProjectKind, SourceId};

use super::archive::read_entry;
use super::{
    ModpackContents, ModpackError, ModpackFailure, ModpackLicense, ModpackMod, PackView, Result,
    external, override_jars, sort_mods, store_state, unavailable,
};
use crate::DiscoveryErrorCode;

/// O manifesto de um zip de modpack da CurseForge.
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    #[serde(default)]
    minecraft: ManifestMinecraft,
    #[serde(default)]
    version: String,
    #[serde(default)]
    files: Vec<ManifestFile>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestMinecraft {
    #[serde(default)]
    version: String,
    #[serde(default)]
    mod_loaders: Vec<ManifestLoader>,
}

#[derive(Debug, Deserialize)]
struct ManifestLoader {
    /// `forge-47.2.0`, `neoforge-21.1.1`, `fabric-0.15.0`.
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
}

/// A lista de mods de um modpack da CurseForge.
pub(super) async fn contents(
    client: &CurseforgeClient,
    http: &warden_http::HttpClient,
    pack: &PackView<'_>,
    project_id: &str,
    cancel: &CancellationToken,
) -> Result<ModpackContents> {
    let id: u64 = project_id.trim().parse().map_err(|_| {
        ModpackError::unreadable(format!("ID da CurseForge inválido: {project_id}"))
    })?;
    let modpack = client
        .project(id, Some(cancel))
        .await
        .map_err(|error| source_error(error, project_id))?;
    let page_url = modpack
        .links
        .website_url
        .clone()
        .filter(|url| !url.trim().is_empty());
    if modpack.is_distribution_blocked() {
        return Err(blocked(page_url.as_deref()));
    }
    let (file_id, same_target) = pick_file(&modpack, pack.target)
        .ok_or_else(|| ModpackError::unreadable("o modpack não tem arquivos"))?;
    let url = match client.download_url(id, file_id, Some(cancel)).await {
        Ok(url) => url,
        Err(warden_curseforge::Error::DistributionBlocked { .. }) => {
            return Err(blocked(page_url.as_deref()));
        }
        Err(error) => return Err(source_error(error, project_id)),
    };
    let entry = read_entry(
        http,
        SourceId::Curseforge,
        url.as_str(),
        MANIFEST_FILE,
        warden_http::RangeOptions::default(),
        cancel,
    )
    .await?;
    let manifest: Manifest = serde_json::from_slice(&entry.bytes)
        .map_err(|e| ModpackError::unreadable(format!("{MANIFEST_FILE} inválido: {e}")))?;
    let project_ids: Vec<u64> = manifest.files.iter().map(|file| file.project_id).collect();
    let file_ids: Vec<u64> = manifest.files.iter().map(|file| file.file_id).collect();
    let (projects, files) = tokio::join!(
        client.projects(&project_ids, Some(cancel)),
        client.files_by_id(&file_ids, Some(cancel)),
    );
    let projects = projects.map_err(|error| source_error(error, project_id))?;
    let files = files.map_err(|error| source_error(error, project_id))?;
    let mut mods = Vec::new();
    let mut seen = HashSet::new();
    for item in &manifest.files {
        if !seen.insert(item.project_id) {
            continue;
        }
        let project = projects
            .iter()
            .find(|project| project.id == item.project_id);
        let file = files.iter().find(|file| file.id == item.file_id);
        if let Some(project) = project.filter(|project| !is_non_mod(project)) {
            mods.push(store_mod(pack, project, item.file_id, file));
        } else if project.is_none() {
            mods.push(missing_mod(item));
        }
    }
    mods.extend(
        override_jars(&entry.names)
            .iter()
            .map(|name| external(name)),
    );
    sort_mods(&mut mods);
    let loaders = manifest
        .minecraft
        .mod_loaders
        .iter()
        .filter_map(|loader| loader.id.split('-').next())
        .map(str::to_owned)
        .collect();
    Ok(ModpackContents {
        source: SourceId::Curseforge,
        project_id: project_id.to_owned(),
        title: modpack.name.clone(),
        version_number: if manifest.version.trim().is_empty() {
            file_name(&modpack, file_id)
        } else {
            manifest.version.clone()
        },
        game_versions: if manifest.minecraft.version.is_empty() {
            Vec::new()
        } else {
            vec![manifest.minecraft.version.clone()]
        },
        loaders,
        same_target,
        // A CurseForge não informa a licença do projeto: a interface avisa.
        license: ModpackLicense {
            name: None,
            free: false,
        },
        page_url,
        mods,
        whole_download: entry.whole_download,
    })
}

/// O arquivo do modpack para o pack (o mais novo da versão do Minecraft e do loader do pack) e
/// se ele é do mesmo alvo; sem nenhum, o arquivo principal.
fn pick_file(modpack: &Mod, target: &PackTarget) -> Option<(u64, bool)> {
    let loaders = loader_types(target, ProjectKind::Modpack);
    let matches = |index: &&FileIndex| {
        index.game_version == target.minecraft
            && index
                .mod_loader
                .is_some_and(|loader| loaders.contains(&loader))
    };
    if let Some(index) = modpack
        .latest_files_indexes
        .iter()
        .filter(matches)
        .max_by_key(|index| index.file_id)
    {
        return Some((index.file_id, true));
    }
    if modpack.main_file_id != 0 {
        return Some((modpack.main_file_id, false));
    }
    modpack
        .latest_files
        .iter()
        .map(|file| file.id)
        .max()
        .map(|id| (id, false))
}

/// Nome legível do arquivo do modpack (quando o manifesto não traz a versão).
fn file_name(modpack: &Mod, file_id: u64) -> String {
    modpack
        .latest_files
        .iter()
        .find(|file| file.id == file_id)
        .map(|file| file.display_name.clone())
        .or_else(|| {
            modpack
                .latest_files_indexes
                .iter()
                .find(|index| index.file_id == file_id)
                .map(|index| index.filename.clone())
        })
        .unwrap_or_else(|| file_id.to_string())
}

/// Resource packs e shaders do manifesto ficam fora: a lista é de mods.
fn is_non_mod(project: &Mod) -> bool {
    matches!(
        project.class_id,
        Some(
            warden_curseforge::ProjectClass::ResourcePack | warden_curseforge::ProjectClass::Shader
        )
    )
}

fn store_mod(pack: &PackView<'_>, project: &Mod, file_id: u64, file: Option<&File>) -> ModpackMod {
    let project_id = project.id.to_string();
    let author = project
        .authors
        .first()
        .map(|author| author.name.as_str())
        .unwrap_or_default();
    let identity = Identity {
        author,
        slug: &project.slug,
        title: &project.name,
    };
    let sha1 = file.and_then(File::sha1);
    let (state, pack_path) = store_state(
        pack,
        SourceId::Curseforge,
        &project_id,
        identity,
        sha1.as_deref(),
        hit_compatible(project, ProjectKind::Mod, pack.target),
    );
    ModpackMod {
        key: SourceId::Curseforge.key(&project_id),
        source: Some(SourceId::Curseforge),
        project_id: Some(project_id),
        version_id: Some(file_id.to_string()),
        title: project.name.clone(),
        summary: project.summary.clone(),
        icon_url: project.logo.as_ref().and_then(|logo| {
            [&logo.thumbnail_url, &logo.url]
                .into_iter()
                .flatten()
                .find(|url| !url.is_empty())
                .cloned()
        }),
        file_name: file.map(|file| file.file_name.clone()),
        state,
        pack_path,
    }
}

/// Projeto que a CurseForge não devolveu (removido): sem versão para o pack.
fn missing_mod(item: &ManifestFile) -> ModpackMod {
    let project_id = item.project_id.to_string();
    ModpackMod {
        key: SourceId::Curseforge.key(&project_id),
        source: Some(SourceId::Curseforge),
        title: project_id.clone(),
        project_id: Some(project_id),
        version_id: Some(item.file_id.to_string()),
        summary: String::new(),
        icon_url: None,
        file_name: None,
        state: super::ModpackModState::NoVersion,
        pack_path: None,
    }
}

fn blocked(page: Option<&str>) -> ModpackFailure {
    let error = ModpackError::new(
        DiscoveryErrorCode::ModpackBlocked,
        "o autor do modpack bloqueou apps de terceiros",
    );
    match page {
        Some(page) => error.param("page", page),
        None => error,
    }
    .into()
}

fn source_error(error: warden_curseforge::Error, project_id: &str) -> ModpackFailure {
    match error {
        warden_curseforge::Error::ModNotFound { .. } => ModpackFailure::Source(
            warden_project::Error::new(
                warden_project::ProjectErrorCode::ItemNotFound,
                format!("modpack {project_id} não encontrado na CurseForge"),
            )
            .param("path", project_id),
        ),
        other => unavailable(SourceId::Curseforge, other),
    }
}
