//! Critério 2 da P1-06: jar corrompido ou malicioso gera erro (ou aviso), nunca pânico.
//!
//! Três frentes: bytes aleatórios, jars válidos com bytes trocados ou cortados, e cabeçalhos
//! de zip forjados (contagem de entradas absurda, ZIP64, caminhos estranhos).

// Auxiliares de teste fora de funções #[test]: falhar com mensagem é o comportamento certo.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::{JarBuilder, fabric_mod_json};
use proptest::prelude::*;
use warden_jarmeta::{Limits, WarningCode, read_jar_bytes};

/// Jar realista: descritores de vários loaders, manifesto, classes e um jar embutido.
fn sample_jar() -> Vec<u8> {
    let inner = JarBuilder::new()
        .file("fabric.mod.json", fabric_mod_json("inner", "1.0.0", &[]))
        .class("i/A.class", 61)
        .build();
    JarBuilder::new()
        .file("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\nImplementation-Version: 3.1\n")
        .file(
            "fabric.mod.json",
            r#"{"schemaVersion": 1, "id": "amostra", "version": "3.1",
                "depends": {"minecraft": "1.20.x", "fabricloader": ">=0.15"},
                "jars": [{"file": "META-INF/jars/inner.jar"}], "mixins": ["a.mixins.json"]}"#,
        )
        .file(
            "META-INF/mods.toml",
            "modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"MIT\"\n[[mods]]\nmodId=\"amostra\"\nversion=\"${file.jarVersion}\"\n[[dependencies.amostra]]\nmodId=\"minecraft\"\nmandatory=true\nversionRange=\"[1.20.1,1.21)\"\n",
        )
        .file("mcmod.info", r#"[{"modid": "amostra", "mcversion": "1.12.2",}]"#)
        .stored("META-INF/jars/inner.jar", &inner)
        .class("a/B.class", 61)
        .build()
}

fn small_limits() -> Limits {
    Limits {
        max_nested_jar_bytes: 1024 * 1024,
        max_total_nested_bytes: 4 * 1024 * 1024,
        ..Limits::default()
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2000, ..ProptestConfig::default() })]

    #[test]
    fn bytes_aleatorios(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let _ = read_jar_bytes(&bytes, &small_limits());
    }

    #[test]
    fn bytes_aleatorios_com_assinatura_de_zip(tail in prop::collection::vec(any::<u8>(), 0..2048)) {
        let mut bytes = b"PK\x03\x04".to_vec();
        bytes.extend(tail);
        bytes.extend_from_slice(b"PK\x05\x06");
        bytes.extend_from_slice(&[0u8; 18]);
        let _ = read_jar_bytes(&bytes, &small_limits());
    }

    #[test]
    fn jar_valido_com_bytes_trocados(
        edits in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..16),
    ) {
        let mut bytes = sample_jar();
        for (index, value) in edits {
            let i = index.index(bytes.len());
            bytes[i] = value;
        }
        if let Ok(meta) = read_jar_bytes(&bytes, &small_limits()) {
            // O que sair precisa continuar serializável.
            prop_assert!(serde_json::to_string(&meta).is_ok());
        }
    }

    #[test]
    fn jar_valido_cortado(cut in any::<prop::sample::Index>()) {
        let bytes = sample_jar();
        let len = cut.index(bytes.len());
        let _ = read_jar_bytes(&bytes[..len], &small_limits());
    }
}

#[test]
fn amostra_intacta_e_lida_sem_avisos_inesperados() {
    let meta = read_jar_bytes(&sample_jar(), &small_limits()).expect("válido");
    assert_eq!(meta.mods.len(), 3);
    assert_eq!(meta.walk().len(), 2);
    let codes: Vec<_> = meta.warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes, [WarningCode::LenientJson]);
}

/// Fim do diretório central (EOCD) que promete 65.535 entradas num arquivo de 22 bytes.
#[test]
fn eocd_com_contagem_absurda() {
    let mut eocd = b"PK\x05\x06".to_vec();
    eocd.extend_from_slice(&[0, 0, 0, 0]); // disco e disco do diretório
    eocd.extend_from_slice(&u16::MAX.to_le_bytes()); // entradas neste disco
    eocd.extend_from_slice(&u16::MAX.to_le_bytes()); // entradas no total
    eocd.extend_from_slice(&u32::MAX.to_le_bytes()); // tamanho do diretório
    eocd.extend_from_slice(&0u32.to_le_bytes()); // início do diretório
    eocd.extend_from_slice(&0u16.to_le_bytes()); // comentário
    assert!(read_jar_bytes(&eocd, &Limits::default()).is_err());
}

/// Registro ZIP64 que promete 2^62 entradas.
#[test]
fn zip64_com_contagem_absurda() {
    let mut bytes = Vec::new();
    // Registro de fim de diretório ZIP64.
    let zip64_offset = u64::try_from(bytes.len()).expect("cabe");
    bytes.extend_from_slice(b"PK\x06\x06");
    bytes.extend_from_slice(&44u64.to_le_bytes());
    bytes.extend_from_slice(&[45, 0, 45, 0]);
    bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&(1u64 << 62).to_le_bytes());
    bytes.extend_from_slice(&(1u64 << 62).to_le_bytes());
    bytes.extend_from_slice(&(1u64 << 40).to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    // Localizador ZIP64.
    bytes.extend_from_slice(b"PK\x06\x07");
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&zip64_offset.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    // EOCD com os campos em 0xFFFF... (manda ler o ZIP64).
    bytes.extend_from_slice(b"PK\x05\x06");
    bytes.extend_from_slice(&[0xFF; 16]);
    bytes.extend_from_slice(&0u16.to_le_bytes());
    assert!(read_jar_bytes(&bytes, &Limits::default()).is_err());
}

/// Caminhos estranhos não escapam de nada: nada é extraído; só viram nomes.
#[test]
fn caminhos_estranhos_nos_jars_embutidos() {
    let jar = JarBuilder::new()
        .file(
            "fabric.mod.json",
            fabric_mod_json(
                "x",
                "1",
                &[
                    "../../../etc/passwd",
                    "C:\\\\Windows\\\\x.jar",
                    "/abs.jar",
                    "",
                ],
            ),
        )
        .stored("../../../etc/passwd", b"PK nada")
        .build();
    let meta = read_jar_bytes(&jar, &Limits::default()).expect("lido");
    assert_eq!(meta.nested.len(), 4);
    assert!(meta.nested.iter().all(|n| n.metadata.is_none()));
    assert!(meta.warnings.iter().all(|w| matches!(
        w.code,
        WarningCode::MissingNestedJar | WarningCode::UnreadableNestedJar
    )));
}
