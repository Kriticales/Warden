//! `.properties` do Java, com as regras do `java.util.Properties.load`.
//!
//! - Linha de comentário: o primeiro caractere que não é espaço é `#` ou `!`.
//! - Linha lógica: termina com um número ímpar de `\` → continua na próxima linha física (os
//!   espaços do começo da próxima são ignorados).
//! - Chave: até o primeiro `=`, `:` ou espaço sem `\` antes; depois, espaços, um `=`/`:`
//!   opcional e mais espaços; o resto da linha lógica é o valor (espaços finais fazem parte).
//! - Escapes: `\uXXXX`, `\t`, `\n`, `\r`, `\f`; `\x` qualquer vira `x`.
//!
//! A chave repetida vale pela última ocorrência, como no Java.

use std::fmt::Write as _;

use super::{Builder, EditStyle, Parsed, Rendered, ValueEntry, invalid};
use crate::error::Result;
use crate::format::ConfigFormat;
use crate::path::KeyPath;
use crate::text::{DocText, strip_line_comment};
use crate::tree::ConfigValue;

const FORMAT: ConfigFormat = ConfigFormat::Properties;

fn is_blank(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{c}')
}

/// Um caractere da linha lógica e a posição dele no corpo.
type Located = (usize, char);

pub(crate) fn parse(doc: &DocText<'_>) -> Result<Parsed> {
    let mut builder = Builder::new(doc);
    let escape_unicode = doc.body.is_ascii();
    let mut index = 0;
    while index < doc.lines.len() {
        let first_index = index;
        let Some(first) = doc.lines.get(index) else {
            break;
        };
        let text = doc.line_text(index);
        let skipped = text.len() - text.trim_start_matches(is_blank).len();
        let content = text.get(skipped..).unwrap_or("");
        index += 1;
        if content.is_empty() || content.starts_with(['#', '!']) {
            continue;
        }
        // Junta as linhas físicas da linha lógica, guardando a posição de cada caractere.
        let mut chars: Vec<Located> = Vec::new();
        let mut segment_start = first.start + skipped;
        let mut segment_end = first.end;
        let mut logical_end;
        loop {
            let segment = doc.body.get(segment_start..segment_end).unwrap_or("");
            let trailing = segment.len() - segment.trim_end_matches('\\').len();
            let continues = trailing % 2 == 1;
            let kept_end = if continues {
                segment_end - 1
            } else {
                segment_end
            };
            chars.extend(
                doc.body
                    .get(segment_start..kept_end)
                    .unwrap_or("")
                    .char_indices()
                    .map(|(offset, c)| (segment_start + offset, c)),
            );
            logical_end = segment_end;
            if !continues {
                break;
            }
            let Some(next) = doc.lines.get(index) else {
                break;
            };
            index += 1;
            let next_text = doc.line_text(index - 1);
            let next_skip = next_text.len() - next_text.trim_start_matches(is_blank).len();
            segment_start = next.start + next_skip;
            segment_end = next.end;
        }
        let (key_chars, value_start, has_separator) = split_key(&chars);
        let key = unescape(key_chars).map_err(|offset| {
            doc.parse_error(FORMAT, offset, "escape \\uXXXX malformado na chave")
        })?;
        let value_chars = chars.get(value_start..).unwrap_or(&[]);
        let value = unescape(value_chars).map_err(|offset| {
            doc.parse_error(FORMAT, offset, "escape \\uXXXX malformado no valor")
        })?;
        let span_start = value_chars
            .first()
            .map_or(logical_end, |(offset, _)| *offset);
        let comment =
            doc.leading_comment(first_index, |line| strip_line_comment(line, &['#', '!']));
        builder.value(ValueEntry {
            path: KeyPath::from_keys([key]),
            value: ConfigValue::String(value),
            line_index: first_index,
            span: span_start..logical_end,
            style: EditStyle::Properties {
                needs_separator: !has_separator,
                escape_unicode,
            },
            comment,
            cfg_type: None,
        });
    }
    Ok(builder.finish())
}

/// Separa a chave do valor como o `Properties.load0`: devolve os caracteres da chave, a posição
/// (em `chars`) do primeiro caractere do valor e se houve separador (`=`, `:` ou espaço).
fn split_key(chars: &[Located]) -> (&[Located], usize, bool) {
    let mut key_len = 0;
    let mut value_start = chars.len();
    let mut has_separator = false;
    let mut preceding_backslash = false;
    while let Some(&(_, c)) = chars.get(key_len) {
        if (c == '=' || c == ':' || is_blank(c)) && !preceding_backslash {
            value_start = key_len + 1;
            has_separator = true;
            break;
        }
        preceding_backslash = c == '\\' && !preceding_backslash;
        key_len += 1;
    }
    let mut seen_symbol = chars
        .get(key_len)
        .is_some_and(|&(_, c)| c == '=' || c == ':');
    while let Some(&(_, c)) = chars.get(value_start) {
        if !is_blank(c) {
            if !seen_symbol && (c == '=' || c == ':') {
                seen_symbol = true;
            } else {
                break;
            }
        }
        value_start += 1;
    }
    (
        chars.get(..key_len).unwrap_or(&[]),
        value_start,
        has_separator,
    )
}

/// Desfaz os escapes; em `\u` malformado devolve a posição no corpo.
fn unescape(chars: &[Located]) -> Result<String, usize> {
    let mut out = String::with_capacity(chars.len());
    let mut position = 0;
    while let Some(&(offset, c)) = chars.get(position) {
        position += 1;
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(&(_, escaped)) = chars.get(position) else {
            break;
        };
        position += 1;
        match escaped {
            'u' => {
                let digits: String = chars
                    .get(position..position + 4)
                    .unwrap_or(&[])
                    .iter()
                    .map(|&(_, c)| c)
                    .collect();
                let unit = (digits.len() == 4)
                    .then(|| u16::from_str_radix(&digits, 16).ok())
                    .flatten()
                    .ok_or(offset)?;
                position += 4;
                push_utf16(&mut out, unit, chars, &mut position);
            }
            't' => out.push('\t'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            'f' => out.push('\u{c}'),
            other => out.push(other),
        }
    }
    Ok(out)
}

/// Junta pares substitutos (`\uD83D\uDE00`); unidade solta vira `U+FFFD`, como o `String` do
/// Java faria ao converter para UTF-8.
fn push_utf16(out: &mut String, unit: u16, chars: &[Located], position: &mut usize) {
    if (0xD800..0xDC00).contains(&unit) {
        let next: String = chars
            .get(*position..*position + 6)
            .unwrap_or(&[])
            .iter()
            .map(|&(_, c)| c)
            .collect();
        if let Some(low) = next
            .strip_prefix("\\u")
            .and_then(|hex| u16::from_str_radix(hex, 16).ok())
            .filter(|low| (0xDC00..0xE000).contains(low))
        {
            *position += 6;
            out.extend(char::decode_utf16([unit, low]).map(|c| c.unwrap_or('\u{fffd}')));
            return;
        }
    }
    out.extend(char::decode_utf16([unit]).map(|c| c.unwrap_or('\u{fffd}')));
}

/// Escreve o valor com os escapes necessários para o `Properties.load` ler o mesmo texto.
pub(crate) fn escape_value(text: &str, escape_unicode: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for (position, c) in text.chars().enumerate() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            ' ' if position == 0 => out.push_str("\\ "),
            '=' | ':' if position == 0 => {
                out.push('\\');
                out.push(c);
            }
            c if (c.is_control() || (escape_unicode && !c.is_ascii())) => {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    let _ = write!(out, "\\u{unit:04X}"); // escrever numa String não falha
                }
            }
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn render(
    path: &KeyPath,
    value: &ConfigValue,
    needs_separator: bool,
    escape_unicode: bool,
) -> Result<Rendered> {
    let text = value
        .as_plain_text()
        .ok_or_else(|| invalid(path, ".properties só guarda texto, não listas nem nulo"))?;
    let mut rendered = escape_value(&text, escape_unicode);
    if needs_separator && !rendered.is_empty() {
        rendered.insert(0, '=');
    }
    Ok(Rendered {
        text: rendered,
        expected: ConfigValue::String(text),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Vec<(String, String)> {
        let doc = DocText::from_str(text);
        parse(&doc)
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| {
                let key = entry.path.to_string();
                let Some(ConfigValue::String(value)) = entry.value else {
                    panic!()
                };
                (key, value)
            })
            .collect()
    }

    #[test]
    fn regras_do_java() {
        let text = "# c\n! c\n  a = 1\nb:2\nc 3\nd\ne=\\u00e9\\t\\\\x\nf = linha \\\n    continua\ng=\\\\\nh\\ i=j\n";
        let pairs = read(text);
        let expected = [
            ("a", "1"),
            ("b", "2"),
            ("c", "3"),
            ("d", ""),
            ("e", "é\t\\x"),
            ("f", "linha continua"),
            ("g", "\\"),
            ("\"h i\"", "j"),
        ];
        let expected: Vec<(String, String)> = expected
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        assert_eq!(pairs, expected);
    }

    #[test]
    fn separadores_combinados() {
        let pairs = read("k  =  v  \nm :=x\n");
        assert_eq!(pairs[0], ("k".into(), "v  ".into()));
        assert_eq!(pairs[1], ("m".into(), "=x".into()));
    }

    #[test]
    fn pares_substitutos_e_erros() {
        assert_eq!(read("a=\\uD83D\\uDE00\n")[0].1, "😀");
        assert_eq!(read("a=\\uD83D\n")[0].1, "\u{fffd}");
        let doc = DocText::from_str("a=\\u12\n");
        assert!(matches!(
            parse(&doc),
            Err(crate::ConfigError::Parse { line: 1, .. })
        ));
        let doc = DocText::from_str("\\uZZZZ=1\n");
        assert!(parse(&doc).is_err());
    }

    #[test]
    fn escapes_na_escrita() {
        assert_eq!(escape_value(" a=b", false), "\\ a=b");
        assert_eq!(escape_value("=x", false), "\\=x");
        assert_eq!(escape_value("é\n\\", true), "\\u00E9\\n\\\\");
        assert_eq!(escape_value("é", false), "é");
        assert_eq!(escape_value("😀", true), "\\uD83D\\uDE00");
        assert_eq!(escape_value("\u{1}", false), "\\u0001");
    }

    #[test]
    fn chave_sem_separador_ganha_igual() {
        let path = KeyPath::from_keys(["k"]);
        let rendered = render(&path, &ConfigValue::Integer(5), true, false).unwrap();
        assert_eq!(rendered.text, "=5");
        assert_eq!(rendered.expected, ConfigValue::String("5".into()));
        assert!(render(&path, &ConfigValue::Null, false, false).is_err());
    }
}
