//! Leitura do pack para um formato de outro launcher: o que vai por referência, o que iria
//! embutido, o que se perde e o que impede (ARCHITECTURE §12).
//!
//! A classificação reproduz as regras do `packwiz modrinth export` e `curseforge export`
//! (R3 §2.3): o `.mrpack` leva por referência os links de `cdn.modrinth.com`, `github.com`,
//! `raw.githubusercontent.com` e `gitlab.com`, e embute todo o resto (inclusive a CurseForge);
//! o zip da CurseForge leva por referência o que tem `[update.curseforge]` e embute todo o
//! resto.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use warden_core::CancellationToken;
use warden_curseforge::{CurseforgeClient, Distribution, FileRef, HashAlgo};
use warden_modrinth::{HashAlgorithm, ModrinthClient};
use warden_packwiz::{Metafile, PACK_FILE, PackIndex, PackManifest, Side};

use super::model::{
    FormatAnalysis, FormatItem, FormatLoss, ItemOrigin, ItemOutcome, LauncherFormat, LossKind,
};
use crate::native::err;
use crate::{Error, ExportErrorCode, Result};

/// Sites cujo link o `packwiz modrinth export` mantém como referência.
const MRPACK_HOSTS: [&str; 4] = [
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

/// Quem responde às conferências de Modrinth e CurseForge.
#[derive(Clone, Copy)]
pub struct Lookups<'a> {
    /// Cliente do Modrinth (troca de fonte por hash).
    pub modrinth: &'a ModrinthClient,
    /// Cliente da CurseForge; `None` sem a chave do usuário.
    pub curseforge: Option<&'a CurseforgeClient>,
}

/// Um mod (metafile) do pack.
#[derive(Debug, Clone)]
pub(crate) struct PackMod {
    /// Caminho do `.pw.toml`.
    pub(crate) path: String,
    /// Onde o jar ficaria no pack (pasta do metafile + `filename`).
    pub(crate) dest: String,
    pub(crate) meta: Metafile,
    pub(crate) origin: ItemOrigin,
    pub(crate) host: Option<String>,
}

/// O pack lido dos arquivos da cópia de staging.
#[derive(Debug)]
pub(crate) struct Contents {
    pub(crate) manifest: PackManifest,
    pub(crate) mods: Vec<PackMod>,
    /// Arquivos do índice que não são metafiles (configs, jars locais…).
    pub(crate) plain: Vec<String>,
    pub(crate) preserve: usize,
}

fn host_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    Some(match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

fn is_jar(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".jar")
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub(crate) fn read_contents(files: &BTreeMap<String, Vec<u8>>) -> Result<Contents> {
    let text = |path: &str| -> Result<&str> {
        let bytes = files
            .get(path)
            .ok_or_else(|| Error::internal(format!("falta {path} na cópia do pack")))?;
        std::str::from_utf8(bytes).map_err(err)
    };
    let manifest = PackManifest::parse(text(PACK_FILE)?).map_err(err)?.value;
    let index = PackIndex::parse(text("index.toml")?).map_err(err)?.value;
    let mut mods = Vec::new();
    let mut plain = Vec::new();
    let mut preserve = 0;
    for entry in index.normalized_entries() {
        preserve += usize::from(entry.preserve);
        if !entry.metafile {
            plain.push(entry.file);
            continue;
        }
        let meta = Metafile::parse(text(&entry.file)?).map_err(err)?.value;
        let dir = entry.file.rsplit_once('/').map_or("", |(dir, _)| dir);
        let dest = if dir.is_empty() {
            meta.filename.clone()
        } else {
            format!("{dir}/{}", meta.filename)
        };
        let host = host_of(&meta.download.url);
        let origin = if meta.curseforge_ids().is_some() {
            ItemOrigin::Curseforge
        } else if meta.modrinth_ids().is_some() {
            ItemOrigin::Modrinth
        } else {
            ItemOrigin::Link
        };
        mods.push(PackMod {
            path: entry.file,
            dest,
            meta,
            origin,
            host,
        });
    }
    Ok(Contents {
        manifest,
        mods,
        plain,
        preserve,
    })
}

/// O `.mrpack` leva o mod por referência (o packwiz grava o link em `files[]`).
fn mrpack_direct(m: &PackMod) -> bool {
    let mode = m.meta.download.mode.as_str();
    (mode.is_empty() || mode == "url")
        && m
            .host
            .as_deref()
            .is_some_and(|host| MRPACK_HOSTS.contains(&host))
}

/// O mesmo arquivo no Modrinth, achado pelo hash.
#[derive(Debug, Clone)]
pub(crate) struct SwapTarget {
    pub(crate) title: String,
    pub(crate) project_id: String,
    pub(crate) version_id: String,
    pub(crate) filename: String,
    pub(crate) url: String,
    pub(crate) sha512: String,
}

/// Um arquivo que iria embutido, com o destino dentro de `overrides/`.
#[derive(Debug, Clone)]
pub(crate) struct Embed {
    pub(crate) path: String,
    pub(crate) dest: String,
}

/// O resultado da leitura, com o que a exportação precisa para agir.
#[derive(Debug)]
pub(crate) struct Plan {
    pub(crate) analysis: FormatAnalysis,
    pub(crate) contents: Contents,
    /// Trocas possíveis, por caminho do `.pw.toml`.
    pub(crate) swaps: BTreeMap<String, SwapTarget>,
    /// Mods da CurseForge com troca possível cujo arquivo a CurseForge libera para terceiros
    /// (ou não se sabe, sem a chave): se a pessoa não trocar, vão embutidos.
    pub(crate) swap_kept: BTreeMap<String, KeptOutcome>,
    /// Embutidos que já são certos (sem depender de troca).
    pub(crate) embeds: Vec<Embed>,
}

/// O que acontece com um mod trocável se a pessoa preferir não trocar.
#[derive(Debug, Clone)]
pub(crate) enum KeptOutcome {
    /// A CurseForge libera: o jar vai embutido.
    Embed,
    /// A CurseForge bloqueou ou removeu: não dá para embutir.
    Impossible,
    /// Sem a chave da CurseForge não dá para saber nem embutir.
    NeedsKey,
}

fn sha1_of(m: &PackMod) -> Option<String> {
    (m.meta.download.hash_format == "sha1" && !m.meta.download.hash.is_empty())
        .then(|| m.meta.download.hash.to_ascii_lowercase())
}

fn lookup_failed(source: &str, error: &(impl std::error::Error + 'static)) -> Error {
    Error::new(
        ExportErrorCode::LookupFailed,
        format!("não foi possível consultar {source}"),
    )
    .with_param("source", source)
    .with_detail(warden_core::error_chain(error))
}

struct CurseforgeVerdict {
    swap: Option<SwapTarget>,
    distribution: Option<Distribution>,
}

/// Para os mods da CurseForge que o `.mrpack` embutiria: acha o mesmo arquivo no Modrinth (por
/// hash) e, havendo a chave, consulta a situação de distribuição na CurseForge.
async fn resolve_curseforge(
    candidates: &[&PackMod],
    lookups: &Lookups<'_>,
    cancel: &CancellationToken,
) -> Result<Vec<CurseforgeVerdict>> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let ids = |m: &PackMod| -> Result<(u32, u32)> {
        m.meta.curseforge_ids().ok_or_else(|| {
            Error::internal(format!("{} não tem [update.curseforge]", m.path))
        })
    };
    let mut hashes: Vec<Option<String>> = candidates.iter().map(|m| sha1_of(m)).collect();
    // O hash do metafile pode não ser sha1 (md5, murmur2): a CurseForge devolve o sha1.
    if let Some(curseforge) = lookups.curseforge
        && hashes.iter().any(Option::is_none)
    {
        let mut wanted = Vec::new();
        for (m, hash) in candidates.iter().zip(&hashes) {
            if hash.is_none() {
                wanted.push(u64::from(ids(m)?.1));
            }
        }
        let files = curseforge
            .files_by_id(&wanted, Some(cancel))
            .await
            .map_err(|error| lookup_failed("a CurseForge", &error))?;
        for (m, hash) in candidates.iter().zip(&mut hashes) {
            if hash.is_none() {
                let id = u64::from(ids(m)?.1);
                *hash = files
                    .iter()
                    .find(|file| file.id == id)
                    .and_then(|file| file.hashes.iter().find(|h| h.algo == HashAlgo::Sha1))
                    .map(|h| h.value.to_ascii_lowercase());
            }
        }
    }
    let known: Vec<String> = hashes.iter().flatten().cloned().collect();
    let versions = if known.is_empty() {
        HashMap::new()
    } else {
        lookups
            .modrinth
            .version_files(&known, HashAlgorithm::Sha1, Some(cancel))
            .await
            .map_err(|error| lookup_failed("o Modrinth", &error))?
    };
    let distributions = if let Some(curseforge) = lookups.curseforge {
        let mut refs = Vec::new();
        for m in candidates {
            let (project, file) = ids(m)?;
            refs.push(FileRef::new(u64::from(project), u64::from(file)));
        }
        let found = curseforge
            .distribution(&refs, Some(cancel))
            .await
            .map_err(|error| lookup_failed("a CurseForge", &error))?;
        found.into_iter().map(|item| Some(item.distribution)).collect()
    } else {
        vec![None; candidates.len()]
    };
    let mut verdicts = Vec::new();
    for ((m, hash), distribution) in candidates.iter().zip(&hashes).zip(distributions) {
        let swap = hash.as_ref().and_then(|hash| {
            let version = versions.get(hash)?;
            let file = version
                .files
                .iter()
                .find(|file| file.hashes.sha1.eq_ignore_ascii_case(hash))?;
            if file.url.is_empty() || file.hashes.sha512.is_empty() {
                return None;
            }
            Some(SwapTarget {
                title: m.meta.name.clone(),
                project_id: version.project_id.clone(),
                version_id: version.id.clone(),
                filename: file.filename.clone(),
                url: file.url.clone(),
                sha512: file.hashes.sha512.to_ascii_lowercase(),
            })
        });
        verdicts.push(CurseforgeVerdict { swap, distribution });
    }
    Ok(verdicts)
}

fn count(contents: &Contents, keep: impl Fn(&PackMod) -> bool) -> usize {
    contents.mods.iter().filter(|m| keep(m)).count()
}

fn losses(format: LauncherFormat, contents: &Contents) -> Vec<FormatLoss> {
    let item = |kind, count| FormatLoss { kind, count };
    let mut list = Vec::new();
    let mut counted = |kind, count: usize| {
        if count > 0 {
            list.push(item(kind, count));
        }
    };
    let optional = count(contents, |m| m.meta.option.as_ref().is_some_and(|o| o.optional));
    match format {
        LauncherFormat::Mrpack => {
            counted(LossKind::OptionalTexts, optional);
            counted(LossKind::Preserve, contents.preserve);
            counted(LossKind::Pinned, count(contents, |m| m.meta.pin));
            counted(
                LossKind::UpdateSources,
                count(contents, |m| m.meta.update.is_some()),
            );
        }
        LauncherFormat::Curseforge => {
            counted(
                LossKind::SideNotKept,
                count(contents, |m| m.meta.side == Side::Client),
            );
            counted(
                LossKind::ServerOnlyLeft,
                count(contents, |m| !m.meta.side.on_client()),
            );
            counted(LossKind::OptionalTexts, optional);
            counted(LossKind::Preserve, contents.preserve);
            counted(LossKind::Pinned, count(contents, |m| m.meta.pin));
            counted(
                LossKind::UpdateSources,
                count(contents, |m| m.meta.modrinth_ids().is_some()),
            );
        }
    }
    list.push(item(LossKind::ConfigsEverywhere, 0));
    list.push(item(LossKind::NoAutoUpdate, 0));
    list
}

fn push_embed(m: &PackMod, embeds: &mut Vec<Embed>, items: &mut Vec<FormatItem>) {
    embeds.push(Embed {
        path: m.path.clone(),
        dest: m.dest.clone(),
    });
    items.push(FormatItem {
        path: m.path.clone(),
        name: m.meta.name.clone(),
        filename: m.meta.filename.clone(),
        origin: m.origin,
        host: m.host.clone(),
        outcome: ItemOutcome::Embed,
    });
}

/// Lê os arquivos da cópia limpa e monta o plano do formato.
pub(crate) async fn plan(
    files: &BTreeMap<String, Vec<u8>>,
    format: LauncherFormat,
    lookups: &Lookups<'_>,
    cancel: &CancellationToken,
) -> Result<Plan> {
    let contents = read_contents(files)?;
    let mut references = 0;
    let mut items = Vec::new();
    let mut embeds = Vec::new();
    let mut swaps = BTreeMap::new();
    let mut swap_kept = BTreeMap::new();
    let mut candidates = Vec::new();
    for m in &contents.mods {
        match format {
            LauncherFormat::Mrpack if mrpack_direct(m) => references += 1,
            LauncherFormat::Mrpack if m.meta.curseforge_ids().is_some() => candidates.push(m),
            LauncherFormat::Mrpack => push_embed(m, &mut embeds, &mut items),
            LauncherFormat::Curseforge if !m.meta.side.on_client() => {}
            LauncherFormat::Curseforge if m.meta.curseforge_ids().is_some() => references += 1,
            LauncherFormat::Curseforge => push_embed(m, &mut embeds, &mut items),
        }
    }
    let verdicts = resolve_curseforge(&candidates, lookups, cancel).await?;
    for (m, verdict) in candidates.iter().zip(verdicts) {
        let item = |outcome| FormatItem {
            path: m.path.clone(),
            name: m.meta.name.clone(),
            filename: m.meta.filename.clone(),
            origin: ItemOrigin::Curseforge,
            host: None,
            outcome,
        };
        let kept = match &verdict.distribution {
            Some(Distribution::Allowed { .. }) => KeptOutcome::Embed,
            Some(_) => KeptOutcome::Impossible,
            None => KeptOutcome::NeedsKey,
        };
        if let Some(target) = verdict.swap {
            items.push(item(ItemOutcome::Swap {
                required: matches!(kept, KeptOutcome::Impossible),
                project_id: target.project_id.clone(),
                version_id: target.version_id.clone(),
                filename: target.filename.clone(),
            }));
            swaps.insert(m.path.clone(), target);
            swap_kept.insert(m.path.clone(), kept);
            continue;
        }
        match verdict.distribution {
            Some(Distribution::Allowed { .. }) => {
                embeds.push(Embed {
                    path: m.path.clone(),
                    dest: m.dest.clone(),
                });
                items.push(item(ItemOutcome::Embed));
            }
            Some(Distribution::Blocked { page_url }) => {
                items.push(item(ItemOutcome::Blocked { page_url }));
            }
            Some(Distribution::Missing) => items.push(item(ItemOutcome::Unavailable)),
            None => {
                return Err(Error::new(
                    ExportErrorCode::CurseforgeKeyMissing,
                    "conferir os mods da CurseForge exige a chave da CurseForge",
                ));
            }
        }
    }
    // Jars soltos na pasta do pack: arquivo de terceiros que iria dentro do arquivo gerado.
    for path in contents.plain.iter().filter(|path| is_jar(path)) {
        embeds.push(Embed {
            path: path.clone(),
            dest: path.clone(),
        });
        items.push(FormatItem {
            path: path.clone(),
            name: file_name(path).to_owned(),
            filename: file_name(path).to_owned(),
            origin: ItemOrigin::Local,
            host: None,
            outcome: ItemOutcome::Embed,
        });
    }
    items.sort_by(|a, b| a.path.cmp(&b.path));
    let version = contents.manifest.version.trim();
    let analysis = FormatAnalysis {
        format,
        name: contents.manifest.name.clone(),
        version: (!version.is_empty()).then(|| version.to_owned()),
        references,
        embedded: embeds.len(),
        losses: losses(format, &contents),
        items,
    };
    Ok(Plan {
        analysis,
        contents,
        swaps,
        swap_kept,
        embeds,
    })
}

/// Os caminhos permitidos em `overrides/` depois de aplicadas as escolhas: arquivos do índice
/// que não são metafiles mais o destino dos jars embutidos.
pub(crate) fn allowed_overrides(plan: &Plan, embeds: &[Embed]) -> BTreeSet<String> {
    plan.contents
        .plain
        .iter()
        .cloned()
        .chain(embeds.iter().map(|embed| embed.dest.clone()))
        .collect()
}
