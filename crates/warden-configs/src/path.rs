//! Caminho de uma chave dentro da config (`seção.chave`, `lista[2].nome`).

use std::fmt;

use serde::{Deserialize, Serialize};

/// Um passo do caminho: o nome de uma chave (ou seção) ou a posição numa lista.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PathSegment {
    /// Nome de chave, tabela, objeto ou categoria (já sem aspas nem escapes).
    Key(String),
    /// Posição numa lista (começa em 0).
    Index(usize),
}

/// Caminho completo de uma chave, da raiz do arquivo até ela.
///
/// Na interface vai como lista (`["general", "maxStack"]`, `["lista", 0, "nome"]`); para
/// exibição, [`fmt::Display`] junta com `.` e põe entre aspas os nomes com espaço, ponto,
/// colchete ou aspas.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyPath(pub Vec<PathSegment>);

impl KeyPath {
    /// Caminho vazio (a raiz do arquivo).
    #[must_use]
    pub fn root() -> Self {
        Self(Vec::new())
    }

    /// Caminho feito só de nomes de chave.
    #[must_use]
    pub fn from_keys<I, S>(keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(
            keys.into_iter()
                .map(|key| PathSegment::Key(key.into()))
                .collect(),
        )
    }

    /// Novo caminho com mais um nome de chave no fim.
    #[must_use]
    pub fn child_key(&self, key: &str) -> Self {
        let mut segments = self.0.clone();
        segments.push(PathSegment::Key(key.to_owned()));
        Self(segments)
    }

    /// Novo caminho com mais uma posição de lista no fim.
    #[must_use]
    pub fn child_index(&self, index: usize) -> Self {
        let mut segments = self.0.clone();
        segments.push(PathSegment::Index(index));
        Self(segments)
    }

    /// Passos do caminho.
    #[must_use]
    pub fn segments(&self) -> &[PathSegment] {
        &self.0
    }

    /// Quantidade de passos (0 na raiz).
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` na raiz.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Último passo, se houver.
    #[must_use]
    pub fn last(&self) -> Option<&PathSegment> {
        self.0.last()
    }

    /// `true` se `self` começa com todos os passos de `prefix`.
    #[must_use]
    pub fn starts_with(&self, prefix: &KeyPath) -> bool {
        self.0.starts_with(&prefix.0)
    }
}

fn needs_quotes(key: &str) -> bool {
    key.is_empty()
        || key
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '.' | '[' | ']' | '"' | '\\'))
}

impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("(raiz)");
        }
        for (position, segment) in self.0.iter().enumerate() {
            match segment {
                PathSegment::Index(index) => write!(f, "[{index}]")?,
                PathSegment::Key(key) => {
                    if position > 0 {
                        f.write_str(".")?;
                    }
                    if needs_quotes(key) {
                        f.write_str("\"")?;
                        for c in key.chars() {
                            if matches!(c, '"' | '\\') {
                                f.write_str("\\")?;
                            }
                            write!(f, "{c}")?;
                        }
                        f.write_str("\"")?;
                    } else {
                        f.write_str(key)?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exibe_com_pontos_colchetes_e_aspas() {
        let path = KeyPath::from_keys(["general", "max stack"])
            .child_index(2)
            .child_key("a.b");
        assert_eq!(path.to_string(), "general.\"max stack\"[2].\"a.b\"");
        assert_eq!(KeyPath::root().to_string(), "(raiz)");
        assert_eq!(KeyPath::from_keys([""]).to_string(), "\"\"");
        assert_eq!(KeyPath::from_keys(["x\"y\\"]).to_string(), "\"x\\\"y\\\\\"");
        assert_eq!(KeyPath::root().child_index(0).to_string(), "[0]");
    }

    #[test]
    fn serializa_como_lista() {
        let path = KeyPath::from_keys(["a"]).child_index(3);
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "[\"a\",3]");
        let back: KeyPath = serde_json::from_str(&json).unwrap();
        assert_eq!(back, path);
    }

    #[test]
    fn prefixos_e_tamanho() {
        let base = KeyPath::from_keys(["a", "b"]);
        let child = base.child_key("c");
        assert!(child.starts_with(&base));
        assert!(!base.starts_with(&child));
        assert_eq!(child.len(), 3);
        assert!(!child.is_empty());
        assert!(KeyPath::root().is_empty());
        assert_eq!(child.last(), Some(&PathSegment::Key("c".into())));
    }
}
