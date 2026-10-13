//! Parte comum a todos os formatos: depois que o módulo do formato separou referências e
//! arquivos, aqui entram a higiene obrigatória (verificação da ARCHITECTURE §6.4 mais a lista
//! de ignorados da captura, §8.4), a identificação pelo hash dos jars soltos em `mods/`, os
//! nomes únicos dos `.pw.toml` (§6.2) e o resumo para a tela.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;

use warden_core::CancellationToken;
use warden_instance::CaptureIgnore;
use warden_packwiz::hygiene::{FsEntry, FsKind, HygieneReason, PatternCategory, scan_entries};
use warden_packwiz::{Metafile, MetafileSource, PackManifest, Side, unique_metafile_stem};

use crate::archive::{EntryKind, Source};
use crate::error::Result;
use crate::lookup::{JarFacts, Stores, identify};
use crate::model::{
    Content, ImportExcludedItem, ImportExcludedReason, ImportFormat, ImportItemSource,
    ImportLicense, ImportLocalFile, ImportPlan, ImportProfile, ImportReference, ImportReviewItem,
    ImportReviewReason, ImportSummary, LocalJar,
};

/// Um item que vira `.pw.toml`.
#[derive(Debug, Clone)]
pub(crate) struct RefDraft {
    /// Pasta do metafile (`mods`, `resourcepacks`…).
    pub(crate) dir: String,
    /// Base do nome do arquivo (slug do projeto).
    pub(crate) slug: String,
    pub(crate) metafile: Metafile,
    pub(crate) source: ImportItemSource,
    pub(crate) by_hash: bool,
    pub(crate) review: Option<ImportReviewReason>,
    /// Nome base que o modpack já trazia (instância do Prism), mantido se estiver livre.
    pub(crate) stem: Option<String>,
}

/// Um arquivo candidato a entrar no pack (overrides, arquivos da instância).
#[derive(Debug, Clone)]
pub(crate) struct RawFile {
    /// Caminho no pack novo.
    pub(crate) path: String,
    pub(crate) content: Content,
    pub(crate) size: u64,
    pub(crate) kind: EntryKind,
    pub(crate) review: Option<ImportReviewReason>,
}

/// O que o módulo do formato entrega.
#[derive(Debug)]
pub(crate) struct Draft {
    pub(crate) kind: ImportFormat,
    pub(crate) origin: PathBuf,
    pub(crate) root: String,
    pub(crate) manifest: PackManifest,
    pub(crate) license: Option<ImportLicense>,
    pub(crate) references: Vec<RefDraft>,
    pub(crate) raw: Vec<RawFile>,
    pub(crate) excluded: Vec<ImportExcludedItem>,
    pub(crate) preferences: BTreeMap<String, Content>,
    pub(crate) profile: Option<ImportProfile>,
    pub(crate) staging: Option<PathBuf>,
    pub(crate) unavailable: Vec<String>,
}

impl Draft {
    pub(crate) fn new(
        kind: ImportFormat,
        origin: PathBuf,
        root: String,
        manifest: PackManifest,
    ) -> Self {
        Self {
            kind,
            origin,
            root,
            manifest,
            license: None,
            references: Vec::new(),
            raw: Vec::new(),
            excluded: Vec::new(),
            preferences: BTreeMap::new(),
            profile: None,
            staging: None,
            unavailable: Vec::new(),
        }
    }
}

fn excluded_reason(reason: &HygieneReason) -> Option<ImportExcludedReason> {
    match reason {
        HygieneReason::MatchesPattern { category, .. } => Some(match category {
            PatternCategory::Runtime => ImportExcludedReason::Runtime,
            PatternCategory::SecretsAndTools => ImportExcludedReason::Tools,
            PatternCategory::ServerOnly => ImportExcludedReason::ServerData,
            PatternCategory::AnyDepth | PatternCategory::Required => ImportExcludedReason::Junk,
        }),
        HygieneReason::Symlink => Some(ImportExcludedReason::Symlink),
        HygieneReason::UnknownRootItem => Some(ImportExcludedReason::UnknownRoot),
        HygieneReason::CacheLikeFolder { .. } | HygieneReason::LargeFile { .. } => None,
    }
}

fn covers(item: &str, is_dir: bool, path: &str) -> bool {
    path == item
        || (is_dir
            && path.len() > item.len()
            && path.starts_with(item)
            && path.as_bytes()[item.len()] == b'/')
}

/// Higiene: separa o que não entra. Devolve os itens excluídos e, para os que a pessoa pode
/// marcar, os arquivos que eles cobrem.
/// Arquivos cobertos por cada item que a pessoa pode marcar para entrar.
type Includable = BTreeMap<String, Vec<(String, Content)>>;

/// O resultado da higiene: os itens que não entram, os que a pessoa pode marcar e as
/// referências (pelo destino do arquivo) que caíram em lixo.
struct Hygiene {
    excluded: Vec<ImportExcludedItem>,
    includable: Includable,
    dropped_refs: HashSet<usize>,
}

/// Higiene sobre os arquivos e sobre o destino das referências (um `.mrpack` real lista em
/// `files[]` um jar de `mods/.connector/temp/`). Referência só sai por lixo de verdade; item
/// desconhecido na raiz não a tira.
#[allow(clippy::too_many_lines)]
fn hygiene(raw: &mut Vec<RawFile>, refs: &[String]) -> Hygiene {
    let mut entries: Vec<FsEntry> = Vec::new();
    let mut dirs = BTreeSet::new();
    for path in refs {
        let mut parent = path.as_str();
        while let Some((up, _)) = parent.rsplit_once('/') {
            dirs.insert(up.to_owned());
            parent = up;
        }
        entries.push(FsEntry {
            path: path.clone(),
            kind: FsKind::File,
            size: 0,
        });
    }
    for file in raw.iter() {
        let mut parent = file.path.as_str();
        while let Some((up, _)) = parent.rsplit_once('/') {
            dirs.insert(up.to_owned());
            parent = up;
        }
        entries.push(FsEntry {
            path: file.path.clone(),
            kind: if file.kind == EntryKind::Symlink {
                FsKind::Symlink
            } else {
                FsKind::File
            },
            size: file.size,
        });
    }
    entries.extend(dirs.into_iter().map(|path| FsEntry {
        path,
        kind: FsKind::Dir,
        size: 0,
    }));
    let capture = CaptureIgnore::builtin();
    let mut items: Vec<ImportExcludedItem> = Vec::new();
    for finding in scan_entries(&entries, None) {
        let reasons: Vec<ImportExcludedReason> =
            finding.reasons.iter().filter_map(excluded_reason).collect();
        let Some(&first) = reasons
            .iter()
            .find(|reason| **reason != ImportExcludedReason::UnknownRoot)
            .or_else(|| reasons.first())
        else {
            continue;
        };
        let mut can_include = reasons
            .iter()
            .all(|reason| *reason == ImportExcludedReason::UnknownRoot);
        let mut reason = first;
        // Item desconhecido que a captura sempre ignora (§8.4) é dado de execução.
        if can_include
            && (if finding.is_dir {
                capture.skips_dir(&finding.path)
            } else {
                capture.ignores(&finding.path)
            })
        {
            can_include = false;
            reason = ImportExcludedReason::Runtime;
        }
        items.push(ImportExcludedItem {
            path: finding.path,
            is_dir: finding.is_dir,
            files: 0,
            reason,
            can_include,
        });
    }
    // Lista de ignorados da captura (§8.4): o que a verificação do pack não cobre.
    for file in raw.iter() {
        if items
            .iter()
            .any(|item| covers(&item.path, item.is_dir, &file.path))
        {
            continue;
        }
        let mut cut = None;
        let mut end = 0;
        while let Some(offset) = file.path[end..].find('/') {
            let dir = &file.path[..end + offset];
            if capture.skips_dir(dir) {
                cut = Some((dir.to_owned(), true));
                break;
            }
            end += offset + 1;
        }
        if cut.is_none() && capture.ignores(&file.path) {
            cut = Some((file.path.clone(), false));
        }
        if let Some((path, is_dir)) = cut
            && !items.iter().any(|item| item.path == path)
        {
            items.push(ImportExcludedItem {
                path,
                is_dir,
                files: 0,
                reason: ImportExcludedReason::Runtime,
                can_include: false,
            });
        }
    }
    let mut includable: BTreeMap<String, Vec<(String, Content)>> = BTreeMap::new();
    let mut kept = Vec::with_capacity(raw.len());
    for file in raw.drain(..) {
        match items
            .iter_mut()
            .find(|item| covers(&item.path, item.is_dir, &file.path))
        {
            Some(item) => {
                item.files += 1;
                if item.can_include && file.kind == EntryKind::File {
                    includable
                        .entry(item.path.clone())
                        .or_default()
                        .push((file.path, file.content));
                }
            }
            None => kept.push(file),
        }
    }
    *raw = kept;
    let mut dropped_refs = HashSet::new();
    for (position, path) in refs.iter().enumerate() {
        if let Some(item) = items
            .iter_mut()
            .find(|item| !item.can_include && covers(&item.path, item.is_dir, path))
        {
            item.files += 1;
            dropped_refs.insert(position);
        }
    }
    items.retain(|item| item.files > 0);
    items.sort_by(|a, b| a.path.cmp(&b.path));
    Hygiene {
        excluded: items,
        includable,
        dropped_refs,
    }
}

fn is_loose_jar(path: &str) -> bool {
    path.strip_prefix("mods/").is_some_and(|name| {
        !name.contains('/')
            && std::path::Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
    })
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// Monta o plano: higiene, jars soltos pelo hash, nomes dos metafiles e o resumo.
///
/// Erros: os da leitura dos jars soltos da origem.
#[allow(clippy::too_many_lines)]
pub(crate) async fn finish(
    mut draft: Draft,
    source: &mut Source,
    stores: Stores<'_>,
    cancel: &CancellationToken,
) -> Result<ImportPlan> {
    // Arquivos que repetem o destino de uma referência: a referência vale.
    let reference_files: HashSet<String> = draft
        .references
        .iter()
        .map(|item| crate::common::join(&item.dir, &item.metafile.filename))
        .collect();
    draft
        .raw
        .retain(|file| !reference_files.contains(&file.path));
    // Só um arquivo por caminho (o último vence, como no launcher: client-overrides depois).
    let mut by_path: BTreeMap<String, RawFile> = BTreeMap::new();
    for file in draft.raw.drain(..) {
        by_path.insert(file.path.clone(), file);
    }
    draft.raw = by_path.into_values().collect();

    // Arquivos de controle e pastas do git/Warden vindos do modpack nunca entram.
    let mut reserved_items: BTreeMap<String, (bool, u32)> = BTreeMap::new();
    draft.raw.retain(|file| {
        if reserved(&file.path) {
            let top = file.path.split('/').next().unwrap_or_default().to_owned();
            reserved_items
                .entry(top)
                .or_insert((file.path.contains('/'), 0))
                .1 += 1;
            false
        } else {
            true
        }
    });
    let ref_paths: Vec<String> = draft
        .references
        .iter()
        .map(|item| crate::common::join(&item.dir, &item.metafile.filename))
        .collect();
    let Hygiene {
        mut excluded,
        includable,
        dropped_refs,
    } = hygiene(&mut draft.raw, &ref_paths);
    draft.references = draft
        .references
        .into_iter()
        .enumerate()
        .filter(|(position, _)| !dropped_refs.contains(position))
        .map(|(_, item)| item)
        .collect();
    excluded.extend(
        reserved_items
            .into_iter()
            .map(|(path, (is_dir, files))| ImportExcludedItem {
                path,
                is_dir,
                files,
                reason: ImportExcludedReason::OtherLauncher,
                can_include: false,
            }),
    );
    excluded.append(&mut draft.excluded);

    // Jars soltos em mods/: pelo hash no Modrinth e na CurseForge.
    let mut jar_positions = Vec::new();
    let mut jar_facts = Vec::new();
    for (position, file) in draft.raw.iter().enumerate() {
        if file.kind == EntryKind::File && is_loose_jar(&file.path) {
            if cancel.is_cancelled() {
                return Err(crate::error::Error::new(
                    crate::error::ImportErrorCode::Internal,
                    "cancelado",
                ));
            }
            let bytes = read_content(source, &file.content)?;
            jar_positions.push(position);
            jar_facts.push(JarFacts::of(&bytes));
        }
    }
    let identified = identify(stores, &jar_facts, cancel).await;
    draft.unavailable.extend(identified.unavailable);
    let mut local_files = Vec::new();
    let mut local_jars = Vec::new();
    let mut review_raw = Vec::new();
    let mut removed = HashSet::new();
    for ((position, facts), found) in jar_positions
        .into_iter()
        .zip(&jar_facts)
        .zip(identified.found)
    {
        let file = &draft.raw[position];
        match found {
            Ok(found) => {
                let mut metafile = found.metafile;
                let mut review = None;
                if file.review == Some(ImportReviewReason::ClientOverride) {
                    metafile.side = Side::Client;
                } else if file.review.is_some() {
                    review = file.review;
                }
                draft.references.push(RefDraft {
                    dir: "mods".to_owned(),
                    slug: found.slug,
                    metafile,
                    source: found.source,
                    by_hash: true,
                    review,
                    stem: None,
                });
                removed.insert(position);
            }
            Err(reason) => {
                local_files.push(ImportLocalFile {
                    path: file.path.clone(),
                    name: file_name(&file.path),
                    reason,
                });
                local_jars.push(LocalJar {
                    path: file.path.clone(),
                    sha256: facts.sha256.clone(),
                });
            }
        }
    }
    let mut raw: Vec<RawFile> = draft
        .raw
        .into_iter()
        .enumerate()
        .filter(|(position, _)| !removed.contains(position))
        .map(|(_, file)| file)
        .collect();
    for file in &raw {
        if let Some(reason) = file.review {
            review_raw.push(ImportReviewItem {
                path: file.path.clone(),
                name: file_name(&file.path),
                reason,
            });
        }
    }

    // Nomes únicos dos metafiles no pack inteiro.
    let mut taken: Vec<String> = raw
        .iter()
        .filter_map(|file| warden_packwiz::metafile_stem(&file.path).map(str::to_owned))
        .collect();
    let mut files: BTreeMap<String, Content> = BTreeMap::new();
    let mut references = Vec::new();
    let mut review = Vec::new();
    for item in &draft.references {
        let source = match item.source {
            ImportItemSource::Modrinth => MetafileSource::Modrinth,
            ImportItemSource::Curseforge => MetafileSource::CurseForge,
            ImportItemSource::Url => MetafileSource::Url,
        };
        let slug = {
            let slug = warden_packwiz::slugify_name(&item.slug);
            if slug.is_empty() {
                warden_packwiz::slugify_name(item.metafile.filename.trim_end_matches(".jar"))
            } else {
                slug
            }
        };
        let slug = if slug.is_empty() {
            "item".to_owned()
        } else {
            slug
        };
        let stem = match &item.stem {
            Some(stem) if !taken.iter().any(|t| t.eq_ignore_ascii_case(stem)) => stem.clone(),
            _ => unique_metafile_stem(&slug, source, taken.iter().map(String::as_str)),
        };
        taken.push(stem.clone());
        let path = crate::common::join(&item.dir, &format!("{stem}.pw.toml"));
        files.insert(
            path.clone(),
            Content::Bytes(item.metafile.to_toml_string().into_bytes()),
        );
        if let Some(reason) = item.review {
            review.push(ImportReviewItem {
                path: path.clone(),
                name: item.metafile.name.clone(),
                reason,
            });
        }
        references.push(ImportReference {
            path,
            name: item.metafile.name.clone(),
            source: item.source,
            by_hash: item.by_hash,
        });
    }
    review.extend(review_raw);
    let mut file_count = 0u32;
    let mut server_files = 0u32;
    for file in raw.drain(..) {
        if file.path.starts_with("server-overrides/") {
            server_files += 1;
        } else if !local_jars.iter().any(|jar| jar.path == file.path) {
            file_count += 1;
        }
        files.insert(file.path, file.content);
    }

    let manifest = draft.manifest;
    let loaders = manifest.loaders();
    let (loader, loader_version) = loaders.first().map_or((None, None), |(loader, version)| {
        (Some(loader.key().to_owned()), Some((*version).to_owned()))
    });
    let origin_stamp = std::fs::metadata(&draft.origin)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|metadata| (metadata.len(), metadata.modified().ok()));
    let mut unavailable = draft.unavailable;
    unavailable.sort();
    unavailable.dedup();
    Ok(ImportPlan {
        summary: ImportSummary {
            kind: draft.kind,
            file_name: crate::archive::display_name(&draft.origin),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            minecraft: manifest.minecraft_version().unwrap_or_default().to_owned(),
            loader,
            loader_version,
            license: draft.license,
            references,
            local_files,
            review,
            excluded,
            files: file_count,
            server_files,
            preferences: draft.preferences.keys().cloned().collect(),
            profile: draft.profile,
            unavailable_sources: unavailable,
        },
        origin: draft.origin,
        origin_stamp,
        root: draft.root,
        manifest,
        files,
        includable,
        preferences: draft.preferences,
        staging: draft.staging,
        local_jars,
    })
}

/// Lê os bytes de um conteúdo que não depende da origem (`None` para [`Content::Entry`]).
pub(crate) fn read_detached(content: &Content) -> Option<Result<Vec<u8>>> {
    match content {
        Content::Bytes(bytes) => Some(Ok(bytes.clone())),
        Content::Entry(_) => None,
        Content::Staged(path) => Some(std::fs::read(path).map_err(|error| {
            crate::error::Error::internal(format!("cópia de staging ilegível: {}", error.kind()))
        })),
    }
}

/// Lê os bytes de um conteúdo do plano.
pub(crate) fn read_content(source: &mut Source, content: &Content) -> Result<Vec<u8>> {
    match (read_detached(content), content) {
        (Some(bytes), _) => bytes,
        (None, Content::Entry(name)) => source.read(name),
        (None, _) => Err(crate::error::Error::internal("conteúdo sem origem")),
    }
}

/// Caminhos que nenhum modpack pode gravar: os arquivos de controle e as pastas do Warden e do
/// git (um `overrides/.git/hooks/…` executaria código no próximo commit).
pub(crate) fn reserved(path: &str) -> bool {
    let first = path
        .split('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        first.as_str(),
        ".git"
            | ".warden"
            | "pack.toml"
            | "index.toml"
            | ".packwizignore"
            | ".gitignore"
            | ".gitattributes"
            | "changelog.md"
    ) || path
        .split('/')
        .any(|segment| segment.eq_ignore_ascii_case(".git"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn raw(path: &str) -> RawFile {
        RawFile {
            path: path.to_owned(),
            content: Content::Bytes(Vec::new()),
            size: 1,
            kind: EntryKind::File,
            review: None,
        }
    }

    #[test]
    fn higiene_separa_lixo_e_desconhecidos() {
        let mut files = vec![
            raw("config/create-common.toml"),
            raw("config/create-client-1.toml.bak"),
            raw(".mixin.out/class/A.class"),
            raw(".mixin.out/class/B.class"),
            raw("mods/.connector/temp/x.jar"),
            raw("cache/c2me-dfc/a"),
            raw("logs/latest.log"),
            raw("local/kubejs/x.js"),
            raw("hotbar.nbt"),
            raw("emi.json"),
            raw("server-overrides/server.properties"),
            raw("options.txt"),
        ];
        let Hygiene {
            excluded,
            includable,
            dropped_refs,
        } = hygiene(
            &mut files,
            &[
                "mods/.connector/temp/x$y.jar".to_owned(),
                "mods/create.jar".to_owned(),
                "cache/z.jar".to_owned(),
            ],
        );
        // Referência em lixo sai; em pasta desconhecida, fica.
        assert_eq!(dropped_refs, HashSet::from([0]));
        let kept: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            kept,
            vec![
                "config/create-common.toml",
                "server-overrides/server.properties",
                "options.txt"
            ]
        );
        let by_path: BTreeMap<&str, &ImportExcludedItem> = excluded
            .iter()
            .map(|item| (item.path.as_str(), item))
            .collect();
        assert_eq!(by_path[".mixin.out"].files, 2);
        assert_eq!(by_path[".mixin.out"].reason, ImportExcludedReason::Runtime);
        assert!(!by_path[".mixin.out"].can_include);
        assert_eq!(
            by_path["config/create-client-1.toml.bak"].reason,
            ImportExcludedReason::Junk
        );
        assert_eq!(
            by_path["mods/.connector"].reason,
            ImportExcludedReason::Runtime
        );
        assert_eq!(by_path["mods/.connector"].files, 2);
        assert_eq!(by_path["cache"].reason, ImportExcludedReason::UnknownRoot);
        assert!(by_path["cache"].can_include);
        assert!(by_path["emi.json"].can_include);
        assert_eq!(by_path["logs"].reason, ImportExcludedReason::Runtime);
        assert!(!by_path["local"].can_include);
        assert!(!by_path["hotbar.nbt"].can_include);
        assert_eq!(includable["cache"].len(), 1);
        assert_eq!(includable["emi.json"].len(), 1);
        assert!(!includable.contains_key("logs"));
    }

    #[test]
    fn caminhos_reservados() {
        assert!(reserved("pack.toml"));
        assert!(reserved(".git/hooks/pre-commit"));
        assert!(reserved("config/.git/config"));
        assert!(reserved(".warden/project.toml"));
        assert!(reserved("CHANGELOG.md"));
        assert!(!reserved("config/pack.toml"));
        assert!(!reserved("mods/a.jar"));
    }

    #[test]
    fn jar_solto_so_direto_em_mods() {
        assert!(is_loose_jar("mods/a.jar"));
        assert!(is_loose_jar("mods/A.JAR"));
        assert!(!is_loose_jar("mods/sub/a.jar"));
        assert!(!is_loose_jar("config/a.jar"));
        assert!(!is_loose_jar("mods/a.zip"));
    }
}
