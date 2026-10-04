//! Utilitários dos testes de integração: leitura do corpus real.

#![allow(dead_code, unreachable_pub)]
// cada arquivo de teste usa uma parte
// Utilitários só de teste: falhar com pânico é o jeito de reprovar o teste.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use warden_configs::{CfgType, ConfigEntry, ConfigFormat, ConfigValue};

/// Pasta do corpus (`tests/corpus/<formato>/`).
pub fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
}

/// Pasta de cada formato no corpus.
pub fn folder(format: ConfigFormat) -> &'static str {
    match format {
        ConfigFormat::Toml => "toml",
        ConfigFormat::Json => "json",
        ConfigFormat::Json5 => "json5",
        ConfigFormat::Properties => "properties",
        ConfigFormat::LegacyCfg => "cfg",
        ConfigFormat::OptionsTxt => "options",
    }
}

/// Arquivo do corpus: nome e bytes.
pub struct Sample {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Todos os arquivos de um formato, em ordem de nome.
pub fn samples(format: ConfigFormat) -> Vec<Sample> {
    let dir = corpus_root().join(folder(format));
    let mut found: Vec<Sample> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| {
            let path = entry.unwrap().path();
            Sample {
                name: path.file_name().unwrap().to_string_lossy().into_owned(),
                bytes: std::fs::read(&path).unwrap(),
            }
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Um valor diferente do atual e aceito pelo tipo da chave, para testar a edição.
pub fn different_value(entry: &ConfigEntry) -> Option<ConfigValue> {
    let value = entry.value.as_ref()?;
    let typed_default = |ty: Option<CfgType>| match ty {
        Some(CfgType::Boolean) => ConfigValue::Bool(true),
        Some(CfgType::Integer) => ConfigValue::Integer(7),
        Some(CfgType::Double) => ConfigValue::Float(2.5),
        _ => ConfigValue::String("warden".into()),
    };
    Some(match value {
        ConfigValue::Bool(flag) => ConfigValue::Bool(!flag),
        ConfigValue::Integer(number) => ConfigValue::Integer(number.wrapping_add(1)),
        ConfigValue::Float(number) if number.is_finite() => ConfigValue::Float(number + 0.5),
        ConfigValue::Float(_) => ConfigValue::Float(1.0),
        ConfigValue::String(_)
            if entry.cfg_type.is_some_and(|ty| {
                matches!(ty, CfgType::Boolean | CfgType::Integer | CfgType::Double)
            }) =>
        {
            typed_default(entry.cfg_type)
        }
        ConfigValue::String(text) => ConfigValue::String(format!("{text}x")),
        ConfigValue::Datetime(_) => return None,
        ConfigValue::Null => ConfigValue::Integer(1),
        ConfigValue::List(items) => {
            let mut items = items.clone();
            match items.first().cloned() {
                Some(first)
                    if !matches!(first, ConfigValue::String(_)) || entry.cfg_type.is_none() =>
                {
                    items.push(first);
                }
                Some(_) => items.push(ConfigValue::String("warden:item".into())),
                None => items.push(match entry.cfg_type {
                    Some(_) => match typed_default(entry.cfg_type) {
                        ConfigValue::String(_) => ConfigValue::String("warden:item".into()),
                        other => other,
                    },
                    None => ConfigValue::String("warden".into()),
                }),
            }
            ConfigValue::List(items)
        }
    })
}

/// Linhas (separadas por `\n`) que diferem entre dois textos com o mesmo número de linhas.
pub fn differing_lines(before: &[u8], after: &[u8]) -> Option<Vec<usize>> {
    let old: Vec<&[u8]> = before.split(|byte| *byte == b'\n').collect();
    let new: Vec<&[u8]> = after.split(|byte| *byte == b'\n').collect();
    if old.len() != new.len() {
        return None;
    }
    Some(
        old.iter()
            .zip(&new)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(index, _)| index + 1)
            .collect(),
    )
}
