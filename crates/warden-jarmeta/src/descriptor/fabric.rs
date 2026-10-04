//! `fabric.mod.json` (R3 §4.1; <https://wiki.fabricmc.net/documentation:fabric_mod_json_spec>).
//!
//! Campos conforme o `V1ModMetadataParser` e o `V0ModMetadataParser` do fabric-loader: no
//! esquema 1, `depends`/`recommends`/`suggests`/`conflicts`/`breaks` com predicado único ou
//! lista (OU); `requires` é ignorado com aviso. No esquema 0 (sem `schemaVersion`), `requires`
//! vira obrigatória, `conflicts` vira `breaks` e `recommends` vira `suggests`, como no loader.

use serde_json::{Map, Value};

use super::{Sink, fill_well_known, string, strings};
use crate::model::{
    Dependency, DependencyKind, DescriptorKind, Environment, Loader, ModMetadata, WarningCode,
};
use crate::range::VersionRange;

/// Resultado da leitura do `fabric.mod.json`.
pub(crate) struct FabricMod {
    pub(crate) module: ModMetadata,
    /// Caminhos dos jars embutidos (`jars[].file`).
    pub(crate) jars: Vec<String>,
}

pub(crate) fn parse(text: &str, sink: &mut Sink<'_>) -> Option<FabricMod> {
    let value = sink.json(text)?;
    let Some(root) = value.as_object() else {
        sink.warn(WarningCode::InvalidField, "a raiz não é um objeto");
        return None;
    };
    let Some(id) = string(root.get("id")) else {
        sink.warn(WarningCode::MissingField, "falta o campo 'id'");
        return None;
    };
    let schema = root.get("schemaVersion").and_then(Value::as_u64);
    match (root.get("schemaVersion"), schema) {
        (None, _) | (_, Some(1)) => {}
        (Some(raw), _) => sink.warn(
            WarningCode::InvalidField,
            format!("schemaVersion {raw} desconhecido; lido como esquema 1"),
        ),
    }
    let legacy = root.get("schemaVersion").is_none();
    let version = string(root.get("version"));
    if version.is_none() {
        sink.warn(WarningCode::MissingField, "falta o campo 'version'");
    }
    let mut module = ModMetadata {
        source: DescriptorKind::FabricModJson,
        loader: Loader::Fabric,
        id,
        version,
        name: string(root.get("name")),
        description: string(root.get("description")),
        authors: people(root.get("authors")),
        licenses: strings(root.get("license")),
        provides: strings(root.get("provides")),
        environment: Environment::Both,
        dependencies: Vec::new(),
        minecraft: None,
        loader_version: None,
        java: None,
        mixins: Vec::new(),
    };
    let mut jars = Vec::new();
    if legacy {
        module.environment = match root.get("side").and_then(Value::as_str) {
            Some("client") => Environment::Client,
            Some("server") => Environment::Server,
            _ => Environment::Both,
        };
        for (key, kind) in [
            ("requires", DependencyKind::Required),
            ("conflicts", DependencyKind::Breaks),
            ("recommends", DependencyKind::Suggests),
        ] {
            read_dependencies(root, key, kind, &mut module, sink);
        }
        module.mixins = legacy_mixins(root.get("mixins"));
    } else {
        module.environment = environment(root.get("environment"), sink);
        for (key, kind) in [
            ("depends", DependencyKind::Required),
            ("recommends", DependencyKind::Recommends),
            ("suggests", DependencyKind::Suggests),
            ("conflicts", DependencyKind::Conflicts),
            ("breaks", DependencyKind::Breaks),
        ] {
            read_dependencies(root, key, kind, &mut module, sink);
        }
        if root.contains_key("requires") {
            sink.warn(
                WarningCode::InvalidField,
                "'requires' não existe no esquema 1 e é ignorado pelo Fabric",
            );
        }
        module.mixins = mixins(root.get("mixins"));
        jars = nested_jars(root.get("jars"), sink);
    }
    fill_well_known(&mut module, &["fabricloader"]);
    Some(FabricMod { module, jars })
}

/// `authors`: textos ou objetos `{ "name": ... }`.
pub(crate) fn people(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return strings(value);
    };
    items
        .iter()
        .filter_map(|item| match item {
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => string(o.get("name")),
            _ => None,
        })
        .collect()
}

fn environment(value: Option<&Value>, sink: &mut Sink<'_>) -> Environment {
    let values: Vec<String> = strings(value);
    let has = |s: &str| values.iter().any(|v| v == s);
    if values.is_empty() || has("*") || (has("client") && has("server")) {
        if value.is_some() && values.is_empty() {
            sink.warn(
                WarningCode::InvalidField,
                "'environment' com tipo inesperado",
            );
        }
        Environment::Both
    } else if has("client") {
        Environment::Client
    } else if has("server") {
        Environment::Server
    } else {
        sink.warn(
            WarningCode::InvalidField,
            format!("'environment' desconhecido: {}", values.join(", ")),
        );
        Environment::Both
    }
}

/// Predicados de uma dependência: texto (um predicado) ou lista (OU).
fn predicates(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::String(s) => Some(vec![s.clone()]),
        Value::Array(items) => items
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => None,
    }
}

fn read_dependencies(
    root: &Map<String, Value>,
    key: &str,
    kind: DependencyKind,
    module: &mut ModMetadata,
    sink: &mut Sink<'_>,
) {
    let Some(container) = root.get(key) else {
        return;
    };
    let Some(container) = container.as_object() else {
        sink.warn(
            WarningCode::InvalidField,
            format!("'{key}' não é um objeto"),
        );
        return;
    };
    // Ordem por id: a ordem dos mapas do serde_json muda com a feature `preserve_order`, que
    // outras crates do workspace podem ligar.
    let mut entries: Vec<(&String, &Value)> = container.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    for (id, value) in entries {
        let Some(predicates) = predicates(value) else {
            sink.warn(
                WarningCode::InvalidField,
                format!("'{key}.{id}': a faixa precisa ser texto ou lista de textos"),
            );
            continue;
        };
        let range = VersionRange::Fabric { predicates };
        sink.check_range(&format!("{key}.{id}"), &range);
        module.dependencies.push(Dependency {
            id: id.clone(),
            kind,
            range,
            side: crate::model::Side::Both,
            ordering: crate::model::LoadOrdering::None,
            reason: None,
        });
    }
}

/// `mixins`: textos ou objetos `{ "config": ..., "environment": ... }`.
fn mixins(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match item {
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => string(o.get("config")),
            _ => None,
        })
        .collect()
}

/// Esquema 0: `mixins` é `{ "client": ..., "common": ..., "server": ... }`.
fn legacy_mixins(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Object(o)) = value else {
        return Vec::new();
    };
    ["client", "common", "server"]
        .iter()
        .flat_map(|side| strings(o.get(*side)))
        .collect()
}

fn nested_jars(value: Option<&Value>, sink: &mut Sink<'_>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(items) = value.as_array() else {
        sink.warn(WarningCode::InvalidField, "'jars' não é uma lista");
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let file = item.as_object().and_then(|o| string(o.get("file")));
            if file.is_none() {
                sink.warn(WarningCode::InvalidField, "item de 'jars' sem 'file'");
            }
            file
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Warning;

    fn read(text: &str) -> (Option<FabricMod>, Vec<Warning>) {
        let mut warnings = Vec::new();
        let mut sink = Sink {
            path: "fabric.mod.json",
            warnings: &mut warnings,
        };
        let parsed = parse(text, &mut sink);
        (parsed, warnings)
    }

    #[test]
    fn esquema_1_completo() {
        let (parsed, warnings) = read(
            r#"{
              "schemaVersion": 1, "id": "sodium", "version": "0.5.13+mc1.20.1",
              "name": "Sodium", "description": "Rápido",
              "authors": ["JellySquid", {"name": "IMS", "contact": {}}, 3],
              "license": ["LGPL-3.0-only"], "provides": ["indium"],
              "environment": "client",
              "mixins": ["sodium.mixins.json", {"config": "x.json", "environment": "client"}, 1],
              "jars": [{"file": "META-INF/jars/a.jar"}, {"nada": 1}],
              "depends": {"fabricloader": ">=0.12.0", "minecraft": ["1.20", "1.20.1"],
                          "java": ">=17", "fabric-api": "*"},
              "recommends": {"modmenu": "*"}, "suggests": {"x": "*"},
              "conflicts": {"optifabric": "*"}, "breaks": {"iris": "<1.6"},
              "custom": {"lithium:options": {}}
            }"#,
        );
        let parsed = parsed.expect("lido");
        let m = parsed.module;
        assert_eq!(m.id, "sodium");
        assert_eq!(m.authors, ["JellySquid", "IMS"]);
        assert_eq!(m.environment, Environment::Client);
        assert_eq!(m.mixins, ["sodium.mixins.json", "x.json"]);
        assert_eq!(parsed.jars, ["META-INF/jars/a.jar"]);
        assert_eq!(m.dependencies.len(), 8);
        assert_eq!(
            m.minecraft,
            Some(VersionRange::Fabric {
                predicates: vec!["1.20".into(), "1.20.1".into()]
            })
        );
        assert_eq!(m.loader_version, Some(VersionRange::fabric(">=0.12.0")));
        assert_eq!(m.java, Some(VersionRange::fabric(">=17")));
        let kinds: Vec<_> = m.dependencies.iter().map(|d| d.kind).collect();
        assert!(kinds.contains(&DependencyKind::Breaks));
        assert!(kinds.contains(&DependencyKind::Conflicts));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0].code, WarningCode::InvalidField);
    }

    #[test]
    fn esquema_0_legado() {
        let (parsed, warnings) = read(
            r#"{"id": "old", "version": "1.0", "side": "client",
                "requires": {"fabric": "*"}, "conflicts": {"x": "*"}, "recommends": {"y": "*"},
                "mixins": {"client": "a.json", "common": ["b.json"]}}"#,
        );
        let m = parsed.expect("lido").module;
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(m.environment, Environment::Client);
        assert_eq!(m.mixins, ["a.json", "b.json"]);
        let kinds: Vec<_> = m
            .dependencies
            .iter()
            .map(|d| (d.id.as_str(), d.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                ("fabric", DependencyKind::Required),
                ("x", DependencyKind::Breaks),
                ("y", DependencyKind::Suggests)
            ]
        );
        let (parsed, _) = read(r#"{"id": "s", "side": "server"}"#);
        assert_eq!(
            parsed.expect("lido").module.environment,
            Environment::Server
        );
    }

    #[test]
    fn campos_ruins_viram_avisos() {
        let (parsed, warnings) = read(
            r#"{"schemaVersion": 2, "id": "x", "environment": 5, "requires": {},
                "depends": {"a": 5, "b": ">=1.x"}, "breaks": [], "jars": {}}"#,
        );
        let m = parsed.expect("lido").module;
        assert_eq!(
            m.dependencies.len(),
            1,
            "só a dependência com faixa de texto"
        );
        let codes: Vec<_> = warnings.iter().map(|w| w.code).collect();
        assert!(codes.contains(&WarningCode::MissingField), "sem version");
        assert!(codes.contains(&WarningCode::InvalidVersionRange));
        assert_eq!(
            codes
                .iter()
                .filter(|c| **c == WarningCode::InvalidField)
                .count(),
            6,
            "{warnings:?}"
        );
        let (_, warnings) =
            read(r#"{"schemaVersion": 1, "id": "x", "version": "1", "environment": "quantum"}"#);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn sem_id_ou_nao_objeto_e_ignorado() {
        let (parsed, warnings) = read(r#"{"schemaVersion": 1}"#);
        assert!(parsed.is_none());
        assert_eq!(warnings[0].code, WarningCode::MissingField);
        let (parsed, warnings) = read("[1]");
        assert!(parsed.is_none());
        assert_eq!(warnings[0].code, WarningCode::InvalidField);
        let (parsed, warnings) = read("{nada");
        assert!(parsed.is_none());
        assert_eq!(warnings[0].code, WarningCode::InvalidJson);
    }

    #[test]
    fn json_com_virgula_sobrando_e_lido_com_aviso() {
        let (parsed, warnings) = read(r#"{"schemaVersion": 1, "id": "x", "version": "1",}"#);
        assert!(parsed.is_some());
        assert_eq!(warnings[0].code, WarningCode::LenientJson);
    }
}
