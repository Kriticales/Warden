//! Ruído conhecido (`data/log-noise.toml`): linhas inofensivas que ganham o selo "comum,
//! geralmente inofensivo" e ficam fora de "Só problemas" (ARCHITECTURE §9.8).

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::{Regex, RegexSet};
use serde::Deserialize;

use crate::error::{DiagnosticsError, Result};

/// O arquivo embutido.
pub(crate) const EMBEDDED: &str = include_str!("../../data/log-noise.toml");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoiseFile {
    version: u32,
    pattern: Vec<NoiseDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoiseDef {
    id: String,
    origin: String,
    note: String,
    regex: String,
    example: String,
}

/// Uma regra de ruído.
#[derive(Debug)]
pub(crate) struct NoiseRule {
    /// Id estável (vai para a interface).
    pub(crate) id: String,
}

/// O catálogo compilado.
#[derive(Debug)]
pub(crate) struct NoiseCatalog {
    pub(crate) rules: Vec<NoiseRule>,
    set: RegexSet,
}

impl NoiseCatalog {
    /// A regra que reconhece a linha (a primeira linha da mensagem).
    pub(crate) fn matching(&self, first_line: &str) -> Option<u16> {
        self.set
            .matches(first_line)
            .iter()
            .next()
            .and_then(|index| u16::try_from(index).ok())
    }

    /// O id da regra.
    pub(crate) fn id(&self, index: u16) -> &str {
        self.rules
            .get(usize::from(index))
            .map_or("", |rule| rule.id.as_str())
    }
}

/// Lê e valida o catálogo: versão 1, ids únicos, campos preenchidos, expressões que compilam e
/// cada uma reconhece o próprio exemplo.
pub(crate) fn parse(text: &str) -> Result<NoiseCatalog> {
    let invalid =
        |message: String| DiagnosticsError::Internal(format!("log-noise.toml: {message}"));
    let file: NoiseFile = toml::from_str(text).map_err(|error| invalid(error.to_string()))?;
    if file.version != 1 {
        return Err(invalid(format!("versão {} desconhecida", file.version)));
    }
    let mut ids = BTreeSet::new();
    let mut rules = Vec::with_capacity(file.pattern.len());
    let mut sources = Vec::with_capacity(file.pattern.len());
    for def in file.pattern {
        if !ids.insert(def.id.clone()) {
            return Err(invalid(format!("id repetido: {}", def.id)));
        }
        if def.origin.trim().is_empty() || def.note.trim().is_empty() {
            return Err(invalid(format!(
                "{}: origem e nota são obrigatórias",
                def.id
            )));
        }
        let regex = Regex::new(&def.regex)
            .map_err(|error| invalid(format!("{}: expressão inválida: {error}", def.id)))?;
        if !regex.is_match(&def.example) {
            return Err(invalid(format!(
                "{}: a expressão não reconhece o exemplo",
                def.id
            )));
        }
        sources.push(def.regex);
        rules.push(NoiseRule { id: def.id });
    }
    let set = RegexSet::new(&sources).map_err(|error| invalid(error.to_string()))?;
    Ok(NoiseCatalog { rules, set })
}

static CATALOG: LazyLock<std::result::Result<NoiseCatalog, String>> =
    LazyLock::new(|| parse(EMBEDDED).map_err(|error| error.to_string()));

/// O catálogo embutido, compilado uma vez.
pub(crate) fn embedded() -> Result<&'static NoiseCatalog> {
    CATALOG
        .as_ref()
        .map_err(|message| DiagnosticsError::Internal(message.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogo_embutido_e_valido() {
        let catalog = embedded().unwrap();
        let file: NoiseFile = toml::from_str(EMBEDDED).unwrap();
        assert!(catalog.rules.len() >= 10);
        for (index, def) in file.pattern.iter().enumerate() {
            assert_eq!(
                catalog.matching(&def.example).map(usize::from),
                Some(index),
                "{}",
                def.id
            );
            assert_eq!(catalog.id(u16::try_from(index).unwrap()), def.id);
        }
        assert_eq!(catalog.matching("Exception in thread main"), None);
    }

    /// Cada exemplo é uma linha real de um recorte citado na origem.
    #[test]
    fn exemplos_estao_nos_recortes_citados() {
        let file: NoiseFile = toml::from_str(EMBEDDED).unwrap();
        let matrix = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../warden-launcher/tests/matrix");
        for def in &file.pattern {
            let cited: Vec<&str> = def
                .origin
                .trim_start_matches("L-05 ")
                .split(", ")
                .map(|part| part.split(' ').next().unwrap_or_default())
                .collect();
            assert!(!cited.is_empty(), "{}", def.id);
            for name in cited {
                let text = std::fs::read_to_string(matrix.join(name))
                    .unwrap_or_else(|error| panic!("{}: {name}: {error}", def.id));
                let example = def.example.trim_end();
                assert!(
                    text.contains(example),
                    "{}: {name} não tem o exemplo",
                    def.id
                );
            }
        }
    }

    #[test]
    fn rejeita_catalogo_invalido() {
        let base = "version = 1\n[[pattern]]\nid = \"a\"\norigin = \"o\"\nnote = \"n\"\n";
        assert!(parse(&format!("{base}regex = '^x'\nexample = \"x\"\n")).is_ok());
        assert!(parse(&format!("{base}regex = '^x'\nexample = \"y\"\n")).is_err());
        assert!(parse(&format!("{base}regex = '('\nexample = \"x\"\n")).is_err());
        let twice = format!("{base}regex = '^x'\nexample = \"x\"\n{}", &base[12..]);
        assert!(parse(&format!("{twice}regex = '^x'\nexample = \"x\"\n")).is_err());
        assert!(parse("version = 2\npattern = []\n").is_err());
    }
}
