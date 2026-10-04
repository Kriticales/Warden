//! `options.txt` do Minecraft: uma opção por linha, `chave:valor` (o primeiro `:` separa; o
//! valor pode ter `:`, como em `lastServer:127.0.0.1:25565`).
//!
//! Linhas sem `:` são mantidas e ignoradas. O valor é texto; duas formas JSON que o jogo grava
//! são lidas como tal: texto entre aspas (`soundDevice:""`, versões novas) e listas
//! (`resourcePacks:["vanilla"]`). Números e liga/desliga ficam como texto (o tipo é inferido
//! pelo formulário, C-04).

use jsonc_parser::ast::Value;
use jsonc_parser::{CollectOptions, ParseOptions};

use super::{Builder, EditStyle, OptionsStyle, Parsed, Rendered, ValueEntry, invalid, json};
use crate::error::Result;
use crate::path::KeyPath;
use crate::text::DocText;
use crate::tree::ConfigValue;

pub(crate) fn parse(doc: &DocText<'_>) -> Parsed {
    let mut builder = Builder::new(doc);
    for (index, line) in doc.lines.iter().enumerate() {
        let text = doc.line_text(index);
        let Some((key, raw)) = text.split_once(':') else {
            continue;
        };
        let value_start = line.start + key.len() + 1;
        let (value, style) = read_value(raw);
        builder.value(ValueEntry {
            path: KeyPath::from_keys([key]),
            value,
            line_index: index,
            span: value_start..line.end,
            style: EditStyle::Options(style),
            comment: None,
            cfg_type: None,
        });
    }
    builder.finish()
}

fn strict_json(raw: &str) -> Option<ConfigValue> {
    let options = ParseOptions {
        allow_comments: false,
        allow_loose_object_property_names: false,
        allow_trailing_commas: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    };
    let parsed = jsonc_parser::parse_to_ast(raw, &CollectOptions::default(), &options).ok()?;
    match parsed.value? {
        Value::StringLit(literal) => Some(ConfigValue::String(literal.value.to_string())),
        Value::Array(array) => {
            let mut items = Vec::new();
            for element in &array.elements {
                items.push(match element {
                    Value::StringLit(literal) => ConfigValue::String(literal.value.to_string()),
                    Value::NumberLit(literal) => json::number(literal.value),
                    Value::BooleanLit(literal) => ConfigValue::Bool(literal.value),
                    _ => return None,
                });
            }
            Some(ConfigValue::List(items))
        }
        _ => None,
    }
}

fn read_value(raw: &str) -> (ConfigValue, OptionsStyle) {
    let looks_quoted = raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"');
    if (looks_quoted || raw.starts_with('['))
        && let Some(value) = strict_json(raw)
    {
        let style = if matches!(value, ConfigValue::List(_)) {
            OptionsStyle::JsonList
        } else {
            OptionsStyle::Quoted
        };
        return (value, style);
    }
    (ConfigValue::String(raw.to_owned()), OptionsStyle::Raw)
}

fn compact_json(value: &ConfigValue) -> Option<String> {
    match value {
        ConfigValue::List(items) => {
            let parts: Option<Vec<String>> = items.iter().map(compact_json).collect();
            Some(format!("[{}]", parts?.join(",")))
        }
        ConfigValue::String(text) => Some(json::quote(text, '"')),
        ConfigValue::Null => None,
        other => other.as_plain_text(),
    }
}

pub(crate) fn render(path: &KeyPath, value: &ConfigValue, style: OptionsStyle) -> Result<Rendered> {
    if let ConfigValue::List(items) = value {
        if items
            .iter()
            .any(|item| matches!(item, ConfigValue::List(_) | ConfigValue::Null))
        {
            return Err(invalid(
                path,
                "options.txt só aceita listas de valores simples",
            ));
        }
        let text = compact_json(value).unwrap_or_default();
        return Ok(Rendered {
            text,
            expected: value.clone(),
        });
    }
    let text = value
        .as_plain_text()
        .ok_or_else(|| invalid(path, "options.txt não tem valor nulo"))?;
    if text.contains(['\n', '\r']) {
        return Err(invalid(
            path,
            "options.txt não aceita quebra de linha no valor",
        ));
    }
    if style == OptionsStyle::Quoted && matches!(value, ConfigValue::String(_)) {
        return Ok(Rendered {
            text: json::quote(&text, '"'),
            expected: ConfigValue::String(text),
        });
    }
    // Um texto cru que pareça JSON seria relido como outro tipo.
    let (reread, _) = read_value(&text);
    if reread != ConfigValue::String(text.clone()) {
        return Err(invalid(
            path,
            "esse texto seria lido como JSON pelo jogo; use o editor de texto",
        ));
    }
    Ok(Rendered {
        text: text.clone(),
        expected: ConfigValue::String(text),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_valores_crus_aspas_e_listas() {
        let doc = DocText::from_str(
            "version:3955\nlastServer:127.0.0.1:25565\nsoundDevice:\"\"\nresourcePacks:[\"vanilla\",\"file/x.zip\"]\nsemdoispontos\nbad:[1,\n",
        );
        let parsed = parse(&doc);
        let values: Vec<_> = parsed
            .entries
            .iter()
            .map(|e| e.value.clone().unwrap())
            .collect();
        assert_eq!(values[0], ConfigValue::String("3955".into()));
        assert_eq!(values[1], ConfigValue::String("127.0.0.1:25565".into()));
        assert_eq!(values[2], ConfigValue::String(String::new()));
        assert_eq!(
            values[3],
            ConfigValue::List(vec![
                ConfigValue::String("vanilla".into()),
                ConfigValue::String("file/x.zip".into())
            ])
        );
        assert_eq!(values[4], ConfigValue::String("[1,".into()));
        assert_eq!(parsed.entries.len(), 5);
        assert_eq!(parsed.styles[2], EditStyle::Options(OptionsStyle::Quoted));
    }

    #[test]
    fn escrita_por_estilo() {
        let path = KeyPath::from_keys(["k"]);
        let quoted = render(
            &path,
            &ConfigValue::String("a\"b".into()),
            OptionsStyle::Quoted,
        );
        assert_eq!(quoted.unwrap().text, "\"a\\\"b\"");
        let list = ConfigValue::List(vec![
            ConfigValue::String("a".into()),
            ConfigValue::Integer(1),
        ]);
        assert_eq!(
            render(&path, &list, OptionsStyle::Raw).unwrap().text,
            "[\"a\",1]"
        );
        let raw = render(&path, &ConfigValue::Float(0.5), OptionsStyle::Raw).unwrap();
        assert_eq!(
            (raw.text.as_str(), raw.expected),
            ("0.5", ConfigValue::String("0.5".into()))
        );
        assert!(
            render(
                &path,
                &ConfigValue::String("[\"x\"]".into()),
                OptionsStyle::Raw
            )
            .is_err()
        );
        assert!(
            render(
                &path,
                &ConfigValue::String("a\nb".into()),
                OptionsStyle::Raw
            )
            .is_err()
        );
        assert!(render(&path, &ConfigValue::Null, OptionsStyle::Raw).is_err());
        let nested = ConfigValue::List(vec![ConfigValue::List(vec![])]);
        assert!(render(&path, &nested, OptionsStyle::JsonList).is_err());
    }
}
