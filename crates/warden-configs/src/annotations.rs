//! Metadados explícitos que o próprio arquivo escreve no comentário de uma chave.
//!
//! Esta crate lê só o que a C-01 pede: a faixa (`Range:` do `ForgeConfigSpec`/`ModConfigSpec` e
//! `[range: … ~ …]` do `Configuration` do Forge antigo) e os valores permitidos
//! (`Allowed Values:`). O padrão (`Default:`, `[default: …]`) é da C-06 e as demais camadas do
//! formulário são da C-04; o comentário inteiro fica em
//! [`ConfigEntry::comment`](crate::ConfigEntry::comment) para elas.
//!
//! Formatos de origem (R5B §2.1): `ModConfigSpec.defineInRange` grava `" Range: "` com
//! `min ~ max`, `> min` ou `< max`; `defineEnum` grava `"Allowed Values: "` com os nomes
//! separados por vírgula; `Configuration.getInt`/`getFloat` acrescentam
//! `" [range: min ~ max, default: d]"` ao fim do comentário.

use crate::tree::ValueRange;

/// Faixa e valores permitidos tirados do comentário de uma chave.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Annotations {
    pub range: Option<ValueRange>,
    pub allowed_values: Option<Vec<String>>,
}

/// Lê `Range:` e `Allowed Values:` das linhas do comentário (TOML do Forge/NeoForge).
pub(crate) fn from_spec_comment(comment: &str) -> Annotations {
    let mut annotations = Annotations::default();
    for line in comment.lines() {
        let line = line.trim();
        if let Some(rest) = strip_label(line, "Range:") {
            annotations.range = Some(parse_spec_range(rest));
        } else if let Some(rest) = strip_label(line, "Allowed Values:") {
            let values: Vec<String> = rest
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect();
            if !values.is_empty() {
                annotations.allowed_values = Some(values);
            }
        }
    }
    annotations
}

/// Lê `[range: min ~ max` do comentário de uma propriedade do `.cfg` do Forge antigo.
pub(crate) fn from_legacy_comment(comment: &str) -> Annotations {
    let mut annotations = Annotations::default();
    if let Some(position) = comment.rfind("[range:") {
        let rest = comment.get(position + "[range:".len()..).unwrap_or("");
        let end = rest.find([',', ']']).unwrap_or(rest.len());
        let raw = rest.get(..end).unwrap_or("").trim();
        if let Some((min, max)) = raw.split_once('~') {
            annotations.range = Some(ValueRange {
                min: parse_number(min),
                max: parse_number(max),
                raw: raw.to_owned(),
            });
        }
    }
    annotations
}

fn strip_label<'t>(line: &'t str, label: &str) -> Option<&'t str> {
    let head = line.get(..label.len())?;
    if head.eq_ignore_ascii_case(label) {
        line.get(label.len()..).map(str::trim)
    } else {
        None
    }
}

fn parse_spec_range(raw: &str) -> ValueRange {
    let (min, max) = if let Some((min, max)) = raw.split_once('~') {
        (parse_number(min), parse_number(max))
    } else if let Some(min) = raw.strip_prefix('>') {
        (parse_number(min.trim_start_matches('=')), None)
    } else if let Some(max) = raw.strip_prefix('<') {
        (None, parse_number(max.trim_start_matches('=')))
    } else {
        (None, None)
    };
    ValueRange {
        min,
        max,
        raw: raw.to_owned(),
    }
}

fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();
    let text = text.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(text);
    text.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faixa_e_valores_do_forge() {
        let comment =
            "Max chunks per tick.\nRange: 1 ~ 64\nAllowed Values: SILENCED, DEV_SHORT , ,PROD";
        let found = from_spec_comment(comment);
        assert_eq!(
            found.range,
            Some(ValueRange {
                min: Some(1.0),
                max: Some(64.0),
                raw: "1 ~ 64".into()
            })
        );
        assert_eq!(
            found.allowed_values,
            Some(vec!["SILENCED".into(), "DEV_SHORT".into(), "PROD".into()])
        );
    }

    #[test]
    fn faixas_abertas_e_ilegiveis() {
        let open_min = from_spec_comment("Range: > 0").range.unwrap();
        assert_eq!((open_min.min, open_min.max), (Some(0.0), None));
        let open_max = from_spec_comment(" range: < 2.5").range.unwrap();
        assert_eq!((open_max.min, open_max.max), (None, Some(2.5)));
        let odd = from_spec_comment("Range: muitos").range.unwrap();
        assert_eq!((odd.min, odd.max, odd.raw.as_str()), (None, None, "muitos"));
        assert_eq!(from_spec_comment("Allowed Values:").allowed_values, None);
        assert_eq!(from_spec_comment("Sem metadados"), Annotations::default());
    }

    #[test]
    fn faixa_do_cfg_antigo() {
        let comment = "Velocidade [range: 0.0 ~ 1.0E9, default: 1.5]";
        let range = from_legacy_comment(comment).range.unwrap();
        assert_eq!((range.min, range.max), (Some(0.0), Some(1.0e9)));
        assert_eq!(range.raw, "0.0 ~ 1.0E9");
        let ints = from_legacy_comment("[range: -2147483648 ~ 2147483647]")
            .range
            .unwrap();
        assert_eq!(ints.min, Some(-2_147_483_648.0));
        assert_eq!(from_legacy_comment("[default: 5]").range, None);
        assert_eq!(from_legacy_comment("[range: 5]").range, None);
    }
}
