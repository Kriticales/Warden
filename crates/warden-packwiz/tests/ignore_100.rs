//! CA-4 da P1-01: o matcher do `.packwizignore` dá o mesmo resultado que o `packwiz refresh`
//! real para 100 caminhos (fixture gravada por `cargo xtask fixtures-packwiz`; o teste
//! `packwiz_real` repete a comparação ao vivo quando o binário está disponível).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use warden_packwiz::PackwizIgnore;

fn fixture(name: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ignore-100")
        .join(name);
    fs::read_to_string(path).unwrap()
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

#[test]
fn ca_4_cem_caminhos_iguais_ao_packwiz_refresh() {
    let paths = lines(&fixture("caminhos.txt"));
    assert_eq!(paths.len(), 100);
    let indexed: BTreeSet<String> = lines(&fixture("indexados-pack-file-relativo.txt"))
        .into_iter()
        .collect();
    let ignore = PackwizIgnore::for_pack(Some(&fixture("packwizignore.txt")));
    let mut differences = Vec::new();
    for path in &paths {
        let ours = !ignore.is_excluded(path);
        let packwiz = indexed.contains(path);
        if ours != packwiz {
            differences.push(format!("{path}: Warden={ours} packwiz={packwiz}"));
        }
    }
    assert!(differences.is_empty(), "{differences:#?}");
    // O índice do packwiz não tem nada fora da lista (além do que o Warden prevê).
    assert!(indexed.iter().all(|path| paths.contains(path)));
    // A lista exercita os dois lados.
    assert!(
        indexed.len() > 20 && indexed.len() < 80,
        "{}",
        indexed.len()
    );
}

#[test]
fn pack_file_absoluto_perde_os_padroes_ancorados() {
    // Evidência do achado registrado em FIXTURES.md: com `--pack-file` absoluto o packwiz
    // indexa arquivos que os padrões ancorados deveriam excluir.
    let relative: BTreeSet<String> = lines(&fixture("indexados-pack-file-relativo.txt"))
        .into_iter()
        .collect();
    let absolute: BTreeSet<String> = lines(&fixture("indexados-pack-file-absoluto.txt"))
        .into_iter()
        .collect();
    assert!(relative.is_subset(&absolute));
    for leaked in [
        "saves/Mundo Novo/level.dat",
        ".warden/estado.json",
        "kubejs/config/web_server.json",
        "export.zip",
    ] {
        assert!(absolute.contains(leaked), "{leaked}");
        assert!(!relative.contains(leaked), "{leaked}");
    }
}
