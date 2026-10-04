//! Comparação semântica: o que mudou nos valores, ignorando formatação, comentários e ordem.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::document::ConfigDocument;
use crate::error::Result;
use crate::format::ConfigFormat;
use crate::path::KeyPath;
use crate::tree::{ConfigTree, ConfigValue};

/// Uma mudança de valor entre duas versões de uma config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "camelCase")]
pub enum SemanticChange {
    /// A chave só existe na versão nova.
    Added {
        /// Caminho.
        path: KeyPath,
        /// Valor novo.
        value: ConfigValue,
    },
    /// A chave só existe na versão antiga.
    Removed {
        /// Caminho.
        path: KeyPath,
        /// Valor antigo.
        value: ConfigValue,
    },
    /// A chave existe nas duas com valores diferentes.
    Changed {
        /// Caminho.
        path: KeyPath,
        /// Valor antigo.
        before: ConfigValue,
        /// Valor novo.
        after: ConfigValue,
    },
}

impl SemanticChange {
    /// Caminho da chave.
    #[must_use]
    pub fn path(&self) -> &KeyPath {
        match self {
            Self::Added { path, .. } | Self::Removed { path, .. } | Self::Changed { path, .. } => {
                path
            }
        }
    }
}

/// Resultado de [`compare`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticDiff {
    /// Mudanças de valor, na ordem do arquivo antigo (removidas e alteradas) seguidas das
    /// chaves novas na ordem do arquivo novo.
    pub changes: Vec<SemanticChange>,
    /// `true` se os bytes mudaram (pode ser só formatação ou comentário).
    pub text_changed: bool,
}

impl SemanticDiff {
    /// `true` se nenhum valor mudou (mesmo que a formatação tenha mudado).
    #[must_use]
    pub fn is_same_values(&self) -> bool {
        self.changes.is_empty()
    }
}

/// Compara duas versões de um arquivo do mesmo formato pelos valores.
///
/// Ignora espaços, comentários, ordem das chaves, estilo de aspas e grafia dos números
/// (`1.50` = `1.5`). Listas são comparadas na ordem. Chave repetida vale pela última ocorrência.
pub fn compare(format: ConfigFormat, before: &[u8], after: &[u8]) -> Result<SemanticDiff> {
    let old = ConfigDocument::parse(format, before)?;
    let new = ConfigDocument::parse(format, after)?;
    Ok(SemanticDiff {
        changes: diff_trees(old.tree(), new.tree()),
        text_changed: before != after,
    })
}

fn effective_values(tree: &ConfigTree) -> (Vec<&KeyPath>, HashMap<&KeyPath, &ConfigValue>) {
    let mut order = Vec::new();
    let mut map = HashMap::new();
    for entry in tree.values() {
        if let Some(value) = &entry.value
            && map.insert(&entry.path, value).is_none()
        {
            order.push(&entry.path);
        }
    }
    (order, map)
}

/// Mudanças de valor entre duas árvores.
pub(crate) fn diff_trees(before: &ConfigTree, after: &ConfigTree) -> Vec<SemanticChange> {
    let (old_order, old) = effective_values(before);
    let (new_order, new) = effective_values(after);
    let mut changes = Vec::new();
    for path in &old_order {
        let Some(old_value) = old.get(path) else {
            continue;
        };
        match new.get(path) {
            None => changes.push(SemanticChange::Removed {
                path: (*path).clone(),
                value: (*old_value).clone(),
            }),
            Some(new_value) if !old_value.same_as(new_value) => {
                changes.push(SemanticChange::Changed {
                    path: (*path).clone(),
                    before: (*old_value).clone(),
                    after: (*new_value).clone(),
                });
            }
            Some(_) => {}
        }
    }
    let known: HashSet<&KeyPath> = old_order.iter().copied().collect();
    for path in new_order {
        if !known.contains(path)
            && let Some(value) = new.get(path)
        {
            changes.push(SemanticChange::Added {
                path: path.clone(),
                value: (*value).clone(),
            });
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatacao_e_comentarios_nao_contam() {
        let before = b"# a\nx = 1.50\n[t]\ny = 'z'\n";
        let after = b"[t]\ny = \"z\" # outro\n\n\nx = 1.5\n";
        // `x` saiu da raiz para dentro de `[t]`: o caminho muda.
        let diff = compare(ConfigFormat::Toml, before, after).unwrap();
        assert!(diff.text_changed);
        assert_eq!(diff.changes.len(), 2);
        let reordered = b"x = 1.5\n\n[t]\n  y = \"z\"\n";
        let same = compare(ConfigFormat::Toml, before, reordered).unwrap();
        assert!(same.is_same_values());
        assert!(same.text_changed);
        let identical = compare(ConfigFormat::Toml, before, before).unwrap();
        assert!(!identical.text_changed);
    }

    #[test]
    fn adicionadas_removidas_e_alteradas() {
        let before = b"a=1\nb=2\nb=3\nc=4\n";
        let after = b"a=1\nb=9\nd=5\n";
        let diff = compare(ConfigFormat::Properties, before, after).unwrap();
        let s = |t: &str| ConfigValue::String(t.into());
        let key = |k: &str| KeyPath::from_keys([k]);
        assert_eq!(
            diff.changes,
            vec![
                SemanticChange::Changed {
                    path: key("b"),
                    before: s("3"),
                    after: s("9")
                },
                SemanticChange::Removed {
                    path: key("c"),
                    value: s("4")
                },
                SemanticChange::Added {
                    path: key("d"),
                    value: s("5")
                },
            ]
        );
        assert_eq!(diff.changes[2].path(), &key("d"));
    }

    #[test]
    fn erro_de_leitura_aparece() {
        assert!(compare(ConfigFormat::Json, b"{", b"{}").is_err());
        assert!(compare(ConfigFormat::Json, b"{}", b"{").is_err());
    }
}
