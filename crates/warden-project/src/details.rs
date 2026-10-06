//! Detalhes de um item, para o painel lateral (SPEC T07; CA-T07-02).
//!
//! Modrinth: projeto e versão pelo cliente, que guarda no cache local; sem internet, o cache
//! serve. CurseForge: tudo ao vivo e só em memória (termos da API, R3 §3.2); sem internet ou sem
//! chave, os dados do `.pw.toml` (nome, arquivo, lado e hash) continuam no painel.

use std::path::Path;

use serde::Serialize;
use warden_core::CancellationToken;
use warden_curseforge::CurseforgeClient;
use warden_modrinth::{ModrinthClient, Project, ProjectType, Version};

use crate::Result;
use crate::inventory::{InventoryItem, ItemSource, ItemState, Scan, readable_file_version, scan};

/// Formato da descrição longa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DescriptionFormat {
    /// Markdown (Modrinth).
    Markdown,
    /// HTML (CurseForge); a interface higieniza antes de mostrar.
    Html,
}

/// De onde vieram os detalhes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DetailsSource {
    /// Dados atuais: buscados agora, ou do cache do Modrinth ainda dentro da validade (24 h),
    /// sem precisar da rede.
    Live,
    /// Cache do Modrinth vencido, usado porque a rede falhou.
    Cache,
    /// Sem internet e sem cache (a CurseForge sempre cai aqui sem rede).
    Offline,
    /// CurseForge sem chave (ou com a chave recusada).
    NoKey,
    /// Arquivo local ou link direto: não há fonte para consultar.
    None,
    /// A fonte respondeu com erro (projeto removido, resposta inválida…); só os dados do
    /// `.pw.toml` aparecem.
    Unavailable,
}

/// Versão instalada.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    /// Número legível.
    pub number: String,
    /// Versões do Minecraft.
    pub game_versions: Vec<String>,
    /// Loaders, em minúsculas (`fabric`, `forge`…).
    pub loaders: Vec<String>,
    /// Data de publicação (RFC 3339).
    pub published: Option<String>,
    /// Nome do arquivo.
    pub file_name: String,
    /// Tamanho em bytes.
    pub size_bytes: Option<f64>,
}

/// Arquivo do item, sempre lido do pack (CA-T07-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    /// Nome do arquivo.
    pub file_name: String,
    /// Formato do hash (`sha1`, `sha256`, `sha512`, `murmur2`…).
    pub hash_format: String,
    /// Hash.
    pub hash: String,
    /// Link do download, quando o metafile tem um.
    pub url: Option<String>,
}

/// Tudo o que o painel de detalhes mostra.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemDetails {
    /// A linha do inventário (nome, lado, fonte…), sempre preenchida.
    pub item: InventoryItem,
    /// De onde vieram os dados da fonte.
    pub source: DetailsSource,
    /// Título.
    pub title: String,
    /// Autores (vazio no Modrinth: o cliente ainda não consulta a equipe).
    pub authors: Vec<String>,
    /// Página do projeto.
    pub page_url: Option<String>,
    /// Ícone.
    pub icon_url: Option<String>,
    /// Resumo curto.
    pub summary: Option<String>,
    /// Descrição longa.
    pub description: Option<String>,
    /// Formato da descrição.
    pub description_format: DescriptionFormat,
    /// Versão instalada.
    pub version: Option<VersionInfo>,
    /// Arquivo, do `.pw.toml` (ou do índice, num arquivo local).
    pub file: Option<FileInfo>,
    /// Notas da versão instalada.
    pub changelog: Option<String>,
}

/// Detalhes de um item pelo caminho relativo à pasta do pack. Só falha se o pack não puder ser
/// lido ou o item não estiver no inventário ([`crate::ProjectErrorCode::ItemNotFound`],
/// parâmetro `path`); falta de rede, de chave ou de cache aparece em [`ItemDetails::source`].
pub async fn item_details(
    root: &Path,
    path: &str,
    modrinth: Option<&ModrinthClient>,
    curseforge: Option<&CurseforgeClient>,
    cancel: Option<&CancellationToken>,
) -> Result<ItemDetails> {
    let scan = scan(root)?;
    let mut item = scan.require(path)?.clone();
    if item.version.is_none() {
        item.version = item.file_name.as_deref().map(readable_file_version);
    }
    let mut details = ItemDetails {
        title: item.name.clone(),
        file: file_info(&scan, &item),
        item,
        source: DetailsSource::None,
        authors: Vec::new(),
        page_url: None,
        icon_url: None,
        summary: None,
        description: None,
        description_format: DescriptionFormat::Markdown,
        version: None,
        changelog: None,
    };
    if details.item.state != ItemState::Ok {
        return Ok(details);
    }
    match details.item.source {
        ItemSource::Modrinth => modrinth_details(&mut details, modrinth, cancel).await,
        ItemSource::Curseforge => curseforge_details(&mut details, curseforge, cancel).await,
        ItemSource::Url | ItemSource::Local => {}
    }
    Ok(details)
}

fn file_info(scan: &Scan, item: &InventoryItem) -> Option<FileInfo> {
    if let Some(metafile) = scan.metafiles.get(&item.path) {
        return Some(FileInfo {
            file_name: metafile.filename.clone(),
            hash_format: metafile.download.hash_format.clone(),
            hash: metafile.download.hash.clone(),
            url: (!metafile.download.url.is_empty()).then(|| metafile.download.url.clone()),
        });
    }
    let (format, hash) = scan.hashes.get(&item.path)?;
    Some(FileInfo {
        file_name: item.file_name.clone().unwrap_or_default(),
        hash_format: format.clone(),
        hash: hash.clone(),
        url: None,
    })
}

async fn modrinth_details(
    details: &mut ItemDetails,
    client: Option<&ModrinthClient>,
    cancel: Option<&CancellationToken>,
) {
    let (Some(client), Some(project_id)) = (client, details.item.project_id.clone()) else {
        details.source = DetailsSource::Offline;
        return;
    };
    let before = match client.cache() {
        Some(cache) => cache.project(&project_id).await.ok().flatten(),
        None => None,
    };
    match client.project(&project_id, cancel).await {
        Ok(project) => {
            details.source = match &before {
                Some(cached) if !cached.fresh => {
                    // O cliente devolve o projeto vencido quando a rede falha; se a busca deu
                    // certo, o cache foi regravado com um horário mais novo.
                    let after = match client.cache() {
                        Some(cache) => cache.project(&project_id).await.ok().flatten(),
                        None => None,
                    };
                    if after.is_some_and(|after| after.fetched_at > cached.fetched_at) {
                        DetailsSource::Live
                    } else {
                        DetailsSource::Cache
                    }
                }
                _ => DetailsSource::Live,
            };
            apply_modrinth_project(details, &project);
        }
        Err(error) if error.is_offline() => details.source = DetailsSource::Offline,
        Err(_) => details.source = DetailsSource::Unavailable,
    }
    let Some(version_id) = details.item.source_version_id.clone() else {
        return;
    };
    let version = match client.version(&version_id, cancel).await {
        Ok(version) => Some(version),
        // Sem rede, a versão sem as notas (gravada pelo inventário) ainda serve.
        Err(_) => match client.cache() {
            Some(cache) => cache
                .versions(std::slice::from_ref(&version_id), false)
                .await
                .ok()
                .and_then(|mut versions| versions.pop()),
            None => None,
        },
    };
    if let Some(version) = version {
        apply_modrinth_version(details, &version);
    }
}

fn apply_modrinth_project(details: &mut ItemDetails, project: &Project) {
    details.title.clone_from(&project.title);
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
    details.page_url = Some(format!("https://modrinth.com/{kind}/{slug}"));
    details.icon_url.clone_from(&project.icon_url);
    details.summary = non_empty(&project.description);
    details.description = non_empty(&project.body);
    details.description_format = DescriptionFormat::Markdown;
    details.item.summary.clone_from(&details.summary);
    details.item.icon_url.clone_from(&project.icon_url);
}

fn apply_modrinth_version(details: &mut ItemDetails, version: &Version) {
    let file = version
        .files
        .iter()
        .find(|file| Some(&file.filename) == details.item.file_name.as_ref())
        .or_else(|| version.primary_file());
    #[allow(clippy::cast_precision_loss)] // Tamanhos de arquivo cabem com folga em f64.
    let size_bytes = file.map(|file| file.size as f64);
    details.version = Some(VersionInfo {
        number: version.version_number.clone(),
        game_versions: version.game_versions.clone(),
        loaders: version.loaders.clone(),
        published: non_empty(&version.date_published),
        file_name: file.map_or_else(
            || details.item.file_name.clone().unwrap_or_default(),
            |file| file.filename.clone(),
        ),
        size_bytes,
    });
    if !version.version_number.trim().is_empty() {
        details.item.version = Some(version.version_number.clone());
    }
    details.changelog = version.changelog.as_deref().and_then(non_empty);
}

async fn curseforge_details(
    details: &mut ItemDetails,
    client: Option<&CurseforgeClient>,
    cancel: Option<&CancellationToken>,
) {
    details.description_format = DescriptionFormat::Html;
    let Some(client) = client.filter(|client| client.has_key()) else {
        details.source = DetailsSource::NoKey;
        return;
    };
    let ids = details
        .item
        .project_id
        .as_deref()
        .and_then(|id| id.parse::<u64>().ok())
        .zip(
            details
                .item
                .source_version_id
                .as_deref()
                .and_then(|id| id.parse::<u64>().ok()),
        );
    let Some((project_id, file_id)) = ids else {
        // Metafile em modo CurseForge sem `[update.curseforge]`: não há o que consultar.
        details.source = DetailsSource::Unavailable;
        return;
    };
    let project = match client.project(project_id, cancel).await {
        Ok(project) => project,
        Err(error) => {
            details.source = curseforge_failure(&error);
            return;
        }
    };
    details.source = DetailsSource::Live;
    details.title.clone_from(&project.name);
    details.authors = project
        .authors
        .iter()
        .map(|author| author.name.clone())
        .collect();
    details.page_url.clone_from(&project.links.website_url);
    details.icon_url = project
        .logo
        .as_ref()
        .and_then(|logo| logo.thumbnail_url.clone().or_else(|| logo.url.clone()));
    details.summary = non_empty(&project.summary);
    details.item.summary.clone_from(&details.summary);
    details.item.icon_url.clone_from(&details.icon_url);
    // Descrição e arquivo são complementares: uma falha neles não apaga o projeto já obtido.
    details.description = client
        .description(project_id, cancel)
        .await
        .ok()
        .as_deref()
        .and_then(non_empty);
    if let Ok(file) = client.file(project_id, file_id, cancel).await {
        let number = if file.display_name.trim().is_empty() {
            readable_file_version(&file.file_name)
        } else {
            file.display_name.clone()
        };
        #[allow(clippy::cast_precision_loss)] // Tamanhos de arquivo cabem com folga em f64.
        let size_bytes = Some(file.file_length as f64);
        details.version = Some(VersionInfo {
            number,
            game_versions: file.minecraft_versions(),
            loaders: file
                .loaders()
                .into_iter()
                .filter_map(warden_curseforge::ModLoaderType::tag)
                .map(str::to_lowercase)
                .collect(),
            published: file.file_date.clone(),
            file_name: file.file_name.clone(),
            size_bytes,
        });
    }
}

fn curseforge_failure(error: &warden_curseforge::Error) -> DetailsSource {
    if error.is_key_problem() {
        DetailsSource::NoKey
    } else if error.is_offline() {
        DetailsSource::Offline
    } else {
        DetailsSource::Unavailable
    }
}

fn non_empty(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_owned())
}
