//! Camada 3: o que se deduz das frases do comentário (confiança baixa, mostrado como
//! "deduzido"; R5B §2.2).
//!
//! - padrão: `Defaults to X`, `Default: X` (no meio da frase), `default is X`,
//!   `default value: X`;
//! - faixa: `between X and Y`, `(min X, max Y)`, `from X to Y`;
//! - cor: valor de texto `#RRGGBB`.
//!
//! As frases são em inglês porque é assim que os mods escrevem. Só se usa o que a camada 1 não
//! respondeu; nada aqui inventa padrão onde o texto não diz um.

/// Faixa deduzida.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InferredRange {
    pub min: f64,
    pub max: f64,
    /// O trecho do comentário de onde saiu.
    pub text: String,
}

/// Procura `label` sem diferenciar maiúsculas; devolve o texto que vem depois.
fn after_phrase<'t>(text: &'t str, phrase: &str) -> Option<&'t str> {
    let lower = text.to_ascii_lowercase();
    let position = lower.find(phrase)?;
    text.get(position + phrase.len()..)
}

/// Primeiro "token" de um valor: até espaço, vírgula, ponto final, `)` ou `;`, sem aspas.
fn token(text: &str) -> Option<String> {
    let text = text.trim_start().trim_start_matches(':').trim_start();
    let (quoted, body) = match text.chars().next()? {
        quote @ ('"' | '\'' | '`') => (Some(quote), text.get(1..)?),
        _ => (None, text),
    };
    let value = if let Some(quote) = quoted {
        body.split(quote).next()?
    } else {
        {
            let end = body
                .char_indices()
                .find(|&(index, c)| {
                    c.is_whitespace()
                        || matches!(c, ',' | ')' | ';' | ']')
                        || (c == '.' && !next_is_digit(body, index))
                })
                .map_or(body.len(), |(index, _)| index);
            body.get(..end)?
        }
    };
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Ponto decimal (`1.5`) e não ponto final (`5.`).
fn next_is_digit(text: &str, dot: usize) -> bool {
    text.get(dot + 1..)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c.is_ascii_digit())
}

/// Padrão escrito numa frase do comentário.
pub(crate) fn default(comment: &str) -> Option<String> {
    for phrase in [
        "defaults to",
        "default value is",
        "default value:",
        "default is",
        "default:",
        "default =",
    ] {
        if let Some(found) = after_phrase(comment, phrase).and_then(token) {
            return Some(found);
        }
    }
    None
}

fn number(text: &str) -> Option<f64> {
    let text = text.trim().trim_end_matches(['.', ',', ')']);
    let text = text.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(text);
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

/// Faixa escrita numa frase do comentário.
pub(crate) fn range(comment: &str) -> Option<InferredRange> {
    let lower = comment.to_ascii_lowercase();
    for (open, middle) in [("between ", " and "), ("from ", " to "), ("min ", ", max ")] {
        let mut search = 0;
        while let Some(found) = lower.get(search..).and_then(|rest| rest.find(open)) {
            let start = search + found;
            search = start + open.len();
            let rest = lower.get(search..).unwrap_or("");
            let Some(split) = rest.find(middle) else {
                break;
            };
            let first = rest.get(..split).unwrap_or("");
            let after = rest.get(split + middle.len()..).unwrap_or("");
            let second_end = after
                .char_indices()
                .find(|&(index, c)| {
                    c.is_whitespace()
                        || matches!(c, ',' | ')' | ';')
                        || (c == '.' && !next_is_digit(after, index))
                })
                .map_or(after.len(), |(index, _)| index);
            let second = after.get(..second_end).unwrap_or("");
            if let (Some(min), Some(max)) = (number(first), number(second))
                && min <= max
            {
                let end = search + split + middle.len() + second_end;
                return Some(InferredRange {
                    min,
                    max,
                    text: comment.get(start..end).unwrap_or("").trim().to_owned(),
                });
            }
        }
    }
    None
}

/// `true` para `#RRGGBB`.
pub(crate) fn is_hex_color(value: &str) -> bool {
    value
        .strip_prefix('#')
        .is_some_and(|hex| hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padroes_em_frases() {
        assert_eq!(
            default("Logs out tags. Defaults to DEV_SHORT.").as_deref(),
            Some("DEV_SHORT")
        );
        assert_eq!(
            default("Até quantos blocos. Defaults to 20.").as_deref(),
            Some("20")
        );
        assert_eq!(
            default("Scale, default is 1.5 here").as_deref(),
            Some("1.5")
        );
        assert_eq!(
            default("The default value: \"minecraft:stone\"").as_deref(),
            Some("minecraft:stone")
        );
        assert_eq!(default("(default: true)").as_deref(), Some("true"));
        assert_eq!(default("Sem padrão aqui"), None);
        assert_eq!(default("Defaults to"), None);
        assert_eq!(default("Defaults to \"\""), None);
    }

    #[test]
    fn faixas_em_frases() {
        let found = range("Must be between 1 and 64.").unwrap();
        assert_eq!((found.min, found.max), (1.0, 64.0));
        assert_eq!(found.text, "between 1 and 64");
        let floats = range("Chance (min 0.0, max 1.0)").unwrap();
        assert_eq!((floats.min, floats.max), (0.0, 1.0));
        let from = range("Pick a value from -5 to 5; zero disables").unwrap();
        assert_eq!((from.min, from.max), (-5.0, 5.0));
        // "between" sem números não serve; procura o próximo.
        let second = range("Choose between fast and slow, between 2 and 3").unwrap();
        assert_eq!((second.min, second.max), (2.0, 3.0));
        assert_eq!(range("between 9 and 1"), None);
        assert_eq!(range("from here to there"), None);
        assert_eq!(range("nada"), None);
        assert_eq!(range("between 1"), None);
    }

    #[test]
    fn cores() {
        assert!(is_hex_color("#a0FF00"));
        assert!(!is_hex_color("#80a0FF00"));
        assert!(!is_hex_color("#abc"));
        assert!(!is_hex_color("a0FF00"));
        assert!(!is_hex_color("#zzzzzz"));
    }
}
