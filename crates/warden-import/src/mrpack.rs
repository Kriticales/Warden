//! `.mrpack` → pack packwiz (ARCHITECTURE §12.2).
//!
//! - `files[].path` com `..`, caminho absoluto ou letra de drive recusa o arquivo inteiro
//!   (CA-T24-02), antes de qualquer requisição.
//! - Arquivo do `cdn.modrinth.com`: projeto e versão vêm do caminho da URL
//!   (`/data/<projeto>/versions/<versão>/<arquivo>`); URL do CDN fora desse formato:
//!   `POST /v2/version_files` pelo SHA-1, em lote. Arquivo da CDN da CurseForge
//!   (`edge.forgecdn.net/files/<a>/<b>/<arquivo>`, o que a exportação do Warden grava): com a
//!   chave, vira referência à CurseForge. Outros hosts: metafile em modo `url` com o hash.
//! - `env` → lado pela tabela da §6.2; valores fora da especificação (`unknown`) → Cliente e
//!   servidor, para revisar. Nenhum lado obrigatório e algum opcional → `[option]` opcional e
//!   marcado, para revisar.
//! - `overrides/` → arquivos do pack; `client-overrides/` → arquivos do pack, para revisar;
//!   `server-overrides/` → `server-overrides/` do pack. O resto do zip (como o `xmcl.json` de
//!   outro launcher) não entra.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use url::Url;
use warden_core::CancellationToken;
use warden_export::{MrpackEnv, MrpackFile, MrpackIndex};
use warden_modrinth::HashAlgorithm;
use warden_packwiz::{
    CurseForgeFile, Download, HashFormat, Loader, Metafile, ModOption, ModrinthFile, Side,
};

use crate::archive::{EntryKind, Source, unsafe_path, unsafe_reason};
use crate::build::{Draft, RawFile, RefDraft};
use crate::common::{manifest, split_path, stem};
use crate::error::{Error, ImportErrorCode as Code, Result};
use crate::lookup::{Stores, modpack_license};
use crate::model::{
    Content, ImportExcludedItem, ImportExcludedReason, ImportFormat, ImportItemSource,
    ImportReviewReason,
};

const INDEX: &str = "modrinth.index.json";
const OVERRIDES: [(&str, &str, Option<ImportReviewReason>); 3] = [
    ("overrides/", "", None),
    (
        "client-overrides/",
        "",
        Some(ImportReviewReason::ClientOverride),
    ),
    ("server-overrides/", "server-overrides/", None),
];

/// Lado, opcional e se o `env` estava fora da especificação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnvSide {
    pub(crate) side: Side,
    pub(crate) optional: bool,
    pub(crate) unknown: bool,
}

/// `env` do `.mrpack` → lado (ARCHITECTURE §6.2; o inverso do que o `packwiz modrinth export`
/// grava, para a ida e volta devolver o mesmo pack).
pub(crate) fn env_side(env: Option<&MrpackEnv>) -> EnvSide {
    let both = |unknown| EnvSide {
        side: Side::Both,
        optional: false,
        unknown,
    };
    let Some(env) = env else {
        return both(false);
    };
    let known = |value: &str| matches!(value, "required" | "optional" | "unsupported");
    if !known(&env.client) || !known(&env.server) {
        return both(true);
    }
    let client = env.client != "unsupported";
    let server = env.server != "unsupported";
    let side = match (client, server) {
        (true, false) => Side::Client,
        (false, true) => Side::Server,
        (true, true) => Side::Both,
        (false, false) => return both(true),
    };
    let optional = env.client != "required" && env.server != "required";
    EnvSide {
        side,
        optional,
        unknown: false,
    }
}

/// Projeto, versão e nome do arquivo de uma URL do CDN do Modrinth.
pub(crate) fn modrinth_cdn(url: &Url) -> Option<(String, String, String)> {
    if url.host_str() != Some("cdn.modrinth.com") {
        return None;
    }
    let segments: Vec<&str> = url.path_segments()?.collect();
    match segments.as_slice() {
        ["data", project, "versions", version, file]
            if !project.is_empty() && !version.is_empty() =>
        {
            let file = url::form_urlencoded::parse(format!("a={file}").as_bytes())
                .next()
                .map(|(_, value)| value.into_owned())?;
            Some(((*project).to_owned(), (*version).to_owned(), file))
        }
        _ => None,
    }
}

/// ID do arquivo de uma URL da CDN da CurseForge (`/files/<id ÷ 1000>/<id mod 1000>/<nome>`).
pub(crate) fn curseforge_cdn(url: &Url) -> Option<u64> {
    let host = url.host_str()?;
    if !matches!(host, "edge.forgecdn.net" | "mediafilez.forgecdn.net") {
        return None;
    }
    let segments: Vec<&str> = url.path_segments()?.collect();
    match segments.as_slice() {
        ["files", high, low, _] => {
            let high: u64 = high.parse().ok()?;
            let low: u64 = low.parse().ok()?;
            (low < 1000).then(|| high * 1000 + low)
        }
        _ => None,
    }
}

fn valid_hex(text: &str, length: usize) -> Option<String> {
    let text = text.trim().to_ascii_lowercase();
    (text.len() == length && text.bytes().all(|b| b.is_ascii_hexdigit())).then_some(text)
}

/// O melhor hash do arquivo: sha512, senão sha1.
fn best_hash(file: &MrpackFile) -> Option<(HashFormat, String)> {
    file.hashes
        .get("sha512")
        .and_then(|hash| valid_hex(hash, 128))
        .map(|hash| (HashFormat::Sha512, hash))
        .or_else(|| {
            file.hashes
                .get("sha1")
                .and_then(|hash| valid_hex(hash, 40))
                .map(|hash| (HashFormat::Sha1, hash))
        })
}

fn loaders_of(index: &MrpackIndex) -> Vec<(Loader, String)> {
    index
        .dependencies
        .iter()
        .filter_map(|(key, version)| {
            let loader = match key.as_str() {
                "forge" => Loader::Forge,
                "neoforge" => Loader::NeoForge,
                "fabric-loader" => Loader::Fabric,
                "quilt-loader" => Loader::Quilt,
                _ => return None,
            };
            Some((loader, version.clone()))
        })
        .collect()
}

/// Um arquivo de `files[]` em análise.
struct Pending {
    dir: String,
    filename: String,
    url: String,
    hash: (HashFormat, String),
    env: EnvSide,
}

fn url_metafile(item: &Pending) -> Metafile {
    Metafile {
        name: stem(&item.filename),
        filename: item.filename.clone(),
        side: item.env.side.clone(),
        pin: false,
        download: Download {
            url: item.url.clone(),
            hash_format: item.hash.0.as_str().to_owned(),
            hash: item.hash.1.clone(),
            mode: String::new(),
        },
        update: None,
        option: None,
    }
}

fn reference(
    item: &Pending,
    mut metafile: Metafile,
    slug: String,
    source: ImportItemSource,
) -> RefDraft {
    metafile.side = item.env.side.clone();
    let mut review = None;
    if item.env.optional {
        metafile.option = Some(ModOption {
            optional: true,
            description: String::new(),
            default: true,
        });
        review = Some(ImportReviewReason::Optional);
    }
    if item.env.unknown {
        review = Some(ImportReviewReason::UnknownSide);
    }
    RefDraft {
        dir: item.dir.clone(),
        slug,
        metafile,
        source,
        by_hash: false,
        review,
        stem: None,
    }
}

/// Lê o `.mrpack` e monta o rascunho da importação.
///
/// Erros: [`Code::UnsafePath`], [`Code::Unreadable`], [`Code::UnsupportedLoader`],
/// [`Code::MinecraftMissing`].
#[allow(clippy::too_many_lines)]
pub(crate) async fn analyze(
    source: &mut Source,
    origin: &Path,
    stores: Stores<'_>,
    cancel: &CancellationToken,
) -> Result<Draft> {
    let index: MrpackIndex = source.read_json(INDEX)?;
    // Segurança antes de tudo (CA-T24-02): nenhum caminho perigoso, nenhuma requisição.
    for file in &index.files {
        if file.path.trim().is_empty() {
            return Err(unsafe_path(&file.path, "empty"));
        }
        if let Some(reason) = unsafe_reason(&file.path) {
            return Err(unsafe_path(&file.path, reason));
        }
    }
    if !index.game.is_empty() && index.game != "minecraft" {
        return Err(Error::new(
            Code::UnknownFormat,
            format!("o .mrpack é de outro jogo: {}", index.game),
        )
        .with_param("name", source.file_name()));
    }
    let minecraft = index
        .dependencies
        .get("minecraft")
        .cloned()
        .unwrap_or_default();
    let mut pack = manifest(
        &index.name,
        &stem(source.file_name()),
        &minecraft,
        &loaders_of(&index),
    )?;
    pack.version = index.version_id.trim().to_owned();
    index
        .summary
        .as_deref()
        .unwrap_or_default()
        .trim()
        .clone_into(&mut pack.description);
    let mut draft = Draft::new(
        ImportFormat::Mrpack,
        origin.to_path_buf(),
        String::new(),
        pack,
    );

    // Referências.
    let mut modrinth: Vec<(Pending, String, String)> = Vec::new();
    let mut by_sha1: Vec<Pending> = Vec::new();
    let mut by_curseforge: Vec<(Pending, u64)> = Vec::new();
    for file in &index.files {
        let (dir, filename) = split_path(&file.path);
        let env = env_side(file.env.as_ref());
        let Some((link, url)) = file.downloads.iter().find_map(|link| {
            Url::parse(link)
                .ok()
                .filter(|url| url.scheme() == "https" || url.scheme() == "http")
                .map(|url| (link.clone(), url))
        }) else {
            return Err(Error::new(
                Code::Unreadable,
                format!("{} não tem link de download", file.path),
            )
            .with_param("name", source.file_name()));
        };
        let Some(hash) = best_hash(file) else {
            return Err(Error::new(
                Code::Unreadable,
                format!("{} não tem hash válido", file.path),
            )
            .with_param("name", source.file_name()));
        };
        let pending = Pending {
            dir,
            filename,
            url: link,
            hash,
            env,
        };
        if let Some((project, version, _)) = modrinth_cdn(&url) {
            modrinth.push((pending, project, version));
        } else if url.host_str() == Some("cdn.modrinth.com") {
            by_sha1.push(pending);
        } else if let Some(file_id) = curseforge_cdn(&url).filter(|_| stores.curseforge.is_some()) {
            by_curseforge.push((pending, file_id));
        } else {
            draft.references.push(reference(
                &pending,
                url_metafile(&pending),
                stem(&pending.filename),
                ImportItemSource::Url,
            ));
        }
    }

    // CDN do Modrinth fora do formato esperado: pelo SHA-1.
    if !by_sha1.is_empty() {
        let found = match stores.modrinth {
            Some(client) => {
                let hashes: Vec<String> = by_sha1
                    .iter()
                    .filter_map(|item| {
                        index
                            .files
                            .iter()
                            .find(|file| {
                                file.path == crate::common::join(&item.dir, &item.filename)
                            })
                            .and_then(|file| file.hashes.get("sha1"))
                            .and_then(|hash| valid_hex(hash, 40))
                    })
                    .collect();
                match client
                    .version_files(&hashes, HashAlgorithm::Sha1, Some(cancel))
                    .await
                {
                    Ok(map) => map,
                    Err(error) => {
                        tracing::warn!(%error, "version_files falhou na importação");
                        draft.unavailable.push("Modrinth".to_owned());
                        HashMap::new()
                    }
                }
            }
            None => HashMap::new(),
        };
        for item in by_sha1 {
            let sha1 = index
                .files
                .iter()
                .find(|file| file.path == crate::common::join(&item.dir, &item.filename))
                .and_then(|file| file.hashes.get("sha1"))
                .and_then(|hash| valid_hex(hash, 40));
            match sha1.and_then(|sha1| found.get(&sha1)) {
                Some(version) => {
                    let project = version.project_id.clone();
                    let id = version.id.clone();
                    modrinth.push((item, project, id));
                }
                None => draft.references.push(reference(
                    &item,
                    url_metafile(&item),
                    stem(&item.filename),
                    ImportItemSource::Url,
                )),
            }
        }
    }

    // Nomes dos projetos do Modrinth (um pedido em lote).
    let mut titles: BTreeMap<String, (String, String)> = BTreeMap::new();
    if let Some(client) = stores.modrinth
        && !modrinth.is_empty()
    {
        let mut ids: Vec<String> = modrinth
            .iter()
            .map(|(_, project, _)| project.clone())
            .collect();
        ids.sort();
        ids.dedup();
        match client.projects(&ids, Some(cancel)).await {
            Ok(projects) => {
                for project in projects {
                    titles.insert(project.id.clone(), (project.title, project.slug));
                }
            }
            Err(error) => {
                tracing::warn!(%error, "nomes dos projetos do Modrinth indisponíveis na importação");
                draft.unavailable.push("Modrinth".to_owned());
            }
        }
    }
    for (item, project, version) in modrinth {
        let (title, slug) = titles
            .get(&project)
            .cloned()
            .unwrap_or_else(|| (stem(&item.filename), stem(&item.filename)));
        let metafile = Metafile::modrinth(
            &ModrinthFile {
                title,
                project_id: project,
                version_id: version,
                filename: item.filename.clone(),
                url: item.url.clone(),
                hash_format: item.hash.0,
                hash: item.hash.1.clone(),
            },
            item.env.side.clone(),
        );
        draft
            .references
            .push(reference(&item, metafile, slug, ImportItemSource::Modrinth));
    }

    // CDN da CurseForge, com a chave: referência à CurseForge.
    if let Some(client) = stores.curseforge
        && !by_curseforge.is_empty()
    {
        let ids: Vec<u64> = by_curseforge.iter().map(|(_, id)| *id).collect();
        let found = async {
            let files = client.files_by_id(&ids, Some(cancel)).await?;
            let mut project_ids: Vec<u64> = files.iter().map(|file| file.mod_id).collect();
            project_ids.sort_unstable();
            project_ids.dedup();
            let projects = client.projects(&project_ids, Some(cancel)).await?;
            Ok::<_, warden_curseforge::Error>((files, projects))
        }
        .await;
        let (files, projects) = match found {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!(%error, "arquivos da CurseForge indisponíveis na importação");
                draft.unavailable.push("CurseForge".to_owned());
                (Vec::new(), Vec::new())
            }
        };
        for (item, file_id) in by_curseforge {
            let sha1 = index
                .files
                .iter()
                .find(|file| file.path == crate::common::join(&item.dir, &item.filename))
                .and_then(|file| file.hashes.get("sha1"))
                .and_then(|hash| valid_hex(hash, 40));
            let file = files.iter().find(|file| file.id == file_id);
            let ids = file.and_then(|file| {
                Some((
                    u32::try_from(file.mod_id).ok()?,
                    u32::try_from(file.id).ok()?,
                ))
            });
            match (file, ids, sha1) {
                (Some(file), Some((project_id, file_id)), Some(sha1)) => {
                    let project = projects.iter().find(|project| project.id == file.mod_id);
                    let name = project.map_or_else(|| stem(&item.filename), |p| p.name.clone());
                    let slug = project.map_or_else(|| name.clone(), |p| p.slug.clone());
                    let metafile = Metafile::curseforge(
                        &CurseForgeFile {
                            name,
                            project_id,
                            file_id,
                            filename: item.filename.clone(),
                            hash_format: HashFormat::Sha1,
                            hash: sha1,
                        },
                        item.env.side.clone(),
                    );
                    draft.references.push(reference(
                        &item,
                        metafile,
                        slug,
                        ImportItemSource::Curseforge,
                    ));
                }
                _ => draft.references.push(reference(
                    &item,
                    url_metafile(&item),
                    stem(&item.filename),
                    ImportItemSource::Url,
                )),
            }
        }
    }

    // Overrides.
    let mut outside: BTreeMap<String, (bool, u32)> = BTreeMap::new();
    for entry in source.entries()? {
        if entry.kind == EntryKind::Dir || entry.name == INDEX {
            continue;
        }
        let mapped = OVERRIDES.iter().find_map(|(prefix, target, review)| {
            entry
                .name
                .strip_prefix(prefix)
                .filter(|rest| !rest.is_empty())
                .map(|rest| (format!("{target}{rest}"), *review))
        });
        if let Some((path, review)) = mapped {
            draft.raw.push(RawFile {
                path,
                content: Content::Entry(entry.name.clone()),
                size: entry.size,
                kind: entry.kind,
                review,
            });
        } else {
            let (top, is_dir) = match entry.name.split_once('/') {
                Some((top, _)) => (top.to_owned(), true),
                None => (entry.name.clone(), false),
            };
            outside.entry(top).or_insert((is_dir, 0)).1 += 1;
        }
    }
    draft.excluded.extend(
        outside
            .into_iter()
            .map(|(path, (is_dir, files))| ImportExcludedItem {
                path,
                is_dir,
                files,
                reason: ImportExcludedReason::OtherLauncher,
                can_include: false,
            }),
    );

    // Licença do modpack, se ele está publicado no Modrinth (pelo sha512 do próprio arquivo).
    if let Ok(mut file) = std::fs::File::open(origin)
        && let Ok(sha512) = warden_packwiz::hash::hash_reader(HashFormat::Sha512, &mut file)
    {
        draft.license = modpack_license(stores.modrinth, &sha512, cancel).await;
    }
    Ok(draft)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn env(client: &str, server: &str) -> MrpackEnv {
        MrpackEnv {
            client: client.to_owned(),
            server: server.to_owned(),
        }
    }

    #[test]
    fn env_vira_lado_e_inverte_a_exportacao() {
        let side = |c, s| env_side(Some(&env(c, s)));
        assert_eq!(side("required", "required").side, Side::Both);
        assert_eq!(side("required", "unsupported").side, Side::Client);
        assert_eq!(side("unsupported", "required").side, Side::Server);
        let optional = side("optional", "optional");
        assert_eq!(optional.side, Side::Both);
        assert!(optional.optional);
        let client_optional = side("optional", "unsupported");
        assert_eq!(client_optional.side, Side::Client);
        assert!(client_optional.optional);
        assert!(!side("optional", "required").optional);
        let unknown = side("unknown", "unknown");
        assert_eq!(unknown.side, Side::Both);
        assert!(unknown.unknown);
        assert!(side("unsupported", "unsupported").unknown);
        assert_eq!(env_side(None).side, Side::Both);
        assert!(!env_side(None).unknown);
    }

    #[test]
    fn urls_das_cdns() {
        let url = Url::parse(
            "https://cdn.modrinth.com/data/LNytGWDc/versions/k5sGc2zm/create-1.21.1-6.0.6.jar",
        )
        .unwrap();
        assert_eq!(
            modrinth_cdn(&url),
            Some((
                "LNytGWDc".into(),
                "k5sGc2zm".into(),
                "create-1.21.1-6.0.6.jar".into()
            ))
        );
        let encoded =
            Url::parse("https://cdn.modrinth.com/data/A/versions/B/Nome%20com%20espa%C3%A7o.jar")
                .unwrap();
        assert_eq!(modrinth_cdn(&encoded).unwrap().2, "Nome com espaço.jar");
        assert_eq!(
            modrinth_cdn(&Url::parse("https://cdn.modrinth.com/outra/coisa.jar").unwrap()),
            None
        );
        assert_eq!(
            modrinth_cdn(&Url::parse("https://example.com/data/A/versions/B/c.jar").unwrap()),
            None
        );
        let cf = Url::parse("https://edge.forgecdn.net/files/4712/345/jei.jar").unwrap();
        assert_eq!(curseforge_cdn(&cf), Some(4_712_345));
        let low = Url::parse("https://edge.forgecdn.net/files/0/200/swap.jar").unwrap();
        assert_eq!(curseforge_cdn(&low), Some(200));
        assert_eq!(
            curseforge_cdn(&Url::parse("https://edge.forgecdn.net/files/1/2000/x.jar").unwrap()),
            None
        );
    }
}
