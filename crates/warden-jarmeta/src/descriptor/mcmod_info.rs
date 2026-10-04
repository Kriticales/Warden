//! `mcmod.info` (Forge 1.7.10 a 1.12.2; R3 §4.4).
//!
//! JSON escrito à mão, muitas vezes inválido: lido no modo tolerante. Formatos aceitos: lista de
//! mods, `{"modListVersion": 2, "modList": [...]}` e um objeto só. Serve para **identificar** o
//! mod (modid, versão, `mcversion`); as listas de dependência só valem com
//! `useDependencyInformation: true` (como no FML) e, nesse caso, `requiredMods` vira
//! obrigatória, `dependencies` vira "carrega depois" e `dependants` vira "carrega antes", com a
//! faixa Maven depois de `@`.

use serde_json::Value;

use super::fabric::people;
use super::{Sink, string, strings};
use crate::model::{
    Dependency, DependencyKind, DescriptorKind, Environment, LoadOrdering, Loader, ModMetadata,
    Side, WarningCode,
};
use crate::range::VersionRange;

pub(crate) fn parse(text: &str, sink: &mut Sink<'_>) -> Vec<ModMetadata> {
    let Some(value) = sink.json(text) else {
        return Vec::new();
    };
    let entries: Vec<&Value> = match &value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(o) if o.contains_key("modList") => {
            if let Some(Value::Array(items)) = o.get("modList") {
                items.iter().collect()
            } else {
                sink.warn(WarningCode::InvalidField, "'modList' não é uma lista");
                Vec::new()
            }
        }
        Value::Object(_) => vec![&value],
        _ => {
            sink.warn(WarningCode::InvalidField, "a raiz não é lista nem objeto");
            Vec::new()
        }
    };
    let mut mods = Vec::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let Some(o) = entry.as_object() else {
            sink.warn(
                WarningCode::InvalidField,
                format!("item #{index} não é objeto"),
            );
            continue;
        };
        let Some(id) = string(o.get("modid")).filter(|s| !s.trim().is_empty()) else {
            sink.warn(
                WarningCode::MissingField,
                format!("item #{index} sem 'modid'"),
            );
            continue;
        };
        let version = string(o.get("version"));
        if version.as_deref().is_some_and(|v| v.contains("${")) {
            sink.warn(
                WarningCode::UnresolvedPlaceholder,
                format!("{id}: versão com marcador de build não substituído"),
            );
        }
        let minecraft = match string(o.get("mcversion")) {
            Some(mc) if mc.contains("${") => {
                sink.warn(
                    WarningCode::UnresolvedPlaceholder,
                    format!("{id}: mcversion com marcador de build não substituído"),
                );
                None
            }
            Some(mc) if !mc.trim().is_empty() => Some(VersionRange::Exact { version: mc }),
            _ => None,
        };
        let mut authors = people(o.get("authorList"));
        if authors.is_empty() {
            authors = people(o.get("authors"));
        }
        let mut module = ModMetadata {
            source: DescriptorKind::McmodInfo,
            loader: Loader::Forge,
            id,
            version,
            name: string(o.get("name")),
            description: string(o.get("description")),
            authors,
            licenses: Vec::new(),
            provides: Vec::new(),
            environment: Environment::Both,
            dependencies: Vec::new(),
            minecraft,
            loader_version: None,
            java: None,
            mixins: Vec::new(),
        };
        if o.get("useDependencyInformation").and_then(Value::as_bool) == Some(true) {
            read_dependency_information(o, &mut module, sink);
        }
        mods.push(module);
    }
    mods
}

/// Listas de dependência, que só valem com `useDependencyInformation: true`.
fn read_dependency_information(
    o: &serde_json::Map<String, Value>,
    module: &mut ModMetadata,
    sink: &mut Sink<'_>,
) {
    for (key, kind, ordering) in [
        ("requiredMods", DependencyKind::Required, LoadOrdering::None),
        (
            "dependencies",
            DependencyKind::Optional,
            LoadOrdering::After,
        ),
        ("dependants", DependencyKind::Optional, LoadOrdering::Before),
    ] {
        for reference in strings(o.get(key)) {
            module
                .dependencies
                .push(reference_dependency(&reference, kind, ordering, sink));
        }
    }
    for dep in &mut module.dependencies {
        if dep.kind == DependencyKind::Required
            && let Some(other) = o.get("dependencies").map(|v| strings(Some(v)))
            && other
                .iter()
                .any(|r| r.split('@').next() == Some(dep.id.as_str()))
        {
            dep.ordering = LoadOrdering::After;
        }
    }
    let loader = module
        .dependencies
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case("forge"))
        .map(|d| d.range.clone());
    module.loader_version = loader;
}

/// `modid` ou `modid@faixa` (faixa Maven).
fn reference_dependency(
    reference: &str,
    kind: DependencyKind,
    ordering: LoadOrdering,
    sink: &mut Sink<'_>,
) -> Dependency {
    let (id, range) = match reference.split_once('@') {
        Some((id, spec)) => (id.trim(), VersionRange::maven(spec.trim())),
        None => (reference.trim(), VersionRange::Any),
    };
    sink.check_range(id, &range);
    Dependency {
        id: id.to_owned(),
        kind,
        range,
        side: Side::Both,
        ordering,
        reason: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Warning;

    fn read(text: &str) -> (Vec<ModMetadata>, Vec<Warning>) {
        let mut warnings = Vec::new();
        let mut sink = Sink {
            path: "mcmod.info",
            warnings: &mut warnings,
        };
        (parse(text, &mut sink), warnings)
    }

    /// Critério 2 da P1-06: `mcmod.info` com vírgula sobrando é lido.
    #[test]
    fn ca_p1_06_2_mcmod_info_com_virgula_sobrando() {
        let (mods, warnings) = read(
            r#"[
{
  "modid": "examplemod",
  "name": "Example Mod",
  "description": "Uma descrição
em duas linhas",
  "version": "1.0",
  "mcversion": "1.7.10",
  "authorList": ["Fulano", "Beltrano",],
  "dependencies": [],
},
]"#,
        );
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].id, "examplemod");
        assert_eq!(mods[0].authors, ["Fulano", "Beltrano"]);
        assert_eq!(
            mods[0].description.as_deref(),
            Some("Uma descrição\nem duas linhas")
        );
        assert_eq!(
            mods[0].minecraft,
            Some(VersionRange::Exact {
                version: "1.7.10".into()
            })
        );
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, WarningCode::LenientJson);
    }

    #[test]
    fn formato_mod_list_e_dependencias() {
        let (mods, warnings) = read(
            r#"{"modListVersion": 2, "modList": [
                {"modid": "a", "version": "${version}", "mcversion": "${mcversion}",
                 "authors": ["X"], "useDependencyInformation": true,
                 "requiredMods": ["Forge@[10.13.4.1614,)", "b"], "dependencies": ["b", "c@[1.0"],
                 "dependants": ["d"]},
                {"modid": "e", "requiredMods": ["f"]}, {"name": "sem id"}, 3]}"#,
        );
        assert_eq!(mods.len(), 2);
        let a = &mods[0];
        assert_eq!(a.authors, ["X"]);
        assert_eq!(a.minecraft, None);
        assert_eq!(
            a.loader_version,
            Some(VersionRange::maven("[10.13.4.1614,)"))
        );
        let summary: Vec<_> = a
            .dependencies
            .iter()
            .map(|d| (d.id.as_str(), d.kind, d.ordering))
            .collect();
        assert_eq!(
            summary,
            [
                ("Forge", DependencyKind::Required, LoadOrdering::None),
                ("b", DependencyKind::Required, LoadOrdering::After),
                ("b", DependencyKind::Optional, LoadOrdering::After),
                ("c", DependencyKind::Optional, LoadOrdering::After),
                ("d", DependencyKind::Optional, LoadOrdering::Before),
            ]
        );
        assert!(
            mods[1].dependencies.is_empty(),
            "sem useDependencyInformation"
        );
        let codes: Vec<_> = warnings.iter().map(|w| w.code).collect();
        assert_eq!(
            codes,
            [
                WarningCode::UnresolvedPlaceholder,
                WarningCode::UnresolvedPlaceholder,
                WarningCode::InvalidVersionRange,
                WarningCode::MissingField,
                WarningCode::InvalidField,
            ]
        );
    }

    #[test]
    fn objeto_unico_e_raizes_ruins() {
        let (mods, _) = read(r#"{"modid": "solo", "mcversion": ""}"#);
        assert_eq!(mods[0].id, "solo");
        assert_eq!(mods[0].minecraft, None);
        let (mods, warnings) = read(r#"{"modList": {}}"#);
        assert!(mods.is_empty());
        assert_eq!(warnings[0].code, WarningCode::InvalidField);
        let (mods, warnings) = read("42");
        assert!(mods.is_empty());
        assert_eq!(warnings[0].code, WarningCode::InvalidField);
        let (mods, warnings) = read("não é json");
        assert!(mods.is_empty());
        assert_eq!(warnings[0].code, WarningCode::InvalidJson);
    }
}
