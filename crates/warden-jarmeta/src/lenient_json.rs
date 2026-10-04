//! JSON tolerante para descritores escritos à mão (R3 §4.4: "muitos arquivos são JSON
//! inválido").
//!
//! Primeiro tenta o JSON estrito. Se falhar, corrige numa só passada os erros comuns nos
//! `mcmod.info` e `fabric.mod.json` reais e tenta de novo:
//!
//! - vírgula sobrando antes de `}` ou `]` (e vírgulas repetidas);
//! - comentários `//` e `/* */`;
//! - quebra de linha, tabulação e outros caracteres de controle dentro de texto;
//! - barra invertida que não forma um escape válido (`"C:\pasta"`);
//! - vírgula faltando entre `}`/`]`/texto e o próximo valor;
//! - texto sem as aspas de fechamento no fim do arquivo;
//! - lixo depois do primeiro valor.
//!
//! A profundidade continua limitada pelo `serde_json` (128 níveis), então entrada maliciosa
//! vira erro, não estouro de pilha.

use std::fmt::Write as _;

use serde_json::Value;

/// Resultado da leitura tolerante.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Parsed {
    pub(crate) value: Value,
    /// `true` se foi preciso corrigir o texto.
    pub(crate) lenient: bool,
}

/// Lê `text` como JSON, corrigindo os erros comuns se o estrito falhar.
pub(crate) fn parse(text: &str) -> Result<Parsed, String> {
    match serde_json::from_str::<Value>(text) {
        Ok(value) => Ok(Parsed {
            value,
            lenient: false,
        }),
        Err(strict) => {
            let fixed = sanitize(text);
            let mut stream = serde_json::Deserializer::from_str(&fixed).into_iter::<Value>();
            match stream.next() {
                Some(Ok(value)) => Ok(Parsed {
                    value,
                    lenient: true,
                }),
                Some(Err(e)) => Err(format!("{strict} (no modo tolerante: {e})")),
                None => Err(strict.to_string()),
            }
        }
    }
}

/// Último caractere significativo já escrito fora de texto (para decidir vírgulas faltando).
fn last_significant(out: &str) -> Option<char> {
    out.chars().rev().find(|c| !c.is_whitespace())
}

/// Posição do próximo caractere que não é espaço nem comentário, a partir de `i`.
fn skip_trivia(chars: &[char], mut i: usize) -> usize {
    loop {
        match (chars.get(i), chars.get(i + 1)) {
            (Some(c), _) if c.is_whitespace() => i += 1,
            (Some('/'), Some('/')) => {
                while chars.get(i).is_some_and(|c| *c != '\n') {
                    i += 1;
                }
            }
            (Some('/'), Some('*')) => {
                i += 2;
                while i < chars.len()
                    && !(chars.get(i) == Some(&'*') && chars.get(i + 1) == Some(&'/'))
                {
                    i += 1;
                }
                i = (i + 2).min(chars.len());
            }
            _ => return i,
        }
    }
}

fn sanitize(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0;
    let mut in_string = false;
    let mut closed_string = false;
    while let Some(&c) = chars.get(i) {
        if in_string {
            match c {
                '"' => {
                    in_string = false;
                    closed_string = true;
                    out.push('"');
                }
                '\\' => match chars.get(i + 1) {
                    Some(&e @ ('"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't')) => {
                        out.push('\\');
                        out.push(e);
                        i += 1;
                    }
                    Some('u')
                        if chars
                            .get(i + 2..i + 6)
                            .is_some_and(|h| h.iter().all(char::is_ascii_hexdigit)) =>
                    {
                        out.push('\\');
                    }
                    _ => out.push_str("\\\\"),
                },
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if u32::from(c) < 0x20 => {
                    // Escrever numa String nunca falha.
                    let _ = write!(out, "\\u{:04x}", u32::from(c));
                }
                c => out.push(c),
            }
            i += 1;
            continue;
        }
        match c {
            '/' if matches!(chars.get(i + 1), Some('/' | '*')) => {
                i = skip_trivia(&chars, i);
                out.push(' ');
                continue;
            }
            ',' => {
                let next = skip_trivia(&chars, i + 1);
                let dangling = matches!(chars.get(next), None | Some('}' | ']' | ','));
                if !dangling {
                    out.push(',');
                }
            }
            '{' | '[' | '"' => {
                let previous = last_significant(&out);
                let after_value =
                    matches!(previous, Some('}' | ']')) || (previous == Some('"') && closed_string);
                if after_value {
                    out.push(',');
                }
                if c == '"' {
                    in_string = true;
                }
                out.push(c);
            }
            c => out.push(c),
        }
        if !c.is_whitespace() && c != '"' {
            closed_string = false;
        }
        i += 1;
    }
    if in_string {
        out.push('"');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    fn lenient(text: &str) -> Value {
        let parsed = parse(text).expect(text);
        assert!(parsed.lenient, "esperava modo tolerante para {text:?}");
        parsed.value
    }

    #[test]
    fn json_valido_nao_usa_modo_tolerante() {
        let parsed = parse(r#"{"a": [1, 2]}"#).expect("válido");
        assert!(!parsed.lenient);
        assert_eq!(parsed.value, json!({"a": [1, 2]}));
    }

    #[test]
    fn corrige_os_erros_comuns() {
        assert_eq!(
            lenient(r#"[{"modid": "x", "authorList": ["a", "b",],},]"#),
            json!([{"modid": "x", "authorList": ["a", "b"]}])
        );
        assert_eq!(lenient("{\"a\": 1,, \"b\": 2}"), json!({"a": 1, "b": 2}));
        assert_eq!(
            lenient("{\n// comentário\n\"a\": /* x */ 1}"),
            json!({"a": 1})
        );
        assert_eq!(
            lenient("{\"d\": \"linha 1\nlinha 2\ttab\u{1}\"}"),
            json!({"d": "linha 1\nlinha 2\ttab\u{1}"})
        );
        assert_eq!(
            lenient(r#"{"p": "C:\pasta\x", "u": "\u00e9"}"#),
            json!({"p": "C:\\pasta\\x", "u": "é"})
        );
        assert_eq!(
            lenient(r#"[{"a": 1} {"b": 2}]"#),
            json!([{"a": 1}, {"b": 2}])
        );
        assert_eq!(
            lenient(r#"{"a": "x" "b": "y"}"#),
            json!({"a": "x", "b": "y"})
        );
    }

    #[test]
    fn texto_sem_fechamento_e_lixo_no_fim() {
        // Aspas fechadas no fim, mas sem a chave final: continua inválido.
        assert!(parse(r#"{"a": "sem fim"#).is_err());
        assert!(parse(r#"["sem fim"#).is_err());
        assert!(parse("{\"a\": 1 /* sem fim").is_err());
        assert_eq!(lenient(r#"{"a": 1} lixo"#), json!({"a": 1}));
        assert_eq!(
            lenient(
                "{\"a\": 1, // fim
}"
            ),
            json!({"a": 1})
        );
    }

    #[test]
    fn erro_quando_nao_ha_conserto() {
        let err = parse("{\"a\" 1}").expect_err("inválido");
        assert!(err.contains("modo tolerante"), "{err}");
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
        let deep = "[".repeat(10_000);
        assert!(parse(&deep).is_err(), "profundidade excessiva vira erro");
    }

    proptest! {
        #[test]
        fn nunca_entra_em_panico(text in ".{0,200}") {
            let _ = parse(&text);
        }

        #[test]
        fn json_gerado_e_lido_igual(value in arb_json()) {
            let text = serde_json::to_string_pretty(&value).expect("json");
            let parsed = parse(&text).expect("válido");
            prop_assert!(!parsed.lenient);
            prop_assert_eq!(parsed.value, value.clone());
            // Com vírgulas sobrando, o resultado é o mesmo.
            let with_commas = add_trailing_commas(&text);
            prop_assert_eq!(parse(&with_commas).expect("tolerante").value, value);
        }
    }

    /// Põe vírgula no fim de cada linha seguida de uma linha que fecha `}` ou `]`.
    fn add_trailing_commas(pretty: &str) -> String {
        let lines: Vec<&str> = pretty.lines().collect();
        let mut out = String::new();
        for (i, line) in lines.iter().enumerate() {
            out.push_str(line);
            let closes_next = lines
                .get(i + 1)
                .is_some_and(|n| n.trim_start().starts_with(['}', ']']));
            if closes_next && !line.ends_with(['{', '[']) {
                out.push(',');
            }
            out.push('\n');
        }
        out
    }

    fn arb_json() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i32>().prop_map(|n| json!(n)),
            "[a-z ,\\]}{\"\\\\]{0,8}".prop_map(Value::String),
        ];
        leaf.prop_recursive(3, 24, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 1..4).prop_map(Value::Array),
                prop::collection::btree_map("[a-z]{1,4}", inner, 1..4)
                    .prop_map(|m| Value::Object(m.into_iter().collect())),
            ]
        })
    }
}
