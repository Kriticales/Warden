//! Camada 1: metadados que o próprio arquivo escreve no comentário de uma chave (R5B §2.1).
//!
//! A faixa e os valores permitidos do `ModConfigSpec`/`ForgeConfigSpec` e o `[range: …]` do
//! `Configuration` já vêm da leitura (C-01, `annotations.rs`). Aqui entram os que faltam:
//!
//! - `Default: X` do `ModConfigSpec` do NeoForge (só nas chaves com faixa);
//! - `[default: X]` e `[range: …, default: X]` do `Configuration` do Forge antigo;
//! - `Min: X`, `Max: Y` e `Valid values:` (um valor por linha) do `@Config` do Forge 1.12.2.
//!
//! E a descrição limpa: o comentário sem as linhas (ou o trecho final) de metadados, que o
//! formulário já mostra nos próprios campos.

use crate::format::ConfigFormat;

/// O que o comentário diz de forma explícita, além da faixa e dos valores da leitura.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Explicit {
    /// Padrão como está escrito (`2.0`, `true`, `[a, b]`).
    pub default: Option<String>,
    /// `Min:` do `@Config`.
    pub min: Option<f64>,
    /// `Max:` do `@Config`.
    pub max: Option<f64>,
    /// `Valid values:` do `@Config`.
    pub valid_values: Option<Vec<String>>,
    /// O comentário sem os metadados (`None` se não sobrou texto).
    pub description: Option<String>,
}

/// Lê os metadados explícitos do comentário de uma chave no formato dado.
pub(crate) fn read(format: ConfigFormat, comment: Option<&str>) -> Explicit {
    let Some(comment) = comment else {
        return Explicit::default();
    };
    match format {
        ConfigFormat::Toml => spec_comment(comment),
        ConfigFormat::LegacyCfg => legacy_comment(comment),
        _ => Explicit {
            description: clean(comment.lines()),
            ..Explicit::default()
        },
    }
}

fn strip_label<'t>(line: &'t str, label: &str) -> Option<&'t str> {
    let head = line.get(..label.len())?;
    head.eq_ignore_ascii_case(label)
        .then(|| line.get(label.len()..).map(str::trim))
        .flatten()
}

/// Junta as linhas que sobraram; `None` se nenhuma tem texto.
fn clean<'t>(lines: impl Iterator<Item = &'t str>) -> Option<String> {
    let kept: Vec<&str> = lines.map(str::trim).collect();
    let first = kept.iter().position(|line| !line.is_empty())?;
    let last = kept.iter().rposition(|line| !line.is_empty())?;
    Some(kept.get(first..=last)?.join("\n"))
}

/// Comentário do `ModConfigSpec`/`ForgeConfigSpec` (TOML).
fn spec_comment(comment: &str) -> Explicit {
    let mut found = Explicit::default();
    let mut rest = Vec::new();
    for line in comment.lines() {
        let trimmed = line.trim();
        if let Some(value) = strip_label(trimmed, "Default:") {
            if !value.is_empty() {
                found.default = Some(value.to_owned());
            }
        } else if strip_label(trimmed, "Range:").is_none()
            && strip_label(trimmed, "Allowed Values:").is_none()
        {
            rest.push(trimmed);
        }
    }
    found.description = clean(rest.into_iter());
    found
}

/// Comentário do `Configuration` (`[range: …, default: …]`) e do `@Config` (`Min:`, `Max:`,
/// `Valid values:`) do Forge antigo.
fn legacy_comment(comment: &str) -> Explicit {
    let mut found = Explicit::default();
    let mut text = comment.trim_end();
    // `Configuration` acrescenta o trecho entre colchetes ao fim do comentário.
    if text.ends_with(']')
        && let Some(open) = text.rfind('[')
    {
        let inner = text.get(open + 1..text.len() - 1).unwrap_or("");
        let lower = inner.to_ascii_lowercase();
        if lower.starts_with("range:") || lower.starts_with("default:") {
            if let Some(position) = lower.find("default:") {
                let value = inner
                    .get(position + "default:".len()..)
                    .unwrap_or("")
                    .trim();
                found.default = Some(value.to_owned());
            }
            text = text.get(..open).unwrap_or("");
        }
    }
    let mut rest = Vec::new();
    let mut in_values = false;
    let mut values = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if in_values {
            if trimmed.is_empty() {
                in_values = false;
            } else {
                values.push(trimmed.to_owned());
            }
            continue;
        }
        if let Some(value) = strip_label(trimmed, "Min:") {
            found.min = parse_number(value);
        } else if let Some(value) = strip_label(trimmed, "Max:") {
            found.max = parse_number(value);
        } else if strip_label(trimmed, "Valid values:").is_some_and(str::is_empty) {
            in_values = true;
        } else {
            rest.push(trimmed);
        }
    }
    if !values.is_empty() {
        found.valid_values = Some(values);
    }
    found.description = clean(rest.into_iter());
    found
}

fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();
    let text = text.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(text);
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padrao_e_descricao_do_neoforge() {
        let found = read(
            ConfigFormat::Toml,
            Some(" Quanto empurra.\n Default: 2.0\n Range: 0.0 ~ 100.0"),
        );
        assert_eq!(found.default.as_deref(), Some("2.0"));
        assert_eq!(found.description.as_deref(), Some("Quanto empurra."));
        let only_meta = read(ConfigFormat::Toml, Some("Allowed Values: A, B\nDefault:"));
        assert_eq!(only_meta, Explicit::default());
    }

    #[test]
    fn colchetes_do_configuration() {
        let found = read(
            ConfigFormat::LegacyCfg,
            Some("Velocidade do vento\n [range: 0.0 ~ 0.5, default: 0.25]"),
        );
        assert_eq!(found.default.as_deref(), Some("0.25"));
        assert_eq!(found.description.as_deref(), Some("Velocidade do vento"));
        let only_default = read(ConfigFormat::LegacyCfg, Some("Liga tudo [default: true]"));
        assert_eq!(only_default.default.as_deref(), Some("true"));
        assert_eq!(only_default.description.as_deref(), Some("Liga tudo"));
        let other = read(ConfigFormat::LegacyCfg, Some("Usa [x] no fim [nota]"));
        assert_eq!(other.default, None);
        assert_eq!(other.description.as_deref(), Some("Usa [x] no fim [nota]"));
    }

    #[test]
    fn min_max_e_valores_do_config_anotado() {
        let found = read(
            ConfigFormat::LegacyCfg,
            Some("Limite\nMin: -2147483648\nMax: 4097\n"),
        );
        assert_eq!(
            (found.min, found.max),
            (Some(-2_147_483_648.0), Some(4097.0))
        );
        assert_eq!(found.description.as_deref(), Some("Limite"));
        let values = read(
            ConfigFormat::LegacyCfg,
            Some("Modo\nValid values:\nWHITELIST\nBLACKLIST"),
        );
        assert_eq!(
            values.valid_values,
            Some(vec!["WHITELIST".into(), "BLACKLIST".into()])
        );
        assert_eq!(values.description.as_deref(), Some("Modo"));
        let bad = read(ConfigFormat::LegacyCfg, Some("Min: muito\nMax: NaN"));
        assert_eq!((bad.min, bad.max), (None, None));
    }

    #[test]
    fn outros_formatos_so_limpam() {
        let found = read(ConfigFormat::Json5, Some("\n Ajuda \n\n"));
        assert_eq!(found.description.as_deref(), Some("Ajuda"));
        assert_eq!(read(ConfigFormat::Json, None), Explicit::default());
    }
}
