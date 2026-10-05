//! Avaliação pura das regras da ARCHITECTURE §9.2.

use std::collections::{HashMap, HashSet};

use warden_jarmeta::{
    DependencyKind, DescriptorKind, Environment, Loader, MavenFlavor, ModMetadata, Side as JarSide,
    VersionRange,
};
use warden_packwiz::Side;

use crate::Result;
use crate::model::{Evidence, Finding, ItemRef, RuleCode, Severity, SideChoice, SuggestedFix};
use crate::pretest::curated;
use crate::pretest::mc_version::{for_fabric, release_cmp};
use crate::pretest::{ApiRelationKind, Input, PackItem, Pass, ReleaseChannel};

#[derive(Clone, Copy)]
struct SeenMod<'a> {
    item: &'a PackItem,
    meta: &'a ModMetadata,
}

#[allow(clippy::too_many_lines)] // Uma passagem preserva os índices de mods e evidências entre as regras.
pub(super) fn analyze(input: &Input) -> Result<Vec<Finding>> {
    let catalogs = curated::get()?;
    let mut findings = Vec::new();
    let mut hashes: HashMap<&str, &PackItem> = HashMap::new();
    let mut projects: HashMap<(&str, &str), &PackItem> = HashMap::new();
    let mut cross_source: HashMap<String, (&PackItem, crate::Source)> = HashMap::new();
    let mut ids: HashMap<String, Vec<(&PackItem, Option<&str>)>> = HashMap::new();

    for item in &input.items {
        if let Some(hash) = item.file_hash.as_deref().filter(|s| !s.is_empty())
            && let Some(other) = hashes.insert(hash, item)
        {
            findings.push(pair_finding(
                "E_DUPLICATE_FILE",
                Severity::Error,
                other,
                item,
                installed_ev(other, format!("hash dos bytes: {hash}")),
                installed_ev(item, format!("hash dos bytes: {hash}")),
            ));
        }
        if let Some(api) = &item.api {
            let source = match api.source {
                crate::Source::Modrinth => "modrinth",
                crate::Source::CurseForge => "curseforge",
            };
            if !api.project_id.is_empty()
                && let Some(other) = projects.insert((source, &api.project_id), item)
            {
                findings.push(pair_finding(
                    "E_DUPLICATE_FILE",
                    Severity::Error,
                    other,
                    item,
                    api_ev(other, "project_id"),
                    api_ev(item, "project_id"),
                ));
            }
            if let Some(slug) = api.project_slug.as_deref().filter(|v| !v.is_empty())
                && let Some((other, source)) =
                    cross_source.insert(slug.to_ascii_lowercase(), (item, api.source))
                && source != api.source
            {
                findings.push(pair_finding(
                    "E_DUPLICATE_FILE",
                    Severity::Error,
                    other,
                    item,
                    api_ev(other, "slug"),
                    api_ev(item, "slug"),
                ));
            }
            if !api.loaders.is_empty()
                && !api_loader_matches(&api.loaders, input.pack.loader, &input.pack.minecraft)
            {
                let severity = if input.pack.loader == Loader::NeoForge
                    && api.loaders.contains(&Loader::Forge)
                    && release_cmp(&input.pack.minecraft, "1.20.2")
                        .is_some_and(std::cmp::Ordering::is_ge)
                {
                    Severity::Warning
                } else {
                    Severity::Error
                };
                findings.push(single(
                    "E_LOADER",
                    severity,
                    item,
                    Evidence::Api {
                        source: api.source,
                        url: api.url.clone(),
                        field: "loaders".into(),
                    },
                ));
            }
            if !api.game_versions.is_empty() && !api.game_versions.contains(&input.pack.minecraft) {
                let acceptable = input
                    .pack
                    .acceptable_game_versions
                    .iter()
                    .any(|v| api.game_versions.contains(v));
                findings.push(single(
                    "E_MCVERSION",
                    if acceptable {
                        Severity::Warning
                    } else {
                        Severity::Error
                    },
                    item,
                    Evidence::Api {
                        source: api.source,
                        url: api.url.clone(),
                        field: "game_versions".into(),
                    },
                ));
            }
            if api.release != ReleaseChannel::Release {
                findings.push(single(
                    "I_PRERELEASE",
                    Severity::Info,
                    item,
                    api_ev(item, "version_type"),
                ));
            }
            if api.archived {
                findings.push(single(
                    "W_OBSOLETE",
                    Severity::Warning,
                    item,
                    api_ev(item, "status"),
                ));
            }
            if api.download_blocked {
                findings.push(single(
                    "W_DOWNLOAD_BLOCKED",
                    Severity::Warning,
                    item,
                    api_ev(item, "allow_mod_distribution"),
                ));
            }
            if api.client_side == Some(true)
                && api.server_side == Some(false)
                && item.side.on_server()
            {
                findings.push(
                    single(
                        "W_SIDE",
                        Severity::Warning,
                        item,
                        api_ev(item, "server_side"),
                    )
                    .with_fix(SuggestedFix::SetSide {
                        item: item_ref(item),
                        side: SideChoice::Client,
                    }),
                );
            }
            if api.client_side == Some(false)
                && api.server_side == Some(true)
                && item.side.on_client()
            {
                findings.push(
                    single(
                        "W_SIDE",
                        Severity::Warning,
                        item,
                        api_ev(item, "client_side"),
                    )
                    .with_fix(SuggestedFix::SetSide {
                        item: item_ref(item),
                        side: SideChoice::Server,
                    }),
                );
            }
            for relation in &api.relations {
                let present = input.items.iter().find(|other| {
                    other.api.as_ref().is_some_and(|a| {
                        a.source == api.source && a.project_id == relation.project_id
                    })
                });
                match (relation.kind, present) {
                    (ApiRelationKind::Incompatible, Some(other)) => findings.push(pair_finding(
                        "W_INCOMPATIBLE_API",
                        Severity::Warning,
                        item,
                        other,
                        Evidence::Api {
                            source: api.source,
                            url: api.url.clone(),
                            field: relation.field.clone(),
                        },
                        api_ev(other, "project_id"),
                    )),
                    (ApiRelationKind::Required, None) => findings.push(
                        single(
                            "E_MISSING_DEP",
                            Severity::Error,
                            item,
                            Evidence::Api {
                                source: api.source,
                                url: api.url.clone(),
                                field: relation.field.clone(),
                            },
                        )
                        .with_fix(SuggestedFix::AddDependency {
                            mod_id: relation.project_id.clone(),
                            version_range: None,
                        }),
                    ),
                    _ => {}
                }
            }
        }
        if let (Some(actual), Some(expected)) =
            (item.resource_pack_format, input.pack.resource_pack_format)
            && actual != expected
        {
            findings.push(single(
                "I_PACK_FORMAT",
                Severity::Info,
                item,
                installed_ev(
                    item,
                    format!("pack_format = {actual}; esperado = {expected}"),
                ),
            ));
        }
    }

    if let Some(minimum) = input
        .pack
        .minimum_java_major
        .filter(|v| *v > input.pack.java_major)
    {
        findings.push(
            Finding::new(
                code("E_JAVA"),
                Severity::Error,
                key("E_JAVA"),
                Evidence::PackFile {
                    path: "pack.toml".into(),
                    excerpt: format!(
                        "Minecraft {} / {:?} exige Java {minimum}; escolhido {}",
                        input.pack.minecraft, input.pack.loader, input.pack.java_major
                    ),
                },
            )
            .with_fix(SuggestedFix::ChangeJava {
                major: Some(minimum.into()),
            }),
        );
    }
    let mut seen = Vec::new();
    if input.pass == Pass::Complete {
        let connector = input.items.iter().any(|item| {
            item.jar.as_ref().is_some_and(|jar| {
                jar.mods
                    .iter()
                    .any(|m| m.id == "connector" || m.id == "sinytra_connector")
            })
        });
        for item in &input.items {
            let Some(jar) = &item.jar else { continue };
            let optifine = item.filename.to_ascii_lowercase().starts_with("optifine_");
            if optifine {
                ids.entry("optifine".into()).or_default().push((item, None));
            }
            let mods = jar.effective_mods_for_loader(input.pack.loader);
            let fabric_only = matches!(input.pack.loader, Loader::Forge | Loader::NeoForge)
                && !mods.is_empty()
                && mods
                    .iter()
                    .all(|m| m.source == DescriptorKind::FabricModJson);
            let wrong_neoforge_descriptor = input.pack.loader == Loader::NeoForge
                && release_cmp(&input.pack.minecraft, "1.20.2")
                    .is_some_and(std::cmp::Ordering::is_ge)
                && mods.iter().any(|m| m.source == DescriptorKind::ModsToml)
                && !jar.descriptors.contains(&DescriptorKind::NeoForgeModsToml);
            let library = jar
                .manifest
                .as_ref()
                .and_then(|m| m.fml_mod_type.as_deref())
                .is_some_and(|t| matches!(t, "LIBRARY" | "GAMELIBRARY" | "LANGPROVIDER"));
            let coremod = jar
                .manifest
                .as_ref()
                .is_some_and(warden_jarmeta::ManifestInfo::is_coremod);
            if (mods.is_empty()
                && !library
                && !coremod
                && !optifine
                && jar.loader_services.is_empty())
                || (fabric_only && !connector)
                || wrong_neoforge_descriptor
            {
                let desc = mods.first().map(|m| m.source.path());
                let evidence = desc.map_or_else(
                    || installed_ev(item, format!("sem descritor para {:?}", input.pack.loader)),
                    |path| {
                        jar_ev(
                            item,
                            path,
                            format!(
                                "loader do pack: {:?}; descritor encontrado: {path}",
                                input.pack.loader
                            ),
                        )
                    },
                );
                findings.push(single("E_LOADER", Severity::Error, item, evidence));
            }
            if !fabric_only || connector {
                let mut local_ids = HashSet::new();
                for (_, nested) in jar.walk() {
                    for meta in nested.effective_mods_for_loader(input.pack.loader) {
                        if matches!(input.pack.loader, Loader::Forge | Loader::NeoForge)
                            && meta.source == DescriptorKind::FabricModJson
                            && !connector
                        {
                            continue;
                        }
                        if input.pack.loader == Loader::NeoForge
                            && release_cmp(&input.pack.minecraft, "1.20.2")
                                .is_some_and(std::cmp::Ordering::is_ge)
                            && meta.source == DescriptorKind::ModsToml
                        {
                            continue;
                        }
                        if local_ids.insert(meta.id.as_str()) {
                            seen.push(SeenMod { item, meta });
                        }
                    }
                }
            }
            if let Some(major) = jar
                .max_class_version
                .or(jar.class_version)
                .and_then(warden_jarmeta::ClassVersion::java_feature_version)
                && major > input.pack.java_major
            {
                findings.push(
                    single(
                        "E_JAVA",
                        Severity::Error,
                        item,
                        installed_ev(
                            item,
                            format!(
                                "classe Java {major}; Java escolhido {}",
                                input.pack.java_major
                            ),
                        ),
                    )
                    .with_fix(SuggestedFix::ChangeJava {
                        major: Some(major.into()),
                    }),
                );
            }
            if let Some(manifest) = &jar.manifest
                && manifest.is_coremod()
            {
                findings.push(single(
                    "I_LEGACY_COREMOD",
                    Severity::Info,
                    item,
                    Evidence::JarMetadata {
                        item: item_ref(item),
                        file_in_jar: "META-INF/MANIFEST.MF".into(),
                        excerpt: format!(
                            "FMLCorePlugin={:?}; TweakClass={:?}",
                            manifest.fml_core_plugin, manifest.tweak_class
                        ),
                    },
                ));
            }
            if let Some(language) = &jar.language_loader
                && let Some(version) = input.pack.language_loader_versions.get(&language.name)
            {
                let flavor = match input.pack.loader {
                    Loader::Forge => MavenFlavor::for_forge(&input.pack.loader_version),
                    Loader::NeoForge => MavenFlavor::for_neoforge(&input.pack.loader_version),
                    _ => MavenFlavor::default(),
                };
                if matches_range(&language.version, version, flavor) == Some(false) {
                    let descriptor = if jar.descriptors.contains(&DescriptorKind::NeoForgeModsToml)
                    {
                        DescriptorKind::NeoForgeModsToml.path()
                    } else {
                        DescriptorKind::ModsToml.path()
                    };
                    findings.push(single(
                        "E_LOADERVERSION",
                        Severity::Error,
                        item,
                        jar_ev(
                            item,
                            descriptor,
                            format!(
                                "modLoader = {}; loaderVersion = {:?}; instalado = {version}",
                                language.name, language.version
                            ),
                        ),
                    ));
                }
            }
        }
        for m in &seen {
            ids.entry(m.meta.id.to_lowercase())
                .or_default()
                .push((m.item, m.meta.version.as_deref()));
            for provided in &m.meta.provides {
                ids.entry(provided.to_lowercase())
                    .or_default()
                    .push((m.item, m.meta.version.as_deref()));
            }
        }
        let mut declared: HashMap<&str, Vec<&PackItem>> = HashMap::new();
        for m in &seen {
            declared.entry(&m.meta.id).or_default().push(m.item);
        }
        for (id, owners) in &declared {
            let unique: HashSet<&str> = owners.iter().map(|item| item.path.as_str()).collect();
            if unique.len() > 1 {
                let first = owners[0];
                for other in owners.iter().skip(1).filter(|i| i.path != first.path) {
                    findings.push(pair_finding(
                        "E_DUPLICATE_ID",
                        Severity::Error,
                        first,
                        other,
                        jar_ev(first, "mod id", *id),
                        jar_ev(other, "mod id", *id),
                    ));
                }
            }
        }
        for m in &seen {
            inspect_mod(input, m, &ids, &mut findings);
        }
    }

    // Na passagem rápida, IDs fornecidos pelo nome do arquivo são apenas pistas; regras curadas
    // exigem IDs do jar para evitar falsos positivos.
    if input.pass == Pass::Complete {
        for category in &catalogs.categories {
            let matched: Vec<_> = category
                .members
                .iter()
                .filter_map(|id| ids.get(id).and_then(|v| v.first().map(|x| x.0)))
                .collect();
            if matched.len() > 1 {
                let mut finding = pair_finding(
                    "E_EXCLUSIVE",
                    Severity::Error,
                    matched[0],
                    matched[1],
                    Evidence::Curated {
                        rule_id: category.id.clone(),
                        note: category.note.clone(),
                    },
                    jar_ev(matched[1], "mod id", category.members.join(", ")),
                );
                for item in matched.iter().skip(2) {
                    finding = finding.with_item(item_ref(item));
                }
                findings.push(finding);
            }
        }
        for conflict in &catalogs.conflicts {
            let (Some(first), Some(second)) = (
                ids.get(&conflict.first).and_then(|v| v.first()),
                ids.get(&conflict.second).and_then(|v| v.first()),
            ) else {
                continue;
            };
            if let Some(min) = &conflict.second_min_version {
                let Some(version) = second.1 else { continue };
                if warden_jarmeta::version::flexver::compare(version, min).is_lt() {
                    continue;
                }
            }
            findings.push(pair_finding(
                if conflict.severity == "error" {
                    "E_KNOWN_CONFLICT"
                } else {
                    "W_KNOWN_CONFLICT"
                },
                if conflict.severity == "error" {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                first.0,
                second.0,
                Evidence::Curated {
                    rule_id: conflict.id.clone(),
                    note: conflict.note.clone(),
                },
                jar_ev(second.0, "mod id", &conflict.second),
            ));
        }
        for entry in &catalogs.obsolete {
            if entry
                .loader
                .as_deref()
                .is_some_and(|l| l != loader_name(input.pack.loader))
                || !release_cmp(&input.pack.minecraft, &entry.min_minecraft)
                    .is_some_and(std::cmp::Ordering::is_ge)
            {
                continue;
            }
            if let Some((item, _)) = ids.get(&entry.mod_id).and_then(|v| v.first()) {
                let mut finding = single(
                    "W_OBSOLETE",
                    Severity::Warning,
                    item,
                    Evidence::Curated {
                        rule_id: entry.id.clone(),
                        note: entry.note.clone(),
                    },
                );
                if let Some(replacement) = &entry.replacement {
                    finding = finding.with_param("replacement", replacement);
                }
                findings.push(finding);
            }
        }
    }
    Ok(findings)
}

#[allow(clippy::too_many_lines)] // As três faixas e as oito relações usam o mesmo contexto do mod.
fn inspect_mod(
    input: &Input,
    m: &SeenMod<'_>,
    ids: &HashMap<String, Vec<(&PackItem, Option<&str>)>>,
    out: &mut Vec<Finding>,
) {
    let meta = m.meta;
    let flavor = match input.pack.loader {
        Loader::Forge => MavenFlavor::for_forge(&input.pack.loader_version),
        Loader::NeoForge => MavenFlavor::for_neoforge(&input.pack.loader_version),
        _ => MavenFlavor::default(),
    };
    for (range, version, rule) in [
        (
            meta.minecraft.as_ref(),
            for_fabric(
                &input.pack.minecraft,
                input.pack.minecraft_release_target.as_deref(),
            ),
            "E_MCVERSION",
        ),
        (
            meta.loader_version.as_ref(),
            Some(input.pack.loader_version.clone()),
            "E_LOADERVERSION",
        ),
        (
            meta.java.as_ref(),
            Some(input.pack.java_major.to_string()),
            "E_JAVA",
        ),
    ] {
        if let (Some(range), Some(version)) = (range, version)
            && matches_range(range, &version, flavor) == Some(false)
        {
            if rule == "E_MCVERSION"
                && m.item
                    .accepted_minecraft_versions
                    .contains(&input.pack.minecraft)
            {
                continue;
            }
            let acceptable = rule == "E_MCVERSION"
                && input
                    .pack
                    .acceptable_game_versions
                    .iter()
                    .filter_map(|v| for_fabric(v, input.pack.minecraft_release_target.as_deref()))
                    .any(|v| matches_range(range, &v, flavor) == Some(true));
            let mut finding = single(
                rule,
                if acceptable {
                    Severity::Warning
                } else {
                    Severity::Error
                },
                m.item,
                jar_ev(
                    m.item,
                    meta.source.path(),
                    format!("{}: {:?}; recebido {version}", meta.id, range),
                ),
            );
            if rule == "E_JAVA" {
                finding = finding.with_fix(SuggestedFix::ChangeJava { major: None });
            } else {
                finding = finding.with_fix(SuggestedFix::UpdateItem {
                    item: item_ref(m.item),
                });
            }
            out.push(finding);
        }
    }
    if meta.environment == Environment::Client && m.item.side.on_server() {
        out.push(
            single(
                "W_SIDE",
                Severity::Warning,
                m.item,
                jar_ev(m.item, meta.source.path(), "environment: client"),
            )
            .with_fix(SuggestedFix::SetSide {
                item: item_ref(m.item),
                side: SideChoice::Client,
            }),
        );
    }
    if meta.environment == Environment::Server && m.item.side.on_client() {
        out.push(
            single(
                "W_SIDE",
                Severity::Warning,
                m.item,
                jar_ev(m.item, meta.source.path(), "environment: server"),
            )
            .with_fix(SuggestedFix::SetSide {
                item: item_ref(m.item),
                side: SideChoice::Server,
            }),
        );
    }
    for dep in &meta.dependencies {
        if builtin(&dep.id) || !side_applies(dep.side, &m.item.side) {
            continue;
        }
        let present = ids.get(&dep.id.to_lowercase()).and_then(|owners| {
            owners
                .iter()
                .find(|(item, _)| side_applies(dep.side, &item.side))
        });
        let evidence = jar_ev(
            m.item,
            meta.source.path(),
            format!(
                "{} -> {} ({:?}, {:?})",
                meta.id, dep.id, dep.kind, dep.range
            ),
        );
        match (dep.kind, present) {
            (DependencyKind::Required, None) => out.push(
                single("E_MISSING_DEP", Severity::Error, m.item, evidence).with_fix(
                    SuggestedFix::AddDependency {
                        mod_id: dep.id.clone(),
                        version_range: Some(format!("{:?}", dep.range)),
                    },
                ),
            ),
            (DependencyKind::Required | DependencyKind::Optional, Some((other, Some(version))))
                if matches_range(&dep.range, version, flavor) == Some(false) =>
            {
                out.push(
                    pair_finding(
                        "E_DEP_VERSION",
                        Severity::Error,
                        m.item,
                        other,
                        evidence,
                        jar_ev(other, "version", *version),
                    )
                    .with_fix(SuggestedFix::UpdateItem {
                        item: item_ref(other),
                    }),
                );
            }
            (DependencyKind::Breaks | DependencyKind::Incompatible, Some((other, version)))
                if matches!(dep.range, VersionRange::Any)
                    || version
                        .is_some_and(|v| matches_range(&dep.range, v, flavor) == Some(true)) =>
            {
                out.push(
                    pair_finding(
                        "E_BREAKS",
                        Severity::Error,
                        m.item,
                        other,
                        evidence,
                        jar_ev(other, "mod id", &dep.id),
                    )
                    .with_fix(SuggestedFix::RemoveItem {
                        item: item_ref(other),
                    }),
                );
            }
            (DependencyKind::Conflicts | DependencyKind::Discouraged, Some((other, version)))
                if matches!(dep.range, VersionRange::Any)
                    || version
                        .is_some_and(|v| matches_range(&dep.range, v, flavor) == Some(true)) =>
            {
                out.push(pair_finding(
                    "W_CONFLICTS",
                    Severity::Warning,
                    m.item,
                    other,
                    evidence,
                    jar_ev(other, "mod id", &dep.id),
                ));
            }
            (DependencyKind::Recommends, None) => {
                out.push(single("W_CONFLICTS", Severity::Warning, m.item, evidence));
            }
            (DependencyKind::Recommends, Some((other, Some(version))))
                if matches_range(&dep.range, version, flavor) == Some(false) =>
            {
                out.push(pair_finding(
                    "W_CONFLICTS",
                    Severity::Warning,
                    m.item,
                    other,
                    evidence,
                    jar_ev(other, "version", *version),
                ));
            }
            _ => {}
        }
    }
}

fn matches_range(range: &VersionRange, version: &str, flavor: MavenFlavor) -> Option<bool> {
    range.matches_with(version, flavor).ok()
}

fn builtin(id: &str) -> bool {
    matches!(
        id.to_ascii_lowercase().as_str(),
        "minecraft" | "java" | "fabricloader" | "quilt_loader" | "forge" | "neoforge"
    )
}

fn side_applies(side: JarSide, installed: &Side) -> bool {
    match side {
        JarSide::Both => true,
        JarSide::Client => installed.on_client(),
        JarSide::Server => installed.on_server(),
    }
}

fn api_loader_matches(loaders: &[Loader], loader: Loader, minecraft: &str) -> bool {
    loaders.contains(&loader)
        || loader == Loader::NeoForge
            && loaders.contains(&Loader::Forge)
            && release_cmp(minecraft, "1.20.2").is_some_and(std::cmp::Ordering::is_lt)
}

fn loader_name(loader: Loader) -> &'static str {
    match loader {
        Loader::Fabric => "fabric",
        Loader::Quilt => "quilt",
        Loader::Forge => "forge",
        Loader::NeoForge => "neoforge",
    }
}

fn item_ref(item: &PackItem) -> ItemRef {
    ItemRef::Metafile {
        path: item.path.clone(),
    }
}
#[allow(clippy::expect_used)] // Os códigos são literais deste módulo, conferidos pelos testes.
fn code(text: &str) -> RuleCode {
    RuleCode::parse(text).expect("código fixo de regra")
}
fn key(text: &str) -> String {
    format!("diagnostics.pretest.{}", text.to_ascii_lowercase())
}
fn single(rule: &str, severity: Severity, item: &PackItem, evidence: Evidence) -> Finding {
    Finding::new(code(rule), severity, key(rule), evidence).with_item(item_ref(item))
}
fn pair_finding(
    rule: &str,
    severity: Severity,
    first: &PackItem,
    second: &PackItem,
    evidence: Evidence,
    other: Evidence,
) -> Finding {
    single(rule, severity, first, evidence)
        .with_item(item_ref(second))
        .with_evidence(other)
}
fn installed_ev(item: &PackItem, excerpt: impl Into<String>) -> Evidence {
    let path = item.path.rsplit_once('/').map_or_else(
        || item.filename.clone(),
        |(folder, _)| format!("{folder}/{}", item.filename),
    );
    Evidence::PackFile {
        path,
        excerpt: excerpt.into(),
    }
}
#[allow(clippy::expect_used)] // Chamada apenas no ramo que confirmou os metadados de API.
fn api_ev(item: &PackItem, field: &str) -> Evidence {
    let api = item.api.as_ref().expect("chamada só para item com API");
    Evidence::Api {
        source: api.source,
        url: api.url.clone(),
        field: field.into(),
    }
}
fn jar_ev(item: &PackItem, file: &str, excerpt: impl Into<String>) -> Evidence {
    let excerpt = excerpt.into();
    let file = if matches!(file, "mod id" | "version") {
        let Some(meta) = item.jar.as_ref().and_then(|jar| jar.mods.first()) else {
            return installed_ev(item, excerpt);
        };
        meta.source.path()
    } else {
        file
    };
    Evidence::JarMetadata {
        item: item_ref(item),
        file_in_jar: file.into(),
        excerpt,
    }
}
