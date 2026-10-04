//! Entradas malformadas nunca causam pânico (QUALITY §9.6): bytes aleatórios, texto com a
//! sintaxe dos formatos embaralhada e arquivos reais corrompidos. Quando a leitura dá certo, os
//! invariantes continuam valendo (bytes idênticos sem edição, edição conferida).

// Auxiliares de teste fora de #[test]: falhar com pânico reprova o teste.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use proptest::prelude::*;
use warden_configs::{
    ConfigDocument, ConfigEdit, ConfigFormat, ConfigValue, ConfigsErrorCode, compare,
};
use warden_core::{DomainCode, DomainError};

const INVALID: DomainCode<ConfigsErrorCode> = DomainCode::Domain(ConfigsErrorCode::InvalidValue);

fn any_format() -> impl Strategy<Value = ConfigFormat> {
    prop::sample::select(ConfigFormat::ALL.to_vec())
}

/// Texto feito dos pedaços que mais confundem os parsers.
fn syntax_soup() -> impl Strategy<Value = String> {
    let pieces = prop::sample::select(vec![
        "\n",
        "\r\n",
        "\r",
        " ",
        "\t",
        "#",
        "!",
        "//",
        "/*",
        "*/",
        "=",
        ":",
        "{",
        "}",
        "[",
        "]",
        "[[",
        "]]",
        "<",
        ">",
        "\"",
        "'",
        "'''",
        "\"\"\"",
        "\\",
        "\\u",
        "\\uD83D",
        ",",
        ".",
        "~",
        "S:",
        "B:",
        "I:",
        "D:",
        "true",
        "false",
        "null",
        "NaN",
        "Infinity",
        "0x1F",
        "-1.5e3",
        "key",
        "a b",
        "é",
        "\u{feff}",
        "~CONFIG_VERSION:",
        "START: \"x\"",
        "0",
        "1979-05-27T07:32:00Z",
    ]);
    prop::collection::vec(pieces, 0..80).prop_map(|parts| parts.concat())
}

/// Lê e, se der certo, confere os invariantes de ida e volta e de edição.
fn exercise(format: ConfigFormat, bytes: &[u8]) {
    let Ok(document) = ConfigDocument::parse(format, bytes) else {
        return;
    };
    assert_eq!(document.as_bytes(), bytes);
    assert_eq!(document.apply(&[]).unwrap().as_bytes(), bytes);
    let _problems = document.rewrite_problems();
    let first_value = document.tree().values().last().cloned();
    if let Some(entry) = first_value
        && let Some(value) = common::different_value(&entry)
        && let Ok(edited) = document.apply(&[ConfigEdit::new(entry.path.clone(), value)])
    {
        let diff = compare(format, bytes, edited.as_bytes()).unwrap();
        assert_eq!(diff.changes.len(), 1);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2_000, ..ProptestConfig::default() })]

    #[test]
    fn bytes_aleatorios_nunca_causam_panico(
        format in any_format(),
        bytes in prop::collection::vec(any::<u8>(), 0..400),
    ) {
        exercise(format, &bytes);
    }

    #[test]
    fn sintaxe_embaralhada_nunca_causa_panico(format in any_format(), text in syntax_soup()) {
        exercise(format, text.as_bytes());
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 300, ..ProptestConfig::default() })]

    /// Arquivo real com um trecho apagado, repetido ou trocado.
    #[test]
    fn arquivos_reais_corrompidos_nunca_causam_panico(
        format in any_format(),
        pick in any::<prop::sample::Index>(),
        start in any::<prop::sample::Index>(),
        length in 0usize..64,
        operation in 0u8..3,
        filler in any::<u8>(),
    ) {
        let samples = common::samples(format);
        let sample = pick.get(&samples);
        let mut bytes = sample.bytes.clone();
        let at = start.index(bytes.len().max(1)).min(bytes.len());
        let end = (at + length).min(bytes.len());
        match operation {
            0 => { bytes.drain(at..end); }
            1 => {
                let copy: Vec<u8> = bytes[at..end].to_vec();
                bytes.splice(at..at, copy);
            }
            _ => {
                for byte in &mut bytes[at..end] {
                    *byte = filler;
                }
            }
        }
        exercise(format, &bytes);
    }
}

fn single_key_document(format: ConfigFormat) -> (&'static [u8], warden_configs::KeyPath) {
    use warden_configs::KeyPath;
    match format {
        ConfigFormat::Toml => (b"[a]\nk = \"x\" # c\n", KeyPath::from_keys(["a", "k"])),
        ConfigFormat::Json => (b"{\n  // c\n  \"k\": \"x\"\n}", KeyPath::from_keys(["k"])),
        ConfigFormat::Json5 => (b"{ k: 'x', }", KeyPath::from_keys(["k"])),
        ConfigFormat::Properties => (b"# c\nk=x\n", KeyPath::from_keys(["k"])),
        ConfigFormat::LegacyCfg => (b"a {\n    S:k=x\n}\n", KeyPath::from_keys(["a", "k"])),
        ConfigFormat::OptionsTxt => (b"k:x\n", KeyPath::from_keys(["k"])),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1_000, ..ProptestConfig::default() })]

    /// Qualquer texto gravado volta igual na releitura, ou é recusado com erro claro.
    #[test]
    fn qualquer_texto_volta_igual_ou_e_recusado(format in any_format(), text in any::<String>()) {
        let (bytes, path) = single_key_document(format);
        let document = ConfigDocument::parse(format, bytes).unwrap();
        match document.apply(&[ConfigEdit::new(path.clone(), ConfigValue::String(text.clone()))]) {
            Ok(edited) => {
                prop_assert_eq!(
                    edited.get(&path).and_then(|entry| entry.value.clone()),
                    Some(ConfigValue::String(text))
                );
            }
            Err(error) => prop_assert_eq!(error.code(), INVALID, "{}", error),
        }
    }

    /// Números e liga/desliga voltam com o mesmo valor (como texto nos formatos sem tipos).
    #[test]
    fn numeros_voltam_iguais(format in any_format(), integer in any::<i64>(), float in any::<f64>()) {
        let (bytes, path) = single_key_document(format);
        let document = ConfigDocument::parse(format, bytes).unwrap();
        for value in [ConfigValue::Integer(integer), ConfigValue::Float(float), ConfigValue::Bool(integer % 2 == 0)] {
            match document.apply(&[ConfigEdit::new(path.clone(), value.clone())]) {
                Ok(edited) => {
                    let found = edited.get(&path).and_then(|entry| entry.value.clone()).unwrap();
                    let typed = matches!(format, ConfigFormat::Toml | ConfigFormat::Json | ConfigFormat::Json5);
                    if typed {
                        prop_assert!(found.same_as(&value), "{:?} != {:?}", found, value);
                    } else {
                        prop_assert_eq!(found, ConfigValue::String(value.as_plain_text().unwrap()));
                    }
                }
                Err(error) => prop_assert_eq!(error.code(), INVALID, "{}", error),
            }
        }
    }
}
