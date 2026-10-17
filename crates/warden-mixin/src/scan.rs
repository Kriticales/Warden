//! Camada A: lê de um jar (e dos embutidos) as configs de mixin que o loader do pack carrega,
//! os refmaps e, pela camada B ([`crate::class`]), as classes de mixin.
//!
//! Os descritores vêm da `warden-jarmeta` (o mesmo leitor do resto do diagnóstico, com os
//! mesmos limites). Regras de leitura do S-R5-1 §9:
//!
//! - só as configs do loader do pack: no Fabric e no Quilt, as do `fabric.mod.json` (e do
//!   `quilt.mod.json`); no Forge, as do `MixinConfigs` do manifesto; no NeoForge, as do
//!   `[[mixins]]` do descritor e as do manifesto. O `fabric.mod.json` vale no Forge/NeoForge só
//!   para jar sem descritor do loader (mod Fabric pelo Sinytra Connector), marcado
//!   [`ScannedMod::connector`];
//! - o raio-x olha o cliente (o Testar): configs com `"environment": "server"` e a lista
//!   `server` das configs ficam de fora;
//! - config declarada e ausente no jar é ignorada sem erro (costuma estar noutro jar do mesmo
//!   mod, como o `citadel.mixins.json` no Alex's Mobs);
//! - jars embutidos (`META-INF/jars/`, `META-INF/jarjar/`) são lidos sempre: no Sodium para
//!   NeoForge, todos os mixins estão no embutido.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

use warden_jarmeta::{DependencyKind, DescriptorKind, JarMetadata, Limits, Loader};
use zip::ZipArchive;

use crate::class::{ConfigDefaults, parse_mixin_class, string_constants};
use crate::model::{DeclaredDep, DepKind, IssueKind, JarScan, ScanIssue, ScannedMod};
use crate::options::declared_disables;
use crate::refmap::Refmap;

/// Tamanho máximo de uma classe de mixin ou de uma config (descompactado).
const MAX_ENTRY_BYTES: u64 = 4 * 1024 * 1024;
/// Quantas classes `*Config*`/`*Plugin*`/`*Compat*` têm os textos lidos por jar.
const MAX_TEXT_CLASSES: usize = 60;
/// Pacote do `MixinSquared`.
const MIXIN_SQUARED: &str = "com/bawnorton/mixinsquared";

/// Os limites da leitura dos descritores, sem ler cabeçalhos de classe (o raio-x não precisa
/// da versão do Java).
fn descriptor_limits(limits: &Limits) -> Limits {
    Limits {
        max_class_headers: 0,
        ..*limits
    }
}

/// Lê um jar do disco. Um jar ilegível vira um mod com o problema [`IssueKind::UnreadableJar`]
/// (nunca um erro: o resto do pack continua).
#[must_use]
pub fn scan_jar_file(path: &Path, loader: Loader, limits: &Limits) -> JarScan {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let meta = warden_jarmeta::read_jar_file(path, &descriptor_limits(limits));
    let zip = File::open(path)
        .ok()
        .and_then(|file| ZipArchive::new(BufReader::new(file)).ok());
    match (meta, zip) {
        (Ok(meta), Some(mut zip)) => {
            let mut out = Vec::new();
            let mut budget = limits.max_total_nested_bytes;
            scan_archive(
                &mut zip,
                &meta,
                "",
                &name,
                loader,
                limits,
                &mut budget,
                &mut out,
            );
            JarScan { mods: out }
        }
        _ => unreadable(&name),
    }
}

/// Lê um jar já na memória (mesmas regras de [`scan_jar_file`]).
#[must_use]
pub fn scan_jar_bytes(bytes: &[u8], file_name: &str, loader: Loader, limits: &Limits) -> JarScan {
    let meta = warden_jarmeta::read_jar_bytes(bytes, &descriptor_limits(limits));
    let zip = ZipArchive::new(Cursor::new(bytes)).ok();
    match (meta, zip) {
        (Ok(meta), Some(mut zip)) => {
            let mut out = Vec::new();
            let mut budget = limits.max_total_nested_bytes;
            scan_archive(
                &mut zip,
                &meta,
                "",
                file_name,
                loader,
                limits,
                &mut budget,
                &mut out,
            );
            JarScan { mods: out }
        }
        _ => unreadable(file_name),
    }
}

fn unreadable(file_name: &str) -> JarScan {
    JarScan {
        mods: vec![ScannedMod {
            mod_id: format!("?{file_name}"),
            issues: vec![ScanIssue {
                kind: IssueKind::UnreadableJar,
                file: file_name.to_owned(),
            }],
            ..ScannedMod::default()
        }],
    }
}

/// Uma config declarada e se ela vale no cliente.
struct DeclaredConfig {
    name: String,
    client: bool,
}

/// Lê o jar `zip` (metadados `meta`) e, em profundidade, os embutidos.
#[allow(clippy::too_many_arguments)]
fn scan_archive<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    meta: &JarMetadata,
    path: &str,
    file_name: &str,
    loader: Loader,
    limits: &Limits,
    budget: &mut u64,
    out: &mut Vec<ScannedMod>,
) {
    let mut scanned = scan_one(zip, meta, path, file_name, loader);
    scanned.nested_count = u32::try_from(meta.nested.len()).unwrap_or(u32::MAX);
    out.push(scanned);

    for nested in &meta.nested {
        let Some(nested_meta) = nested.metadata.as_deref() else {
            continue;
        };
        let entry = nested.path.trim_start_matches('/');
        let max = limits.max_nested_jar_bytes.min(*budget);
        let Some(bytes) = read_entry(zip, entry, max) else {
            continue;
        };
        *budget = budget.saturating_sub(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        let Ok(mut inner) = ZipArchive::new(Cursor::new(bytes)) else {
            continue;
        };
        let child = if path.is_empty() {
            nested.path.clone()
        } else {
            format!("{path}!/{}", nested.path)
        };
        let child_name = entry.rsplit('/').next().unwrap_or(entry);
        scan_archive(
            &mut inner,
            nested_meta,
            &child,
            child_name,
            loader,
            limits,
            budget,
            out,
        );
    }
}

/// Bytes de uma entrada, até `max` descompactados; `None` se faltar, passar ou falhar.
fn read_entry<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str, max: u64) -> Option<Vec<u8>> {
    let index = zip.index_for_name(name)?;
    let file = zip.by_index(index).ok()?;
    let mut buf = Vec::new();
    file.take(max.saturating_add(1))
        .read_to_end(&mut buf)
        .ok()?;
    (u64::try_from(buf.len()).unwrap_or(u64::MAX) <= max).then_some(buf)
}

/// Resultado de ler uma entrada pequena (config, classe).
enum Small {
    Missing,
    TooLarge,
    Bytes(Vec<u8>),
}

fn read_small<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str) -> Small {
    if zip.index_for_name(name).is_none() {
        return Small::Missing;
    }
    read_entry(zip, name, MAX_ENTRY_BYTES).map_or(Small::TooLarge, Small::Bytes)
}

/// JSON tolerante: alguns autores deixam quebras de linha cruas dentro de textos.
fn lenient_json(bytes: &[u8]) -> Option<serde_json::Value> {
    if let Ok(value) = serde_json::from_slice(bytes) {
        return Some(value);
    }
    let text = String::from_utf8_lossy(bytes).replace(['\n', '\r', '\t'], " ");
    serde_json::from_str(&text).ok()
}

/// Ids que não são mods (dependência do jogo, do loader ou do Java).
fn is_platform(id: &str) -> bool {
    matches!(
        id,
        "minecraft"
            | "java"
            | "fabricloader"
            | "fabric-loader"
            | "quilt_loader"
            | "forge"
            | "neoforge"
    )
}

#[allow(clippy::too_many_lines)]
fn scan_one<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    meta: &JarMetadata,
    path: &str,
    file_name: &str,
    loader: Loader,
) -> ScannedMod {
    let mods = meta.mods_for_loader(loader);
    let fabric_style = mods.first().is_some_and(|m| {
        matches!(
            m.source,
            DescriptorKind::FabricModJson | DescriptorKind::QuiltModJson
        )
    });
    let connector = fabric_style && matches!(loader, Loader::Forge | Loader::NeoForge);

    let mut scanned = ScannedMod {
        path: path.to_owned(),
        mod_id: mods
            .first()
            .map_or_else(|| format!("?{file_name}"), |m| m.id.clone()),
        version: mods
            .first()
            .and_then(|m| m.version.clone())
            .unwrap_or_default(),
        connector,
        ..ScannedMod::default()
    };
    let mut deps = BTreeMap::new();
    for module in &mods {
        for dep in &module.dependencies {
            if is_platform(&dep.id) || dep.id == module.id {
                continue;
            }
            let kind = match dep.kind {
                DependencyKind::Required => DepKind::Required,
                DependencyKind::Optional
                | DependencyKind::Recommends
                | DependencyKind::Suggests => DepKind::Optional,
                _ => continue,
            };
            deps.entry(dep.id.clone()).or_insert(kind);
        }
    }
    scanned.deps = deps
        .into_iter()
        .map(|(id, kind)| DeclaredDep { id, kind })
        .collect();

    // Configs do loader do pack.
    let mut declared: Vec<DeclaredConfig> = Vec::new();
    if fabric_style {
        let environments = fabric_environments(zip);
        for module in &mods {
            for config in &module.mixins {
                let client = environments
                    .get(config.as_str())
                    .is_none_or(|env| env != "server");
                declared.push(DeclaredConfig {
                    name: config.clone(),
                    client,
                });
            }
        }
    } else {
        if loader == Loader::NeoForge {
            for module in &mods {
                declared.extend(module.mixins.iter().map(|name| DeclaredConfig {
                    name: name.clone(),
                    client: true,
                }));
            }
        }
        if matches!(loader, Loader::Forge | Loader::NeoForge)
            && let Some(manifest) = &meta.manifest
        {
            declared.extend(manifest.mixin_configs.iter().map(|name| DeclaredConfig {
                name: name.clone(),
                client: true,
            }));
        }
    }
    let mut seen = BTreeSet::new();
    declared.retain(|c| {
        let name = c.name.trim();
        !name.is_empty() && seen.insert(name.to_owned())
    });

    let mut names = BTreeMap::new();
    let mut plugins = Vec::new();
    for config in declared.iter().filter(|c| c.client) {
        let file = config.name.trim().trim_start_matches('/');
        let bytes = match read_small(zip, file) {
            Small::Missing => {
                scanned.issues.push(issue(IssueKind::MissingConfig, file));
                continue;
            }
            Small::TooLarge => {
                scanned.issues.push(issue(IssueKind::TooLarge, file));
                continue;
            }
            Small::Bytes(bytes) => bytes,
        };
        let Some(json) = lenient_json(&bytes) else {
            scanned.issues.push(issue(IssueKind::InvalidConfig, file));
            continue;
        };
        scanned.configs.push(config.name.trim().to_owned());
        let package = json
            .get("package")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .replace('.', "/");
        let priority = json
            .get("priority")
            .and_then(serde_json::Value::as_i64)
            .and_then(|p| i32::try_from(p).ok())
            .unwrap_or(1000);
        let plugin = json
            .get("plugin")
            .and_then(serde_json::Value::as_str)
            .filter(|p| !p.trim().is_empty());
        if let Some(plugin) = plugin {
            plugins.push(plugin.replace('.', "/"));
        }
        let default_require = json
            .get("injectors")
            .and_then(|i| i.get("defaultRequire"))
            .and_then(serde_json::Value::as_i64)
            .and_then(|r| i32::try_from(r).ok());
        let refmap = json
            .get("refmap")
            .and_then(serde_json::Value::as_str)
            .and_then(|r| match read_small(zip, r.trim_start_matches('/')) {
                Small::Bytes(bytes) => Some(Refmap::parse(&bytes)),
                Small::Missing | Small::TooLarge => None,
            })
            .unwrap_or_default();
        names.extend(refmap.learned_names());
        let defaults = ConfigDefaults {
            name: config.name.trim(),
            priority,
            plugin: plugin.is_some(),
            default_require,
            refmap: &refmap,
        };
        let mut listed = Vec::new();
        for key in ["mixins", "client"] {
            if let Some(list) = json.get(key).and_then(serde_json::Value::as_array) {
                listed.extend(list.iter().filter_map(serde_json::Value::as_str));
            }
        }
        for mixin in listed {
            let relative = mixin.trim().replace('.', "/");
            if relative.is_empty() {
                continue;
            }
            let internal = if package.is_empty() {
                relative
            } else {
                format!("{package}/{relative}")
            };
            let class_file = format!("{internal}.class");
            match read_small(zip, &class_file) {
                Small::Missing => scanned
                    .issues
                    .push(issue(IssueKind::MissingClass, &class_file)),
                Small::TooLarge => scanned.issues.push(issue(IssueKind::TooLarge, &class_file)),
                Small::Bytes(bytes) => match parse_mixin_class(
                    &bytes,
                    &internal,
                    &defaults,
                    &mut scanned.other_annotations,
                ) {
                    Ok(class) => scanned.classes.push(class),
                    Err(kind) => scanned.issues.push(issue(kind, &class_file)),
                },
            }
        }
    }
    scanned.names = names.into_iter().collect();

    // Pacotes, o `MixinSquared` e as classes cujos textos dizem com quem o mod convive.
    let mut packages = BTreeSet::new();
    let mut text_classes: Vec<String> = plugins.iter().map(|p| format!("{p}.class")).collect();
    let mut extra = 0;
    // Nome que não se lê (fora do UTF-8 e do CP437 do zip) não é classe que o Mixin acharia.
    for entry in zip.file_names().flatten() {
        let Some(stem) = entry.strip_suffix(".class") else {
            continue;
        };
        if entry.starts_with("META-INF/") {
            continue;
        }
        if let Some((package, simple)) = stem.rsplit_once('/') {
            packages.insert(package.to_owned());
            if extra < MAX_TEXT_CLASSES
                && (simple.contains("Config")
                    || simple.contains("Plugin")
                    || simple.contains("Compat"))
            {
                text_classes.push(entry.to_string());
                extra += 1;
            }
        }
    }
    scanned.mixin_squared = packages
        .iter()
        .any(|p| p == MIXIN_SQUARED || p.starts_with(&format!("{MIXIN_SQUARED}/")));
    scanned.packages = packages.into_iter().collect();
    if !scanned.classes.is_empty() {
        let mut texts = BTreeSet::new();
        text_classes.sort();
        text_classes.dedup();
        for class_file in text_classes {
            if let Small::Bytes(bytes) = read_small(zip, &class_file) {
                string_constants(&bytes, &mut texts);
            }
        }
        scanned.plugin_strings = texts.into_iter().collect();
    }

    // Opções de outros mods que este desliga no próprio descritor.
    let mut disables = BTreeSet::new();
    for kind in [
        DescriptorKind::FabricModJson,
        DescriptorKind::QuiltModJson,
        DescriptorKind::ModsToml,
        DescriptorKind::NeoForgeModsToml,
    ] {
        if let Small::Bytes(bytes) = read_small(zip, kind.path()) {
            disables.extend(declared_disables(&String::from_utf8_lossy(&bytes)));
        }
    }
    scanned.disables = disables.into_iter().collect();
    scanned
}

fn issue(kind: IssueKind, file: &str) -> ScanIssue {
    ScanIssue {
        kind,
        file: file.to_owned(),
    }
}

/// `environment` de cada config na lista `mixins` do `fabric.mod.json` (a `warden-jarmeta`
/// guarda só os nomes).
fn fabric_environments<R: Read + Seek>(zip: &mut ZipArchive<R>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Small::Bytes(bytes) = read_small(zip, "fabric.mod.json") else {
        return out;
    };
    let Some(json) = lenient_json(&bytes) else {
        return out;
    };
    if let Some(list) = json.get("mixins").and_then(serde_json::Value::as_array) {
        for entry in list {
            if let (Some(config), Some(env)) = (
                entry.get("config").and_then(serde_json::Value::as_str),
                entry.get("environment").and_then(serde_json::Value::as_str),
            ) {
                out.insert(config.to_owned(), env.to_owned());
            }
        }
    }
    out
}
