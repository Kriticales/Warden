//! JSON, JSONC e JSON5 com `jsonc-parser` (árvore com posições).
//!
//! O `jsonc-parser` 0.34 aceita a sintaxe toda do JSON5 (aspas simples, chaves sem aspas,
//! vírgulas finais, hexadecimal, `Infinity`/`NaN`, `+1`, `.5`), então um parser só cobre os três
//! formatos. Avaliação da C-01 com o corpus: leu os 52 arquivos JSON/JSONC/JSON5; o `json-five`
//! (alternativa da ARCHITECTURE §10) não devolveu o texto idêntico em 10 deles e ficou de fora.
//! A leitura é tolerante como o Gson dos mods (comentários e vírgulas finais em `.json`).

use std::fmt::Write as _;

use jsonc_parser::ast::{Array, Object, ObjectPropName, Value};
use jsonc_parser::{CollectOptions, ParseOptions};

use super::{Builder, EditStyle, Parsed, Rendered, ValueEntry, invalid};
use crate::error::Result;
use crate::format::ConfigFormat;
use crate::path::KeyPath;
use crate::text::DocText;
use crate::tree::{ConfigValue, float_text};

pub(crate) fn parse(format: ConfigFormat, doc: &DocText<'_>) -> Result<Parsed> {
    let result = jsonc_parser::parse_to_ast(
        doc.body,
        &CollectOptions::default(),
        &ParseOptions::default(),
    )
    .map_err(|error| doc.parse_error(format, error.range().start, error.kind().to_string()))?;
    let mut walker = Walker {
        builder: Builder::new(doc),
    };
    if let Some(root) = &result.value {
        walker.value(root, &KeyPath::root(), None);
    }
    Ok(walker.builder.finish())
}

struct Walker<'d, 'a> {
    builder: Builder<'d, 'a>,
}

impl Walker<'_, '_> {
    fn comment(&self, line_index: usize) -> Option<String> {
        self.builder
            .doc
            .leading_comment(line_index, strip_json_comment)
    }

    fn value(&mut self, value: &Value<'_>, path: &KeyPath, key_start: Option<usize>) {
        let start = key_start.unwrap_or_else(|| value_range(value).start);
        let line_index = self.builder.doc.line_index_of(start);
        let comment = key_start.and_then(|_| self.comment(line_index));
        match value {
            Value::Object(object) => {
                self.builder.section(path.clone(), line_index, comment);
                self.object(object, path);
            }
            Value::Array(array) if !is_scalar_array(array) => {
                self.builder.section(path.clone(), line_index, comment);
                for (index, element) in array.elements.iter().enumerate() {
                    self.value(element, &path.child_index(index), None);
                }
            }
            _ => {
                let Some(converted) = convert(value) else {
                    return;
                };
                let range = value_range(value);
                let single_quote = matches!(value, Value::StringLit(_))
                    && self
                        .builder
                        .doc
                        .body
                        .get(range.start..)
                        .is_some_and(|raw| raw.starts_with('\''));
                self.builder.value(ValueEntry {
                    path: path.clone(),
                    value: converted,
                    line_index,
                    span: range.start..range.end,
                    style: EditStyle::Json { single_quote },
                    comment,
                    cfg_type: None,
                });
            }
        }
    }

    fn object(&mut self, object: &Object<'_>, path: &KeyPath) {
        for property in &object.properties {
            let key_start = match &property.name {
                ObjectPropName::String(literal) => literal.range.start,
                ObjectPropName::Word(word) => word.range.start,
            };
            self.value(
                &property.value,
                &path.child_key(property.name.as_str()),
                Some(key_start),
            );
        }
    }
}

fn value_range(value: &Value<'_>) -> jsonc_parser::common::Range {
    match value {
        Value::StringLit(literal) => literal.range,
        Value::NumberLit(literal) => literal.range,
        Value::BooleanLit(literal) => literal.range,
        Value::Object(object) => object.range,
        Value::Array(array) => array.range,
        Value::NullKeyword(keyword) => keyword.range,
    }
}

fn is_scalar_array(array: &Array<'_>) -> bool {
    array.elements.iter().all(|element| match element {
        Value::Object(_) => false,
        Value::Array(inner) => is_scalar_array(inner),
        _ => true,
    })
}

/// Tira os marcadores de uma linha de comentário JSON (`//`, `/*`, `*`, `*/`).
fn strip_json_comment(text: &str) -> Option<&str> {
    let rest = if let Some(rest) = text.strip_prefix("//") {
        rest
    } else if let Some(rest) = text.strip_prefix("/*") {
        rest.trim_start_matches('*')
    } else if text == "*/" {
        ""
    } else if text.starts_with('*') {
        text.trim_start_matches('*')
    } else {
        return None;
    };
    let rest = rest.trim_end();
    let rest = rest.strip_suffix("*/").unwrap_or(rest);
    Some(rest.trim())
}

fn convert(value: &Value<'_>) -> Option<ConfigValue> {
    Some(match value {
        Value::StringLit(literal) => ConfigValue::String(literal.value.to_string()),
        Value::NumberLit(literal) => number(literal.value),
        Value::BooleanLit(literal) => ConfigValue::Bool(literal.value),
        Value::NullKeyword(_) => ConfigValue::Null,
        Value::Array(array) => {
            ConfigValue::List(array.elements.iter().filter_map(convert).collect())
        }
        Value::Object(_) => return None,
    })
}

/// Número JSON/JSON5: inteiro quando cabe em `i64` e não tem ponto nem expoente; senão
/// número com casas decimais (hexadecimal vira inteiro).
pub(crate) fn number(raw: &str) -> ConfigValue {
    let (negative, unsigned) = match raw.as_bytes().first() {
        Some(b'-') => (true, raw.get(1..).unwrap_or("")),
        Some(b'+') => (false, raw.get(1..).unwrap_or("")),
        _ => (false, raw),
    };
    if let Some(hex) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        if let Ok(value) = i64::from_str_radix(hex, 16) {
            return ConfigValue::Integer(if negative {
                value.wrapping_neg()
            } else {
                value
            });
        }
        return ConfigValue::Float(f64::NAN);
    }
    let is_integer = !unsigned.is_empty() && unsigned.bytes().all(|byte| byte.is_ascii_digit());
    if is_integer && let Ok(value) = raw.trim_start_matches('+').parse::<i64>() {
        return ConfigValue::Integer(value);
    }
    let magnitude = match unsigned {
        "Infinity" => f64::INFINITY,
        "NaN" => f64::NAN,
        other => other.parse::<f64>().unwrap_or(f64::NAN),
    };
    ConfigValue::Float(if negative { -magnitude } else { magnitude })
}

/// Texto JSON de uma string, com as aspas pedidas.
pub(crate) fn quote(text: &str, quote: char) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_control() => {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    let _ = write!(out, "\\u{unit:04x}"); // escrever numa String não falha
                }
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

fn render_value(
    format: ConfigFormat,
    path: &KeyPath,
    value: &ConfigValue,
    quote_char: char,
) -> Result<String> {
    Ok(match value {
        ConfigValue::Null => "null".to_owned(),
        ConfigValue::Bool(flag) => flag.to_string(),
        ConfigValue::Integer(number) => number.to_string(),
        ConfigValue::Float(number) => {
            if !number.is_finite() && format != ConfigFormat::Json5 {
                return Err(invalid(path, "JSON não aceita NaN nem infinito (só JSON5)"));
            }
            float_text(*number)
        }
        ConfigValue::String(text) | ConfigValue::Datetime(text) => quote(text, quote_char),
        ConfigValue::List(items) => {
            let rendered: Result<Vec<String>> = items
                .iter()
                .map(|item| render_value(format, path, item, quote_char))
                .collect();
            format!("[{}]", rendered?.join(", "))
        }
    })
}

pub(crate) fn render(
    format: ConfigFormat,
    path: &KeyPath,
    value: &ConfigValue,
    single_quote: bool,
) -> Result<Rendered> {
    let quote_char = if single_quote && format == ConfigFormat::Json5 {
        '\''
    } else {
        '"'
    };
    let text = render_value(format, path, value, quote_char)?;
    let expected = match value {
        ConfigValue::Datetime(text) => ConfigValue::String(text.clone()),
        other => other.clone(),
    };
    Ok(Rendered { text, expected })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeros() {
        assert_eq!(number("42"), ConfigValue::Integer(42));
        assert_eq!(number("-7"), ConfigValue::Integer(-7));
        assert_eq!(number("+3"), ConfigValue::Integer(3));
        assert_eq!(number("0x1F"), ConfigValue::Integer(31));
        assert_eq!(number("-0x10"), ConfigValue::Integer(-16));
        assert_eq!(number("1.5"), ConfigValue::Float(1.5));
        assert_eq!(number(".5"), ConfigValue::Float(0.5));
        assert_eq!(number("1e3"), ConfigValue::Float(1000.0));
        assert_eq!(number("-Infinity"), ConfigValue::Float(f64::NEG_INFINITY));
        assert!(matches!(number("NaN"), ConfigValue::Float(v) if v.is_nan()));
        assert_eq!(number("99999999999999999999"), ConfigValue::Float(1e20));
        assert!(matches!(number("0xZZ"), ConfigValue::Float(v) if v.is_nan()));
    }

    #[test]
    fn aspas_e_escapes() {
        assert_eq!(quote("a\"b\\c\n\u{1}", '"'), "\"a\\\"b\\\\c\\n\\u0001\"");
        assert_eq!(quote("it's", '\''), "'it\\'s'");
        assert_eq!(quote("é\t\r\u{8}\u{c}", '"'), "\"é\\t\\r\\b\\f\"");
    }

    #[test]
    fn comentarios_json() {
        assert_eq!(strip_json_comment("// texto"), Some("texto"));
        assert_eq!(strip_json_comment("/* bloco */"), Some("bloco"));
        assert_eq!(strip_json_comment("* meio"), Some("meio"));
        assert_eq!(strip_json_comment("*/"), Some(""));
        assert_eq!(strip_json_comment("\"chave\": 1"), None);
    }

    #[test]
    fn nao_finitos_so_no_json5() {
        let path = KeyPath::from_keys(["x"]);
        assert!(
            render(
                ConfigFormat::Json,
                &path,
                &ConfigValue::Float(f64::NAN),
                false
            )
            .is_err()
        );
        let ok = render(
            ConfigFormat::Json5,
            &path,
            &ConfigValue::Float(f64::NAN),
            false,
        )
        .unwrap();
        assert_eq!(ok.text, "NaN");
        let dt = render(
            ConfigFormat::Json,
            &path,
            &ConfigValue::Datetime("2020".into()),
            false,
        )
        .unwrap();
        assert_eq!(dt.expected, ConfigValue::String("2020".into()));
    }
}
