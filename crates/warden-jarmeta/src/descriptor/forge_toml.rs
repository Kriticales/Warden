//! `META-INF/mods.toml` (Forge 1.13+) e `META-INF/neoforge.mods.toml` (NeoForge 20.5+) (R3 §4.3).
//!
//! Campos conforme o `ModInfo` do FML do Forge 1.20.1 e a documentação do NeoForge:
//!
//! - `[[mods]]`: `modId`, `version` (com `${file.jarVersion}` = `Implementation-Version` do
//!   manifesto), `displayName`, `description`, `authors`;
//! - `[[dependencies.<modId>]]`: `modId`, `versionRange` (Maven; ausente = qualquer), `ordering`,
//!   `side`, `reason`; obrigatoriedade por `type` (NeoForge: `required`, `optional`,
//!   `incompatible`, `discouraged`; padrão `required`) ou `mandatory` (Forge, obrigatório no
//!   arquivo; se faltar, o FML recusa o jar e aqui vira aviso e dependência obrigatória);
//! - `[features.<modId>] javaVersion` (NeoForge), `[[mixins]] config` (NeoForge),
//!   `clientSideOnly` (Forge), `modLoader` e `loaderVersion`.
//!
//! Um `mods.toml` que depende de `neoforge` (e não de `forge`) é de NeoForge 1.20.1 a 20.4: o mod
//! fica marcado com o loader NeoForge.

use toml::{Table, Value};

use super::{Sink, fill_well_known};
use crate::model::{
    Dependency, DependencyKind, DescriptorKind, Environment, LanguageLoader, LoadOrdering, Loader,
    ModMetadata, Side, WarningCode,
};
use crate::range::VersionRange;

/// Resultado da leitura.
pub(crate) struct ForgeToml {
    pub(crate) mods: Vec<ModMetadata>,
    pub(crate) language_loader: Option<LanguageLoader>,
}

fn text(table: &Table, key: &str) -> Option<String> {
    table.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Booleano tolerante: `true`, ou o texto `"true"` que alguns autores escrevem.
fn boolean(table: &Table, key: &str, sink: &mut Sink<'_>, owner: &str) -> Option<bool> {
    match table.get(key)? {
        Value::Boolean(b) => Some(*b),
        Value::String(s) if s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("false") => {
            sink.warn(
                WarningCode::InvalidField,
                format!("{owner}: '{key}' é texto, não booleano"),
            );
            Some(s.eq_ignore_ascii_case("true"))
        }
        other => {
            sink.warn(
                WarningCode::InvalidField,
                format!("{owner}: '{key}' com valor inesperado {other}"),
            );
            None
        }
    }
}

/// Troca `${file.jarVersion}` pela versão do manifesto. Outros marcadores ficam e geram aviso.
fn resolve_version(
    raw: &str,
    jar_version: Option<&str>,
    sink: &mut Sink<'_>,
    owner: &str,
) -> String {
    let mut out = raw.to_owned();
    if let Some(v) = jar_version {
        out = out.replace("${file.jarVersion}", v);
    }
    if out.contains("${") {
        sink.warn(
            WarningCode::UnresolvedPlaceholder,
            format!("{owner}: versão '{raw}' com marcador sem valor"),
        );
    }
    out
}

pub(crate) fn parse(
    text_content: &str,
    kind: DescriptorKind,
    jar_version: Option<&str>,
    sink: &mut Sink<'_>,
) -> Option<ForgeToml> {
    let root: Table = match toml::from_str(text_content) {
        Ok(t) => t,
        Err(e) => {
            sink.warn(WarningCode::InvalidToml, e.to_string().trim().to_owned());
            return None;
        }
    };
    let language_loader = text(&root, "modLoader").map(|name| {
        let version = text(&root, "loaderVersion").map_or(VersionRange::Any, VersionRange::maven);
        sink.check_range("loaderVersion", &version);
        LanguageLoader { name, version }
    });
    let client_side_only = boolean(&root, "clientSideOnly", sink, "raiz").unwrap_or(false);
    let licenses: Vec<String> = text(&root, "license").into_iter().collect();
    let mixins: Vec<String> = root
        .get("mixins")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|m| m.as_table().and_then(|t| text(t, "config")))
                .collect()
        })
        .unwrap_or_default();
    let dependencies = root.get("dependencies").and_then(Value::as_table);
    let features = root.get("features").and_then(Value::as_table);

    let Some(mods) = root.get("mods").and_then(Value::as_array) else {
        sink.warn(WarningCode::MissingField, "falta a lista [[mods]]");
        return Some(ForgeToml {
            mods: Vec::new(),
            language_loader,
        });
    };
    let context = FileContext {
        kind,
        jar_version,
        client_side_only,
        licenses: &licenses,
        mixins: &mixins,
        dependencies,
        features,
    };
    let mut out = Vec::new();
    for (index, entry) in mods.iter().enumerate() {
        if let Some(module) = read_mod(entry, index, &context, sink) {
            out.push(module);
        }
    }
    let known_ids: Vec<&String> = out.iter().map(|m| &m.id).collect();
    if let Some(deps) = dependencies {
        let mut orphans: Vec<&String> = deps.keys().filter(|k| !known_ids.contains(k)).collect();
        orphans.sort();
        for orphan in orphans {
            sink.warn(
                WarningCode::InvalidField,
                format!("dependencies.{orphan} não corresponde a nenhum [[mods]] (ignorado)"),
            );
        }
    }
    Some(ForgeToml {
        mods: out,
        language_loader,
    })
}

/// O que vale para todos os `[[mods]]` do arquivo.
struct FileContext<'a> {
    kind: DescriptorKind,
    jar_version: Option<&'a str>,
    client_side_only: bool,
    licenses: &'a [String],
    mixins: &'a [String],
    dependencies: Option<&'a Table>,
    features: Option<&'a Table>,
}

fn read_mod(
    entry: &Value,
    index: usize,
    context: &FileContext<'_>,
    sink: &mut Sink<'_>,
) -> Option<ModMetadata> {
    let neoforge_file = context.kind == DescriptorKind::NeoForgeModsToml;
    let Some(table) = entry.as_table() else {
        sink.warn(
            WarningCode::InvalidField,
            format!("[[mods]] #{index} não é tabela"),
        );
        return None;
    };
    let Some(id) = text(table, "modId") else {
        sink.warn(
            WarningCode::MissingField,
            format!("[[mods]] #{index} sem 'modId'"),
        );
        return None;
    };
    let version = text(table, "version")
        .map(|raw| resolve_version(&raw, context.jar_version, sink, &format!("mods.{id}")));
    let authors = match table.get("authors") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };
    let mut module = ModMetadata {
        source: context.kind,
        loader: if neoforge_file {
            Loader::NeoForge
        } else {
            Loader::Forge
        },
        id: id.clone(),
        version,
        name: text(table, "displayName"),
        description: text(table, "description").map(|d| d.trim().to_owned()),
        authors,
        licenses: context.licenses.to_vec(),
        provides: Vec::new(),
        environment: if context.client_side_only {
            Environment::Client
        } else {
            Environment::Both
        },
        dependencies: Vec::new(),
        minecraft: None,
        loader_version: None,
        java: None,
        mixins: context.mixins.to_vec(),
    };
    if let Some(list) = context.dependencies.and_then(|d| d.get(&id)) {
        match list.as_array() {
            Some(items) => {
                for item in items {
                    if let Some(dep) = dependency(item, &id, neoforge_file, sink) {
                        module.dependencies.push(dep);
                    }
                }
            }
            None => sink.warn(
                WarningCode::InvalidField,
                format!("dependencies.{id} não é uma lista de tabelas"),
            ),
        }
    }
    if let Some(java) = context
        .features
        .and_then(|f| f.get(&id))
        .and_then(Value::as_table)
        .and_then(|f| text(f, "javaVersion"))
    {
        let range = VersionRange::maven(java);
        sink.check_range(&format!("features.{id}.javaVersion"), &range);
        module.java = Some(range);
    }
    let depends_on = |name: &str| module.dependencies.iter().any(|d| d.id == name);
    if !neoforge_file && depends_on("neoforge") && !depends_on("forge") {
        module.loader = Loader::NeoForge;
    }
    fill_well_known(&mut module, &["forge", "neoforge"]);
    Some(module)
}

fn dependency(
    item: &Value,
    owner: &str,
    neoforge_file: bool,
    sink: &mut Sink<'_>,
) -> Option<Dependency> {
    let Some(table) = item.as_table() else {
        sink.warn(
            WarningCode::InvalidField,
            format!("dependencies.{owner}: item não é tabela"),
        );
        return None;
    };
    let Some(id) = text(table, "modId") else {
        sink.warn(
            WarningCode::MissingField,
            format!("dependencies.{owner}: item sem 'modId'"),
        );
        return None;
    };
    let label = format!("dependencies.{owner}.{id}");
    let kind = if let Some(kind) = text(table, "type") {
        match kind.to_ascii_lowercase().as_str() {
            "required" => DependencyKind::Required,
            "optional" => DependencyKind::Optional,
            "incompatible" => DependencyKind::Incompatible,
            "discouraged" => DependencyKind::Discouraged,
            other => {
                sink.warn(
                    WarningCode::InvalidField,
                    format!("{label}: type '{other}' desconhecido; tratado como obrigatória"),
                );
                DependencyKind::Required
            }
        }
    } else if let Some(mandatory) = boolean(table, "mandatory", sink, &label) {
        if mandatory {
            DependencyKind::Required
        } else {
            DependencyKind::Optional
        }
    } else {
        if !neoforge_file {
            sink.warn(
                WarningCode::MissingField,
                format!(
                    "{label}: falta 'mandatory' (o Forge recusa o jar); tratado como obrigatória"
                ),
            );
        }
        DependencyKind::Required
    };
    let range = text(table, "versionRange").map_or(VersionRange::Any, VersionRange::maven);
    sink.check_range(&label, &range);
    let side = match text(table, "side")
        .map(|s| s.to_ascii_uppercase())
        .as_deref()
    {
        None | Some("BOTH") => Side::Both,
        Some("CLIENT") => Side::Client,
        Some("SERVER") => Side::Server,
        Some(other) => {
            sink.warn(
                WarningCode::InvalidField,
                format!("{label}: side '{other}' desconhecido"),
            );
            Side::Both
        }
    };
    let ordering = match text(table, "ordering")
        .map(|s| s.to_ascii_uppercase())
        .as_deref()
    {
        None | Some("NONE") => LoadOrdering::None,
        Some("BEFORE") => LoadOrdering::Before,
        Some("AFTER") => LoadOrdering::After,
        Some(other) => {
            sink.warn(
                WarningCode::InvalidField,
                format!("{label}: ordering '{other}' desconhecido"),
            );
            LoadOrdering::None
        }
    };
    Some(Dependency {
        id,
        kind,
        range,
        side,
        ordering,
        reason: text(table, "reason"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Warning;

    fn read(
        text: &str,
        kind: DescriptorKind,
        jar_version: Option<&str>,
    ) -> (Option<ForgeToml>, Vec<Warning>) {
        let mut warnings = Vec::new();
        let mut sink = Sink {
            path: kind.path(),
            warnings: &mut warnings,
        };
        (parse(text, kind, jar_version, &mut sink), warnings)
    }

    #[test]
    fn mods_toml_do_forge_1_20_1() {
        let (parsed, warnings) = read(
            r#"
modLoader = "javafml"
loaderVersion = "[47,)"
license = "MIT"
[[mods]]
modId = "examplemod"
version = "${file.jarVersion}"
displayName = "Example"
authors = "Alguém"
description = '''
Várias linhas.
'''
[[mods]]
modId = "second"
version = "2.0"
[[dependencies.examplemod]]
modId = "forge"
mandatory = true
versionRange = "[47.1.0,)"
ordering = "NONE"
side = "BOTH"
[[dependencies.examplemod]]
modId = "minecraft"
mandatory = true
versionRange = "[1.20.1,1.21)"
[[dependencies.examplemod]]
modId = "jei"
mandatory = false
ordering = "AFTER"
side = "CLIENT"
"#,
            DescriptorKind::ModsToml,
            Some("1.4.2"),
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        let parsed = parsed.expect("lido");
        assert_eq!(
            parsed.language_loader,
            Some(LanguageLoader {
                name: "javafml".into(),
                version: VersionRange::maven("[47,)")
            })
        );
        let m = &parsed.mods[0];
        assert_eq!(m.loader, Loader::Forge);
        assert_eq!(m.version.as_deref(), Some("1.4.2"));
        assert_eq!(m.description.as_deref(), Some("Várias linhas."));
        assert_eq!(m.authors, ["Alguém"]);
        assert_eq!(m.licenses, ["MIT"]);
        assert_eq!(m.minecraft, Some(VersionRange::maven("[1.20.1,1.21)")));
        assert_eq!(m.loader_version, Some(VersionRange::maven("[47.1.0,)")));
        let jei = &m.dependencies[2];
        assert_eq!(
            (jei.kind, jei.side, jei.ordering),
            (DependencyKind::Optional, Side::Client, LoadOrdering::After)
        );
        assert_eq!(jei.range, VersionRange::Any);
        assert_eq!(parsed.mods[1].id, "second");
        assert!(parsed.mods[1].dependencies.is_empty());
    }

    #[test]
    fn neoforge_mods_toml() {
        let (parsed, warnings) = read(
            r#"
modLoader = "javafml"
loaderVersion = "[4,)"
license = "LGPL-3.0"
[[mods]]
modId = "sodium"
version = "0.6.0"
[[mixins]]
config = "sodium.mixins.json"
[[mixins]]
config = "sodium-neoforge.mixins.json"
[features.sodium]
javaVersion = "[21,)"
[[dependencies.sodium]]
modId = "neoforge"
type = "required"
versionRange = "[21.1.0,)"
[[dependencies.sodium]]
modId = "embeddium"
type = "incompatible"
reason = "Mesmo papel."
[[dependencies.sodium]]
modId = "oculus"
type = "DISCOURAGED"
"#,
            DescriptorKind::NeoForgeModsToml,
            None,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        let m = &parsed.expect("lido").mods[0];
        assert_eq!(m.loader, Loader::NeoForge);
        assert_eq!(
            m.mixins,
            ["sodium.mixins.json", "sodium-neoforge.mixins.json"]
        );
        assert_eq!(m.java, Some(VersionRange::maven("[21,)")));
        assert_eq!(m.loader_version, Some(VersionRange::maven("[21.1.0,)")));
        assert_eq!(m.dependencies[1].kind, DependencyKind::Incompatible);
        assert_eq!(m.dependencies[1].reason.as_deref(), Some("Mesmo papel."));
        assert_eq!(m.dependencies[2].kind, DependencyKind::Discouraged);
    }

    #[test]
    fn mods_toml_de_neoforge_1_20_1_e_cliente() {
        let (parsed, _) = read(
            r#"
modLoader = "javafml"
loaderVersion = "[1,)"
clientSideOnly = true
[[mods]]
modId = "x"
[[dependencies.x]]
modId = "neoforge"
mandatory = true
"#,
            DescriptorKind::ModsToml,
            None,
        );
        let m = &parsed.expect("lido").mods[0];
        assert_eq!(m.loader, Loader::NeoForge);
        assert_eq!(m.environment, Environment::Client);
        assert_eq!(m.version, None);
    }

    #[test]
    fn problemas_viram_avisos() {
        let (parsed, warnings) = read(
            r#"
modLoader = "javafml"
loaderVersion = "[47"
clientSideOnly = "true"
mods = [{ modId = "a", version = "${file.jarVersion}" }, { displayName = "sem id" }, 5]
[dependencies]
a = [{ modId = "b", versionRange = "(1.0)", mandatory = "sim" },
     { modId = "c", side = "WEIRD", ordering = "SIDEWAYS", type = "maybe" },
     { modId = "d" }, { versionRange = "1" }, 7]
fantasma = []
"#,
            DescriptorKind::ModsToml,
            None,
        );
        let parsed = parsed.expect("lido");
        assert_eq!(parsed.mods.len(), 1);
        assert_eq!(parsed.mods[0].environment, Environment::Client);
        assert_eq!(parsed.mods[0].dependencies.len(), 3);
        let codes: Vec<WarningCode> = warnings.iter().map(|w| w.code).collect();
        assert_eq!(
            codes
                .iter()
                .filter(|c| **c == WarningCode::InvalidVersionRange)
                .count(),
            2,
            "{warnings:?}"
        );
        assert!(codes.contains(&WarningCode::UnresolvedPlaceholder));
        assert_eq!(codes.len(), 15, "{warnings:?}");
    }

    #[test]
    fn toml_invalido_ou_sem_mods() {
        let (parsed, warnings) = read("modId = = 1", DescriptorKind::ModsToml, None);
        assert!(parsed.is_none());
        assert_eq!(warnings[0].code, WarningCode::InvalidToml);
        let (parsed, warnings) = read("modLoader = \"lowcodefml\"", DescriptorKind::ModsToml, None);
        let parsed = parsed.expect("lido");
        assert!(parsed.mods.is_empty());
        assert_eq!(
            parsed.language_loader.map(|l| l.version),
            Some(VersionRange::Any)
        );
        assert_eq!(warnings[0].code, WarningCode::MissingField);
        let (_, warnings) = read(
            "[[mods]]\nmodId = \"a\"\n[dependencies]\na = 1",
            DescriptorKind::ModsToml,
            None,
        );
        assert_eq!(warnings.len(), 1);
    }
}
