//! `quilt.mod.json` (R3 §4.2; RFC 0002 do Quilt,
//! <https://github.com/QuiltMC/rfcs/blob/main/specification/0002-quilt.mod.json.md>).
//!
//! O Warden só identifica mods Quilt (ARCHITECTURE §3): lê id, versão, nomes, ambiente,
//! `depends`, `breaks`, `provides`, `jars` e `mixin`. As faixas usam a mesma sintaxe de
//! operadores do Fabric e são avaliadas pelo dialeto do Fabric; `{"all": [...]}` vira um
//! predicado com os termos juntos (E), `{"any": [...]}` ou lista vira OU. O campo `unless` e a
//! dependência dada como lista (qualquer uma serve) não são avaliados: geram aviso.

use serde_json::Value;

use super::fabric::people;
use super::{Sink, fill_well_known, string, strings};
use crate::model::{
    Dependency, DependencyKind, DescriptorKind, Environment, LoadOrdering, Loader, ModMetadata,
    Side, WarningCode,
};
use crate::range::VersionRange;

/// Resultado da leitura do `quilt.mod.json`.
pub(crate) struct QuiltMod {
    pub(crate) module: ModMetadata,
    pub(crate) jars: Vec<String>,
}

pub(crate) fn parse(text: &str, sink: &mut Sink<'_>) -> Option<QuiltMod> {
    let value = sink.json(text)?;
    let Some(loader) = value.get("quilt_loader").and_then(Value::as_object) else {
        sink.warn(WarningCode::MissingField, "falta o objeto 'quilt_loader'");
        return None;
    };
    let Some(id) = string(loader.get("id")) else {
        sink.warn(WarningCode::MissingField, "falta 'quilt_loader.id'");
        return None;
    };
    let metadata = loader.get("metadata");
    let mut module = ModMetadata {
        source: DescriptorKind::QuiltModJson,
        loader: Loader::Quilt,
        id,
        version: string(loader.get("version")),
        name: string(metadata.and_then(|m| m.get("name"))),
        description: string(metadata.and_then(|m| m.get("description"))),
        authors: contributors(metadata.and_then(|m| m.get("contributors"))),
        licenses: licenses(metadata.and_then(|m| m.get("license"))),
        provides: provides(loader.get("provides")),
        environment: match value
            .get("minecraft")
            .and_then(|m| m.get("environment"))
            .and_then(Value::as_str)
        {
            Some("client") => Environment::Client,
            Some("dedicated_server") => Environment::Server,
            _ => Environment::Both,
        },
        dependencies: Vec::new(),
        minecraft: None,
        loader_version: None,
        java: None,
        mixins: strings(value.get("mixin")),
    };
    for (key, kind) in [
        ("depends", DependencyKind::Required),
        ("breaks", DependencyKind::Breaks),
    ] {
        let Some(list) = loader.get(key) else {
            continue;
        };
        let Some(items) = list.as_array() else {
            sink.warn(
                WarningCode::InvalidField,
                format!("'{key}' não é uma lista"),
            );
            continue;
        };
        for item in items {
            if let Some(dep) = dependency(item, key, kind, sink) {
                module.dependencies.push(dep);
            }
        }
    }
    fill_well_known(&mut module, &["quilt_loader"]);
    let jars = strings(loader.get("jars"));
    Some(QuiltMod { module, jars })
}

fn contributors(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Object(map)) => {
            let mut names: Vec<String> = map.keys().cloned().collect();
            names.sort();
            names
        }
        other => people(other),
    }
}

fn licenses(value: Option<&Value>) -> Vec<String> {
    let one = |v: &Value| match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => string(o.get("id")),
        _ => None,
    };
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(one).collect(),
        Some(v) => one(v).into_iter().collect(),
        None => Vec::new(),
    }
}

fn provides(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match item {
            Value::String(s) => Some(strip_group(s)),
            Value::Object(o) => string(o.get("id")).map(|s| strip_group(&s)),
            _ => None,
        })
        .collect()
}

/// `grupo:modid` vira `modid`.
fn strip_group(id: &str) -> String {
    id.rsplit(':').next().unwrap_or(id).to_owned()
}

fn versions(value: Option<&Value>) -> Option<VersionRange> {
    let texts = |items: &Vec<Value>| -> Option<Vec<String>> {
        items
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect()
    };
    match value {
        None => Some(VersionRange::Any),
        Some(Value::String(s)) => Some(VersionRange::fabric(s.clone())),
        Some(Value::Array(items)) => {
            texts(items).map(|predicates| VersionRange::Fabric { predicates })
        }
        Some(Value::Object(o)) => {
            if let Some(Value::Array(all)) = o.get("all") {
                texts(all).map(|terms| VersionRange::fabric(terms.join(" ")))
            } else if let Some(Value::Array(any)) = o.get("any") {
                texts(any).map(|predicates| VersionRange::Fabric { predicates })
            } else {
                None
            }
        }
        Some(_) => None,
    }
}

fn dependency(
    item: &Value,
    key: &str,
    kind: DependencyKind,
    sink: &mut Sink<'_>,
) -> Option<Dependency> {
    let (id, range, optional, reason) = match item {
        Value::String(s) => (strip_group(s), VersionRange::Any, false, None),
        Value::Object(o) => {
            let Some(id) = string(o.get("id")) else {
                sink.warn(
                    WarningCode::MissingField,
                    format!("item de '{key}' sem 'id'"),
                );
                return None;
            };
            let Some(range) = versions(o.get("versions")) else {
                sink.warn(
                    WarningCode::InvalidField,
                    format!("'{key}.{id}': 'versions' em formato não suportado"),
                );
                return None;
            };
            if o.contains_key("unless") {
                sink.warn(
                    WarningCode::InvalidField,
                    format!("'{key}.{id}': 'unless' não é avaliado pelo Warden"),
                );
            }
            let optional = o.get("optional").and_then(Value::as_bool).unwrap_or(false);
            (strip_group(&id), range, optional, string(o.get("reason")))
        }
        _ => {
            sink.warn(
                WarningCode::InvalidField,
                format!("item de '{key}' em formato não suportado (lista de alternativas?)"),
            );
            return None;
        }
    };
    sink.check_range(&format!("{key}.{id}"), &range);
    let kind = if optional && kind == DependencyKind::Required {
        DependencyKind::Optional
    } else {
        kind
    };
    Some(Dependency {
        id,
        kind,
        range,
        side: Side::Both,
        ordering: LoadOrdering::None,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Warning;

    fn read(text: &str) -> (Option<QuiltMod>, Vec<Warning>) {
        let mut warnings = Vec::new();
        let mut sink = Sink {
            path: "quilt.mod.json",
            warnings: &mut warnings,
        };
        (parse(text, &mut sink), warnings)
    }

    #[test]
    fn descritor_completo() {
        let (parsed, warnings) = read(
            r#"{"schema_version": 1,
              "quilt_loader": {"group": "org.quiltmc", "id": "qsl", "version": "6.1.2",
                "metadata": {"name": "QSL", "description": "d",
                             "contributors": {"Zé": "Owner", "Ana": "Dev"},
                             "license": [{"id": "Apache-2.0"}, "MIT", 1]},
                "provides": ["org.quiltmc:fabric-api", {"id": "x", "version": "1"}, 2],
                "jars": ["META-INF/jars/a.jar"],
                "depends": ["minecraft", {"id": "quilt_loader", "versions": ">=0.19"},
                            {"id": "org.quiltmc:qfapi", "versions": {"all": [">=1", "<2"]},
                             "optional": true, "reason": "compat"},
                            {"id": "java", "versions": {"any": ["17", ">=21"]}},
                            {"id": "w", "versions": [">=1"], "unless": "z"}],
                "breaks": [{"id": "sodium", "versions": "<0.5"}]},
              "mixin": ["qsl.mixins.json"],
              "minecraft": {"environment": "dedicated_server"}}"#,
        );
        let parsed = parsed.expect("lido");
        let m = parsed.module;
        assert_eq!(m.id, "qsl");
        assert_eq!(m.authors, ["Ana", "Zé"]);
        assert_eq!(m.licenses, ["Apache-2.0", "MIT"]);
        assert_eq!(m.provides, ["fabric-api", "x"]);
        assert_eq!(m.environment, Environment::Server);
        assert_eq!(m.mixins, ["qsl.mixins.json"]);
        assert_eq!(parsed.jars, ["META-INF/jars/a.jar"]);
        assert_eq!(m.dependencies.len(), 6);
        assert_eq!(m.dependencies[2].id, "qfapi");
        assert_eq!(m.dependencies[2].kind, DependencyKind::Optional);
        assert_eq!(m.dependencies[2].range, VersionRange::fabric(">=1 <2"));
        assert_eq!(m.dependencies[2].reason.as_deref(), Some("compat"));
        assert_eq!(m.minecraft, Some(VersionRange::Any));
        assert_eq!(m.loader_version, Some(VersionRange::fabric(">=0.19")));
        assert!(m.java.is_some());
        assert_eq!(m.dependencies[5].kind, DependencyKind::Breaks);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn formatos_nao_suportados_viram_aviso() {
        let (parsed, warnings) = read(
            r#"{"quilt_loader": {"id": "x", "depends": [[ "a", "b" ], {"versions": "1"},
                {"id": "c", "versions": {"none": []}}, {"id": "d", "versions": 5},
                {"id": "e", "versions": ">=1.x"}], "breaks": {}},
              "minecraft": {"environment": "client"}}"#,
        );
        let m = parsed.expect("lido").module;
        assert_eq!(m.environment, Environment::Client);
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(warnings.len(), 6, "{warnings:?}");
        let (parsed, warnings) = read(r#"{"quilt_loader": {}}"#);
        assert!(parsed.is_none());
        assert_eq!(warnings[0].code, WarningCode::MissingField);
        let (parsed, _) = read(r#"{"schema_version": 1}"#);
        assert!(parsed.is_none());
    }
}
