//! Documento de config: os bytes originais, a árvore e as edições mínimas.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::annotations;
use crate::compare::{SemanticChange, diff_trees};
use crate::error::{ConfigError, Result};
use crate::format::ConfigFormat;
use crate::formats::{self, EditStyle};
use crate::path::KeyPath;
use crate::text::DocText;
use crate::tree::{ConfigEntry, ConfigTree, ConfigValue, EntryKind, TextSpan};

/// Pedido de edição: "esta chave passa a valer isto".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEdit {
    /// Caminho da chave (precisa existir no arquivo).
    pub path: KeyPath,
    /// Valor novo.
    pub value: ConfigValue,
}

impl ConfigEdit {
    /// Edição de `path` para `value`.
    #[must_use]
    pub fn new(path: KeyPath, value: ConfigValue) -> Self {
        Self { path, value }
    }
}

/// Uma chave cujo valor o Warden não consegue reescrever sem mudar o que o jogo lê; o
/// formulário deve mandar o arquivo (ou o trecho) para o editor de texto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteProblem {
    /// Caminho da chave.
    pub path: KeyPath,
    /// O que deu errado.
    pub reason: String,
}

/// Um arquivo de config lido: guarda os bytes exatos e a árvore com as posições dos valores.
#[derive(Debug, Clone)]
pub struct ConfigDocument {
    bytes: Vec<u8>,
    tree: ConfigTree,
    styles: Vec<EditStyle>,
}

struct Replacement {
    span: TextSpan,
    text: String,
}

impl ConfigDocument {
    /// Lê os bytes de um arquivo no formato dado.
    ///
    /// Erros: [`ConfigError::TooLarge`], [`ConfigError::NotUtf8`] e [`ConfigError::Parse`]
    /// (com linha e coluna). Nenhuma entrada, por pior que seja, causa pânico.
    pub fn parse(format: ConfigFormat, bytes: &[u8]) -> Result<Self> {
        let doc = DocText::new(bytes)?;
        let parsed = formats::parse(format, &doc)?;
        let mut entries = parsed.entries;
        for entry in &mut entries {
            if let Some(span) = entry.value_span.as_mut() {
                span.start += doc.bom_len;
                span.end += doc.bom_len;
            }
            let found = match (format, entry.comment.as_deref()) {
                (ConfigFormat::Toml, Some(comment)) => annotations::from_spec_comment(comment),
                (ConfigFormat::LegacyCfg, Some(comment)) => {
                    annotations::from_legacy_comment(comment)
                }
                _ => continue,
            };
            entry.range = found.range;
            entry.allowed_values = found.allowed_values;
        }
        let tree = ConfigTree {
            format,
            has_bom: doc.bom_len > 0,
            line_ending: doc.line_ending.to_owned(),
            entries,
        };
        Ok(Self {
            bytes: bytes.to_vec(),
            tree,
            styles: parsed.styles,
        })
    }

    /// Formato do documento.
    #[must_use]
    pub fn format(&self) -> ConfigFormat {
        self.tree.format
    }

    /// Árvore com as entradas.
    #[must_use]
    pub fn tree(&self) -> &ConfigTree {
        &self.tree
    }

    /// Bytes do arquivo, exatamente como foram lidos (ou como a edição os deixou). Gravar isto
    /// sem edição reproduz o arquivo byte a byte (CA-T12-01).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consome o documento e devolve os bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Entrada com esse caminho (a última, se a chave se repete).
    #[must_use]
    pub fn get(&self, path: &KeyPath) -> Option<&ConfigEntry> {
        self.tree.get(path)
    }

    /// Aplica as edições e devolve o documento novo.
    ///
    /// Só os bytes do valor de cada chave mudam; uma edição que repete o valor atual não muda
    /// nada. O resultado é relido e comparado com o original: se algo além das chaves pedidas
    /// mudou, ou se uma chave não ficou com o valor pedido, nada é devolvido
    /// ([`ConfigError::EditNotConfirmed`]).
    ///
    /// Outros erros: [`ConfigError::PathNotFound`], [`ConfigError::NotEditable`] (seções),
    /// [`ConfigError::InvalidValue`] (valor que o formato ou o tipo da chave não aceita) e
    /// [`ConfigError::DuplicateEdit`].
    pub fn apply(&self, edits: &[ConfigEdit]) -> Result<Self> {
        self.apply_inner(edits, false)
    }

    fn locate(&self, path: &KeyPath) -> Result<(usize, &ConfigEntry)> {
        let found = self
            .tree
            .entries
            .iter()
            .enumerate()
            .rev()
            .find(|(_, entry)| &entry.path == path);
        let (index, entry) =
            found.ok_or_else(|| ConfigError::PathNotFound { path: path.clone() })?;
        if entry.kind == EntryKind::Section {
            return Err(ConfigError::NotEditable {
                path: path.clone(),
                reason: "é uma seção; edite as chaves dentro dela".into(),
            });
        }
        Ok((index, entry))
    }

    fn apply_inner(&self, edits: &[ConfigEdit], force: bool) -> Result<Self> {
        let mut seen = HashSet::new();
        let mut replacements = Vec::new();
        let mut expected = Vec::new();
        for edit in edits {
            if !seen.insert(&edit.path) {
                return Err(ConfigError::DuplicateEdit {
                    path: edit.path.clone(),
                });
            }
            let (index, entry) = self.locate(&edit.path)?;
            let style = self.styles.get(index).unwrap_or(&EditStyle::Section);
            let rendered = formats::render(
                self.format(),
                style,
                &edit.path,
                &edit.value,
                &self.tree.line_ending,
            )?;
            let unchanged = entry
                .value
                .as_ref()
                .is_some_and(|current| current.same_as(&rendered.expected));
            if unchanged && !force {
                continue;
            }
            let span = entry.value_span.ok_or_else(|| ConfigError::NotEditable {
                path: edit.path.clone(),
                reason: "sem posição de valor no arquivo".into(),
            })?;
            replacements.push(Replacement {
                span,
                text: rendered.text,
            });
            if !unchanged {
                expected.push((edit.path.clone(), rendered.expected));
            }
        }
        if replacements.is_empty() {
            return Ok(self.clone());
        }
        let bytes = splice(&self.bytes, replacements)?;
        let edited =
            Self::parse(self.format(), &bytes).map_err(|error| ConfigError::EditNotConfirmed {
                detail: format!("o resultado não pôde ser relido: {error}"),
            })?;
        confirm(&self.tree, &edited.tree, &expected)?;
        Ok(edited)
    }

    /// Confere se cada valor do arquivo pode ser reescrito pelo Warden sem mudar o que o jogo
    /// lê: escreve de novo todos os valores (com o próprio valor atual), relê e compara. Lista
    /// vazia = o formulário pode editar qualquer chave; senão, as chaves que precisam do editor de
    /// texto (ARCHITECTURE §10.1, "a leitura de teste não volta").
    #[must_use]
    pub fn rewrite_problems(&self) -> Vec<RewriteProblem> {
        let edits: Vec<ConfigEdit> = self
            .tree
            .values()
            .filter_map(|entry| {
                entry
                    .value
                    .clone()
                    .map(|value| ConfigEdit::new(entry.path.clone(), value))
            })
            .collect();
        let unique = dedup_last(edits);
        if self.apply_inner(&unique, true).is_ok() {
            return Vec::new();
        }
        unique
            .into_iter()
            .filter_map(|edit| {
                let path = edit.path.clone();
                self.apply_inner(&[edit], true)
                    .err()
                    .map(|error| RewriteProblem {
                        path,
                        reason: error.to_string(),
                    })
            })
            .collect()
    }
}

/// Mantém só a última edição de cada caminho (chaves repetidas no arquivo).
fn dedup_last(edits: Vec<ConfigEdit>) -> Vec<ConfigEdit> {
    let mut seen = HashSet::new();
    let mut kept: Vec<ConfigEdit> = edits
        .into_iter()
        .rev()
        .filter(|edit| seen.insert(edit.path.clone()))
        .collect();
    kept.reverse();
    kept
}

fn splice(original: &[u8], mut replacements: Vec<Replacement>) -> Result<Vec<u8>> {
    replacements.sort_by_key(|replacement| replacement.span.start);
    let mut out = Vec::with_capacity(original.len() + 64);
    let mut cursor = 0;
    for replacement in replacements {
        let TextSpan { start, end } = replacement.span;
        let before = original.get(cursor..start).filter(|_| start <= end);
        let Some(before) = before else {
            return Err(ConfigError::EditNotConfirmed {
                detail: format!("trechos de edição sobrepostos ou inválidos em {start}..{end}"),
            });
        };
        out.extend_from_slice(before);
        out.extend_from_slice(replacement.text.as_bytes());
        cursor = end;
    }
    let rest = original
        .get(cursor..)
        .ok_or_else(|| ConfigError::EditNotConfirmed {
            detail: format!("posição {cursor} fora do arquivo"),
        })?;
    out.extend_from_slice(rest);
    Ok(out)
}

/// Confirma que só as chaves de `expected` mudaram, e para os valores esperados.
fn confirm(
    before: &ConfigTree,
    after: &ConfigTree,
    expected: &[(KeyPath, ConfigValue)],
) -> Result<()> {
    let changes = diff_trees(before, after);
    for change in &changes {
        let wanted = expected.iter().find(|(path, _)| path == change.path());
        let ok = match (change, wanted) {
            (SemanticChange::Changed { after, .. }, Some((_, value))) => after.same_as(value),
            _ => false,
        };
        if !ok {
            return Err(ConfigError::EditNotConfirmed {
                detail: format!("mudança inesperada na releitura: {change:?}"),
            });
        }
    }
    if changes.len() != expected.len() {
        let missing: Vec<String> = expected
            .iter()
            .filter(|(path, _)| !changes.iter().any(|change| change.path() == path))
            .map(|(path, _)| path.to_string())
            .collect();
        return Err(ConfigError::EditNotConfirmed {
            detail: format!(
                "a releitura não encontrou a mudança em: {}",
                missing.join(", ")
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(format: ConfigFormat, text: &str) -> ConfigDocument {
        ConfigDocument::parse(format, text.as_bytes()).unwrap()
    }

    fn key(parts: &[&str]) -> KeyPath {
        KeyPath::from_keys(parts.iter().copied())
    }

    fn edited(document: &ConfigDocument, edits: &[ConfigEdit]) -> String {
        String::from_utf8(document.apply(edits).unwrap().into_bytes()).unwrap()
    }

    #[test]
    fn edicao_muda_so_o_valor() {
        let text = "\u{feff}# topo\r\n[geral]\r\n\t#Range: 1 ~ 64\r\n\tmax = 16 # fim\r\n";
        let document = doc(ConfigFormat::Toml, text);
        assert!(document.tree().has_bom);
        let entry = document.get(&key(&["geral", "max"])).unwrap();
        assert_eq!(entry.range.as_ref().and_then(|r| r.max), Some(64.0));
        let out = edited(
            &document,
            &[ConfigEdit::new(
                key(&["geral", "max"]),
                ConfigValue::Integer(32),
            )],
        );
        assert_eq!(out, text.replace("16", "32"));
    }

    #[test]
    fn mesma_edicao_nao_muda_bytes() {
        let text = "x = 1.50\n";
        let document = doc(ConfigFormat::Toml, text);
        let out = document
            .apply(&[ConfigEdit::new(key(&["x"]), ConfigValue::Float(1.5))])
            .unwrap();
        assert_eq!(out.as_bytes(), text.as_bytes());
        assert_eq!(document.apply(&[]).unwrap().as_bytes(), text.as_bytes());
    }

    #[test]
    fn erros_de_edicao() {
        let document = doc(ConfigFormat::Toml, "[t]\nx = 1\n");
        let missing = document.apply(&[ConfigEdit::new(key(&["y"]), ConfigValue::Integer(1))]);
        assert!(matches!(missing, Err(ConfigError::PathNotFound { .. })));
        let section = document.apply(&[ConfigEdit::new(key(&["t"]), ConfigValue::Integer(1))]);
        assert!(matches!(section, Err(ConfigError::NotEditable { .. })));
        let edit = ConfigEdit::new(key(&["t", "x"]), ConfigValue::Integer(2));
        let twice = document.apply(&[edit.clone(), edit]);
        assert!(matches!(twice, Err(ConfigError::DuplicateEdit { .. })));
        let null = document.apply(&[ConfigEdit::new(key(&["t", "x"]), ConfigValue::Null)]);
        assert!(matches!(null, Err(ConfigError::InvalidValue { .. })));
    }

    #[test]
    fn varias_edicoes_e_chave_repetida() {
        let document = doc(ConfigFormat::Properties, "a=1\nb=2\na=3\n");
        let out = edited(
            &document,
            &[
                ConfigEdit::new(key(&["a"]), ConfigValue::String("x".into())),
                ConfigEdit::new(key(&["b"]), ConfigValue::Bool(true)),
            ],
        );
        assert_eq!(out, "a=1\nb=true\na=x\n");
    }

    #[test]
    fn splice_recusa_trechos_invalidos() {
        let overlapping = vec![
            Replacement {
                span: TextSpan::new(0, 3),
                text: "x".into(),
            },
            Replacement {
                span: TextSpan::new(1, 2),
                text: "y".into(),
            },
        ];
        assert!(splice(b"abcd", overlapping).is_err());
        let outside = vec![Replacement {
            span: TextSpan::new(2, 9),
            text: String::new(),
        }];
        assert!(splice(b"abcd", outside).is_err());
    }

    #[test]
    fn confirmacao_pega_mudanca_errada() {
        let before = doc(ConfigFormat::Properties, "a=1\nb=2\n");
        let after = doc(ConfigFormat::Properties, "a=9\nb=3\n");
        let expected = [(key(&["a"]), ConfigValue::String("9".into()))];
        assert!(confirm(before.tree(), after.tree(), &expected).is_err());
        let wrong_value = [(key(&["a"]), ConfigValue::String("8".into()))];
        let only_a = doc(ConfigFormat::Properties, "a=9\nb=2\n");
        assert!(confirm(before.tree(), only_a.tree(), &wrong_value).is_err());
        let not_applied = [(key(&["b"]), ConfigValue::String("7".into()))];
        assert!(confirm(before.tree(), before.tree(), &not_applied).is_err());
        assert!(confirm(before.tree(), only_a.tree(), &expected).is_ok());
    }

    #[test]
    fn reescrita_sem_problemas_e_com_problema() {
        let fine = doc(ConfigFormat::Properties, "a=1\na=2\nb = x\n");
        assert!(fine.rewrite_problems().is_empty());
        // Um texto cru que parece JSON não pode ser regravado cru no options.txt.
        let tricky = doc(ConfigFormat::OptionsTxt, "a:1\nb:[1,\n");
        assert!(tricky.rewrite_problems().is_empty());
        let mut broken = doc(ConfigFormat::OptionsTxt, "a:1\nb:2\n");
        broken.styles[1] = EditStyle::Section;
        let problems = broken.rewrite_problems();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].path, key(&["b"]));
    }
}
