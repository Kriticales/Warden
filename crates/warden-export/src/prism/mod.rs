//! Instância pronta para o Prism (E-04; ADR-0050; ARCHITECTURE §12.3): um `.mrpack` gerado pelo
//! Warden em que o Prism baixa os mods pelos endereços de `files[].downloads` — inclusive os da
//! CurseForge, com a chave do próprio jogador — e que o `packwiz modrinth export` não sabe fazer.

mod model;
mod plan;
mod validate;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use sha2::{Digest as _, Sha256};
use url::Url;
use warden_core::{CancellationToken, Progress, ProgressSink};
use warden_curseforge::{CurseforgeClient, Distribution, FileRef};
use warden_http::{DownloadRequest, HttpClient};
use warden_instance::{DownloadCache, FileOrigin};
use warden_modrinth::{HashAlgorithm, ModrinthClient};
use warden_packwiz::{HashFormat, Side};
use warden_packwiz_cli::Packwiz;
use zip::write::SimpleFileOptions;

pub use model::{
    PrismAnalysis, PrismBlockedMod, PrismLocalFile, PrismOptions, PrismResult, PrismSwap,
};
use plan::{PackPlan, RefItem, RefSource, curseforge_cdn_url, mrpack_loader_key};

use crate::native::{ExportSource, SourceReader, check_destination, indexed_files, prepare_stage};
use crate::{Error, ExportErrorCode, Result};

/// Etapa de resolução dos arquivos (rótulo no catálogo da interface).
pub const STAGE_RESOLVE: &str = "prismResolve";
/// Rótulo da etapa de resolução.
pub const STAGE_RESOLVE_LABEL: &str = "export.prism.resolve";

/// Endereço público da CDN da CurseForge, o que vai gravado no arquivo.
const CURSEFORGE_CDN: &str = "https://edge.forgecdn.net";

/// Servidores que a geração consulta.
pub struct PrismServices {
    /// Cliente HTTP (downloads).
    pub http: HttpClient,
    /// Cliente da CurseForge; sem chave não confere a distribuição.
    pub curseforge: Option<CurseforgeClient>,
    /// Cliente do Modrinth (troca de mods bloqueados).
    pub modrinth: ModrinthClient,
    /// Cache de downloads (`cache/downloads`).
    pub cache: Arc<DownloadCache>,
    /// De onde baixar os arquivos da CurseForge; só os testes trocam (`None` = CDN pública). O
    /// endereço gravado no arquivo é sempre o da CDN pública.
    pub cdn_download_base: Option<Url>,
}

impl PrismServices {
    /// Serviços com a CDN pública da CurseForge.
    #[must_use]
    pub fn new(
        http: HttpClient,
        curseforge: Option<CurseforgeClient>,
        modrinth: ModrinthClient,
        cache: Arc<DownloadCache>,
    ) -> Self {
        Self {
            http,
            curseforge,
            modrinth,
            cache,
            cdn_download_base: None,
        }
    }
}

fn cancelled() -> Error {
    Error::internal("exportação cancelada")
}

fn lookup(error: impl Display) -> Error {
    Error::new(ExportErrorCode::PrismLookupFailed, error.to_string())
}

/// Mod bloqueado, com o alvo da troca (endereço e hashes do arquivo do Modrinth).
struct Blocked {
    view: PrismBlockedMod,
    target: Option<SwapTarget>,
    index: usize,
}

struct SwapTarget {
    url: String,
    filename: String,
    sha1: String,
    sha512: String,
    size: u64,
}

async fn find_blocked(
    services: &PrismServices,
    plan: &PackPlan,
    cancel: &CancellationToken,
) -> Result<Vec<Blocked>> {
    let curseforge: Vec<(usize, u32, u32)> = plan
        .refs
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match r.source {
            RefSource::Curseforge { project, file } => Some((i, project, file)),
            _ => None,
        })
        .collect();
    if curseforge.is_empty() {
        return Ok(Vec::new());
    }
    let client = services
        .curseforge
        .as_ref()
        .filter(|client| client.has_key())
        .ok_or_else(|| {
            Error::new(
                ExportErrorCode::PrismCurseforgeKeyMissing,
                "falta a chave da CurseForge",
            )
        })?;
    let refs: Vec<FileRef> = curseforge
        .iter()
        .map(|&(_, p, f)| FileRef::new(u64::from(p), u64::from(f)))
        .collect();
    let answers = match client.distribution(&refs, Some(cancel)).await {
        Ok(answers) => answers,
        Err(warden_curseforge::Error::Http(warden_http::Error::Cancelled)) => {
            return Err(cancelled());
        }
        Err(warden_curseforge::Error::KeyMissing) => {
            return Err(Error::new(
                ExportErrorCode::PrismCurseforgeKeyMissing,
                "falta a chave da CurseForge",
            ));
        }
        Err(error) => return Err(lookup(error)),
    };
    let mut blocked = Vec::new();
    for (&(index, ..), answer) in curseforge.iter().zip(answers) {
        let page_url = match answer.distribution {
            Distribution::Allowed { .. } => continue,
            Distribution::Blocked { page_url } => page_url,
            Distribution::Missing => None,
        };
        let item = &plan.refs[index];
        blocked.push(Blocked {
            view: PrismBlockedMod {
                path: item.meta_path.clone(),
                name: item.name.clone(),
                file_name: item.filename.clone(),
                page_url,
                swap: None,
            },
            target: None,
            index,
        });
    }
    find_swaps(services, plan, &mut blocked, cancel).await?;
    Ok(blocked)
}

/// Procura o mesmo arquivo no Modrinth, pelo SHA-1 do metafile.
async fn find_swaps(
    services: &PrismServices,
    plan: &PackPlan,
    blocked: &mut [Blocked],
    cancel: &CancellationToken,
) -> Result<()> {
    let sha1_of = |b: &Blocked| {
        let expected = &plan.refs[b.index].expected;
        (expected.format == HashFormat::Sha1).then(|| expected.value.trim().to_ascii_lowercase())
    };
    let hashes: Vec<String> = blocked.iter().filter_map(sha1_of).collect();
    if hashes.is_empty() {
        return Ok(());
    }
    let found = match services
        .modrinth
        .version_files(&hashes, HashAlgorithm::Sha1, Some(cancel))
        .await
    {
        Ok(found) => found,
        Err(warden_modrinth::Error::Http(warden_http::Error::Cancelled)) => {
            return Err(cancelled());
        }
        Err(error) => return Err(lookup(error)),
    };
    for entry in blocked {
        let Some(hash) = sha1_of(entry) else { continue };
        let Some(version) = found.get(&hash) else {
            continue;
        };
        let Some(file) = version
            .files
            .iter()
            .find(|f| f.hashes.sha1.eq_ignore_ascii_case(&hash))
        else {
            continue;
        };
        if file.hashes.sha512.is_empty() || file.size == 0 || !file.url.starts_with("https://") {
            continue;
        }
        entry.view.swap = Some(PrismSwap {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            version_number: version.version_number.clone(),
            file_name: file.filename.clone(),
        });
        entry.target = Some(SwapTarget {
            url: file.url.clone(),
            filename: file.filename.clone(),
            sha1: hash,
            sha512: file.hashes.sha512.to_ascii_lowercase(),
            size: file.size,
        });
    }
    Ok(())
}

fn untrusted_count(plan: &PackPlan) -> usize {
    plan.refs
        .iter()
        .filter(|r| !matches!(r.source, RefSource::Modrinth { .. }))
        .count()
        + plan
            .overrides
            .keys()
            .filter(|p| plan::is_loose_mod_jar(p))
            .count()
}

/// O que a geração vai fazer: contagens, mods bloqueados (com a troca possível) e arquivos de
/// terceiros que pedem confirmação. Só lê o pack e consulta Modrinth e CurseForge; não baixa
/// arquivos nem escreve nada.
pub async fn analyze_prism(
    root: &Path,
    source: &ExportSource,
    services: &PrismServices,
    cancel: &CancellationToken,
) -> Result<PrismAnalysis> {
    let files = indexed_files(&SourceReader::open(root, source)?)?;
    let plan = PackPlan::read(&files, false)?;
    let (blocked, key_missing) = match find_blocked(services, &plan, cancel).await {
        Ok(blocked) => (blocked, false),
        Err(error) if error.is_code(ExportErrorCode::PrismCurseforgeKeyMissing) => {
            (Vec::new(), true)
        }
        Err(error) => return Err(error),
    };
    let count = |f: fn(&RefSource) -> bool| plan.refs.iter().filter(|r| f(&r.source)).count();
    Ok(PrismAnalysis {
        version: plan.manifest.version.trim().to_owned(),
        minecraft: plan.game.minecraft.clone(),
        loader: plan
            .game
            .loader
            .as_ref()
            .map(|l| format!("{} {}", l.loader, l.version))
            .unwrap_or_default(),
        modrinth: count(|s| matches!(s, RefSource::Modrinth { .. })),
        curseforge: count(|s| matches!(s, RefSource::Curseforge { .. })),
        links: count(|s| matches!(s, RefSource::Link { .. })),
        overrides: plan.overrides.len(),
        untrusted: untrusted_count(&plan),
        curseforge_key_missing: key_missing,
        blocked: blocked.into_iter().map(|b| b.view).collect(),
        local_files: plan
            .third_party_locals()
            .into_iter()
            .map(|(path, bytes)| PrismLocalFile {
                path: path.to_owned(),
                bytes,
            })
            .collect(),
    })
}

/// Hashes e tamanho de um arquivo que o Prism vai baixar.
struct Resolved {
    sha1: String,
    sha512: String,
    size: u64,
}

async fn resolve(
    services: &PrismServices,
    item: &RefItem,
    progress: &dyn ProgressSink,
    cancel: &CancellationToken,
) -> Result<Resolved> {
    let from_cache = |cached: warden_instance::CachedFile| Resolved {
        sha1: cached.hashes.sha1,
        sha512: cached.hashes.sha512,
        size: cached.size,
    };
    if let Some(cached) = services
        .cache
        .find(&item.expected)
        .map_err(download_err(item))?
    {
        return Ok(from_cache(cached));
    }
    let (url, origin, headers) = match &item.source {
        RefSource::Modrinth { url } | RefSource::Link { url } => {
            let parsed = Url::parse(url).map_err(download_err(item))?;
            (parsed, FileOrigin::url(url), warden_http::HeaderMap::new())
        }
        RefSource::Curseforge { project, file } => {
            let url = curseforge_cdn_url(
                &cdn_base(services.cdn_download_base.as_ref())?,
                *file,
                &item.filename,
            )?;
            let headers = match &services.curseforge {
                Some(client) => client.download_headers(&url).map_err(download_err(item))?,
                None => warden_http::HeaderMap::new(),
            };
            (
                url,
                FileOrigin::curseforge(u64::from(*project), u64::from(*file)),
                headers,
            )
        }
    };
    let mut request = DownloadRequest::new(url, PathBuf::new());
    request.expected_hashes.push(item.expected.clone());
    request.headers = headers;
    let fetched = services
        .cache
        .download(&services.http, request, &origin, progress, Some(cancel))
        .await
        .map_err(|error| {
            if cancel.is_cancelled() {
                cancelled()
            } else {
                download_err(item)(error)
            }
        })?;
    Ok(from_cache(fetched.file))
}

fn download_err<E: Display>(item: &RefItem) -> impl Fn(E) -> Error + '_ {
    move |error| {
        Error::new(
            ExportErrorCode::PrismDownloadFailed,
            format!("{}: {error}", item.meta_path),
        )
        .with_param("name", item.name.clone())
    }
}

/// Endereço público da CDN da CurseForge de um arquivo (o que vai gravado no `.mrpack`).
pub fn curseforge_file_url(file_id: u32, filename: &str) -> Result<Url> {
    curseforge_cdn_url(&cdn_base(None)?, file_id, filename)
}

fn cdn_base(base: Option<&Url>) -> Result<Url> {
    match base {
        Some(base) => Ok(base.clone()),
        None => Url::parse(CURSEFORGE_CDN).map_err(|e| Error::internal(e.to_string())),
    }
}

/// Uma linha de `files[]` do `modrinth.index.json`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    path: String,
    hashes: IndexHashes,
    env: IndexEnv,
    downloads: Vec<String>,
    file_size: u64,
}

#[derive(Serialize)]
struct IndexHashes {
    sha1: String,
    sha512: String,
}

#[derive(Serialize)]
struct IndexEnv {
    client: &'static str,
    server: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexJson<'a> {
    format_version: u8,
    game: &'static str,
    version_id: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    summary: &'a str,
    files: Vec<IndexFile>,
    dependencies: BTreeMap<&'static str, &'a str>,
}

/// `env` pelo lado do metafile: só valores da especificação, nunca `unknown`.
fn env_for(side: &Side, optional: bool) -> IndexEnv {
    let used = if optional { "optional" } else { "required" };
    let (client, server) = match side {
        Side::Client => (used, "unsupported"),
        Side::Server => ("unsupported", used),
        Side::Unset | Side::Both | Side::Other(_) => (used, used),
    };
    IndexEnv { client, server }
}

/// Uma linha de `files[]` por mod, resource pack ou shader de referência.
async fn build_entries(
    services: &PrismServices,
    plan: &PackPlan,
    swapped: &BTreeMap<usize, &SwapTarget>,
    progress: &dyn ProgressSink,
    cancel: &CancellationToken,
) -> Result<Vec<IndexFile>> {
    progress.stage(STAGE_RESOLVE, STAGE_RESOLVE_LABEL);
    let total = plan.refs.len() as u64;
    let mut files = Vec::with_capacity(plan.refs.len());
    for (position, item) in plan.refs.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        progress.progress(Progress::items(position as u64, Some(total)));
        let folder = item.dest.rsplit_once('/').map_or("", |(dir, _)| dir);
        let (path, download, resolved) = if let Some(target) = swapped.get(&position) {
            let path = if folder.is_empty() {
                target.filename.clone()
            } else {
                format!("{folder}/{}", target.filename)
            };
            (
                path,
                target.url.clone(),
                Resolved {
                    sha1: target.sha1.clone(),
                    sha512: target.sha512.clone(),
                    size: target.size,
                },
            )
        } else {
            let resolved = resolve(services, item, progress, cancel).await?;
            let download = match &item.source {
                RefSource::Modrinth { url } | RefSource::Link { url } => url.clone(),
                RefSource::Curseforge { file, .. } => {
                    curseforge_cdn_url(&cdn_base(None)?, *file, &item.filename)?.into()
                }
            };
            (item.dest.clone(), download, resolved)
        };
        files.push(IndexFile {
            path,
            hashes: IndexHashes {
                sha1: resolved.sha1,
                sha512: resolved.sha512,
            },
            env: env_for(&item.side, item.optional),
            downloads: vec![download],
            file_size: resolved.size,
        });
    }
    progress.progress(Progress::items(total, Some(total)));
    Ok(files)
}

/// Gera a instância pronta para o Prism em `destination` (arquivo novo, fora do pack), a partir
/// da cópia de staging do pack, do estado atual ou de uma versão salva.
///
/// Etapas: confere o destino; copia o pack e roda o `refresh` (como a exportação nativa); confere
/// a distribuição dos mods da CurseForge; resolve `sha1`, `sha512` e tamanho de cada arquivo pelo
/// cache de downloads (baixando o que faltar); monta o zip determinístico; valida o resultado e
/// só então grava o arquivo. Qualquer falha deixa o destino como estava.
#[allow(clippy::too_many_arguments)] // Mesmo desenho de `export`: cada parâmetro é um serviço.
pub async fn export_prism(
    root: &Path,
    source: &ExportSource,
    destination: &Path,
    staging_parent: &Path,
    packwiz: &Packwiz,
    services: &PrismServices,
    options: &PrismOptions,
    progress: &dyn ProgressSink,
    cancel: &CancellationToken,
) -> Result<PrismResult> {
    check_destination(root, destination, false)?;
    let staged = prepare_stage(root, source, staging_parent, packwiz, cancel).await?;
    debug_assert!(
        staged.dir.path().is_dir(),
        "a cópia de staging vive até o fim"
    );
    let plan = PackPlan::read(&staged.files, true)?;

    let locals = plan.third_party_locals();
    if !locals.is_empty() && !options.confirm_local_files {
        return Err(Error::new(
            ExportErrorCode::PrismLocalNeedsConfirmation,
            "há arquivos de terceiros sem confirmação de licença",
        )
        .with_param("count", locals.len().to_string()));
    }

    let blocked = find_blocked(services, &plan, cancel).await?;
    let mut unresolved = Vec::new();
    let mut swapped: BTreeMap<usize, &SwapTarget> = BTreeMap::new();
    for entry in &blocked {
        match (&entry.target, options.swaps.contains(&entry.view.path)) {
            (Some(target), true) => {
                swapped.insert(entry.index, target);
            }
            _ => unresolved.push(entry.view.name.clone()),
        }
    }
    if !unresolved.is_empty() {
        return Err(Error::new(
            ExportErrorCode::PrismBlocked,
            format!("mods bloqueados para terceiros: {}", unresolved.join(", ")),
        )
        .with_param("count", unresolved.len().to_string())
        .with_param("mods", unresolved.join(", ")));
    }

    let mut files = build_entries(services, &plan, &swapped, progress, cancel).await?;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let loader = plan
        .game
        .loader
        .as_ref()
        .and_then(|l| mrpack_loader_key(&l.loader).map(|key| (key, l.version.as_str())));
    let mut dependencies = BTreeMap::from([("minecraft", plan.game.minecraft.as_str())]);
    if let Some((key, version)) = loader {
        dependencies.insert(key, version);
    }
    let index = IndexJson {
        format_version: 1,
        game: "minecraft",
        version_id: plan.manifest.version.trim(),
        name: plan.manifest.name.trim(),
        summary: plan.manifest.description.trim(),
        files,
        dependencies,
    };
    let mut json = serde_json::to_vec_pretty(&index).map_err(|e| Error::internal(e.to_string()))?;
    json.push(b'\n');
    let bytes = build_zip(&json, &plan.overrides)?;

    let file_paths: BTreeSet<String> = index.files.iter().map(|f| f.path.clone()).collect();
    let override_paths: BTreeSet<String> = plan.overrides.keys().cloned().collect();
    let checked = validate::validate(
        &bytes,
        &validate::Expect {
            minecraft: &plan.game.minecraft,
            loader,
            override_paths: &override_paths,
            file_paths: &file_paths,
        },
    )?;
    if cancel.is_cancelled() {
        return Err(cancelled());
    }
    write_new(destination, &bytes)?;
    Ok(PrismResult {
        path: destination.to_path_buf(),
        bytes: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(&bytes)),
        files: checked.files,
        overrides: checked.overrides,
        untrusted: checked.untrusted,
    })
}

fn build_zip(index: &[u8], overrides: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let zip_err = |e: zip::result::ZipError| Error::internal(e.to_string());
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    zip.start_file("modrinth.index.json", options)
        .map_err(zip_err)?;
    zip.write_all(index)
        .map_err(|e| Error::internal(e.to_string()))?;
    for (path, bytes) in overrides {
        zip.start_file(format!("overrides/{path}"), options)
            .map_err(zip_err)?;
        zip.write_all(bytes)
            .map_err(|e| Error::internal(e.to_string()))?;
    }
    Ok(zip.finish().map_err(zip_err)?.into_inner())
}

/// Grava só em um arquivo novo: se o destino apareceu no meio do caminho, não sobrescreve.
fn write_new(destination: &Path, bytes: &[u8]) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| Error::internal("destino sem pasta pai"))?;
    let mut temp = tempfile::Builder::new()
        .prefix(".warden-prism-")
        .tempfile_in(parent)
        .map_err(|e| Error::internal(e.to_string()))?;
    temp.write_all(bytes)
        .map_err(|e| Error::internal(e.to_string()))?;
    temp.as_file()
        .sync_all()
        .map_err(|e| Error::internal(e.to_string()))?;
    temp.persist_noclobber(destination).map_err(|e| {
        Error::new(
            ExportErrorCode::DestinationNotEmpty,
            format!("o destino já existe: {}", e.error),
        )
    })?;
    Ok(())
}
