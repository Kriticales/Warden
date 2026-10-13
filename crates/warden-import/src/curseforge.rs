//! CurseForge → pack packwiz (ARCHITECTURE §12.2).
//!
//! - **Zip da CurseForge** (`manifest.json`): sem a chave, recusado com
//!   [`Code::CurseforgeKeyMissing`] antes de qualquer requisição (CA-T24-03). Com a chave, o
//!   `packwiz curseforge import` roda numa cópia de staging (nunca no destino); depois o lado
//!   de cada metafile é corrigido pelo Modrinth (SHA-1 → `POST /v2/version_files`) e os
//!   opcionais desligados viram revisão.
//! - **Instância do app da CurseForge** (`minecraftinstance.json`): os `installedAddons` já
//!   trazem projeto, arquivo e SHA-1, então os metafiles saem sem consultar a CurseForge; o lado
//!   é corrigido pelo Modrinth do mesmo jeito.

use std::collections::BTreeMap;
use std::path::Path;

use secrecy::SecretString;
use serde::Deserialize;
use warden_core::CancellationToken;
use warden_packwiz::hygiene::packwizignore_template;
use warden_packwiz::{
    CurseForgeFile, HashFormat, Loader, Metafile, PackIndex, Side, metafile_stem,
};
use warden_packwiz_cli::{Packwiz, RunContext};

use crate::archive::{EntryKind, Source, display_name};
use crate::build::{Draft, RawFile, RefDraft};
use crate::common::{manifest, split_path, stem};
use crate::error::{Error, ImportErrorCode as Code, Result};
use crate::lookup::{Stores, modrinth_sides};
use crate::model::{
    Content, ExcludedItem, ExcludedReason, ImportProfile, ItemSource, ReviewReason, SourceKind,
};

/// O que o zip da CurseForge precisa além das lojas.
pub struct CurseforgeTools<'a> {
    /// O sidecar do packwiz.
    pub packwiz: &'a Packwiz,
    /// A chave da CurseForge (nunca vai para a interface nem para registros).
    pub key: Option<&'a SecretString>,
    /// Pasta das cópias de staging (`cache/staging`).
    pub staging_parent: &'a Path,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModLoader {
    #[serde(default)]
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftInfo {
    #[serde(default)]
    version: String,
    #[serde(default)]
    mod_loaders: Vec<ModLoader>,
    #[serde(default)]
    recommended_ram: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZipManifest {
    minecraft: MinecraftInfo,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    overrides: Option<String>,
}

/// `forge-47.2.0`, `neoforge-21.1.1`, `fabric-0.16.5` (ou `fabric-0.16.5-1.21.1` no app) →
/// loader e versão.
pub(crate) fn parse_loader(id: &str) -> Option<(Loader, String)> {
    let (name, rest) = id.split_once('-')?;
    let loader = match name.to_ascii_lowercase().as_str() {
        "forge" => Loader::Forge,
        "neoforge" => Loader::NeoForge,
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        _ => return None,
    };
    let version = match loader {
        // O app grava `fabric-<loader>-<minecraft>`.
        Loader::Fabric | Loader::Quilt => rest.split('-').next().unwrap_or(rest),
        _ => rest,
    };
    (!version.is_empty()).then(|| (loader, version.to_owned()))
}

fn source_of(metafile: &Metafile) -> ItemSource {
    let update = metafile.update.as_ref();
    if update.is_some_and(|u| u.contains_key("modrinth")) {
        ItemSource::Modrinth
    } else if update.is_some_and(|u| u.contains_key("curseforge")) {
        ItemSource::Curseforge
    } else {
        ItemSource::Url
    }
}

/// Corrige o lado dos metafiles pelo Modrinth (SHA-1).
async fn fix_sides(refs: &mut [RefDraft], stores: Stores<'_>, cancel: &CancellationToken) -> bool {
    let sha1s: Vec<String> = refs
        .iter()
        .filter(|item| {
            item.metafile
                .download
                .hash_format
                .eq_ignore_ascii_case("sha1")
        })
        .map(|item| item.metafile.download.hash.to_ascii_lowercase())
        .collect();
    let Some(sides) = modrinth_sides(stores.modrinth, &sha1s, cancel).await else {
        return false;
    };
    for item in refs {
        if item
            .metafile
            .download
            .hash_format
            .eq_ignore_ascii_case("sha1")
            && let Some(side) = sides.get(&item.metafile.download.hash.to_ascii_lowercase())
        {
            item.metafile.side = side.clone();
        }
    }
    true
}

/// Tira a pasta da cópia e da origem de um texto do packwiz (privacidade, QUALITY §10).
fn redact(text: &str, paths: &[&Path]) -> String {
    let mut text = text.to_owned();
    for path in paths {
        for form in [
            path.display().to_string(),
            path.display().to_string().replace('\\', "/"),
        ] {
            if !form.is_empty() {
                text = text.replace(&form, "<pasta>");
            }
        }
    }
    text
}

/// Lê o zip da CurseForge pelo packwiz numa cópia de staging.
///
/// Erros: [`Code::CurseforgeKeyMissing`] (sem requisição), [`Code::Unreadable`],
/// [`Code::UnsupportedLoader`], [`Code::MinecraftMissing`], [`Code::CurseforgeImportFailed`].
#[allow(clippy::too_many_lines)]
pub(crate) async fn analyze_zip(
    source: &mut Source,
    origin: &Path,
    stores: Stores<'_>,
    tools: &CurseforgeTools<'_>,
    cancel: &CancellationToken,
) -> Result<Draft> {
    let zip: ZipManifest = source.read_json("manifest.json")?;
    let loaders: Vec<(Loader, String)> = zip
        .minecraft
        .mod_loaders
        .iter()
        .filter_map(|loader| parse_loader(&loader.id))
        .collect();
    let mut pack = manifest(
        &zip.name,
        &stem(source.file_name()),
        &zip.minecraft.version,
        &loaders,
    )?;
    let Some(key) = tools.key.filter(|_| stores.curseforge.is_some()) else {
        return Err(Error::new(
            Code::CurseforgeKeyMissing,
            "o zip da CurseForge precisa da chave da CurseForge",
        ));
    };
    pack.version = zip.version.trim().to_owned();
    pack.author = zip.author.trim().to_owned();

    let staging = tools
        .staging_parent
        .join(format!("import-{}", warden_core::PackId::new()));
    let result = async {
        std::fs::create_dir_all(&staging)
            .map_err(|error| Error::internal(format!("cópia de staging: {}", error.kind())))?;
        let write = |name: &str, text: String| {
            std::fs::write(staging.join(name), text)
                .map_err(|error| Error::internal(format!("cópia de staging: {}", error.kind())))
        };
        write("pack.toml", pack.to_toml_string())?;
        write("index.toml", PackIndex::default().to_toml_string())?;
        write(".packwizignore", packwizignore_template())?;
        let absolute = std::path::absolute(origin)
            .map_err(|error| Error::internal(format!("caminho do zip: {}", error.kind())))?;
        if let Err(error) = tools
            .packwiz
            .curseforge_import(&staging, &absolute, Some(key), RunContext::new(cancel))
            .await
        {
            use warden_core::DomainError as _;
            let detail = redact(
                &error.detail().unwrap_or_else(|| error.to_string()),
                &[&staging, &absolute],
            );
            if matches!(
                error,
                warden_packwiz_cli::Error::CurseForgeKeyMissing { .. }
            ) {
                return Err(Error::new(
                    Code::CurseforgeKeyMissing,
                    "o packwiz não recebeu a chave da CurseForge",
                ));
            }
            if matches!(error, warden_packwiz_cli::Error::Cancelled { .. }) {
                return Err(Error::internal("cancelado"));
            }
            return Err(Error::new(
                Code::CurseforgeImportFailed,
                "o packwiz não conseguiu ler o zip da CurseForge",
            )
            .with_detail(detail));
        }
        read_staging(
            &staging,
            source,
            origin,
            pack_after(&staging, pack.clone()),
            &zip,
            stores,
            cancel,
        )
        .await
    }
    .await;
    match result {
        Ok(mut draft) => {
            draft.staging = Some(staging);
            Ok(draft)
        }
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(error)
        }
    }
}

/// O `pack.toml` que o packwiz deixou (com as versões acertadas), ou o nosso.
fn pack_after(
    staging: &Path,
    fallback: warden_packwiz::PackManifest,
) -> warden_packwiz::PackManifest {
    std::fs::read_to_string(staging.join("pack.toml"))
        .ok()
        .and_then(|text| warden_packwiz::PackManifest::parse(&text).ok())
        .map_or(fallback.clone(), |parsed| {
            let mut pack = fallback;
            if let Some(versions) = parsed.value.versions {
                pack.versions = Some(versions);
            }
            pack
        })
}

async fn read_staging(
    staging: &Path,
    source: &mut Source,
    origin: &Path,
    pack: warden_packwiz::PackManifest,
    zip: &ZipManifest,
    stores: Stores<'_>,
    cancel: &CancellationToken,
) -> Result<Draft> {
    let mut draft = Draft::new(
        SourceKind::CurseforgeZip,
        origin.to_path_buf(),
        String::new(),
        pack,
    );
    let mut copy = Source::open_dir(staging)?;
    for entry in copy.entries()? {
        if entry.kind == EntryKind::Dir
            || matches!(
                entry.name.as_str(),
                "pack.toml" | "index.toml" | ".packwizignore"
            )
        {
            continue;
        }
        let (dir, _) = split_path(&entry.name);
        if let Some(stem_name) = metafile_stem(&entry.name)
            && entry.kind == EntryKind::File
        {
            let text = String::from_utf8(copy.read(&entry.name)?)
                .map_err(|_| Error::internal(format!("{} não é texto", entry.name)))?;
            let metafile = Metafile::parse(&text)
                .map_err(|error| Error::internal(format!("{}: {error}", entry.name)))?
                .value;
            let review = metafile
                .option
                .as_ref()
                .filter(|option| option.optional && !option.default)
                .map(|_| ReviewReason::OptionalDisabled);
            draft.references.push(RefDraft {
                dir,
                slug: stem_name.to_owned(),
                source: source_of(&metafile),
                metafile,
                by_hash: false,
                review,
                stem: Some(stem_name.to_owned()),
            });
            continue;
        }
        draft.raw.push(RawFile {
            path: entry.name.clone(),
            content: Content::Staged(staging.join(&entry.name)),
            size: entry.size,
            kind: entry.kind,
            review: None,
        });
    }
    if !fix_sides(&mut draft.references, stores, cancel).await {
        draft.unavailable.push("Modrinth".to_owned());
    }
    // O que o zip traz fora dos overrides (lista de mods em HTML etc.) não entra.
    let overrides = zip
        .overrides
        .clone()
        .unwrap_or_else(|| "overrides".to_owned());
    let mut outside: BTreeMap<String, (bool, u32)> = BTreeMap::new();
    for entry in source.entries()? {
        if entry.kind == EntryKind::Dir || entry.name == "manifest.json" {
            continue;
        }
        let top = entry.name.split('/').next().unwrap_or_default().to_owned();
        if top == overrides {
            continue;
        }
        outside
            .entry(top)
            .or_insert((entry.name.contains('/'), 0))
            .1 += 1;
    }
    draft.excluded.extend(
        outside
            .into_iter()
            .map(|(path, (is_dir, files))| ExcludedItem {
                path,
                is_dir,
                files,
                reason: ExcludedReason::OtherLauncher,
                can_include: false,
            }),
    );
    if let Some(ram) = zip.minecraft.recommended_ram.filter(|ram| *ram >= 512) {
        draft.profile = Some(ImportProfile {
            memory_mb: Some(ram),
            jvm_args: String::new(),
        });
    }
    Ok(draft)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppHash {
    #[serde(default, rename = "type")]
    kind: u32,
    #[serde(default)]
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppFile {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    hashes: Vec<AppHash>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppAddon {
    #[serde(default, rename = "addonID")]
    addon_id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    installed_file: Option<AppFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppLoader {
    #[serde(default)]
    name: String,
    #[serde(default)]
    minecraft_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppInstance {
    #[serde(default)]
    name: String,
    #[serde(default)]
    game_version: String,
    #[serde(default)]
    base_mod_loader: Option<AppLoader>,
    #[serde(default)]
    installed_addons: Vec<AppAddon>,
    #[serde(default)]
    allocated_memory: Option<u32>,
    #[serde(default)]
    java_args_override: Option<String>,
}

/// Arquivos da raiz que são preferências pessoais (perguntadas na tela).
pub(crate) const PREFERENCES: [&str; 2] = ["options.txt", "servers.dat"];

/// Lê a instância do app da CurseForge.
///
/// Erros: [`Code::Unreadable`], [`Code::UnsupportedLoader`], [`Code::MinecraftMissing`].
#[allow(clippy::too_many_lines)]
pub(crate) async fn analyze_app(
    source: &mut Source,
    origin: &Path,
    stores: Stores<'_>,
    cancel: &CancellationToken,
) -> Result<Draft> {
    let instance: AppInstance = source.read_json("minecraftinstance.json")?;
    let loader = instance.base_mod_loader.as_ref();
    let minecraft = loader
        .map(|loader| loader.minecraft_version.clone())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| instance.game_version.clone());
    let loaders: Vec<(Loader, String)> = loader
        .and_then(|loader| parse_loader(&loader.name))
        .into_iter()
        .collect();
    let pack = manifest(&instance.name, &display_name(origin), &minecraft, &loaders)?;
    let mut draft = Draft::new(
        SourceKind::CurseforgeApp,
        origin.to_path_buf(),
        String::new(),
        pack,
    );
    let entries = source.entries()?;
    for addon in &instance.installed_addons {
        let Some(file) = &addon.installed_file else {
            continue;
        };
        let (Ok(project_id), Ok(file_id)) = (u32::try_from(addon.addon_id), u32::try_from(file.id))
        else {
            continue;
        };
        if file.file_name.is_empty()
            || crate::archive::unsafe_reason(&file.file_name).is_some()
            || file.file_name.contains('/')
        {
            continue;
        }
        let dir = ["mods", "resourcepacks", "shaderpacks"]
            .into_iter()
            .find(|dir| {
                entries
                    .iter()
                    .any(|e| e.name == format!("{dir}/{}", file.file_name))
            })
            .unwrap_or("mods");
        let sha1 = file
            .hashes
            .iter()
            .find(|hash| hash.kind == 1 && hash.value.len() == 40)
            .map(|hash| hash.value.to_ascii_lowercase());
        let Some(sha1) = sha1 else {
            continue;
        };
        let name = if addon.name.trim().is_empty() {
            stem(&file.file_name)
        } else {
            addon.name.trim().to_owned()
        };
        draft.references.push(RefDraft {
            dir: dir.to_owned(),
            slug: name.clone(),
            metafile: Metafile::curseforge(
                &CurseForgeFile {
                    name,
                    project_id,
                    file_id,
                    filename: file.file_name.clone(),
                    hash_format: HashFormat::Sha1,
                    hash: sha1,
                },
                Side::Both,
            ),
            source: ItemSource::Curseforge,
            by_hash: false,
            review: None,
            stem: None,
        });
    }
    if !fix_sides(&mut draft.references, stores, cancel).await {
        draft.unavailable.push("Modrinth".to_owned());
    }
    for entry in entries {
        if entry.kind == EntryKind::Dir {
            continue;
        }
        if matches!(
            entry.name.as_str(),
            "minecraftinstance.json" | ".curseclient"
        ) {
            draft.excluded.push(ExcludedItem {
                path: entry.name.clone(),
                is_dir: false,
                files: 1,
                reason: ExcludedReason::OtherLauncher,
                can_include: false,
            });
            continue;
        }
        if PREFERENCES.contains(&entry.name.as_str()) {
            draft
                .preferences
                .insert(entry.name.clone(), Content::Entry(entry.name.clone()));
            continue;
        }
        draft.raw.push(RawFile {
            path: entry.name.clone(),
            content: Content::Entry(entry.name.clone()),
            size: entry.size,
            kind: entry.kind,
            review: None,
        });
    }
    let memory = instance.allocated_memory.filter(|mb| *mb >= 512);
    let args = instance
        .java_args_override
        .unwrap_or_default()
        .trim()
        .to_owned();
    if memory.is_some() || !args.is_empty() {
        draft.profile = Some(ImportProfile {
            memory_mb: memory,
            jvm_args: args,
        });
    }
    Ok(draft)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaders_da_curseforge() {
        assert_eq!(
            parse_loader("forge-47.2.0"),
            Some((Loader::Forge, "47.2.0".into()))
        );
        assert_eq!(
            parse_loader("neoforge-21.1.172"),
            Some((Loader::NeoForge, "21.1.172".into()))
        );
        assert_eq!(
            parse_loader("fabric-0.16.5"),
            Some((Loader::Fabric, "0.16.5".into()))
        );
        assert_eq!(
            parse_loader("fabric-0.16.5-1.21.1"),
            Some((Loader::Fabric, "0.16.5".into()))
        );
        assert_eq!(
            parse_loader("quilt-0.26.0"),
            Some((Loader::Quilt, "0.26.0".into()))
        );
        assert_eq!(parse_loader("forge-"), None);
        assert_eq!(parse_loader("risugami-1"), None);
    }

    #[test]
    fn redacao_tira_as_pastas() {
        let text = redact(
            "falhou em /home/ana/cache/staging/import-1/mods e /home/ana/Downloads/x.zip",
            &[
                Path::new("/home/ana/cache/staging/import-1"),
                Path::new("/home/ana/Downloads/x.zip"),
            ],
        );
        assert_eq!(text, "falhou em <pasta>/mods e <pasta>");
    }
}
