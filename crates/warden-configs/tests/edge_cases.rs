//! Casos de borda de cada formato, com o resultado esperado byte a byte.

// Auxiliares de teste fora de #[test]: falhar com pânico reprova o teste.
#![allow(clippy::unwrap_used, clippy::panic)]

use warden_configs::{
    CfgType, ConfigDocument, ConfigEdit, ConfigError, ConfigFormat, ConfigValue, EntryKind,
    KeyPath, MAX_STRUCTURED_BYTES, compare,
};

fn key(parts: &[&str]) -> KeyPath {
    KeyPath::from_keys(parts.iter().copied())
}

fn edit(format: ConfigFormat, before: &str, path: KeyPath, value: ConfigValue) -> String {
    let document = ConfigDocument::parse(format, before.as_bytes()).unwrap();
    let edited = document.apply(&[ConfigEdit::new(path, value)]).unwrap();
    String::from_utf8(edited.into_bytes()).unwrap()
}

fn s(text: &str) -> ConfigValue {
    ConfigValue::String(text.into())
}

#[test]
fn bom_crlf_e_sem_quebra_final_sao_preservados_em_todos_os_formatos() {
    let cases: [(ConfigFormat, &str, KeyPath, ConfigValue, &str); 6] = [
        (
            ConfigFormat::Toml,
            "\u{feff}a = 1\r\nb = 2",
            key(&["b"]),
            ConfigValue::Integer(3),
            "\u{feff}a = 1\r\nb = 3",
        ),
        (
            ConfigFormat::Json,
            "\u{feff}{\r\n\t\"a\": 1\r\n}",
            key(&["a"]),
            ConfigValue::Integer(2),
            "\u{feff}{\r\n\t\"a\": 2\r\n}",
        ),
        (
            ConfigFormat::Json5,
            "\u{feff}{a:1,}",
            key(&["a"]),
            ConfigValue::Integer(2),
            "\u{feff}{a:2,}",
        ),
        (
            ConfigFormat::Properties,
            "\u{feff}a=1\r\nb=2",
            key(&["b"]),
            s("3"),
            "\u{feff}a=1\r\nb=3",
        ),
        (
            ConfigFormat::LegacyCfg,
            "\u{feff}c {\r\n    I:a=1\r\n}",
            key(&["c", "a"]),
            ConfigValue::Integer(2),
            "\u{feff}c {\r\n    I:a=2\r\n}",
        ),
        (
            ConfigFormat::OptionsTxt,
            "\u{feff}a:1\r\nb:2",
            key(&["b"]),
            s("3"),
            "\u{feff}a:1\r\nb:3",
        ),
    ];
    for (format, before, path, value, after) in cases {
        let document = ConfigDocument::parse(format, before.as_bytes()).unwrap();
        assert!(document.tree().has_bom, "{format}");
        let span = document.get(&path).unwrap().value_span.unwrap();
        assert!(span.start >= 3, "{format}: a posição conta o BOM");
        assert_eq!(edit(format, before, path, value), after, "{format}");
    }
}

#[test]
fn toml_mantem_aspas_simples_e_estrutura() {
    let text = "title = 'x'\n[[mods]]\nid = \"a\"\n[[mods]]\nid = \"b\"\npoint = { x = 1, y = [1, 2] }\nlista = [{ n = 1 }, 2]\nwhen = 1979-05-27T07:32:00Z\na.b.c = true\n";
    let document = ConfigDocument::parse(ConfigFormat::Toml, text.as_bytes()).unwrap();
    let paths: Vec<String> = document
        .tree()
        .entries
        .iter()
        .map(|e| e.path.to_string())
        .collect();
    assert_eq!(
        paths,
        [
            "title",
            "mods",
            "mods[0]",
            "mods[0].id",
            "mods[1]",
            "mods[1].id",
            "mods[1].point",
            "mods[1].point.x",
            "mods[1].point.y",
            "mods[1].lista",
            "mods[1].lista[0]",
            "mods[1].lista[0].n",
            "mods[1].lista[1]",
            "mods[1].when",
            "mods[1].a",
            "mods[1].a.b",
            "mods[1].a.b.c",
        ]
    );
    let out = edit(ConfigFormat::Toml, text, key(&["title"]), s("novo"));
    assert!(out.starts_with("title = 'novo'\n"));
    let out = edit(ConfigFormat::Toml, text, key(&["title"]), s("it's"));
    assert!(out.starts_with("title = \"it's\"\n"));
    let y = key(&["mods"])
        .child_index(1)
        .child_key("point")
        .child_key("y");
    let list = ConfigValue::List(vec![ConfigValue::Integer(3)]);
    assert!(edit(ConfigFormat::Toml, text, y, list).contains("y = [3] }"));
    let when = key(&["mods"]).child_index(1).child_key("when");
    let out = edit(
        ConfigFormat::Toml,
        text,
        when.clone(),
        ConfigValue::Datetime("2020-01-02".into()),
    );
    assert!(out.contains("when = 2020-01-02\n"));
    let document = ConfigDocument::parse(ConfigFormat::Toml, text.as_bytes()).unwrap();
    let bad = document.apply(&[ConfigEdit::new(when, ConfigValue::Datetime("ontem".into()))]);
    assert!(matches!(bad, Err(ConfigError::InvalidValue { .. })));
    let section = document.apply(&[ConfigEdit::new(key(&["mods"]), ConfigValue::Null)]);
    assert!(matches!(section, Err(ConfigError::NotEditable { .. })));
}

#[test]
fn toml_tabelas_fora_de_ordem_saem_na_ordem_do_arquivo() {
    let text = "[a]\nx = 1\n[b]\ny = 2\n[a.c]\nz = 3\n";
    let document = ConfigDocument::parse(ConfigFormat::Toml, text.as_bytes()).unwrap();
    let lines: Vec<(String, usize)> = document
        .tree()
        .entries
        .iter()
        .map(|e| (e.path.to_string(), e.line))
        .collect();
    assert_eq!(
        lines,
        [
            ("a".to_owned(), 1),
            ("a.x".to_owned(), 2),
            ("b".to_owned(), 3),
            ("b.y".to_owned(), 4),
            ("a.c".to_owned(), 5),
            ("a.c.z".to_owned(), 6),
        ]
    );
}

#[test]
fn toml_invalido_informa_linha_e_coluna() {
    let error = ConfigDocument::parse(ConfigFormat::Toml, b"a = 1\nb = = 2\n").unwrap_err();
    let ConfigError::Parse { line, column, .. } = error else {
        panic!("{error:?}")
    };
    assert_eq!(line, 2);
    assert!(column >= 4);
}

#[test]
fn json5_mantem_aspas_simples_e_chaves_sem_aspas() {
    let text = "{\n\t// Comentário\n\tname: 'a',\n\t\"x\": +5, hex: 0xFF, inf: Infinity,\n}";
    let document = ConfigDocument::parse(ConfigFormat::Json5, text.as_bytes()).unwrap();
    let name = document.get(&key(&["name"])).unwrap();
    assert_eq!(name.comment.as_deref(), Some("Comentário"));
    assert_eq!(
        document.get(&key(&["hex"])).unwrap().value,
        Some(ConfigValue::Integer(255))
    );
    let out = edit(ConfigFormat::Json5, text, key(&["name"]), s("d'b"));
    assert!(out.contains("name: 'd\\'b',"));
    let out = edit(
        ConfigFormat::Json5,
        text,
        key(&["inf"]),
        ConfigValue::Float(f64::NEG_INFINITY),
    );
    assert!(out.contains("inf: -Infinity,"));
}

#[test]
fn json_raiz_lista_objetos_em_lista_e_nulo() {
    let text = "[{\"a\": null}, 1, [2, 3]]";
    let document = ConfigDocument::parse(ConfigFormat::Json, text.as_bytes()).unwrap();
    let root = &document.tree().entries[0];
    assert_eq!(
        (root.kind, root.path.is_empty()),
        (EntryKind::Section, true)
    );
    let a = KeyPath::root().child_index(0).child_key("a");
    assert_eq!(document.get(&a).unwrap().value, Some(ConfigValue::Null));
    let out = edit(ConfigFormat::Json, text, a, ConfigValue::Bool(false));
    assert_eq!(out, "[{\"a\": false}, 1, [2, 3]]");
    let inner = KeyPath::root().child_index(2);
    let list = ConfigValue::List(vec![s("x"), ConfigValue::Float(0.5)]);
    assert_eq!(
        edit(ConfigFormat::Json, text, inner, list),
        "[{\"a\": null}, 1, [\"x\", 0.5]]"
    );
    let document = ConfigDocument::parse(ConfigFormat::Json, b"{\"k\": 1}").unwrap();
    let nan = document.apply(&[ConfigEdit::new(key(&["k"]), ConfigValue::Float(f64::NAN))]);
    assert!(matches!(nan, Err(ConfigError::InvalidValue { .. })));
    assert!(
        ConfigDocument::parse(ConfigFormat::Json, b"")
            .unwrap()
            .tree()
            .entries
            .is_empty()
    );
}

#[test]
fn properties_continuacao_separador_e_unicode() {
    let text = "a = um \\\n    dois\nb\nc=\\u00e9\n";
    let out = edit(ConfigFormat::Properties, text, key(&["a"]), s("três"));
    assert_eq!(out, "a = tr\\u00EAs\nb\nc=\\u00e9\n");
    let out = edit(ConfigFormat::Properties, text, key(&["b"]), s(" x"));
    assert_eq!(out, "a = um \\\n    dois\nb=\\ x\nc=\\u00e9\n");
    // Arquivo com UTF-8 cru: o valor novo também vai cru.
    let raw = "nome=José\n";
    assert_eq!(
        edit(ConfigFormat::Properties, raw, key(&["nome"]), s("João")),
        "nome=João\n"
    );
}

#[test]
fn cfg_listas_categorias_entre_aspas_e_tipos() {
    let text = "\"minha cat\" {\r\n    # Itens [range: 0 ~ 5, default: 1]\r\n    S:itens <\r\n        a\r\n        b\r\n     >\r\n    S:vazia <\r\n     >\r\n    D:v=1.0\r\n}\r\n";
    let document = ConfigDocument::parse(ConfigFormat::LegacyCfg, text.as_bytes()).unwrap();
    let items = document.get(&key(&["minha cat", "itens"])).unwrap();
    assert_eq!(items.cfg_type, Some(CfgType::String));
    assert_eq!(items.range.as_ref().and_then(|r| r.max), Some(5.0));
    assert_eq!(items.line, 3);
    let three = ConfigValue::List(vec![s("x"), s("y"), s("z")]);
    let out = edit(
        ConfigFormat::LegacyCfg,
        text,
        key(&["minha cat", "itens"]),
        three,
    );
    assert!(out.contains("S:itens <\r\n        x\r\n        y\r\n        z\r\n     >\r\n"));
    let one = ConfigValue::List(vec![s("novo")]);
    let out = edit(
        ConfigFormat::LegacyCfg,
        text,
        key(&["minha cat", "vazia"]),
        one,
    );
    assert!(out.contains("S:vazia <\r\n        novo\r\n     >\r\n"));
    let empty = ConfigValue::List(vec![]);
    let out = edit(
        ConfigFormat::LegacyCfg,
        text,
        key(&["minha cat", "itens"]),
        empty,
    );
    assert!(out.contains("S:itens <\r\n     >\r\n"));
    let out = edit(
        ConfigFormat::LegacyCfg,
        text,
        key(&["minha cat", "v"]),
        ConfigValue::Integer(3),
    );
    assert!(out.contains("D:v=3.0\r\n"));
}

#[test]
fn options_txt_aspas_listas_e_linhas_desconhecidas() {
    let text = "version:3955\nsoundDevice:\"\"\nresourcePacks:[\"vanilla\"]\nlinha estranha\nlastServer:a:1\n";
    let out = edit(
        ConfigFormat::OptionsTxt,
        text,
        key(&["soundDevice"]),
        s("Fone \"X\""),
    );
    assert!(out.contains("soundDevice:\"Fone \\\"X\\\"\"\n"));
    let packs = ConfigValue::List(vec![s("vanilla"), s("file/a.zip")]);
    let out = edit(
        ConfigFormat::OptionsTxt,
        text,
        key(&["resourcePacks"]),
        packs,
    );
    assert!(out.contains("resourcePacks:[\"vanilla\",\"file/a.zip\"]\n"));
    assert!(out.contains("linha estranha\n"));
    let out = edit(
        ConfigFormat::OptionsTxt,
        text,
        key(&["lastServer"]),
        s("b:2"),
    );
    assert!(out.ends_with("lastServer:b:2\n"));
}

#[test]
fn limites_de_tamanho_e_codificacao() {
    let big = vec![b' '; MAX_STRUCTURED_BYTES + 1];
    for format in ConfigFormat::ALL {
        assert!(matches!(
            ConfigDocument::parse(format, &big),
            Err(ConfigError::TooLarge { .. })
        ));
        assert!(matches!(
            ConfigDocument::parse(format, b"a=\xe9\n"),
            Err(ConfigError::NotUtf8 { offset: 2 })
        ));
    }
}

#[test]
fn comparacao_entre_formatacoes_diferentes_do_mesmo_conteudo() {
    let a = b"{\"a\": 1, \"b\": [1, 2], \"c\": {\"d\": \"x\"}}";
    let b = b"{\n  // reordenado\n  \"c\": { \"d\": \"x\" },\n  \"b\": [1,2,],\n  \"a\": 1,\n}";
    let diff = compare(ConfigFormat::Json, a, b).unwrap();
    assert!(diff.is_same_values() && diff.text_changed);
    let c = b"{\"a\": 1, \"b\": [2, 1], \"c\": {\"d\": \"x\"}}";
    assert_eq!(compare(ConfigFormat::Json, a, c).unwrap().changes.len(), 1);
}
