//! Conformidade: o pack está "como o packwiz produziria" (ARCHITECTURE §6.1, validação)?
//!
//! [`check_conformance`] copia o pack para uma pasta de staging, retrata todos os arquivos
//! da cópia (SHA-256), roda `packwiz refresh` nela e retrata de novo. O pack é conforme se
//! nada mudou. A comparação é entre a cópia antes e depois do refresh (e não com o original),
//! então uma mudança no pack durante a verificação não gera diferença falsa. O original nunca
//! é tocado.
//!
//! Além da lista de arquivos alterados, as entradas do índice são comparadas uma a uma, para a
//! interface dizer **o que** está desatualizado (arquivo novo fora do índice, entrada de
//! arquivo apagado, hash velho).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use warden_packwiz::{IndexEntry, PACK_FILE, PackIndex, PackManifest};

use crate::error::Result;
use crate::runner::{Packwiz, RunContext};
use crate::staging::{Staging, snapshot};

/// Uma diferença entre o pack e o que o packwiz produziria.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Difference {
    /// O refresh mudou o conteúdo deste arquivo (`pack.toml`, o índice…).
    FileChanged {
        /// Caminho relativo à pasta do pack.
        path: String,
    },
    /// O refresh criou este arquivo.
    FileAdded {
        /// Caminho relativo à pasta do pack.
        path: String,
    },
    /// O refresh apagou este arquivo.
    FileRemoved {
        /// Caminho relativo à pasta do pack.
        path: String,
    },
    /// Arquivo que deveria estar no índice e não está.
    IndexEntryAdded {
        /// Caminho do arquivo, como no índice.
        file: String,
    },
    /// Entrada do índice de um arquivo que não existe mais (ou que o `.packwizignore` exclui).
    IndexEntryRemoved {
        /// Caminho do arquivo, como no índice.
        file: String,
    },
    /// Entrada do índice com hash, formato ou marcações diferentes do arquivo atual.
    IndexEntryChanged {
        /// Caminho do arquivo, como no índice.
        file: String,
    },
}

/// Resultado de [`check_conformance`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConformanceReport {
    /// As diferenças, em ordem: arquivos, depois entradas do índice. Vazio = conforme.
    pub differences: Vec<Difference>,
}

impl ConformanceReport {
    /// O pack está exatamente como o packwiz produziria.
    #[must_use]
    pub fn is_conforming(&self) -> bool {
        self.differences.is_empty()
    }
}

/// Verifica a conformidade do pack em `pack_dir`, usando `staging_dir` (vazia ou
/// inexistente; apagada no fim) para a cópia.
///
/// Erros: os de [`Staging::copy_from`] e de [`Packwiz::refresh`] (um pack que o packwiz
/// não consegue ler, como um `pack.toml` quebrado, é erro e não diferença).
pub async fn check_conformance(
    packwiz: &Packwiz,
    pack_dir: &Path,
    staging_dir: &Path,
    ctx: RunContext<'_>,
) -> Result<ConformanceReport> {
    let staging = Staging::copy_from(pack_dir, staging_dir)?;
    let before = snapshot(staging.path(), |_| true)?;
    let index_before = read_index(staging.path());
    packwiz.refresh(staging.path(), false, ctx).await?;
    let after = snapshot(staging.path(), |_| true)?;
    let index_after = read_index(staging.path());
    let mut differences = file_differences(&before, &after);
    differences.extend(index_differences(
        index_before.as_ref(),
        index_after.as_ref(),
    ));
    Ok(ConformanceReport { differences })
}

/// Diferenças entre dois retratos (caminho → hash).
#[must_use]
pub fn file_differences(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<Difference> {
    let mut differences = Vec::new();
    for (path, hash) in after {
        match before.get(path) {
            None => differences.push(Difference::FileAdded { path: path.clone() }),
            Some(old) if old != hash => {
                differences.push(Difference::FileChanged { path: path.clone() });
            }
            Some(_) => {}
        }
    }
    for path in before.keys().filter(|path| !after.contains_key(*path)) {
        differences.push(Difference::FileRemoved { path: path.clone() });
    }
    differences.sort();
    differences
}

/// Diferenças entre as entradas de dois índices. Um índice ilegível conta como vazio.
#[must_use]
pub fn index_differences(before: Option<&PackIndex>, after: Option<&PackIndex>) -> Vec<Difference> {
    let entries = |index: Option<&PackIndex>| -> BTreeMap<String, IndexEntry> {
        index
            .map(PackIndex::normalized_entries)
            .unwrap_or_default()
            .into_iter()
            .map(|entry| (entry.file.clone(), entry))
            .collect()
    };
    let before = entries(before);
    let after = entries(after);
    let mut differences = Vec::new();
    for (file, entry) in &after {
        match before.get(file) {
            None => differences.push(Difference::IndexEntryAdded { file: file.clone() }),
            Some(old) if old != entry => {
                differences.push(Difference::IndexEntryChanged { file: file.clone() });
            }
            Some(_) => {}
        }
    }
    for file in before.keys().filter(|file| !after.contains_key(*file)) {
        differences.push(Difference::IndexEntryRemoved { file: file.clone() });
    }
    differences.sort();
    differences
}

/// O índice do pack, se `pack.toml` e o índice puderem ser lidos.
fn read_index(pack_dir: &Path) -> Option<PackIndex> {
    let manifest = PackManifest::parse(&fs::read_to_string(pack_dir.join(PACK_FILE)).ok()?)
        .ok()?
        .value;
    let path = warden_core::resolve_inside(pack_dir, &manifest.index.file).ok()?;
    PackIndex::parse(&fs::read_to_string(path).ok()?)
        .ok()
        .map(|parsed| parsed.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(items: &[(&str, &str)]) -> BTreeMap<String, String> {
        items
            .iter()
            .map(|(path, hash)| ((*path).to_owned(), (*hash).to_owned()))
            .collect()
    }

    #[test]
    fn diferencas_de_arquivos() {
        let before = map(&[("a", "1"), ("b", "2"), ("c", "3")]);
        let after = map(&[("a", "1"), ("b", "9"), ("d", "4")]);
        assert_eq!(
            file_differences(&before, &after),
            vec![
                Difference::FileChanged { path: "b".into() },
                Difference::FileAdded { path: "d".into() },
                Difference::FileRemoved { path: "c".into() },
            ]
        );
        assert!(file_differences(&before, &before).is_empty());
    }

    #[test]
    fn diferencas_do_indice() {
        let mut old = PackIndex::default();
        old.files.push(IndexEntry::new("config/a.txt", "1"));
        old.files.push(IndexEntry::new("config/b.txt", "2"));
        let mut new = PackIndex::default();
        new.files.push(IndexEntry::new("config/a.txt", "1"));
        new.files.push(IndexEntry::new("config/b.txt", "3"));
        new.files.push(IndexEntry::new("config/c.txt", "4"));
        assert_eq!(
            index_differences(Some(&old), Some(&new)),
            vec![
                Difference::IndexEntryAdded {
                    file: "config/c.txt".into()
                },
                Difference::IndexEntryChanged {
                    file: "config/b.txt".into()
                },
            ]
        );
        assert_eq!(
            index_differences(Some(&new), None),
            vec![
                Difference::IndexEntryRemoved {
                    file: "config/a.txt".into()
                },
                Difference::IndexEntryRemoved {
                    file: "config/b.txt".into()
                },
                Difference::IndexEntryRemoved {
                    file: "config/c.txt".into()
                },
            ]
        );
    }

    #[test]
    fn relatorio_conforme_e_json() {
        let report = ConformanceReport {
            differences: vec![Difference::FileChanged {
                path: "index.toml".into(),
            }],
        };
        assert!(!report.is_conforming());
        assert!(
            ConformanceReport {
                differences: vec![]
            }
            .is_conforming()
        );
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "differences": [{ "kind": "fileChanged", "path": "index.toml" }] })
        );
    }
}
