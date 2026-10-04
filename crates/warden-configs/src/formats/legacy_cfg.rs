//! `.cfg` do `Configuration` do Forge antigo (1.7.10–1.12.2), lido como o `Configuration.load`
//! do Forge (branch `1.12.x`) lê:
//!
//! ```text
//! # Configuration file
//! ~CONFIG_VERSION: 1.2
//!
//! general {
//!     # Comentário
//!     B:enableFeature=true
//!     S:"nome com espaço"=texto
//!     S:blacklist <
//!         minecraft:stone
//!      >
//!     sub {
//!         I:value=3
//!     }
//! }
//! ```
//!
//! - Nomes: letras, dígitos e `._-`, ou qualquer coisa entre aspas; `T:` antes do nome é o tipo.
//! - O valor é todo o resto da linha depois do `=` (sem cortar espaços, como no Forge).
//! - Lista: a linha termina em `<`; cada linha seguinte é um item (com espaços cortados) até
//!   a linha cujo primeiro caractere visível é `>`.
//! - `#` começa comentário fora de listas; `~CONFIG_VERSION:` e `START:`/`END:` são marcadores.
//!
//! Diferenças deliberadas: o Forge lança exceção em arquivo corrompido; aqui vira
//! [`ConfigError::Parse`](crate::ConfigError::Parse) com a linha. O Forge também separa linhas
//! por `\r` sozinho; aqui só `\n` (com ou sem `\r` antes) separa.

use super::{Builder, EditStyle, Parsed, Rendered, ValueEntry, invalid};
use crate::error::Result;
use crate::format::ConfigFormat;
use crate::path::KeyPath;
use crate::text::{DocText, strip_line_comment};
use crate::tree::{CfgType, ConfigValue, float_text};

const FORMAT: ConfigFormat = ConfigFormat::LegacyCfg;

/// `true` se o texto parece um `.cfg` do Forge: lê sem erro e tem pelo menos uma categoria.
pub(crate) fn looks_like_forge(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let doc = DocText::from_str(text);
    parse(&doc).is_ok_and(|parsed| !parsed.entries.is_empty())
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '_' | '-')
}

struct OpenList {
    path: KeyPath,
    ty: CfgType,
    line_index: usize,
    property_indent: String,
    items: Vec<(usize, String)>,
    comment: Option<String>,
}

pub(crate) fn parse(doc: &DocText<'_>) -> Result<Parsed> {
    let mut builder = Builder::new(doc);
    let mut categories: Vec<String> = Vec::new();
    let mut list: Option<OpenList> = None;
    for (index, line) in doc.lines.iter().enumerate() {
        let text = doc.line_text(index);
        let trimmed = text.trim();
        if let Some(open) = list.as_mut() {
            if trimmed.starts_with('>') {
                if let Some(done) = list.take() {
                    close_list(&mut builder, done, line.start);
                }
            } else {
                open.items.push((line.start, trimmed.to_owned()));
            }
            continue;
        }
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('~')
            || is_block_marker(trimmed)
        {
            continue;
        }
        let comment = || doc.leading_comment(index, |text| strip_line_comment(text, &['#']));
        match scan_line(text)
            .map_err(|(offset, message)| doc.parse_error(FORMAT, line.start + offset, message))?
        {
            LineItem::Nothing => {}
            LineItem::Open(name) => {
                categories.push(name);
                builder.section(KeyPath::from_keys(categories.clone()), index, comment());
            }
            LineItem::Close => {
                if categories.pop().is_none() {
                    return Err(doc.parse_error(FORMAT, line.start, "'}' sem categoria aberta"));
                }
            }
            LineItem::Property {
                name,
                ty,
                value_offset,
            } => {
                if categories.is_empty() {
                    return Err(doc.parse_error(
                        FORMAT,
                        line.start,
                        "propriedade fora de categoria",
                    ));
                }
                let raw = text.get(value_offset..).unwrap_or("");
                builder.value(ValueEntry {
                    path: KeyPath::from_keys(categories.iter().cloned()).child_key(&name),
                    value: typed(ty, raw),
                    line_index: index,
                    span: line.start + value_offset..line.end,
                    style: EditStyle::Cfg { ty },
                    comment: comment(),
                    cfg_type: Some(ty),
                });
            }
            LineItem::List { name, ty } => {
                if categories.is_empty() {
                    return Err(doc.parse_error(FORMAT, line.start, "lista fora de categoria"));
                }
                let indent_len = text.len() - text.trim_start().len();
                list = Some(OpenList {
                    path: KeyPath::from_keys(categories.iter().cloned()).child_key(&name),
                    ty,
                    line_index: index,
                    property_indent: text.get(..indent_len).unwrap_or("").to_owned(),
                    items: Vec::new(),
                    comment: comment(),
                });
            }
        }
    }
    if list.is_some() {
        return Err(doc.parse_error(FORMAT, doc.body.len(), "lista sem '>' de fechamento"));
    }
    if !categories.is_empty() {
        return Err(doc.parse_error(FORMAT, doc.body.len(), "categoria sem '}' de fechamento"));
    }
    Ok(builder.finish())
}

fn is_block_marker(trimmed: &str) -> bool {
    ["START:", "END:"].iter().any(|marker| {
        trimmed
            .strip_prefix(marker)
            .is_some_and(|rest| rest.trim().starts_with('"') && rest.trim().ends_with('"'))
    })
}

fn close_list(builder: &mut Builder<'_, '_>, list: OpenList, closing_start: usize) {
    let start = list
        .items
        .first()
        .map_or(closing_start, |(offset, _)| *offset);
    let item_indent = list
        .items
        .first()
        .and_then(|(offset, _)| {
            let line = builder.doc.line_text(builder.doc.line_index_of(*offset));
            let indent = line.len() - line.trim_start().len();
            line.get(..indent).map(str::to_owned)
        })
        .filter(|indent| !indent.is_empty())
        .unwrap_or_else(|| format!("{}    ", list.property_indent));
    let values = list
        .items
        .iter()
        .map(|(_, item)| typed(list.ty, item))
        .collect();
    builder.value(ValueEntry {
        path: list.path,
        value: ConfigValue::List(values),
        line_index: list.line_index,
        span: start..closing_start,
        style: EditStyle::CfgList {
            ty: list.ty,
            item_indent,
        },
        comment: list.comment,
        cfg_type: Some(list.ty),
    });
}

enum LineItem {
    Nothing,
    Open(String),
    Close,
    Property {
        name: String,
        ty: CfgType,
        value_offset: usize,
    },
    List {
        name: String,
        ty: CfgType,
    },
}

/// Lê uma linha fora de lista, caractere a caractere, como o laço do `Configuration.load`.
fn scan_line(line: &str) -> std::result::Result<LineItem, (usize, String)> {
    let mut name: Option<(usize, usize)> = None;
    let mut quoted = false;
    let mut ty = CfgType::Untyped;
    let mut item = LineItem::Nothing;
    let name_text =
        |name: Option<(usize, usize)>| name.and_then(|(start, end)| line.get(start..end));
    for (position, c) in line.char_indices() {
        if is_name_char(c) || (quoted && c != '"') {
            let end = position + c.len_utf8();
            name = Some(name.map_or((position, end), |(start, _)| (start, end)));
            continue;
        }
        if c.is_whitespace() {
            continue;
        }
        match c {
            '#' => break,
            '"' => {
                if quoted {
                    quoted = false;
                } else if name.is_none() {
                    quoted = true;
                }
            }
            '{' => {
                let text = name_text(name).ok_or((position, "categoria sem nome".to_owned()))?;
                item = LineItem::Open(text.to_owned());
                name = None;
            }
            '}' => item = LineItem::Close,
            '=' => {
                let text = name_text(name).ok_or((position, "propriedade sem nome".to_owned()))?;
                return Ok(LineItem::Property {
                    name: text.to_owned(),
                    ty,
                    value_offset: position + 1,
                });
            }
            ':' => {
                let prefix = name_text(name).and_then(|text| text.chars().next());
                ty = prefix.map_or(CfgType::Untyped, CfgType::from_prefix);
                name = None;
            }
            '<' => {
                if position + 1 != line.len() {
                    return Err((
                        position,
                        "'<' de lista precisa ser o último caractere".into(),
                    ));
                }
                let text = name_text(name).ok_or((position, "lista sem nome".to_owned()))?;
                return Ok(LineItem::List {
                    name: text.to_owned(),
                    ty,
                });
            }
            '~' => {}
            other => return Err((position, format!("caractere inesperado {other:?}"))),
        }
    }
    if quoted {
        return Err((line.len(), "aspas sem fechamento".into()));
    }
    Ok(item)
}

/// Valor tipado pelo prefixo; texto que o Forge não leria no tipo fica como texto.
fn typed(ty: CfgType, raw: &str) -> ConfigValue {
    let text = || ConfigValue::String(raw.to_owned());
    match ty {
        CfgType::Boolean if raw.eq_ignore_ascii_case("true") => ConfigValue::Bool(true),
        CfgType::Boolean if raw.eq_ignore_ascii_case("false") => ConfigValue::Bool(false),
        CfgType::Integer => raw
            .parse::<i64>()
            .map_or_else(|_| text(), ConfigValue::Integer),
        CfgType::Double => parse_java_double(raw).map_or_else(text, ConfigValue::Float),
        _ => text(),
    }
}

fn parse_java_double(raw: &str) -> Option<f64> {
    match raw {
        "NaN" => Some(f64::NAN),
        "Infinity" | "+Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        _ if raw
            .chars()
            .any(|c| c.is_ascii_alphabetic() && !matches!(c, 'e' | 'E')) =>
        {
            None
        }
        _ => raw.parse().ok(),
    }
}

fn scalar_text(path: &KeyPath, value: &ConfigValue, ty: CfgType) -> Result<(String, ConfigValue)> {
    let text = match (ty, value) {
        (CfgType::Boolean, ConfigValue::Bool(flag)) => flag.to_string(),
        (CfgType::Boolean, _) => return Err(invalid(path, "a chave B: só aceita true ou false")),
        (CfgType::Integer, ConfigValue::Integer(number)) => number.to_string(),
        (CfgType::Integer, _) => return Err(invalid(path, "a chave I: só aceita número inteiro")),
        (CfgType::Double, ConfigValue::Float(number)) => float_text(*number),
        (CfgType::Double, ConfigValue::Integer(number)) => format!("{number}.0"),
        (CfgType::Double, _) => return Err(invalid(path, "a chave D: só aceita número")),
        (_, other) => other
            .as_plain_text()
            .ok_or_else(|| invalid(path, "valor sem forma de texto no .cfg"))?,
    };
    if text.contains(['\n', '\r']) {
        return Err(invalid(path, "o .cfg não aceita quebra de linha no valor"));
    }
    let expected = typed(ty, &text);
    Ok((text, expected))
}

pub(crate) fn render_scalar(path: &KeyPath, value: &ConfigValue, ty: CfgType) -> Result<Rendered> {
    if matches!(value, ConfigValue::List(_)) {
        return Err(invalid(path, "essa chave não é uma lista"));
    }
    let (text, expected) = scalar_text(path, value, ty)?;
    Ok(Rendered { text, expected })
}

pub(crate) fn render_list(
    path: &KeyPath,
    value: &ConfigValue,
    ty: CfgType,
    item_indent: &str,
    line_ending: &str,
) -> Result<Rendered> {
    let ConfigValue::List(items) = value else {
        return Err(invalid(path, "essa chave é uma lista"));
    };
    let mut text = String::new();
    let mut expected = Vec::with_capacity(items.len());
    for item in items {
        if matches!(item, ConfigValue::List(_)) {
            return Err(invalid(path, "lista dentro de lista não existe no .cfg"));
        }
        let (item_text, item_value) = scalar_text(path, item, ty)?;
        if item_text.trim() != item_text || item_text.starts_with('>') || item_text.ends_with('<') {
            return Err(invalid(
                path,
                "item de lista do .cfg não pode ter espaço nas pontas, começar com '>' nem                  terminar com '<'",
            ));
        }
        text.push_str(item_indent);
        text.push_str(&item_text);
        text.push_str(line_ending);
        expected.push(item_value);
    }
    Ok(Rendered {
        text,
        expected: ConfigValue::List(expected),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::EntryKind;

    const SAMPLE: &str = "# Configuration file\n~CONFIG_VERSION: 1.2\n\ngeneral {\n    # Liga\n    # [default: true]\n    B:enable=true\n    I:max=64\n    D:speed=1.5\n    S:\"com espaço\"=a = b # não é comentário\n    S:list <\n        minecraft:stone\n        # item\n     >\n    S:vazia <\n     >\n\n    \"sub cat\" {\n        I:x=nope\n    }\n}\n";

    #[test]
    fn le_categorias_tipos_e_listas() {
        let doc = DocText::from_str(SAMPLE);
        let parsed = parse(&doc).unwrap();
        let shown: Vec<(String, Option<ConfigValue>)> = parsed
            .entries
            .iter()
            .map(|entry| (entry.path.to_string(), entry.value.clone()))
            .collect();
        let s = |t: &str| ConfigValue::String(t.into());
        assert_eq!(
            shown,
            vec![
                ("general".into(), None),
                ("general.enable".into(), Some(ConfigValue::Bool(true))),
                ("general.max".into(), Some(ConfigValue::Integer(64))),
                ("general.speed".into(), Some(ConfigValue::Float(1.5))),
                (
                    "general.\"com espaço\"".into(),
                    Some(s("a = b # não é comentário"))
                ),
                (
                    "general.list".into(),
                    Some(ConfigValue::List(vec![s("minecraft:stone"), s("# item")]))
                ),
                ("general.vazia".into(), Some(ConfigValue::List(vec![]))),
                ("general.\"sub cat\"".into(), None),
                ("general.\"sub cat\".x".into(), Some(s("nope"))),
            ]
        );
        let enable = &parsed.entries[1];
        assert_eq!(enable.comment.as_deref(), Some("Liga\n[default: true]"));
        assert_eq!(enable.cfg_type, Some(CfgType::Boolean));
        assert_eq!(parsed.entries[0].kind, EntryKind::Section);
        assert_eq!(
            parsed.styles[6],
            EditStyle::CfgList {
                ty: CfgType::String,
                item_indent: "        ".into()
            }
        );
        assert!(looks_like_forge(SAMPLE.as_bytes()));
    }

    #[test]
    fn arquivos_corrompidos_viram_erro_com_linha() {
        let cases = [
            ("}\n", 1),
            ("cat {\n", 1),
            ("B:solto=true\n", 1),
            ("cat {\n  S:l <\n  a\n", 3),
            ("cat {\n  S:l < x\n}\n", 2),
            ("cat {\n  S:\"aberta=1\n}\n", 2),
            ("cat {\n  ?\n}\n", 2),
            ("cat {\n  ={\n}\n", 2),
            ("{\n", 1),
            ("cat {\n  <\n}\n", 2),
        ];
        for (text, line) in cases {
            let doc = DocText::from_str(text);
            match parse(&doc) {
                Err(crate::ConfigError::Parse { line: found, .. }) => {
                    assert!(found >= line, "{text:?}: linha {found}");
                }
                other => panic!("{text:?}: esperava erro, veio {other:?}"),
            }
        }
        assert!(!looks_like_forge(b"[ini]\nkey=value\n"));
        assert!(!looks_like_forge(b"\xff"));
        assert!(!looks_like_forge(b"# so comentario\n"));
    }

    #[test]
    fn marcadores_de_bloco_sao_ignorados() {
        let doc = DocText::from_str("START: \"a\"\ncat {\n    I:x=1\n}\nEND: \"a\"\n");
        assert_eq!(parse(&doc).unwrap().entries.len(), 2);
    }

    #[test]
    fn tipos_na_escrita() {
        let path = KeyPath::from_keys(["c", "k"]);
        let bool_ok = render_scalar(&path, &ConfigValue::Bool(false), CfgType::Boolean).unwrap();
        assert_eq!(bool_ok.text, "false");
        assert!(render_scalar(&path, &ConfigValue::Integer(1), CfgType::Boolean).is_err());
        assert!(render_scalar(&path, &ConfigValue::Float(1.0), CfgType::Integer).is_err());
        assert!(render_scalar(&path, &ConfigValue::Bool(true), CfgType::Double).is_err());
        let double = render_scalar(&path, &ConfigValue::Integer(2), CfgType::Double).unwrap();
        assert_eq!(
            (double.text.as_str(), double.expected),
            ("2.0", ConfigValue::Float(2.0))
        );
        let nan = render_scalar(&path, &ConfigValue::Float(f64::NAN), CfgType::Double).unwrap();
        assert_eq!(nan.text, "NaN");
        let text = render_scalar(&path, &ConfigValue::Integer(5), CfgType::String).unwrap();
        assert_eq!(text.expected, ConfigValue::String("5".into()));
        assert!(
            render_scalar(&path, &ConfigValue::String("a\nb".into()), CfgType::String).is_err()
        );
        assert!(render_scalar(&path, &ConfigValue::List(vec![]), CfgType::String).is_err());
        assert!(render_scalar(&path, &ConfigValue::Null, CfgType::String).is_err());
    }

    #[test]
    fn listas_na_escrita() {
        let path = KeyPath::from_keys(["c", "l"]);
        let items = ConfigValue::List(vec![ConfigValue::Integer(1), ConfigValue::Integer(2)]);
        let rendered = render_list(&path, &items, CfgType::Integer, "  ", "\r\n").unwrap();
        assert_eq!(rendered.text, "  1\r\n  2\r\n");
        let empty = render_list(
            &path,
            &ConfigValue::List(vec![]),
            CfgType::String,
            " ",
            "\n",
        );
        assert_eq!(empty.unwrap().text, "");
        let bad = |item: &str| ConfigValue::List(vec![ConfigValue::String(item.into())]);
        assert!(render_list(&path, &bad(" x"), CfgType::String, "", "\n").is_err());
        assert!(render_list(&path, &bad(">x"), CfgType::String, "", "\n").is_err());
        assert!(render_list(&path, &bad("x<"), CfgType::String, "", "\n").is_err());
        assert!(render_list(&path, &ConfigValue::Integer(1), CfgType::String, "", "\n").is_err());
        let nested = ConfigValue::List(vec![ConfigValue::List(vec![])]);
        assert!(render_list(&path, &nested, CfgType::String, "", "\n").is_err());
    }

    #[test]
    fn doubles_do_java() {
        assert_eq!(parse_java_double("1.0E-4"), Some(1.0e-4));
        assert_eq!(parse_java_double("-Infinity"), Some(f64::NEG_INFINITY));
        assert!(parse_java_double("NaN").is_some_and(f64::is_nan));
        assert_eq!(parse_java_double("1d"), None);
        assert_eq!(parse_java_double("abc"), None);
    }
}
