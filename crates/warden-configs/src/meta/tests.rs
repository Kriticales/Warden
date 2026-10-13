//! Testes do formulário: camadas, trechos em texto, queda para texto e edição.

use super::*;

fn form(path: &str, text: &str) -> ConfigForm {
    build_form(path, text.as_bytes(), None)
}

fn fields(form: &ConfigForm) -> Vec<&ConfigField> {
    form.items
        .iter()
        .filter_map(|item| match item {
            FormItem::Field(field) => Some(field.as_ref()),
            _ => None,
        })
        .collect()
}

fn field<'f>(form: &'f ConfigForm, key: &str) -> &'f ConfigField {
    fields(form)
        .into_iter()
        .find(|field| field.key == key)
        .unwrap_or_else(|| panic!("{key} não está no formulário"))
}

fn keys(parts: &[&str]) -> Vec<FieldKey> {
    parts
        .iter()
        .map(|part| FieldKey::Name((*part).to_owned()))
        .collect()
}

#[test]
fn controles_pelo_valor_e_pelos_metadados_do_toml() {
    let text = "[geral]\n\t#Liga a geração.\n\tenabled = true\n\t# Velocidade. Default: 16\n\t# Range: 1 ~ 64\n\tspeed = 16\n\t#Fator.\n\tfactor = 0.5\n\t#Allowed Values: A, B\n\tmode = \"A\"\n\tnames = [\"x\", \"y\"]\n\tcolor = \"#ff00aa\"\n\twhen = 1979-05-27\n";
    let form = form("config/teste-common.toml", text);
    assert_eq!(form.availability, FormAvailability::Ready);
    assert!(matches!(&form.items[0], FormItem::Section(section) if section.key == "geral"));

    let enabled = field(&form, "geral.enabled");
    assert_eq!(enabled.control, FieldControl::Toggle);
    assert_eq!(enabled.value, FormValue::Bool(true));
    assert_eq!(enabled.description.as_deref(), Some("Liga a geração."));
    assert_eq!(enabled.line, 3);

    let speed = field(&form, "geral.speed");
    assert_eq!(
        speed.control,
        FieldControl::Number {
            integer: true,
            min: Some(1.0),
            max: Some(64.0)
        }
    );
    assert_eq!(speed.confidence, Confidence::High);
    assert_eq!(
        speed.description.as_deref(),
        Some("Velocidade. Default: 16")
    );
    assert_eq!(
        speed.range.as_ref().map(|range| range.inferred),
        Some(false)
    );

    let factor = field(&form, "geral.factor");
    assert_eq!(factor.value, FormValue::Float("0.5".into()));
    assert_eq!(factor.confidence, Confidence::Medium);

    let mode = field(&form, "geral.mode");
    assert_eq!(
        mode.control,
        FieldControl::Options {
            values: vec!["A".into(), "B".into()]
        }
    );
    assert_eq!(
        field(&form, "geral.names").control,
        FieldControl::List {
            item: ListItemKind::Text
        }
    );
    let color = field(&form, "geral.color");
    assert_eq!(color.control, FieldControl::Color);
    assert_eq!(color.confidence, Confidence::Low);
    assert_eq!(field(&form, "geral.when").control, FieldControl::Text);
}

#[test]
fn padrao_explicito_e_deduzido() {
    let text = "# Velocidade\n# Default: 2.0\n# Range: 0.0 ~ 100.0\na = 3.0\n#Alcance. Defaults to 20.\nb = 20\n#Sem padrão\nc = 1\n";
    let form = form("config/x-common.toml", text);
    assert_eq!(
        field(&form, "a").default,
        Some(FieldDefault {
            value: "2.0".into(),
            inferred: false
        })
    );
    assert_eq!(field(&form, "a").description.as_deref(), Some("Velocidade"));
    assert_eq!(
        field(&form, "b").default,
        Some(FieldDefault {
            value: "20".into(),
            inferred: true
        })
    );
    assert_eq!(field(&form, "c").default, None);
}

#[test]
fn faixa_deduzida_tem_confianca_baixa() {
    let form = form(
        "config/x.json5",
        "{\n  // Must be between 1 and 8.\n  \"threads\": 4\n}\n",
    );
    let threads = field(&form, "threads");
    assert_eq!(
        threads.range,
        Some(FieldRange {
            min: Some(1.0),
            max: Some(8.0),
            text: "between 1 and 8".into(),
            inferred: true
        })
    );
    assert_eq!(threads.confidence, Confidence::Low);
}

#[test]
fn cfg_antigo_com_prefixos_faixa_e_valores() {
    let text = "general {\n    # Liga [default: true]\n    B:on=true\n\n    # Vento\n    #  [range: 0.0 ~ 0.5, default: 0.25]\n    D:wind=0.25\n\n    # Modo\n    # Valid values:\n    # A\n    # B\n    S:mode=A\n\n    # Limite\n    # Min: 0\n    # Max: 10\n    I:limit=3\n\n    S:list <\n        a\n        b\n     >\n}\n";
    let form = form("config/teste.cfg", text);
    assert_eq!(form.availability, FormAvailability::Ready);
    let on = field(&form, "general.on");
    assert_eq!(on.control, FieldControl::Toggle);
    assert_eq!(on.default.as_ref().map(|d| d.value.as_str()), Some("true"));
    assert_eq!(on.description.as_deref(), Some("Liga"));
    let wind = field(&form, "general.wind");
    assert_eq!(
        wind.control,
        FieldControl::Number {
            integer: false,
            min: Some(0.0),
            max: Some(0.5)
        }
    );
    assert_eq!(
        wind.default.as_ref().map(|d| d.value.as_str()),
        Some("0.25")
    );
    assert_eq!(
        field(&form, "general.mode").control,
        FieldControl::Options {
            values: vec!["A".into(), "B".into()]
        }
    );
    let limit = field(&form, "general.limit");
    assert_eq!(
        limit.control,
        FieldControl::Number {
            integer: true,
            min: Some(0.0),
            max: Some(10.0)
        }
    );
    assert_eq!(
        limit.range.as_ref().map(|r| r.text.as_str()),
        Some("0.0 ~ 10.0")
    );
    assert_eq!(
        field(&form, "general.list").value,
        FormValue::List(vec!["a".into(), "b".into()])
    );
}

#[test]
fn formatos_sem_tipo_deduzem_pelo_texto() {
    let form = form(
        "options.txt",
        "fov:0.5\nrenderDistance:12\nlang:pt_br\nautoJump:false\nrenderDistance:10\n",
    );
    assert!(matches!(
        field(&form, "fov").control,
        FieldControl::Number { integer: false, .. }
    ));
    assert_eq!(field(&form, "autoJump").control, FieldControl::Toggle);
    assert_eq!(field(&form, "lang").control, FieldControl::Text);
    // A chave repetida aparece uma vez, com o valor que o jogo lê (o último).
    let render: Vec<_> = fields(&form)
        .into_iter()
        .filter(|field| field.key == "renderDistance")
        .collect();
    assert_eq!(render.len(), 1);
    assert_eq!(render[0].value, FormValue::String("10".into()));
    assert_eq!(render[0].line, 5);
}

#[test]
fn trechos_que_so_se_editam_como_texto() {
    let text = "{\n  \"lista\": [{\"a\": 1}, {\"a\": 2}],\n  \"mista\": [1, \"x\"],\n  \"funda\": [[1]],\n  \"nada\": null,\n  \"ok\": 1\n}\n";
    let json = form("config/x.json", text);
    let snippets: Vec<(&str, SnippetReason)> = json
        .items
        .iter()
        .filter_map(|item| match item {
            FormItem::Snippet(snippet) => Some((snippet.key.as_str(), snippet.reason)),
            _ => None,
        })
        .collect();
    assert_eq!(
        snippets,
        [
            ("lista", SnippetReason::ListOfObjects),
            ("mista", SnippetReason::ComplexList),
            ("funda", SnippetReason::ComplexList),
            ("nada", SnippetReason::NullValue),
        ]
    );
    assert_eq!(fields(&json).len(), 1);
    assert!(
        !json
            .items
            .iter()
            .any(|item| matches!(item, FormItem::Section(_)))
    );

    let mut deep = String::new();
    for level in 0..=MAX_FORM_DEPTH {
        let names: Vec<String> = (0..=level).map(|n| format!("s{n}")).collect();
        deep.push('[');
        deep.push_str(&names.join("."));
        deep.push_str("]\n");
    }
    deep.push_str("v = 1\n");
    let deep_form = form("config/fundo.toml", &deep);
    assert!(deep_form.items.iter().any(|item| matches!(
        item,
        FormItem::Snippet(FormSnippet {
            reason: SnippetReason::TooDeep,
            ..
        })
    )));
}

#[test]
fn queda_para_texto_com_o_motivo() {
    let reason = |path: &str, text: &[u8]| match build_form(path, text, None).availability {
        FormAvailability::TextOnly { reason } => reason,
        FormAvailability::Ready => panic!("{path} abriu no formulário"),
    };
    assert_eq!(
        reason("config/a.yml", b"a: 1"),
        TextOnlyReason::UnsupportedFormat
    );
    assert_eq!(
        reason("config/a.cfg", b"[s]\nk = v\n"),
        TextOnlyReason::NotForgeCfg
    );
    assert_eq!(
        reason("config/a.toml", b"a = \xff"),
        TextOnlyReason::NotUtf8
    );
    assert_eq!(
        reason(
            "config/a.toml",
            &vec![b' '; crate::MAX_STRUCTURED_BYTES + 1]
        ),
        TextOnlyReason::TooLarge
    );
    assert_eq!(
        reason("config/a.toml", b"a = 1\nb = \n"),
        TextOnlyReason::ParseFailed { line: 2, column: 5 }
    );
    assert_eq!(
        reason("config/a.toml", b"# so comentario\n"),
        TextOnlyReason::NoKeys
    );
    assert_eq!(
        reason_of(&ConfigError::DuplicateEdit {
            path: KeyPath::root()
        }),
        TextOnlyReason::UnsupportedFormat
    );
}

#[test]
fn edicao_muda_so_o_valor() {
    let text = "\t#Range: 1 ~ 64\r\n\tmax = 16 # fim\r\nnome = 'x'\r\n";
    let out = apply_form_edits(
        "config/a-common.toml",
        text,
        &[FormEdit {
            path: keys(&["max"]),
            value: FormValue::Integer("32".into()),
        }],
    )
    .unwrap();
    assert_eq!(out, text.replace("16", "32"));
    let same = apply_form_edits("config/a-common.toml", text, &[]).unwrap();
    assert_eq!(same, text);
}

#[test]
fn edicao_converte_para_o_tipo_da_chave() {
    let properties = apply_form_edits(
        "x.properties",
        "a=1\nb=true\n",
        &[
            FormEdit {
                path: keys(&["a"]),
                value: FormValue::Integer("2".into()),
            },
            FormEdit {
                path: keys(&["b"]),
                value: FormValue::Bool(false),
            },
        ],
    )
    .unwrap();
    assert_eq!(properties, "a=2\nb=false\n");
    let toml = apply_form_edits(
        "x.toml",
        "l = [1, 2]\nf = 1.0\nd = 1979-05-27\nb = true\n",
        &[
            FormEdit {
                path: keys(&["l"]),
                value: FormValue::List(vec!["3".into()]),
            },
            FormEdit {
                path: keys(&["f"]),
                value: FormValue::Float("2.5".into()),
            },
            FormEdit {
                path: keys(&["d"]),
                value: FormValue::Datetime("1980-01-01".into()),
            },
            FormEdit {
                path: keys(&["b"]),
                value: FormValue::String("false".into()),
            },
        ],
    )
    .unwrap();
    assert_eq!(toml, "l = [3]\nf = 2.5\nd = 1980-01-01\nb = false\n");
}

#[test]
fn edicao_recusa_valores_que_nao_servem() {
    let text = "i = 1\nf = 1.0\nb = true\nl = [1]\ns = \"x\"\n[t]\nk = 1\n";
    let edit = |key: &str, value: FormValue| {
        apply_form_edits(
            "x.toml",
            text,
            &[FormEdit {
                path: keys(&key.split('.').collect::<Vec<_>>()),
                value,
            }],
        )
    };
    assert!(matches!(
        edit("i", FormValue::Integer("1.5".into())),
        Err(ConfigError::InvalidValue { .. })
    ));
    assert!(matches!(
        edit("f", FormValue::Float("NaN".into())),
        Err(ConfigError::InvalidValue { .. })
    ));
    assert!(matches!(
        edit("b", FormValue::String("talvez".into())),
        Err(ConfigError::InvalidValue { .. })
    ));
    assert!(matches!(
        edit("l", FormValue::String("1".into())),
        Err(ConfigError::InvalidValue { .. })
    ));
    assert!(matches!(
        edit("l", FormValue::List(vec!["x".into()])),
        Err(ConfigError::InvalidValue { .. })
    ));
    assert!(matches!(
        edit("s", FormValue::List(vec![])),
        Err(ConfigError::InvalidValue { .. })
    ));
    assert!(matches!(
        edit("t", FormValue::Integer("1".into())),
        Err(ConfigError::NotEditable { .. })
    ));
    assert!(matches!(
        edit("y", FormValue::Integer("1".into())),
        Err(ConfigError::PathNotFound { .. })
    ));
    assert!(matches!(
        apply_form_edits("x.yml", "a: 1", &[]),
        Err(ConfigError::EditNotConfirmed { .. })
    ));
    assert!(matches!(
        apply_form_edits(
            "x.json",
            "{\"n\": null}",
            &[FormEdit {
                path: keys(&["n"]),
                value: FormValue::String("a".into())
            }]
        ),
        Err(ConfigError::InvalidValue { .. })
    ));
    let mixed = to_config_value(
        &KeyPath::root(),
        &ConfigValue::List(vec![ConfigValue::Integer(1), ConfigValue::Bool(true)]),
        &FormValue::List(vec![]),
    );
    assert!(mixed.is_err());
    assert_eq!(
        item_value(&KeyPath::root(), ListItemKind::Bool, "true").unwrap(),
        ConfigValue::Bool(true)
    );
}

#[test]
fn caminhos_vao_e_voltam() {
    let path = KeyPath::from_keys(["a"]).child_index(2);
    let back = from_keys(&to_keys(&path));
    assert_eq!(back, path);
    let json = serde_json::to_string(&to_keys(&path)).unwrap();
    assert_eq!(json, "[\"a\",2]");
}
