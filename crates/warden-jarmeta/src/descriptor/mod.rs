//! Leitores dos arquivos de metadados (R3 §4.1 a §4.4).
//!
//! Cada leitor recebe o texto já decodificado e devolve o que conseguiu ler, registrando em
//! `warnings` o que ignorou. Nenhum leitor falha: um descritor ruim vira aviso, e o resto do
//! jar continua sendo lido.

pub(crate) mod fabric;
pub(crate) mod forge_toml;
pub(crate) mod jarjar;
pub(crate) mod manifest;
pub(crate) mod mcmod_info;
pub(crate) mod quilt;

use serde_json::Value;

use crate::lenient_json;
use crate::model::{Dependency, DependencyKind, ModMetadata, Warning, WarningCode};
use crate::range::VersionRange;

/// Junta os avisos de um leitor, com o caminho do arquivo.
pub(crate) struct Sink<'a> {
    pub(crate) path: &'a str,
    pub(crate) warnings: &'a mut Vec<Warning>,
}

impl Sink<'_> {
    pub(crate) fn warn(&mut self, code: WarningCode, detail: impl Into<String>) {
        self.warnings.push(Warning {
            code,
            path: self.path.to_owned(),
            detail: detail.into(),
        });
    }

    /// Lê JSON (tolerante); devolve `None` e avisa se não der.
    pub(crate) fn json(&mut self, text: &str) -> Option<Value> {
        match lenient_json::parse(text) {
            Ok(parsed) => {
                if parsed.lenient {
                    self.warn(
                        WarningCode::LenientJson,
                        "JSON inválido no modo estrito; lido no modo tolerante",
                    );
                }
                Some(parsed.value)
            }
            Err(e) => {
                self.warn(WarningCode::InvalidJson, e);
                None
            }
        }
    }

    /// Confere a faixa e avisa se o loader a recusaria (a faixa é mantida no modelo).
    pub(crate) fn check_range(&mut self, owner: &str, range: &VersionRange) {
        if let Err(e) = range.validate() {
            self.warn(WarningCode::InvalidVersionRange, format!("{owner}: {e}"));
        }
    }
}

/// Texto de um campo, se for texto.
pub(crate) fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

/// Texto de um campo que pode ser texto ou lista de textos (`license`, `provides`...).
pub(crate) fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

/// Preenche `minecraft`, `loader_version` e `java` a partir das dependências obrigatórias.
pub(crate) fn fill_well_known(module: &mut ModMetadata, loader_ids: &[&str]) {
    let find = |deps: &[Dependency], ids: &[&str]| {
        deps.iter()
            .find(|d| {
                ids.contains(&d.id.as_str())
                    && matches!(d.kind, DependencyKind::Required | DependencyKind::Optional)
            })
            .map(|d| d.range.clone())
    };
    if module.minecraft.is_none() {
        module.minecraft = find(&module.dependencies, &["minecraft"]);
    }
    if module.loader_version.is_none() {
        module.loader_version = find(&module.dependencies, loader_ids);
    }
    if module.java.is_none() {
        module.java = find(&module.dependencies, &["java"]);
    }
}
