//! Padrões das chaves (camada A), "Restaurar padrão" e conferência da faixa (SPEC T12 e T14,
//! parte P1; ARCHITECTURE §10.1; R5B §2.3 e §2.5; C-06).
//!
//! **Camada A**: o padrão que o **próprio arquivo** escreve no comentário da chave, e nada
//! mais. Duas formas:
//!
//! - `# Default: X` do `ModConfigSpec` do NeoForge (só nas chaves com faixa) e de mods que
//!   escrevem a mesma linha ([`DefaultSource::SpecComment`]);
//! - `[default: X]` e `[range: …, default: X]` do `Configuration` do Forge antigo
//!   ([`DefaultSource::LegacyComment`]).
//!
//! A frase "Defaults to X" (camada 3 da C-04) continua aparecendo no formulário como
//! "deduzido", mas **não** conta como padrão conhecido: uma chave sem padrão da camada A nunca
//! aparece como alterada nem pode ser restaurada (CA-T12-09). Padrão que não se lê no tipo da
//! chave (texto num número, por exemplo) também fica "desconhecido".
//!
//! [`restore_defaults`] troca o valor das chaves pelo padrão com a mesma edição mínima do
//! formulário ([`apply_form_edits`]): só os bytes do valor mudam. Nada é gravado aqui; a
//! interface mostra a diferença e grava pelo caminho de sempre.
//!
//! [`range_issues`] lista as chaves com valor fora da faixa ou dos valores permitidos que o
//! arquivo declara (não as deduzidas), para o aviso `W_CONFIG_RANGE` da passagem rápida.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::document::ConfigDocument;
use crate::error::{ConfigError, Result};
use crate::format::ConfigFormat;
use crate::meta::{
    ConfigField, FieldControl, FormEdit, FormItem, FormValue, apply_form_edits, build_form,
    explicit,
};
use crate::path::KeyPath;
use crate::tree::{ConfigEntry, ConfigValue, EntryKind, float_text};

/// De onde veio o padrão de uma chave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DefaultSource {
    /// Linha `Default: X` do comentário (NeoForge).
    SpecComment,
    /// Trecho `[default: X]` do comentário (`Configuration` do Forge antigo).
    LegacyComment,
    /// Frase do comentário ("Defaults to X"): deduzido, não conta como padrão conhecido.
    CommentPhrase,
}

impl DefaultSource {
    /// O padrão vem de um metadado do arquivo (camada A).
    #[must_use]
    pub fn is_known(self) -> bool {
        !matches!(self, Self::CommentPhrase)
    }
}

/// O padrão de uma chave, como o arquivo escreve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDefault {
    /// O padrão como está no comentário.
    pub value: String,
    /// De onde veio.
    pub source: DefaultSource,
}

/// O valor de uma chave comparado ao padrão.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DefaultState {
    /// Sem padrão conhecido (ou padrão que não se lê no tipo da chave).
    Unknown,
    /// Igual ao padrão.
    Same,
    /// Diferente do padrão.
    Changed,
}

/// O padrão da camada A de uma chave, se o comentário dela traz.
#[must_use]
pub fn entry_default(format: ConfigFormat, entry: &ConfigEntry) -> Option<KeyDefault> {
    let source = match format {
        ConfigFormat::Toml => DefaultSource::SpecComment,
        ConfigFormat::LegacyCfg => DefaultSource::LegacyComment,
        _ => return None,
    };
    explicit::read(format, entry.comment.as_deref())
        .default
        .map(|value| KeyDefault { value, source })
}

/// O valor da chave comparado ao padrão da camada A.
#[must_use]
pub fn entry_state(format: ConfigFormat, entry: &ConfigEntry) -> DefaultState {
    if entry.kind != EntryKind::Value {
        return DefaultState::Unknown;
    }
    match (entry.value.as_ref(), entry_default(format, entry)) {
        (Some(value), Some(default)) => state_of(value, &default.value),
        _ => DefaultState::Unknown,
    }
}

/// `value` comparado ao padrão escrito `default`.
#[must_use]
pub fn state_of(value: &ConfigValue, default: &str) -> DefaultState {
    match same_as(value, default) {
        None => DefaultState::Unknown,
        Some(true) => DefaultState::Same,
        Some(false) => DefaultState::Changed,
    }
}

/// Número como o Java escreve (`64`, `0.5`, `1.0E-4`, `2.5d`).
fn number(text: &str) -> Option<f64> {
    let text = text.trim();
    let text = text
        .strip_suffix(['d', 'D', 'f', 'F', 'L', 'l'])
        .unwrap_or(text);
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}

/// Tira as aspas de um texto entre aspas (`"a"`, `'a'`).
fn unquote(text: &str) -> &str {
    let text = text.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = text
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    text
}

/// Os itens de um padrão de lista: `[a, b]`, `["a", "b"]`, a forma do `Configuration`
/// (`[a], [b]`) e o vazio (`[]` ou nada).
fn list_items(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    if text.is_empty() {
        return Some(Vec::new());
    }
    let inner = text.strip_prefix('[')?.strip_suffix(']')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    let items: Vec<&str> = if inner.contains("], [") {
        inner.split("], [").collect()
    } else {
        inner.split(',').collect()
    };
    Some(
        items
            .into_iter()
            .map(|item| unquote(item).to_owned())
            .collect(),
    )
}

/// `Some(true)` se o valor é o padrão; `None` se o padrão não se lê no tipo do valor.
fn same_as(value: &ConfigValue, default: &str) -> Option<bool> {
    let written = default.trim();
    match value {
        ConfigValue::Bool(flag) => match written.to_ascii_lowercase().as_str() {
            "true" => Some(*flag),
            "false" => Some(!*flag),
            _ => None,
        },
        ConfigValue::Integer(current) => written.parse::<i64>().map_or_else(
            |_| number(written).map(|read| close(read, *current as f64)),
            |read| Some(read == *current),
        ),
        ConfigValue::Float(current) => number(written).map(|read| close(read, *current)),
        ConfigValue::String(current) => {
            let expected = unquote(written);
            // `S:` com número dentro (`[range: 0.0 ~ 0.5, default: 0.25]`).
            match (number(current), number(expected)) {
                (Some(left), Some(right)) => Some(close(left, right)),
                _ => Some(current == expected),
            }
        }
        ConfigValue::Datetime(current) => Some(current == written),
        ConfigValue::List(items) => {
            let expected = list_items(written)?;
            if expected.len() != items.len() {
                return Some(false);
            }
            let mut all = true;
            for (item, wanted) in items.iter().zip(&expected) {
                if matches!(item, ConfigValue::List(_)) {
                    return None;
                }
                all &= same_as(item, wanted)?;
            }
            Some(all)
        }
        ConfigValue::Null => None,
    }
}

/// O padrão no tipo do campo, para restaurar; `None` sem padrão conhecido ou se o padrão não
/// se lê no tipo do campo.
#[must_use]
pub fn default_value(field: &ConfigField) -> Option<FormValue> {
    let default = field.default.as_ref().filter(|default| !default.inferred)?;
    let written = default.value.trim();
    Some(match (&field.value, &field.control) {
        (FormValue::Bool(_), _) => match written.to_ascii_lowercase().as_str() {
            "true" => FormValue::Bool(true),
            "false" => FormValue::Bool(false),
            _ => return None,
        },
        (FormValue::Integer(_), _) => {
            let read = written.parse::<i64>().ok().or_else(|| {
                // `64.0` num inteiro: só se for inteiro de verdade.
                let number = number(written)?;
                (number.fract() == 0.0 && number.abs() < 9.0e15).then_some(number as i64)
            })?;
            FormValue::Integer(read.to_string())
        }
        (FormValue::Float(_), _) => {
            let read = number(written)?;
            let clean = written
                .strip_suffix(['d', 'D', 'f', 'F'])
                .unwrap_or(written);
            FormValue::Float(if clean.parse::<f64>().is_ok() {
                clean.to_owned()
            } else {
                float_text(read)
            })
        }
        (FormValue::String(_), FieldControl::Options { values }) => {
            let wanted = unquote(written);
            FormValue::String(values.iter().find(|value| *value == wanted)?.clone())
        }
        (FormValue::String(_), _) => FormValue::String(unquote(written).to_owned()),
        (FormValue::Datetime(_), _) => FormValue::Datetime(written.to_owned()),
        (FormValue::List(_), _) => FormValue::List(list_items(written)?),
    })
}

/// O que "Restaurar padrão" fez.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RestoredDefaults {
    /// O texto com os padrões (igual ao de entrada se nada mudou).
    pub text: String,
    /// As chaves restauradas, como o formulário mostra (`kinetics.maxBeltLength`).
    pub keys: Vec<String>,
}

/// Troca o valor das chaves `keys` (o caminho como o formulário mostra, `geral.maxChunks`) ou,
/// com `None`, de todas as alteradas do arquivo pelo padrão da camada A. Só os bytes dos
/// valores mudam.
///
/// # Errors
///
/// [`ConfigError::NotEditable`] se uma chave pedida não tem padrão conhecido ou já está nele
/// (nada a restaurar) ou se o arquivo não tem formulário; os de [`apply_form_edits`].
pub fn restore_defaults(
    path: &str,
    text: &str,
    keys: Option<&[String]>,
) -> Result<RestoredDefaults> {
    let form = build_form(path, text.as_bytes(), None);
    let fields: Vec<&ConfigField> = form
        .items
        .iter()
        .filter_map(|item| match item {
            FormItem::Field(field) => Some(field.as_ref()),
            _ => None,
        })
        .collect();
    let mut edits = Vec::new();
    let mut restored = Vec::new();
    match keys {
        Some(keys) => {
            for key in keys {
                let field = fields
                    .iter()
                    .find(|field| &field.key == key)
                    .ok_or_else(|| ConfigError::NotEditable {
                        path: KeyPath::from_keys([key.as_str()]),
                        reason: "a chave não está no formulário".into(),
                    })?;
                let value = restorable(field).ok_or_else(|| ConfigError::NotEditable {
                    path: crate::meta::from_keys(&field.path),
                    reason: "a chave não tem padrão conhecido diferente do valor".into(),
                })?;
                edits.push(FormEdit {
                    path: field.path.clone(),
                    value,
                });
                restored.push(field.key.clone());
            }
        }
        None => {
            for field in fields {
                if let Some(value) = restorable(field) {
                    edits.push(FormEdit {
                        path: field.path.clone(),
                        value,
                    });
                    restored.push(field.key.clone());
                }
            }
        }
    }
    let text = if edits.is_empty() {
        text.to_owned()
    } else {
        apply_form_edits(path, text, &edits)?
    };
    Ok(RestoredDefaults {
        text,
        keys: restored,
    })
}

/// O padrão para gravar, se a chave está diferente dele.
fn restorable(field: &ConfigField) -> Option<FormValue> {
    (field.default_state == DefaultState::Changed)
        .then(|| default_value(field))
        .flatten()
}

/// O limite que o arquivo declara para uma chave.
#[derive(Debug, Clone, PartialEq)]
pub enum RangeLimit {
    /// Faixa numérica.
    Range {
        /// Mínimo.
        min: Option<f64>,
        /// Máximo.
        max: Option<f64>,
        /// Como o arquivo escreve (`1 ~ 512`).
        text: String,
    },
    /// Valores permitidos.
    Values(Vec<String>),
}

/// Uma chave com valor fora do que o arquivo declara.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeIssue {
    /// Caminho da chave para mostrar (`general.maxChunks`).
    pub key: String,
    /// Linha da chave (começa em 1).
    pub line: u32,
    /// A linha inteira, aparada.
    pub excerpt: String,
    /// O valor como texto.
    pub value: String,
    /// O limite declarado.
    pub limit: RangeLimit,
    /// O padrão da camada A, quando o arquivo traz.
    pub default: Option<KeyDefault>,
}

/// O valor está fora da faixa ou dos valores permitidos que o arquivo declara (nunca os
/// deduzidos de uma frase do comentário)?
#[must_use]
pub fn entry_range_issue(format: ConfigFormat, entry: &ConfigEntry) -> Option<RangeLimit> {
    let value = entry.value.as_ref()?;
    let found = explicit::read(format, entry.comment.as_deref());
    let values = entry.allowed_values.clone().or(found.valid_values);
    if let (ConfigValue::String(current), Some(values)) = (value, values) {
        return (!values.iter().any(|allowed| allowed == current))
            .then_some(RangeLimit::Values(values));
    }
    let (min, max, text) = match &entry.range {
        Some(range) => (range.min, range.max, range.raw.clone()),
        None if found.min.is_some() || found.max.is_some() => (
            found.min,
            found.max,
            [found.min, found.max]
                .map(|bound| bound.map_or_else(String::new, float_text))
                .join(" ~ "),
        ),
        None => return None,
    };
    let current = match value {
        ConfigValue::Integer(number) => *number as f64,
        ConfigValue::Float(number) => *number,
        ConfigValue::String(text) => number(text)?,
        _ => return None,
    };
    let below = min.is_some_and(|min| current < min);
    let above = max.is_some_and(|max| current > max);
    (below || above).then_some(RangeLimit::Range { min, max, text })
}

/// As chaves do arquivo `path` (caminho relativo, que dá o formato) com valor fora da faixa ou
/// dos valores permitidos que o próprio arquivo declara. Arquivo que não se lê não tem nada.
#[must_use]
pub fn range_issues(path: &str, bytes: &[u8]) -> Vec<RangeIssue> {
    let Some(format) = ConfigFormat::detect(Path::new(path), bytes) else {
        return Vec::new();
    };
    if !matches!(format, ConfigFormat::Toml | ConfigFormat::LegacyCfg) {
        return Vec::new();
    }
    let Ok(document) = ConfigDocument::parse(format, bytes) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(document.as_bytes());
    let lines: Vec<&str> = text.lines().collect();
    document
        .tree()
        .entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Value)
        .filter_map(|entry| {
            let limit = entry_range_issue(format, entry)?;
            let value = entry.value.as_ref()?;
            Some(RangeIssue {
                key: entry.path.to_string(),
                line: u32::try_from(entry.line).unwrap_or(u32::MAX),
                excerpt: lines
                    .get(entry.line.saturating_sub(1))
                    .map(|line| line.trim().to_owned())
                    .unwrap_or_default(),
                value: value.as_plain_text().unwrap_or_else(|| value.to_string()),
                limit,
                default: entry_default(format, entry),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    // Falhar com pânico é o jeito de reprovar o teste.
    #![allow(clippy::unwrap_used, clippy::panic)]

    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::*;

    /// Arquivo do corpus de configs reais (`tests/corpus/FIXTURES.md` diz a origem de cada um).
    fn corpus(folder: &str, name: &str) -> String {
        let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "corpus", folder, name]
            .iter()
            .collect();
        String::from_utf8(std::fs::read(path).unwrap()).unwrap()
    }

    fn fields(path: &str, text: &str) -> Vec<ConfigField> {
        build_form(path, text.as_bytes(), None)
            .items
            .into_iter()
            .filter_map(|item| match item {
                FormItem::Field(field) => Some(*field),
                _ => None,
            })
            .collect()
    }

    fn changed(path: &str, text: &str) -> BTreeSet<String> {
        fields(path, text)
            .into_iter()
            .filter(|field| field.default_state == DefaultState::Changed)
            .map(|field| field.key)
            .collect()
    }

    fn keys(path: &str) -> Vec<FieldKey> {
        path.split('.')
            .map(|key| FieldKey::Name(key.into()))
            .collect()
    }

    /// Linhas diferentes entre dois textos com o mesmo número de linhas.
    fn changed_lines(before: &str, after: &str) -> Vec<usize> {
        let old: Vec<&str> = before.split('\n').collect();
        let new: Vec<&str> = after.split('\n').collect();
        assert_eq!(old.len(), new.len(), "o número de linhas mudou");
        old.iter()
            .zip(&new)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(index, _)| index + 1)
            .collect()
    }

    /// As chaves do TOML diferentes do `# Default:` acima delas, por uma leitura ingênua e
    /// independente do código (linha a linha), para conferir o filtro.
    fn naive_changed_toml(text: &str) -> BTreeSet<String> {
        let mut section = String::new();
        let mut default: Option<String> = None;
        let mut out = BTreeSet::new();
        for line in text.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.to_owned();
                default = None;
            } else if let Some(value) = line.strip_prefix("# Default:") {
                default = Some(value.trim().to_owned());
            } else if line.starts_with('#') || line.is_empty() {
            } else if let Some((key, value)) = line.split_once('=') {
                if let Some(expected) = default.take() {
                    let (a, b) = (value.trim(), expected.as_str());
                    let same = match (a.parse::<f64>(), b.parse::<f64>()) {
                        (Ok(x), Ok(y)) => x == y,
                        _ => a == b,
                    };
                    if !same {
                        out.insert(format!("{section}.{}", key.trim()));
                    }
                }
            }
        }
        out
    }

    #[test]
    fn padrao_da_camada_a_com_a_origem() {
        let text = corpus("toml", "23-create-server.toml");
        let all = fields("config/create-server.toml", &text);
        let timer = all
            .iter()
            .find(|f| f.key == "infrastructure.tickrateSyncTimer")
            .unwrap();
        let default = timer.default.as_ref().unwrap();
        assert_eq!(
            (default.value.as_str(), default.source),
            ("20", DefaultSource::SpecComment)
        );
        assert_eq!(timer.default_state, DefaultState::Same);

        let cfg = corpus("cfg", "05-badmobs.cfg");
        let all = fields("config/badmobs.cfg", &cfg);
        let banned = all.iter().find(|f| f.key == "general.bannedMobs").unwrap();
        assert_eq!(
            banned
                .default
                .as_ref()
                .map(|d| (d.value.as_str(), d.source)),
            Some((
                "[example1], [example2], [example3]",
                DefaultSource::LegacyComment
            ))
        );
        assert_eq!(banned.default_state, DefaultState::Changed);
        assert_eq!(
            default_value(banned),
            Some(FormValue::List(vec![
                "example1".into(),
                "example2".into(),
                "example3".into()
            ]))
        );
    }

    #[test]
    fn ca_t12_09_filtro_mostra_exatamente_as_chaves_diferentes_do_padrao() {
        // Create 1.21.1 (NeoForge): o corpus está todo no padrão; trocamos três valores.
        let text = corpus("toml", "23-create-server.toml");
        assert_eq!(changed("config/create-server.toml", &text), BTreeSet::new());
        let edited = text
            .replace("tickrateSyncTimer = 20", "tickrateSyncTimer = 40")
            .replace(
                "maxFireworkIngredientsInCrafter = 9",
                "maxFireworkIngredientsInCrafter = 12",
            )
            // Sem `Default:`: nunca aparece como alterada (CA-T12-09).
            .replace("bulkPressing = false", "bulkPressing = true");
        assert_ne!(edited, text);
        let found = changed("config/create-server.toml", &edited);
        assert_eq!(found, naive_changed_toml(&edited));
        assert_eq!(
            found,
            BTreeSet::from([
                "infrastructure.tickrateSyncTimer".to_owned(),
                "recipes.maxFireworkIngredientsInCrafter".to_owned(),
            ])
        );

        // Forge 1.12.2: `[default: …]` com lista, liga/desliga e opção.
        let badmobs = corpus("cfg", "05-badmobs.cfg");
        assert_eq!(
            changed("config/badmobs.cfg", &badmobs),
            BTreeSet::from([
                "general.bannedMobs".to_owned(),
                "general.removeAggressively".to_owned()
            ])
        );
        let apiculture = corpus("cfg", "22-apiculture.cfg");
        let found = changed("config/forestry/apiculture.cfg", &apiculture);
        assert!(found.contains("beekeeping.mode"), "{found:?}");
        assert!(
            !found.contains("beekeeping.\"second.princess\""),
            "{found:?}"
        );
        let all = fields("config/forestry/apiculture.cfg", &apiculture);
        let princess = all
            .iter()
            .find(|f| f.key == "beekeeping.\"second.princess\"")
            .unwrap();
        assert_eq!(princess.default_state, DefaultState::Same);
    }

    #[test]
    fn ca_t12_09_sem_padrao_conhecido_nunca_aparece_como_alterada() {
        for folder in ["toml", "cfg", "json", "json5", "properties", "options"] {
            let dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "corpus", folder]
                .iter()
                .collect();
            for file in std::fs::read_dir(dir).unwrap() {
                let file = file.unwrap().path();
                let name = file.file_name().unwrap().to_string_lossy().into_owned();
                let Ok(text) = String::from_utf8(std::fs::read(&file).unwrap()) else {
                    continue;
                };
                let path = format!("config/{name}");
                for field in fields(&path, &text) {
                    let known = field.default.as_ref().is_some_and(|d| !d.inferred);
                    if !known {
                        assert_eq!(
                            field.default_state,
                            DefaultState::Unknown,
                            "{name}: {}",
                            field.key
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn ca_t12_09_restaurar_uma_chave_muda_so_aquela_linha() {
        let text = corpus("toml", "23-create-server.toml")
            .replace("tickrateSyncTimer = 20", "tickrateSyncTimer = 40")
            .replace(
                "maxFireworkIngredientsInCrafter = 9",
                "maxFireworkIngredientsInCrafter = 12",
            );
        let path = "config/create-server.toml";
        let restored = restore_defaults(
            path,
            &text,
            Some(&[keys("infrastructure.tickrateSyncTimer")]),
        )
        .unwrap();
        assert_eq!(restored.keys, vec!["infrastructure.tickrateSyncTimer"]);
        let line = text
            .split('\n')
            .position(|l| l.contains("tickrateSyncTimer = 40"))
            .unwrap();
        assert_eq!(changed_lines(&text, &restored.text), vec![line + 1]);
        assert!(restored.text.contains("\ttickrateSyncTimer = 20"));
        assert!(
            restored
                .text
                .contains("maxFireworkIngredientsInCrafter = 12")
        );

        // O arquivo todo: as duas chaves, e nada mais; depois disso nada fica alterado.
        let all = restore_defaults(path, &text, None).unwrap();
        assert_eq!(all.keys.len(), 2);
        assert_eq!(changed_lines(&text, &all.text).len(), 2);
        assert_eq!(all.text, corpus("toml", "23-create-server.toml"));
        let again = restore_defaults(path, &all.text, None).unwrap();
        assert!(again.keys.is_empty());
        assert_eq!(again.text, all.text);
    }

    #[test]
    fn restaurar_no_forge_antigo_lista_e_opcao() {
        let text = corpus("cfg", "05-badmobs.cfg");
        let path = "config/badmobs.cfg";
        let restored =
            restore_defaults(path, &text, Some(&[keys("general.removeAggressively")])).unwrap();
        assert_eq!(changed_lines(&text, &restored.text).len(), 1);
        assert!(restored.text.contains("B:removeAggressively=false"));
        let all = restore_defaults(path, &text, None).unwrap();
        assert!(changed("config/badmobs.cfg", &all.text).is_empty());
        assert!(all.text.contains("example2"));
        assert!(!all.text.contains("minecraft:creeper"));
    }

    #[test]
    fn restaurar_chave_sem_padrao_ou_ja_no_padrao_e_recusado() {
        let text = corpus("toml", "23-create-server.toml");
        let path = "config/create-server.toml";
        for key in ["recipes.bulkPressing", "infrastructure.tickrateSyncTimer"] {
            let error = restore_defaults(path, &text, Some(&[keys(key)])).unwrap_err();
            assert!(matches!(error, ConfigError::NotEditable { .. }), "{key}");
        }
        let missing = restore_defaults(path, &text, Some(&[keys("nao.existe")]));
        assert!(missing.is_err());
    }

    #[test]
    fn comparacao_pelo_tipo() {
        assert_eq!(
            state_of(&ConfigValue::Integer(64), "64.0"),
            DefaultState::Same
        );
        assert_eq!(
            state_of(&ConfigValue::Integer(64), "abc"),
            DefaultState::Unknown
        );
        assert_eq!(
            state_of(&ConfigValue::Float(0.2), "0.2D"),
            DefaultState::Same
        );
        assert_eq!(
            state_of(&ConfigValue::Bool(true), "TRUE"),
            DefaultState::Same
        );
        assert_eq!(
            state_of(&ConfigValue::Bool(true), "sim"),
            DefaultState::Unknown
        );
        assert_eq!(
            state_of(&ConfigValue::String("0.25".into()), "0.250"),
            DefaultState::Same
        );
        assert_eq!(
            state_of(&ConfigValue::String("a".into()), "\"a\""),
            DefaultState::Same
        );
        let list = ConfigValue::List(vec![
            ConfigValue::Integer(20),
            ConfigValue::String("x".into()),
        ]);
        assert_eq!(state_of(&list, "[20, \"x\"]"), DefaultState::Same);
        // A forma do `Configuration` (`[a], [b]`) é a mesma lista.
        assert_eq!(state_of(&list, "[20], [x]"), DefaultState::Same);
        assert_eq!(state_of(&list, "[20, y]"), DefaultState::Changed);
        assert_eq!(state_of(&ConfigValue::List(vec![]), ""), DefaultState::Same);
        assert_eq!(state_of(&ConfigValue::Null, "1"), DefaultState::Unknown);
    }

    #[test]
    fn ca_t14_10_valor_fora_da_faixa_com_o_padrao() {
        // O caso da SPEC: `maxChunks = 900` com `#Range: 1 ~ 512`, como o NeoForge grava.
        let text = "[general]\n\t#Máximo de chunks.\n\t#Default: 64\n\t#Range: 1 ~ 512\n\tmaxChunks = 900\n\t#Allowed Values: A, B\n\tmode = \"C\"\n\t#Range: 1 ~ 512\n\tok = 3\n";
        let issues = range_issues("config/x-server.toml", text.as_bytes());
        assert_eq!(issues.len(), 2);
        let chunks = &issues[0];
        assert_eq!(
            (chunks.key.as_str(), chunks.line, chunks.value.as_str()),
            ("general.maxChunks", 5, "900")
        );
        assert_eq!(chunks.excerpt, "maxChunks = 900");
        assert_eq!(
            chunks.limit,
            RangeLimit::Range {
                min: Some(1.0),
                max: Some(512.0),
                text: "1 ~ 512".into()
            }
        );
        assert_eq!(
            chunks.default,
            Some(KeyDefault {
                value: "64".into(),
                source: DefaultSource::SpecComment
            })
        );
        assert_eq!(
            issues[1].limit,
            RangeLimit::Values(vec!["A".into(), "B".into()])
        );
        assert_eq!(issues[1].default, None);
        let form = fields("config/x-server.toml", text);
        assert!(form[0].out_of_range && form[1].out_of_range && !form[2].out_of_range);
    }

    #[test]
    fn faixa_do_forge_antigo_e_do_corpus() {
        let text = "general {\n    # Vento [range: 0.0 ~ 0.5, default: 0.25]\n    D:vento=2.0\n    # Texto [range: 0 ~ 10, default: 1]\n    S:n=12\n}\n";
        let issues = range_issues("config/x.cfg", text.as_bytes());
        assert_eq!(
            issues.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(),
            ["general.vento", "general.n"]
        );
        // O corpus real não tem valor fora da faixa, e arquivos sem leitor não têm nada.
        for (folder, name) in [
            ("toml", "23-create-server.toml"),
            ("toml", "27-supplementaries-client.toml"),
            ("cfg", "22-apiculture.cfg"),
        ] {
            let text = corpus(folder, name);
            assert!(range_issues(&format!("config/{name}"), text.as_bytes()).is_empty());
        }
        assert!(range_issues("config/a.json", b"{\"a\": 900}").is_empty());
        assert!(range_issues("config/a.toml", b"a = [").is_empty());
    }
}
