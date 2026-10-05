//! Propriedades do histórico com sequências aleatórias de mudanças (QUALITY §4.1):
//!
//! - depois de salvar, não há alterações não salvas e o ponto de segurança da pasta é igual à
//!   árvore salva;
//! - voltar para qualquer versão salva devolve exatamente os arquivos dela, sem tocar nos
//!   ignorados;
//! - as alterações não salvas batem com um modelo simples (o que mudou desde a última versão).

#![allow(linker_messages)]
// libgit2 no MSVC exporta símbolos; veja o relatório da V-01
// Auxiliares fora de `#[test]`: falhar com pânico reprova o teste que chamou.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeMap;

use common::{TestPack, at, identity};
use proptest::prelude::*;
use warden_versioning::{FileChangeKind, RestoreTarget, SaveVersion};

/// Caminhos possíveis: acentos, espaços, pastas fundas e um ignorado pelo `.gitignore`.
const PATHS: [&str; 8] = [
    "config/ação.toml",
    "config/meu mod/opções.json",
    "mods/sodium.pw.toml",
    "mods/local mod.jar",
    "resourcepacks/a/b/c/fundo.zip",
    "kubejs/server_scripts/receitas.js",
    "README.md",
    "logs/latest.log",
];

const IGNORED: &str = "logs/latest.log";

/// Conteúdos possíveis: vazio, LF, CRLF e binário.
const CONTENTS: [&[u8]; 4] = [b"", b"a = 1\n", b"a = 1\r\nb = 2\r\n", &[0, 255, 13, 10, 0]];

#[derive(Debug, Clone)]
enum Step {
    Write(usize, usize),
    Delete(usize),
    Save,
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        4 => (0..PATHS.len(), 0..CONTENTS.len()).prop_map(|(p, c)| Step::Write(p, c)),
        2 => (0..PATHS.len()).prop_map(Step::Delete),
        1 => Just(Step::Save),
    ]
}

/// Arquivos do modelo que não são ignorados.
fn tracked(model: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    model
        .iter()
        .filter(|(path, _)| path.as_str() != IGNORED)
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn salvar_e_voltar_reproduzem_cada_versao(steps in proptest::collection::vec(step(), 1..25)) {
        let (pack, repo) = TestPack::versioned();
        let mut model: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut last_saved: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut versions: Vec<(String, BTreeMap<String, String>)> = Vec::new();
        let mut minute = 1;

        for step in &steps {
            match step {
                Step::Write(p, c) => {
                    pack.write(PATHS[*p], CONTENTS[*c]);
                    model.insert(PATHS[*p].to_owned(), CONTENTS[*c].to_vec());
                }
                Step::Delete(p) => {
                    if model.remove(PATHS[*p]).is_some() {
                        pack.remove(PATHS[*p]);
                    }
                }
                Step::Save => {
                    if tracked(&model) == last_saved {
                        continue;
                    }
                    let version = format!("0.{}.0", versions.len() + 1);
                    repo.save_version(&SaveVersion {
                        version: &version,
                        tag_message: &version,
                        identity: &identity(),
                        when: at(minute),
                        mark_final: false,
                    })
                    .unwrap();
                    minute += 1;
                    prop_assert!(repo.unsaved_changes().unwrap().is_empty());
                    last_saved = tracked(&model);
                    let mut hashes = pack.hashes();
                    hashes.remove(IGNORED);
                    versions.push((version, hashes));
                }
            }
            // Alterações não salvas = diferença entre o modelo e a última versão salva.
            let expected: Vec<(String, FileChangeKind)> = {
                let now = tracked(&model);
                let mut keys: Vec<&String> = now.keys().chain(last_saved.keys()).collect();
                keys.sort();
                keys.dedup();
                keys.into_iter()
                    .filter_map(|path| match (last_saved.get(path), now.get(path)) {
                        (None, Some(_)) => Some((path.clone(), FileChangeKind::Added)),
                        (Some(_), None) => Some((path.clone(), FileChangeKind::Deleted)),
                        (Some(a), Some(b)) if a != b => Some((path.clone(), FileChangeKind::Modified)),
                        _ => None,
                    })
                    .collect()
            };
            let got: Vec<(String, FileChangeKind)> = repo
                .unsaved_changes()
                .unwrap()
                .into_iter()
                .map(|change| (change.path, change.kind))
                .collect();
            prop_assert_eq!(got, expected);
        }

        let ignored = model.get(IGNORED).cloned();
        for (version, hashes) in versions.iter().rev() {
            repo.restore(&RestoreTarget::Version(version.clone()), &identity(), at(minute))
                .unwrap();
            minute += 1;
            let mut now = pack.hashes();
            now.remove(IGNORED);
            prop_assert_eq!(&now, hashes, "versão {}", version);
            match &ignored {
                Some(bytes) => prop_assert_eq!(&pack.read(IGNORED), bytes),
                None => prop_assert!(!pack.exists(IGNORED)),
            }
        }
    }
}
