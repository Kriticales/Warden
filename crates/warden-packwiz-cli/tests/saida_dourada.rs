//! CA-4: saída real do packwiz (gravada byte a byte em `tests/fixtures/saida/`, ver
//! `FIXTURES.md`) vira linhas limpas, sem ANSI e sem a barra de progresso.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use warden_packwiz_cli::output::{Decoded, LineDecoder, mentions_missing_key};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/saida")
            .join(name),
    )
    .unwrap()
}

/// Decodifica em pedaços de `chunk` bytes, como a leitura do cano entregaria.
fn decode(bytes: &[u8], chunk: usize) -> (Vec<String>, Vec<u8>) {
    let mut decoder = LineDecoder::new();
    let mut items = Vec::new();
    for part in bytes.chunks(chunk) {
        items.extend(decoder.push(part));
    }
    items.extend(decoder.finish());
    let mut lines = Vec::new();
    let mut percents = Vec::new();
    for item in items {
        match item {
            Decoded::Line(line) => lines.push(line),
            Decoded::Progress { label, percent } => {
                assert_eq!(label, "Refreshing index...");
                percents.push(percent);
            }
        }
    }
    (lines, percents)
}

fn expected(name: &str) -> Vec<String> {
    String::from_utf8(fixture(name))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn ca_4_saida_real_vira_linhas_limpas() {
    let raw = fixture("refresh.bin");
    assert!(raw.contains(&0x1b), "a gravação tem ANSI");
    for chunk in [1, 7, 64, 8192] {
        let (lines, percents) = decode(&raw, chunk);
        assert_eq!(lines, expected("refresh.linhas.txt"), "pedaços de {chunk}");
        assert_eq!(percents, [21, 42, 66, 87, 100, 100]);
    }
}

#[test]
fn ca_4_falta_da_chave_e_pack_invalido() {
    let (lines, percents) = decode(&fixture("curseforge-add-sem-chave.bin"), 8192);
    assert_eq!(lines, expected("curseforge-add-sem-chave.linhas.txt"));
    assert!(percents.is_empty());
    assert!(mentions_missing_key(&lines));

    let (lines, _) = decode(&fixture("refresh-pack-invalido.bin"), 3);
    assert_eq!(lines, expected("refresh-pack-invalido.linhas.txt"));
    assert!(!mentions_missing_key(&lines));
}
