//! murmur2 da CurseForge contra vetores calculados pela biblioteca Go que o packwiz usa
//! (`tests/fixtures/murmur2/vetores.txt`; origem em `FIXTURES.md`). A conferência com a API da
//! CurseForge para jars reais fica em `rede_curseforge.rs`.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::io::Cursor;
use std::path::Path;

use warden_packwiz::hash::{
    HashFormat, curseforge_fingerprint, curseforge_fingerprint_reader, hash_reader, murmur2,
};

#[test]
fn vetores_da_biblioteca_go() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/murmur2/vetores.txt");
    let text = fs::read_to_string(path).unwrap();
    let mut count = 0;
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<&str> = line.split(' ').collect();
        let [length, hex, curseforge, raw] = fields.as_slice() else {
            panic!("linha inválida: {line}");
        };
        let data = decode_hex(hex);
        assert_eq!(data.len(), length.parse::<usize>().unwrap());
        let curseforge: u32 = curseforge.parse().unwrap();
        assert_eq!(curseforge_fingerprint(&data), curseforge, "{line}");
        assert_eq!(murmur2(&data, 1), raw.parse::<u32>().unwrap(), "{line}");
        let mut cursor = Cursor::new(&data);
        assert_eq!(
            curseforge_fingerprint_reader(&mut cursor).unwrap(),
            curseforge
        );
        let mut cursor = Cursor::new(&data);
        assert_eq!(
            hash_reader(HashFormat::Murmur2, &mut cursor).unwrap(),
            curseforge.to_string()
        );
        count += 1;
    }
    assert!(count >= 50, "{count}");
}

fn decode_hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
        .collect()
}
