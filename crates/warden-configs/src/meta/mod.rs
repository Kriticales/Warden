//! Formulário das configs: os campos com os metadados em camadas e a edição pelo formulário
//! (SPEC T12, parte P1; ARCHITECTURE §10.1; R5B §2.2; C-04).
//!
//! [`build_form`] lê o arquivo e devolve um [`ConfigForm`]: seções, campos e trechos que só se
//! editam como texto, na ordem do arquivo. Cada [`ConfigField`] junta, nesta ordem de
//! preferência:
//!
//! 1. o que o arquivo declara (`Range`, `Allowed Values`, `Default`, `[range: … default: …]`,
//!    `Min`/`Max`, `Valid values`, prefixo de tipo do `.cfg`): confiança alta;
//! 2. o tipo pelo valor (liga/desliga, número, lista, texto);
//! 3. as frases do comentário (`Defaults to X`, `between X and Y`, cores `#RRGGBB`): confiança
//!    baixa, marcadas "deduzido" ([`FieldDefault::inferred`], [`FieldRange::inferred`]);
//! 4. o rótulo e a ajuda do arquivo de idioma do jar ([`FieldLabel`]), com a chave real.
//!
//! **Só texto** ([`FormAvailability::TextOnly`], com o motivo; ADR-0054): formato sem
//! formulário, `.cfg` que não é do Forge, arquivo acima de 2 MB ou fora de UTF-8, texto que não
//! se lê, arquivo sem nenhuma chave e, principalmente, arquivo que **não volta idêntico** na
//! leitura de teste (o Warden reescreve cada valor com ele mesmo e compara;
//! [`ConfigDocument::rewrite_problems`]). Listas de objetos, listas dentro de listas, `null` e
//! seções muito fundas viram um [`FormSnippet`] ("editar este trecho como texto").
//!
//! [`apply_form_edits`] aplica as edições do formulário ao texto e devolve o texto novo, que a
//! interface grava pelo caminho de sempre (`config_write`, com a diferença antes de gravar e o
//! aviso de alteração externa). Só os bytes do valor mudam; se a releitura ou a comparação byte
//! a byte fora do valor falhar, nada é devolvido e o formulário cai para o texto com o motivo.

mod explicit;
mod inferred;
mod labels;

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_jarmeta::lang::{ConfigLang, LangLocale};

use crate::document::{ConfigDocument, ConfigEdit};
use crate::error::{ConfigError, Result};
use crate::format::ConfigFormat;
use crate::path::{KeyPath, PathSegment};
use crate::tree::{CfgType, ConfigEntry, ConfigValue, EntryKind, ValueKind, float_text};

/// Seções mais fundas que isto (contando a raiz como 0) viram "trecho em texto".
pub const MAX_FORM_DEPTH: usize = 6;

/// Um passo do caminho de uma chave, como a interface manda e recebe.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(untagged)]
pub enum FieldKey {
    /// Nome de chave ou seção.
    Name(String),
    /// Posição numa lista.
    Index(u32),
}

fn to_keys(path: &KeyPath) -> Vec<FieldKey> {
    path.segments()
        .iter()
        .map(|segment| match segment {
            PathSegment::Key(key) => FieldKey::Name(key.clone()),
            PathSegment::Index(index) => FieldKey::Index(u32::try_from(*index).unwrap_or(u32::MAX)),
        })
        .collect()
}

fn from_keys(keys: &[FieldKey]) -> KeyPath {
    KeyPath(
        keys.iter()
            .map(|key| match key {
                FieldKey::Name(name) => PathSegment::Key(name.clone()),
                FieldKey::Index(index) => PathSegment::Index(*index as usize),
            })
            .collect(),
    )
}

/// Valor de um campo. Números vão como texto, para não perder precisão no caminho (um `long`
/// do Java não cabe num número do JavaScript).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum FormValue {
    /// Liga/desliga.
    Bool(bool),
    /// Inteiro, em texto (`"-3"`).
    Integer(String),
    /// Número com casas decimais, em texto (`"1.5"`).
    Float(String),
    /// Texto.
    String(String),
    /// Data e hora do TOML, em texto.
    Datetime(String),
    /// Lista de valores simples, cada um em texto.
    List(Vec<String>),
}

/// Tipo dos itens de uma lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ListItemKind {
    /// Texto.
    Text,
    /// Inteiro.
    Integer,
    /// Número com casas decimais.
    Float,
    /// Liga/desliga (`true`/`false`).
    Bool,
}

/// O controle que o formulário mostra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FieldControl {
    /// Liga/desliga.
    Toggle,
    /// Número, com o limite quando há.
    Number {
        /// Só inteiros.
        integer: bool,
        /// Mínimo (da faixa).
        min: Option<f64>,
        /// Máximo (da faixa).
        max: Option<f64>,
    },
    /// Uma opção de uma lista fechada.
    Options {
        /// Os valores aceitos.
        values: Vec<String>,
    },
    /// Texto livre.
    Text,
    /// Lista de valores.
    List {
        /// Tipo dos itens.
        item: ListItemKind,
    },
    /// Cor `#RRGGBB`, deduzida do valor. Com oito dígitos o canal alfa pode vir na frente ou
    /// atrás (cada mod escolhe), então esse valor fica como texto.
    Color,
}

/// Quanto o formulário confia no tipo e nos limites do campo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// Declarado pelo arquivo, ou tipo inequívoco pelo valor.
    High,
    /// Tipo pelo valor que poderia ser outro (número decimal, texto).
    Medium,
    /// Deduzido das frases do comentário ou do jeito do valor.
    Low,
}

/// Faixa permitida.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FieldRange {
    /// Mínimo.
    pub min: Option<f64>,
    /// Máximo.
    pub max: Option<f64>,
    /// Como o arquivo escreve (`1 ~ 64`, `between 1 and 64`).
    pub text: String,
    /// Tirada de uma frase do comentário (camada 3), não de um metadado do arquivo.
    pub inferred: bool,
}

/// Padrão da chave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FieldDefault {
    /// O padrão como o arquivo escreve.
    pub value: String,
    /// Tirado de uma frase do comentário (camada 3), não de um metadado do arquivo.
    pub inferred: bool,
}

/// Rótulo do arquivo de idioma do jar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FieldLabel {
    /// O nome bonito.
    pub text: String,
    /// A ajuda, quando o mod tem.
    pub tooltip: Option<String>,
    /// A chave de tradução usada.
    pub translation_key: String,
    /// Idioma do rótulo.
    pub locale: LangLocale,
}

/// Um campo editável.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigField {
    /// Caminho da chave (o que volta na edição).
    pub path: Vec<FieldKey>,
    /// A chave real, para mostrar (`kinetics.maxRotationSpeed`).
    pub key: String,
    /// Linha da chave (começa em 1).
    pub line: u32,
    /// Valor atual.
    pub value: FormValue,
    /// Controle.
    pub control: FieldControl,
    /// Confiança no controle e nos limites.
    pub confidence: Confidence,
    /// O comentário do arquivo, sem as linhas de metadados.
    pub description: Option<String>,
    /// Faixa permitida.
    pub range: Option<FieldRange>,
    /// Padrão.
    pub default: Option<FieldDefault>,
    /// Rótulo do jar.
    pub label: Option<FieldLabel>,
}

/// Um título de seção.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormSection {
    /// Caminho da seção.
    pub path: Vec<FieldKey>,
    /// Caminho para mostrar (`kinetics.fluids`).
    pub key: String,
    /// Linha (começa em 1).
    pub line: u32,
    /// Comentário acima da seção.
    pub description: Option<String>,
}

/// Por que um trecho só se edita como texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SnippetReason {
    /// Lista de objetos ou tabelas.
    ListOfObjects,
    /// Lista com listas dentro, ou com itens de tipos diferentes.
    ComplexList,
    /// Valor `null` (o tipo certo é desconhecido).
    NullValue,
    /// Seção funda demais.
    TooDeep,
}

/// Um trecho que o formulário não edita: o botão leva ao editor de texto na linha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormSnippet {
    /// Caminho do trecho.
    pub path: Vec<FieldKey>,
    /// Caminho para mostrar.
    pub key: String,
    /// Linha (começa em 1).
    pub line: u32,
    /// Motivo.
    pub reason: SnippetReason,
}

/// Uma linha do formulário, na ordem do arquivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FormItem {
    /// Título de seção.
    Section(FormSection),
    /// Campo editável.
    Field(Box<ConfigField>),
    /// Trecho só em texto.
    Snippet(FormSnippet),
}

/// Por que o arquivo abre só no editor de texto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum TextOnlyReason {
    /// Formato sem formulário (YAML, scripts, texto solto...).
    UnsupportedFormat,
    /// `.cfg` que não é do `Configuration` do Forge.
    NotForgeCfg,
    /// Acima de 2 MB.
    TooLarge,
    /// Não é UTF-8.
    NotUtf8,
    /// O texto não segue o formato.
    ParseFailed {
        /// Linha.
        line: u32,
        /// Coluna.
        column: u32,
    },
    /// A leitura de teste não volta idêntica: reescrever esta chave mudaria o arquivo.
    NotIdentical {
        /// A primeira chave com problema.
        key: String,
    },
    /// O arquivo não tem nenhuma chave.
    NoKeys,
}

/// Se o formulário serve para o arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FormAvailability {
    /// O formulário serve.
    Ready,
    /// Só o editor de texto, com o motivo.
    TextOnly {
        /// Motivo.
        reason: TextOnlyReason,
    },
}

/// O formulário de um arquivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfigForm {
    /// Se o formulário serve.
    pub availability: FormAvailability,
    /// Linhas do formulário (vazio quando só texto).
    pub items: Vec<FormItem>,
}

impl ConfigForm {
    fn text_only(reason: TextOnlyReason) -> Self {
        Self {
            availability: FormAvailability::TextOnly { reason },
            items: Vec::new(),
        }
    }
}

/// Uma edição pedida pelo formulário.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormEdit {
    /// Caminho da chave.
    pub path: Vec<FieldKey>,
    /// Valor novo.
    pub value: FormValue,
}

fn line_u32(line: usize) -> u32 {
    u32::try_from(line).unwrap_or(u32::MAX)
}

/// O formato do arquivo, ou o motivo de não haver formulário.
fn detect(path: &str, bytes: &[u8]) -> std::result::Result<ConfigFormat, TextOnlyReason> {
    let file = Path::new(path);
    match ConfigFormat::from_file_name(file) {
        None => Err(TextOnlyReason::UnsupportedFormat),
        Some(_) => ConfigFormat::detect(file, bytes).ok_or(TextOnlyReason::NotForgeCfg),
    }
}

fn reason_of(error: &ConfigError) -> TextOnlyReason {
    match error {
        ConfigError::TooLarge { .. } => TextOnlyReason::TooLarge,
        ConfigError::NotUtf8 { .. } => TextOnlyReason::NotUtf8,
        ConfigError::Parse { line, column, .. } => TextOnlyReason::ParseFailed {
            line: line_u32(*line),
            column: line_u32(*column),
        },
        _ => TextOnlyReason::UnsupportedFormat,
    }
}

/// Monta o formulário do arquivo `path` (caminho relativo, que dá o formato e o nome usado nos
/// rótulos) com os bytes `bytes`. `labels` são os rótulos dos jars do pack, quando há.
#[must_use]
pub fn build_form(path: &str, bytes: &[u8], labels: Option<&ConfigLang>) -> ConfigForm {
    let format = match detect(path, bytes) {
        Ok(format) => format,
        Err(reason) => return ConfigForm::text_only(reason),
    };
    let document = match ConfigDocument::parse(format, bytes) {
        Ok(document) => document,
        Err(error) => return ConfigForm::text_only(reason_of(&error)),
    };
    if let Some(problem) = document.rewrite_problems().first() {
        return ConfigForm::text_only(TextOnlyReason::NotIdentical {
            key: problem.path.to_string(),
        });
    }
    let items = items(&document, path, labels);
    if !items
        .iter()
        .any(|item| matches!(item, FormItem::Field(_) | FormItem::Snippet(_)))
    {
        return ConfigForm::text_only(TextOnlyReason::NoKeys);
    }
    ConfigForm {
        availability: FormAvailability::Ready,
        items,
    }
}

fn items(document: &ConfigDocument, path: &str, labels: Option<&ConfigLang>) -> Vec<FormItem> {
    let tree = document.tree();
    let format = tree.format;
    // Seções cujo filho direto é uma posição de lista: listas de objetos.
    let list_parents: HashSet<KeyPath> = tree
        .entries
        .iter()
        .filter(|entry| matches!(entry.path.last(), Some(PathSegment::Index(_))))
        .map(|entry| KeyPath(entry.path.segments()[..entry.path.len() - 1].to_vec()))
        .collect();
    let mut out = Vec::new();
    let mut collapsed: Option<KeyPath> = None;
    let mut seen = HashSet::new();
    for (position, entry) in tree.entries.iter().enumerate() {
        if let Some(prefix) = &collapsed {
            if entry.path.starts_with(prefix) {
                continue;
            }
            collapsed = None;
        }
        if entry.kind == EntryKind::Section {
            // O objeto da raiz do JSON não é uma seção para mostrar.
            if entry.path.is_empty() {
                continue;
            }
            let reason = if list_parents.contains(&entry.path) {
                Some(SnippetReason::ListOfObjects)
            } else if entry.path.len() > MAX_FORM_DEPTH {
                Some(SnippetReason::TooDeep)
            } else {
                None
            };
            match reason {
                Some(reason) => {
                    out.push(snippet(entry, reason));
                    collapsed = Some(entry.path.clone());
                }
                None => out.push(FormItem::Section(FormSection {
                    path: to_keys(&entry.path),
                    key: entry.path.to_string(),
                    line: line_u32(entry.line),
                    description: entry
                        .comment
                        .as_deref()
                        .and_then(|comment| explicit::read(format, Some(comment)).description),
                })),
            }
            continue;
        }
        // Chave repetida (`.properties`, `options.txt`): vale a última, a que o jogo lê.
        let repeated = tree
            .entries
            .get(position + 1..)
            .is_some_and(|rest| rest.iter().any(|later| later.path == entry.path));
        if repeated || !seen.insert(entry.path.clone()) {
            continue;
        }
        out.push(field_item(entry, format, path, labels));
    }
    out
}

fn snippet(entry: &ConfigEntry, reason: SnippetReason) -> FormItem {
    FormItem::Snippet(FormSnippet {
        path: to_keys(&entry.path),
        key: entry.path.to_string(),
        line: line_u32(entry.line),
        reason,
    })
}

fn item_kind(value: &ConfigValue) -> Option<ListItemKind> {
    match value {
        ConfigValue::Bool(_) => Some(ListItemKind::Bool),
        ConfigValue::Integer(_) => Some(ListItemKind::Integer),
        ConfigValue::Float(_) => Some(ListItemKind::Float),
        ConfigValue::String(_) | ConfigValue::Datetime(_) => Some(ListItemKind::Text),
        ConfigValue::Null | ConfigValue::List(_) => None,
    }
}

/// Tipo comum dos itens; `None` se misturados ou com listas dentro. Lista vazia é de texto.
fn list_kind(items: &[ConfigValue]) -> Option<ListItemKind> {
    let mut kinds = items.iter().map(item_kind);
    let first = match kinds.next() {
        None => return Some(ListItemKind::Text),
        Some(kind) => kind?,
    };
    kinds.all(|kind| kind == Some(first)).then_some(first)
}

/// Texto de um valor simples, para o formulário.
fn plain_text(value: &ConfigValue) -> String {
    match value {
        ConfigValue::Float(number) => float_text(*number),
        other => other.as_plain_text().unwrap_or_default(),
    }
}

fn form_value(value: &ConfigValue) -> Option<FormValue> {
    Some(match value {
        ConfigValue::Bool(flag) => FormValue::Bool(*flag),
        ConfigValue::Integer(number) => FormValue::Integer(number.to_string()),
        ConfigValue::Float(number) => FormValue::Float(float_text(*number)),
        ConfigValue::String(text) => FormValue::String(text.clone()),
        ConfigValue::Datetime(text) => FormValue::Datetime(text.clone()),
        ConfigValue::List(items) => FormValue::List(items.iter().map(plain_text).collect()),
        ConfigValue::Null => return None,
    })
}

/// Como um texto de formato sem tipos (`.properties`, `options.txt`) parece ser.
fn untyped_control(text: &str) -> (FieldControl, Confidence) {
    if text == "true" || text == "false" {
        return (FieldControl::Toggle, Confidence::Medium);
    }
    if text.parse::<i64>().is_ok() {
        return (
            FieldControl::Number {
                integer: true,
                min: None,
                max: None,
            },
            Confidence::Medium,
        );
    }
    if text.contains('.') && text.parse::<f64>().is_ok_and(f64::is_finite) {
        return (
            FieldControl::Number {
                integer: false,
                min: None,
                max: None,
            },
            Confidence::Medium,
        );
    }
    (FieldControl::Text, Confidence::Medium)
}

fn field_item(
    entry: &ConfigEntry,
    format: ConfigFormat,
    file_path: &str,
    labels: Option<&ConfigLang>,
) -> FormItem {
    let Some(value) = entry.value.as_ref() else {
        return snippet(entry, SnippetReason::NullValue);
    };
    if matches!(value, ConfigValue::Null) {
        return snippet(entry, SnippetReason::NullValue);
    }
    if let ConfigValue::List(items) = value
        && list_kind(items).is_none()
    {
        return snippet(entry, SnippetReason::ComplexList);
    }
    let Some(form) = form_value(value) else {
        return snippet(entry, SnippetReason::NullValue);
    };
    let found = explicit::read(format, entry.comment.as_deref());
    let description = found.description.clone();

    // Faixa: camada 1 (leitura e `Min`/`Max`) e, sem ela, a camada 3.
    let explicit_range = entry
        .range
        .as_ref()
        .map(|range| FieldRange {
            min: range.min,
            max: range.max,
            text: range.raw.clone(),
            inferred: false,
        })
        .or_else(|| {
            (found.min.is_some() || found.max.is_some()).then(|| FieldRange {
                min: found.min,
                max: found.max,
                text: [found.min, found.max]
                    .map(|bound| bound.map_or_else(String::new, float_text))
                    .join(" ~ "),
                inferred: false,
            })
        });
    let numeric = matches!(value.kind(), ValueKind::Integer | ValueKind::Float);
    let range = explicit_range.or_else(|| {
        let comment = description.as_deref()?;
        let looks_numeric = numeric
            || matches!(format, ConfigFormat::Properties | ConfigFormat::OptionsTxt)
                && value
                    .as_plain_text()
                    .is_some_and(|text| text.parse::<f64>().is_ok());
        looks_numeric
            .then(|| inferred::range(comment))
            .flatten()
            .map(|range| FieldRange {
                min: Some(range.min),
                max: Some(range.max),
                text: range.text,
                inferred: true,
            })
    });

    let default = found
        .default
        .map(|value| FieldDefault {
            value,
            inferred: false,
        })
        .or_else(|| {
            description
                .as_deref()
                .and_then(inferred::default)
                .map(|value| FieldDefault {
                    value,
                    inferred: true,
                })
        });

    let options = entry
        .allowed_values
        .clone()
        .or(found.valid_values)
        .filter(|_| matches!(value, ConfigValue::String(_)));

    let (control, confidence) = control_for(entry, format, value, options, range.as_ref());

    let label = labels
        .and_then(|lang| labels::find(lang, file_path, format, &entry.path))
        .map(|label| FieldLabel {
            text: label.text,
            tooltip: label.tooltip,
            translation_key: label.key,
            locale: label.locale,
        });

    FormItem::Field(Box::new(ConfigField {
        path: to_keys(&entry.path),
        key: entry.path.to_string(),
        line: line_u32(entry.line),
        value: form,
        control,
        confidence,
        description,
        range,
        default,
        label,
    }))
}

fn control_for(
    entry: &ConfigEntry,
    format: ConfigFormat,
    value: &ConfigValue,
    options: Option<Vec<String>>,
    range: Option<&FieldRange>,
) -> (FieldControl, Confidence) {
    let bounds = |integer: bool| FieldControl::Number {
        integer,
        min: range.and_then(|range| range.min),
        max: range.and_then(|range| range.max),
    };
    let bounded = |base: Confidence| match range {
        Some(range) if range.inferred => Confidence::Low,
        Some(_) => Confidence::High,
        None => base,
    };
    if let Some(values) = options {
        return (FieldControl::Options { values }, Confidence::High);
    }
    match value {
        ConfigValue::Bool(_) => (FieldControl::Toggle, Confidence::High),
        ConfigValue::Integer(_) => (bounds(true), bounded(Confidence::High)),
        ConfigValue::Float(_) => (bounds(false), bounded(Confidence::Medium)),
        ConfigValue::List(items) => (
            FieldControl::List {
                item: list_kind(items).unwrap_or(ListItemKind::Text),
            },
            Confidence::High,
        ),
        ConfigValue::Datetime(_) | ConfigValue::Null => (FieldControl::Text, Confidence::Medium),
        ConfigValue::String(text) => {
            if inferred::is_hex_color(text) {
                return (FieldControl::Color, Confidence::Low);
            }
            // `S:` com faixa declarada (`[range: 0.0 ~ 0.5]`): o mod guarda número como texto.
            if let Some(range) = range.filter(|range| !range.inferred)
                && text.parse::<f64>().is_ok_and(f64::is_finite)
            {
                let integer = text.parse::<i64>().is_ok() && !range.text.contains('.');
                return (bounds(integer), Confidence::High);
            }
            let typed_cfg = entry
                .cfg_type
                .is_some_and(|ty| !matches!(ty, CfgType::Untyped));
            if typed_cfg || !matches!(format, ConfigFormat::Properties | ConfigFormat::OptionsTxt) {
                return (FieldControl::Text, Confidence::High);
            }
            match untyped_control(text) {
                (FieldControl::Number { integer, .. }, confidence) => {
                    (bounds(integer), bounded(confidence))
                }
                other => other,
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Edição
// ---------------------------------------------------------------------------------------------

fn invalid(path: &KeyPath, reason: impl Into<String>) -> ConfigError {
    ConfigError::InvalidValue {
        path: path.clone(),
        reason: reason.into(),
    }
}

fn parse_integer(path: &KeyPath, text: &str) -> Result<i64> {
    text.trim()
        .parse::<i64>()
        .map_err(|_| invalid(path, format!("{text:?} não é um número inteiro")))
}

fn parse_float(path: &KeyPath, text: &str) -> Result<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid(path, format!("{text:?} não é um número")))
}

fn parse_bool(path: &KeyPath, text: &str) -> Result<bool> {
    match text.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(invalid(path, format!("{text:?} não é true nem false"))),
    }
}

/// Texto do valor do formulário, para guardar como texto.
fn form_text(value: &FormValue) -> Option<String> {
    match value {
        FormValue::Bool(flag) => Some(flag.to_string()),
        FormValue::Integer(text)
        | FormValue::Float(text)
        | FormValue::String(text)
        | FormValue::Datetime(text) => Some(text.clone()),
        FormValue::List(_) => None,
    }
}

/// Converte o texto de um item para o tipo do item atual.
fn item_value(path: &KeyPath, kind: ListItemKind, text: &str) -> Result<ConfigValue> {
    Ok(match kind {
        ListItemKind::Text => ConfigValue::String(text.to_owned()),
        ListItemKind::Integer => ConfigValue::Integer(parse_integer(path, text)?),
        ListItemKind::Float => ConfigValue::Float(parse_float(path, text)?),
        ListItemKind::Bool => ConfigValue::Bool(parse_bool(path, text)?),
    })
}

/// O valor do formulário no tipo que a chave tem no arquivo: o tipo nunca muda pela edição.
fn to_config_value(
    path: &KeyPath,
    current: &ConfigValue,
    value: &FormValue,
) -> Result<ConfigValue> {
    let text = || form_text(value).ok_or_else(|| invalid(path, "a chave não é uma lista"));
    Ok(match current {
        ConfigValue::Bool(_) => match value {
            FormValue::Bool(flag) => ConfigValue::Bool(*flag),
            other => ConfigValue::Bool(parse_bool(path, &form_text(other).unwrap_or_default())?),
        },
        ConfigValue::Integer(_) => ConfigValue::Integer(parse_integer(path, &text()?)?),
        ConfigValue::Float(_) => ConfigValue::Float(parse_float(path, &text()?)?),
        ConfigValue::String(_) => ConfigValue::String(text()?),
        ConfigValue::Datetime(_) => ConfigValue::Datetime(text()?),
        ConfigValue::List(items) => {
            let FormValue::List(new_items) = value else {
                return Err(invalid(path, "a chave é uma lista"));
            };
            let kind = list_kind(items)
                .ok_or_else(|| invalid(path, "lista com itens de tipos diferentes"))?;
            ConfigValue::List(
                new_items
                    .iter()
                    .map(|item| item_value(path, kind, item))
                    .collect::<Result<_>>()?,
            )
        }
        ConfigValue::Null => return Err(invalid(path, "valor nulo só se edita como texto")),
    })
}

/// Aplica as edições do formulário ao texto do arquivo `path` e devolve o texto novo.
///
/// Só os bytes dos valores mudam ([`ConfigDocument::apply`] relê e confere cada valor). Além
/// disso, os bytes antes do primeiro valor editado e depois do último são comparados com os
/// originais: se algo fora dos valores mudou, nada é devolvido
/// ([`ConfigError::EditNotConfirmed`]) e a interface cai para o editor de texto.
///
/// # Errors
///
/// Os de [`ConfigDocument::parse`] e [`ConfigDocument::apply`], e
/// [`ConfigError::InvalidValue`] para valor que não serve ao tipo da chave.
pub fn apply_form_edits(path: &str, text: &str, edits: &[FormEdit]) -> Result<String> {
    let bytes = text.as_bytes();
    let format = detect(path, bytes).map_err(|_| ConfigError::EditNotConfirmed {
        detail: "o arquivo não tem formulário".into(),
    })?;
    let document = ConfigDocument::parse(format, bytes)?;
    let mut converted = Vec::with_capacity(edits.len());
    let mut first = usize::MAX;
    let mut last = 0;
    for edit in edits {
        let key = from_keys(&edit.path);
        let entry = document
            .get(&key)
            .ok_or_else(|| ConfigError::PathNotFound { path: key.clone() })?;
        let current = entry
            .value
            .as_ref()
            .ok_or_else(|| ConfigError::NotEditable {
                path: key.clone(),
                reason: "é uma seção; edite as chaves dentro dela".into(),
            })?;
        if let Some(span) = entry.value_span {
            first = first.min(span.start);
            last = last.max(span.end);
        }
        let value = to_config_value(&key, current, &edit.value)?;
        converted.push(ConfigEdit::new(key, value));
    }
    let edited = document.apply(&converted)?;
    let out = edited.as_bytes();
    if !converted.is_empty() {
        let tail = bytes.len().saturating_sub(last);
        let same_head = out.get(..first) == bytes.get(..first);
        let same_tail = out.len() >= tail && out.get(out.len() - tail..) == bytes.get(last..);
        if !(same_head && same_tail) {
            return Err(ConfigError::EditNotConfirmed {
                detail: "bytes fora do valor mudariam".into(),
            });
        }
    }
    String::from_utf8(edited.into_bytes()).map_err(|error| ConfigError::EditNotConfirmed {
        detail: error.to_string(),
    })
}

#[cfg(test)]
mod tests;
