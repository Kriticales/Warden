//! Testes de propriedade (`proptest`): ida e volta de cada modelo com conteúdo aleatório,
//! leitores e matcher que nunca entram em pânico com entrada qualquer, e edições mínimas que
//! mudam só o campo pedido.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::io::Cursor;

use proptest::prelude::*;
use warden_packwiz::hash::{HashFormat, curseforge_fingerprint, hash_bytes, hash_reader};
use warden_packwiz::{
    Download, IndexEntry, IndexRef, Metafile, ModOption, PackIndex, PackManifest, PackwizIgnore,
    Side, Value, check_relative_path, clean_path, edit,
};

fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z0-9 ._-]{0,12}",
        any::<String>().prop_map(|s| s.chars().take(16).collect()),
        Just("Jade 🔍 \"aspas\" \\ \t\n\u{7f}".to_owned()),
    ]
}

fn key() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z][a-z0-9-]{0,8}",
        any::<String>().prop_map(|s| s.chars().take(8).collect()),
    ]
}

fn scalar() -> impl Strategy<Value = Value> {
    prop_oneof![
        text().prop_map(Value::String),
        any::<i64>().prop_map(Value::Integer),
        any::<f64>()
            .prop_filter("NaN não é igual a si mesmo", |f| !f.is_nan())
            .prop_map(Value::Float),
        any::<bool>().prop_map(Value::Boolean),
    ]
}

fn value() -> impl Strategy<Value = Value> {
    scalar().prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            prop::collection::btree_map(key(), inner, 0..4).prop_map(Value::Table),
        ]
    })
}

fn value_map() -> impl Strategy<Value = BTreeMap<String, Value>> {
    prop::collection::btree_map(key(), value(), 0..4)
}

fn pack() -> impl Strategy<Value = PackManifest> {
    (
        (text(), text(), text(), text()),
        (text(), text(), text()),
        proptest::option::of(prop::collection::btree_map(key(), text(), 0..4)),
        proptest::option::of(prop::collection::btree_map(key(), value_map(), 0..3)),
        proptest::option::of(value_map()),
    )
        .prop_map(
            |(
                (name, author, version, description),
                (file, hash_format, hash),
                versions,
                export,
                options,
            )| {
                PackManifest {
                    name,
                    author,
                    version,
                    description,
                    pack_format: "packwiz:1.1.0".to_owned(),
                    index: IndexRef {
                        file: if file.is_empty() {
                            "index.toml".to_owned()
                        } else {
                            file
                        },
                        hash_format,
                        hash,
                    },
                    versions,
                    export,
                    options,
                }
            },
        )
}

fn side() -> impl Strategy<Value = Side> {
    prop_oneof![
        Just(Side::Unset),
        Just(Side::Both),
        Just(Side::Client),
        Just(Side::Server),
        "[a-z]{1,6}".prop_map(|s| Side::parse(&s)),
    ]
}

fn metafile() -> impl Strategy<Value = Metafile> {
    (
        (text(), text(), side(), any::<bool>()),
        (text(), text(), text(), text()),
        proptest::option::of(prop::collection::btree_map(key(), value_map(), 0..3)),
        proptest::option::of((any::<bool>(), text(), any::<bool>())),
    )
        .prop_map(
            |((name, filename, side, pin), (url, hash_format, hash, mode), update, option)| {
                Metafile {
                    name,
                    filename,
                    side,
                    pin,
                    download: Download {
                        url,
                        hash_format,
                        hash,
                        mode,
                    },
                    update,
                    option: option.map(|(optional, description, default)| ModOption {
                        optional,
                        description,
                        default,
                    }),
                }
            },
        )
}

fn index_entry() -> impl Strategy<Value = IndexEntry> {
    (
        prop_oneof!["[a-z]{1,4}(/[a-z]{1,4}){0,2}(\\.pw\\.toml)?", text()],
        text(),
        text(),
        prop_oneof![Just(String::new()), "[a-z]{1,4}"],
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(
            |(file, hash, hash_format, alias, metafile, preserve)| IndexEntry {
                file,
                hash,
                hash_format,
                alias,
                metafile,
                preserve,
            },
        )
}

proptest! {
    // Sem arquivo de regressões: um caso que falhar vira teste fixo nos módulos.
    #![proptest_config(ProptestConfig {
        cases: 512,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn pack_ida_e_volta(pack in pack()) {
        let text = pack.to_toml_string();
        let parsed = PackManifest::parse(&text).unwrap();
        prop_assert!(parsed.unknown_keys.is_empty());
        prop_assert_eq!(&parsed.value, &pack);
        prop_assert_eq!(parsed.value.to_toml_string(), text);
    }

    #[test]
    fn metafile_ida_e_volta(metafile in metafile()) {
        let text = metafile.to_toml_string();
        let parsed = Metafile::parse(&text).unwrap();
        prop_assert!(parsed.unknown_keys.is_empty());
        prop_assert_eq!(&parsed.value, &metafile);
        prop_assert_eq!(parsed.value.to_toml_string(), text);
    }

    #[test]
    fn indice_ida_e_volta(format in text(), files in prop::collection::vec(index_entry(), 0..8)) {
        let index = PackIndex { hash_format: if format.is_empty() { "sha256".to_owned() } else { format }, files };
        let text = index.to_toml_string();
        let parsed = PackIndex::parse(&text).unwrap().value;
        prop_assert_eq!(&parsed.files, &index.normalized_entries());
        prop_assert_eq!(parsed.to_toml_string(), text);
    }

    #[test]
    fn leitores_nunca_entram_em_panico(input in any::<String>()) {
        let _ = PackManifest::parse(&input);
        let _ = Metafile::parse(&input);
        let _ = PackIndex::parse(&input);
        let _ = edit::set_metafile_side(&input, &Side::Client);
        let _ = edit::set_index_preserve(&input, "a", true);
        let _ = warden_packwiz::filename_from_url(&input);
    }

    #[test]
    fn leitores_com_toml_quase_valido(
        lines in prop::collection::vec(
            prop_oneof![
                "[a-z-]{1,8} = \"[^\"\\\\]{0,8}\"",
                "[a-z-]{1,8} = [0-9]{1,4}",
                "[a-z-]{1,8} = (true|false)",
                "\\[[a-z-]{1,8}(\\.[a-z-]{1,8})?\\]",
                "\\[\\[[a-z-]{1,8}\\]\\]",
                "[a-z-]{1,8} = \\[\"a\", 1\\]",
            ],
            0..12,
        )
    ) {
        let input = lines.join("\n");
        let _ = PackManifest::parse(&input);
        let _ = Metafile::parse(&input);
        let _ = PackIndex::parse(&input);
    }

    #[test]
    fn matcher_nunca_entra_em_panico(
        lines in prop::collection::vec(any::<String>(), 0..8),
        path in any::<String>(),
    ) {
        let text = lines.join("\n");
        let ignore = PackwizIgnore::for_pack(Some(&text));
        let excluded = ignore.is_excluded(&path);
        prop_assert_eq!(excluded, ignore.excluding_line(&path).is_some());
        if ignore.matches(&path) {
            prop_assert!(excluded);
        }
    }

    #[test]
    fn padrao_ancorado_so_casa_na_raiz(name in "[a-z]{1,8}", dir in "[a-z]{1,8}", file in "[a-z]{1,8}") {
        prop_assume!(dir != name);
        let ignore = PackwizIgnore::from_lines([format!("/{name}/").as_str()]);
        let inside = format!("{name}/{file}");
        let nested = format!("{dir}/{name}/{file}");
        prop_assert!(ignore.is_excluded(&inside));
        prop_assert!(!ignore.is_excluded(&nested));
        let anywhere = PackwizIgnore::from_lines([format!("{name}/").as_str()]);
        prop_assert!(anywhere.is_excluded(&nested));
    }

    #[test]
    fn hash_aos_pedacos_igual_ao_da_memoria(data in prop::collection::vec(any::<u8>(), 0..4096)) {
        for format in HashFormat::ALL {
            let mut cursor = Cursor::new(&data);
            prop_assert_eq!(hash_reader(format, &mut cursor).unwrap(), hash_bytes(format, &data));
        }
        let without_whitespace: Vec<u8> =
            data.iter().copied().filter(|b| ![9, 10, 13, 32].contains(b)).collect();
        prop_assert_eq!(curseforge_fingerprint(&data), curseforge_fingerprint(&without_whitespace));
    }

    #[test]
    fn edicao_muda_so_o_campo(metafile in metafile(), new_side in side(), pin in any::<bool>()) {
        let text = metafile.to_toml_string();
        let edited = edit::set_metafile_side(&text, &new_side).unwrap();
        let edited = edit::set_metafile_pin(&edited, pin).unwrap();
        let parsed = Metafile::parse(&edited).unwrap().value;
        let mut expected = metafile.clone();
        expected.side = if new_side == Side::Unset { Side::Unset } else { new_side };
        expected.pin = pin;
        prop_assert_eq!(parsed, expected);
    }

}

/// Pedaços de caminho que já enganaram (ou poderiam enganar) a validação: `..` com espaço ou
/// ponto no fim, sobrescritos, dispositivos, unidade, fluxo alternativo e separadores.
fn path_segment() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => "[a-zA-Z0-9_.-]{1,10}",
        1 => Just(".".to_owned()),
        1 => Just("..".to_owned()),
        1 => Just(String::new()),
        2 => "\\.{1,3}[ .]{1,3}",
        1 => "[a-z]{1,6}[ .]{1,2}",
        1 => "\\.\\.[\u{b2}\u{b3}\u{b9}\u{2074}\u{a0}\u{3000}\u{ff0e}]",
        1 => "(?i)(con|nul|aux|prn|com[0-9\u{b9}\u{b2}\u{b3}]|lpt[0-9\u{b9}\u{b2}\u{b3}])( ?\\.[a-z]{1,3})?",
        1 => "[a-zA-Z]:",
        1 => "[a-z]{1,4}:[a-z]{1,4}",
        1 => any::<String>().prop_map(|s| s.chars().take(6).collect()),
    ]
}

fn pack_path() -> impl Strategy<Value = String> {
    (
        prop::collection::vec(path_segment(), 1..6),
        prop::collection::vec(prop_oneof![Just("/"), Just("\\")], 6),
    )
        .prop_map(|(segments, separators)| {
            let mut text = String::new();
            for (index, segment) in segments.iter().enumerate() {
                if index > 0 {
                    text.push_str(separators[index]);
                }
                text.push_str(segment);
            }
            text
        })
}

/// A propriedade do `check_relative_path`: o caminho aceito, depois de limpo, não sobe de
/// pasta, não tem raiz nem `:`, e nenhum componente muda de nome no Windows. No Windows, o
/// próprio sistema confere: o `GetFullPathNameW` (por trás de `std::path::absolute`) devolve a
/// raiz seguida exatamente dos componentes limpos.
fn assert_stays_inside(path: &str) -> std::result::Result<(), TestCaseError> {
    if check_relative_path(path).is_err() {
        return Ok(());
    }
    let cleaned = clean_path(&path.replace('\\', "/"));
    prop_assert!(
        !cleaned.starts_with('/') && !cleaned.contains(':'),
        "{path:?}"
    );
    for component in cleaned.split('/') {
        prop_assert!(
            component != ".." && component != "." && !component.is_empty(),
            "{path:?}"
        );
        prop_assert!(!component.ends_with(['.', ' ']), "{path:?}");
    }
    #[cfg(windows)]
    {
        let root = std::path::Path::new(r"C:\warden-raiz-do-teste");
        let absolute = std::path::absolute(root.join(path)).unwrap();
        let expected: std::path::PathBuf = std::iter::once(root.as_os_str())
            .chain(cleaned.split('/').map(std::ffi::OsStr::new))
            .collect();
        prop_assert_eq!(absolute, expected, "{:?}", path);
    }
    Ok(())
}

/// Casos que já falharam ou que a pesquisa da normalização do Win32 apontou.
#[test]
fn caminhos_de_regressao() {
    for path in [
        ".. /x",
        "..  /x",
        ".. ./x",
        "a/.. ",
        "a/.. .",
        "a/...",
        "...",
        ". /x",
        "pack.toml.",
        "index.toml ",
        "config./x",
        "config /x",
        "..\u{2074}",
        "..\u{2074}/x",
        "..\u{a0}/x",
        "..\u{ff0e}/x",
        "\u{ff0e}\u{ff0e}/x",
        "..a/b",
        "COM\u{b9}",
        "lpt\u{b3}.txt",
        "com0",
    ] {
        assert_stays_inside(path).unwrap();
    }
    for path in [".. /x", "a/.. ", "pack.toml.", "config./x", "COM\u{b9}"] {
        assert!(check_relative_path(path).is_err(), "{path:?}");
    }
}

proptest! {
    // Semente fixa: os mesmos 4096 casos em toda execução e em todas as máquinas. Um caso novo
    // que falhar vira linha em `caminhos_de_regressao`.
    #![proptest_config(ProptestConfig {
        cases: 4096,
        failure_persistence: None,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5741_5244_454e),
        ..ProptestConfig::default()
    })]

    #[test]
    fn caminho_aceito_nunca_sai_do_pack(path in pack_path()) {
        assert_stays_inside(&path)?;
    }

    #[test]
    fn caminho_qualquer_nunca_sai_do_pack(path in any::<String>()) {
        assert_stays_inside(&path)?;
        let cleaned = clean_path(&path);
        prop_assert_eq!(clean_path(&cleaned), cleaned);
    }
}
