//! `META-INF/jarjar/metadata.json`: jars embutidos do Forge 1.18.2+ e do NeoForge.
//!
//! Formato (biblioteca `JarJar` do Forge, `JarJarMetadata`):
//! `{"jars": [{"identifier": {"group", "artifact"}, "version": {"range", "artifactVersion"},
//! "path", "isObfuscated"}]}`. A faixa é Maven.

use serde_json::Value;

use super::{Sink, string};
use crate::model::{JarJarEntry, WarningCode};
use crate::range::VersionRange;

/// Um jar embutido: caminho e dados.
pub(crate) struct JarJarItem {
    pub(crate) path: String,
    pub(crate) entry: JarJarEntry,
}

pub(crate) fn parse(text: &str, sink: &mut Sink<'_>) -> Vec<JarJarItem> {
    let Some(value) = sink.json(text) else {
        return Vec::new();
    };
    let Some(jars) = value.get("jars").and_then(Value::as_array) else {
        sink.warn(WarningCode::MissingField, "falta a lista 'jars'");
        return Vec::new();
    };
    let mut out = Vec::new();
    for (index, jar) in jars.iter().enumerate() {
        let Some(path) = string(jar.get("path")) else {
            sink.warn(
                WarningCode::MissingField,
                format!("jars #{index} sem 'path'"),
            );
            continue;
        };
        let identifier = jar.get("identifier");
        let version = jar.get("version");
        let range = string(version.and_then(|v| v.get("range"))).map(VersionRange::maven);
        if let Some(r) = &range {
            sink.check_range(&path, r);
        }
        out.push(JarJarItem {
            entry: JarJarEntry {
                group: string(identifier.and_then(|i| i.get("group"))).unwrap_or_default(),
                artifact: string(identifier.and_then(|i| i.get("artifact"))).unwrap_or_default(),
                artifact_version: string(version.and_then(|v| v.get("artifactVersion"))),
                range,
                obfuscated: jar
                    .get("isObfuscated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            },
            path,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Warning;

    fn read(text: &str) -> (Vec<JarJarItem>, Vec<Warning>) {
        let mut warnings = Vec::new();
        let mut sink = Sink {
            path: "META-INF/jarjar/metadata.json",
            warnings: &mut warnings,
        };
        (parse(text, &mut sink), warnings)
    }

    #[test]
    fn metadados_do_jarjar() {
        let (items, warnings) = read(
            r#"{"jars": [
              {"identifier": {"group": "dev.engine_room.flywheel", "artifact": "flywheel-forge-1.20.1"},
               "version": {"range": "[1.0.0,1.1)", "artifactVersion": "1.0.0-215"},
               "path": "META-INF/jarjar/flywheel.jar", "isObfuscated": false},
              {"identifier": {}, "version": {"range": "[1"}, "path": "META-INF/jarjar/x.jar"},
              {"identifier": {"group": "g", "artifact": "a"}}
            ]}"#,
        );
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].path, "META-INF/jarjar/flywheel.jar");
        assert_eq!(items[0].entry.artifact, "flywheel-forge-1.20.1");
        assert_eq!(
            items[0].entry.artifact_version.as_deref(),
            Some("1.0.0-215")
        );
        assert_eq!(
            items[0].entry.range,
            Some(VersionRange::maven("[1.0.0,1.1)"))
        );
        assert_eq!(items[1].entry.group, "");
        let codes: Vec<_> = warnings.iter().map(|w| w.code).collect();
        assert_eq!(
            codes,
            [WarningCode::InvalidVersionRange, WarningCode::MissingField]
        );
        let (items, warnings) = read("{}");
        assert!(items.is_empty());
        assert_eq!(warnings[0].code, WarningCode::MissingField);
        assert!(read("x").0.is_empty());
    }
}
