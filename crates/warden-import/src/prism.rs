//! Instância do Prism Launcher ou do MultiMC → pack packwiz (ARCHITECTURE §12.2).
//!
//! - `mmc-pack.json` dá o Minecraft e o loader.
//! - `<pasta>/.index/*.pw.toml` (o formato packwiz que o Prism grava) viram metafiles quase
//!   direto: sem os campos `x-prismlauncher-*` (o leitor de metafile só guarda o que o packwiz
//!   conhece) e com o nome de arquivo mantido; o jar correspondente não é copiado.
//! - Jars sem metadados são identificados pelo hash (parte comum, `build`).
//! - `instance.cfg` (`MaxMemAlloc`, `JvmArgs`) vira um perfil do teste (L-08, §7.8).
//! - `options.txt` e `servers.dat` são perguntados; mundos, logs e caches ficam de fora pela
//!   higiene; os arquivos da própria instância (fora da pasta do jogo) não entram.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use warden_packwiz::{Loader, Metafile, metafile_stem};

use crate::archive::{EntryKind, Source};
use crate::build::{Draft, RawFile, RefDraft};
use crate::common::{manifest, split_path};
use crate::curseforge::PREFERENCES;
use crate::error::{Error, Result};
use crate::model::{
    Content, ExcludedItem, ExcludedReason, ImportProfile, ItemSource, SourceKind,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Component {
    #[serde(default)]
    uid: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    cached_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MmcPack {
    #[serde(default)]
    components: Vec<Component>,
}

/// Lê o `instance.cfg` (formato INI do Qt): chave → valor, sem as seções.
pub(crate) fn parse_cfg(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('[') || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let mut value = value.trim().to_owned();
        if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
            value = value[1..value.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");
        }
        map.insert(key.trim().to_owned(), value);
    }
    map
}

/// Memória e argumentos da instância (só o que ela define para si).
pub(crate) fn profile_of(cfg: &BTreeMap<String, String>) -> Option<ImportProfile> {
    let flag = |key: &str| cfg.get(key).is_some_and(|v| v.eq_ignore_ascii_case("true"));
    let memory = (flag("OverrideMemory") || !cfg.contains_key("OverrideMemory"))
        .then(|| cfg.get("MaxMemAlloc"))
        .flatten()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|mb| *mb >= 512);
    let args = (flag("OverrideJavaArgs") || !cfg.contains_key("OverrideJavaArgs"))
        .then(|| cfg.get("JvmArgs"))
        .flatten()
        .map(|v| v.trim().to_owned())
        .unwrap_or_default();
    (memory.is_some() || !args.is_empty()).then_some(ImportProfile {
        memory_mb: memory,
        jvm_args: args,
    })
}

fn loader_of(uid: &str) -> Option<Loader> {
    Some(match uid {
        "net.minecraftforge" => Loader::Forge,
        "net.neoforged" => Loader::NeoForge,
        "net.fabricmc.fabric-loader" => Loader::Fabric,
        "org.quiltmc.quilt-loader" => Loader::Quilt,
        "com.mumfrey.liteloader" => Loader::LiteLoader,
        _ => return None,
    })
}

/// Lê a instância (a origem já aponta para a pasta da instância).
///
/// Erros: [`crate::error::ImportErrorCode::Unreadable`],
/// [`crate::error::ImportErrorCode::UnsupportedLoader`],
/// [`crate::error::ImportErrorCode::MinecraftMissing`].
#[allow(clippy::too_many_lines)]
pub(crate) fn analyze(source: &mut Source, origin: &Path, root: &str) -> Result<Draft> {
    let pack: MmcPack = source.read_json("mmc-pack.json")?;
    let cfg_bytes = source.read("instance.cfg")?;
    let cfg = parse_cfg(&String::from_utf8_lossy(&cfg_bytes));
    let version = |component: &Component| {
        component
            .version
            .clone()
            .or_else(|| component.cached_version.clone())
            .unwrap_or_default()
    };
    let minecraft = pack
        .components
        .iter()
        .find(|c| c.uid == "net.minecraft")
        .map(version)
        .unwrap_or_default();
    let loaders: Vec<(Loader, String)> = pack
        .components
        .iter()
        .filter_map(|c| loader_of(&c.uid).map(|loader| (loader, version(c))))
        .collect();
    let fallback = if root.is_empty() {
        crate::archive::display_name(origin)
    } else {
        root.to_owned()
    };
    let name = cfg.get("name").cloned().unwrap_or_default();
    let pack = manifest(&name, &fallback, &minecraft, &loaders)?;
    let mut draft = Draft::new(SourceKind::Prism, origin.to_path_buf(), root.to_owned(), pack);
    draft.profile = profile_of(&cfg);

    let game = [".minecraft", "minecraft"]
        .into_iter()
        .find(|dir| source.has_dir(dir));
    let mut outside: BTreeMap<String, (bool, u32)> = BTreeMap::new();
    for entry in source.entries()? {
        if entry.kind == EntryKind::Dir {
            continue;
        }
        let inside = game.and_then(|game| {
            entry
                .name
                .strip_prefix(game)
                .and_then(|rest| rest.strip_prefix('/'))
        });
        let Some(path) = inside else {
            let top = entry.name.split('/').next().unwrap_or_default().to_owned();
            outside
                .entry(top)
                .or_insert((entry.name.contains('/'), 0))
                .1 += 1;
            continue;
        };
        let path = path.to_owned();
        // Metadados packwiz que o Prism grava em `<pasta>/.index/`.
        let (dir, file) = split_path(&path);
        if let Some(folder) = dir.strip_suffix("/.index").or_else(|| (dir == ".index").then_some(""))
            && entry.kind == EntryKind::File
        {
            if let Some(stem) = metafile_stem(&file) {
                let text = String::from_utf8(source.read(&entry.name)?).map_err(|_| {
                    Error::internal(format!("{path} não é texto"))
                })?;
                if let Ok(parsed) = Metafile::parse(&text)
                    && !parsed.value.filename.is_empty()
                    && crate::archive::unsafe_reason(&parsed.value.filename).is_none()
                    && !parsed.value.filename.contains('/')
                {
                    let metafile = parsed.value;
                    let update = metafile.update.as_ref();
                    let source_kind = if update.is_some_and(|u| u.contains_key("modrinth")) {
                        ItemSource::Modrinth
                    } else if update.is_some_and(|u| u.contains_key("curseforge")) {
                        ItemSource::Curseforge
                    } else {
                        ItemSource::Url
                    };
                    draft.references.push(RefDraft {
                        dir: folder.to_owned(),
                        slug: stem.to_owned(),
                        metafile,
                        source: source_kind,
                        by_hash: false,
                        review: None,
                        stem: Some(stem.to_owned()),
                    });
                    continue;
                }
            }
            // Outros arquivos do índice do Prism não vão para o pack.
            continue;
        }
        if PREFERENCES.contains(&path.as_str()) {
            draft
                .preferences
                .insert(path, Content::Entry(entry.name.clone()));
            continue;
        }
        draft.raw.push(RawFile {
            path,
            content: Content::Entry(entry.name.clone()),
            size: entry.size,
            kind: entry.kind,
            review: None,
        });
    }
    draft.excluded.extend(outside.into_iter().map(|(path, (is_dir, files))| ExcludedItem {
        path,
        is_dir,
        files,
        reason: ExcludedReason::OtherLauncher,
        can_include: false,
    }));
    Ok(draft)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_cfg_vira_perfil() {
        let cfg = parse_cfg(
            "[General]\nname=Meu pack\nOverrideMemory=true\nMaxMemAlloc=6144\nMinMemAlloc=512\nOverrideJavaArgs=true\nJvmArgs=\"-XX:+UseG1GC -Dx=\\\"y\\\"\"\n",
        );
        assert_eq!(cfg["name"], "Meu pack");
        assert_eq!(
            profile_of(&cfg),
            Some(ImportProfile {
                memory_mb: Some(6144),
                jvm_args: "-XX:+UseG1GC -Dx=\"y\"".into(),
            })
        );
        let off = parse_cfg("OverrideMemory=false\nMaxMemAlloc=6144\nOverrideJavaArgs=false\nJvmArgs=-Xss4m\n");
        assert_eq!(profile_of(&off), None);
        assert_eq!(profile_of(&parse_cfg("MaxMemAlloc=100\n")), None);
    }
}
