//! Formulário com o corpus de configs reais e jars de teste (C-04; SPEC T12, CA-T12-02 e
//! CA-T12-08; ADR-0054).

// Falhar com pânico é o jeito de reprovar o teste.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::io::Write as _;

use common::{corpus_root, folder, samples};
use warden_configs::ConfigFormat;
use warden_configs::meta::{
    ConfigField, ConfigForm, FieldControl, FieldDefault, FormAvailability, FormEdit, FormItem,
    FormValue, apply_form_edits, build_form,
};
use warden_jarmeta::Limits;
use warden_jarmeta::lang::{ConfigLang, LangLocale, read_config_lang_bytes};

fn corpus_text(format: ConfigFormat, name: &str) -> String {
    let path = corpus_root().join(folder(format)).join(name);
    String::from_utf8(std::fs::read(&path).unwrap()).unwrap()
}

fn field<'f>(form: &'f ConfigForm, key: &str) -> &'f ConfigField {
    form.items
        .iter()
        .find_map(|item| match item {
            FormItem::Field(field) if field.key == key => Some(field.as_ref()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{key} não está no formulário"))
}

/// Linhas diferentes entre dois textos com o mesmo número de linhas.
fn changed_lines(before: &str, after: &str) -> Vec<usize> {
    let old: Vec<&str> = before.split('\n').collect();
    let new: Vec<&str> = after.split('\n').collect();
    assert_eq!(old.len(), new.len(), "o número de linhas mudou");
    old.iter()
        .zip(&new)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(index, _)| index + 1)
        .collect()
}

/// Um valor diferente do atual para o controle do campo.
fn other_value(field: &ConfigField) -> Option<FormValue> {
    Some(match (&field.value, &field.control) {
        (_, FieldControl::Options { values }) => {
            let current = match &field.value {
                FormValue::String(text) => text.clone(),
                _ => return None,
            };
            FormValue::String(values.iter().find(|value| **value != current)?.clone())
        }
        (FormValue::Bool(flag), _) => FormValue::Bool(!flag),
        (FormValue::Integer(text), _) => {
            let number: i64 = text.parse().ok()?;
            FormValue::Integer(number.checked_add(1).unwrap_or(number - 1).to_string())
        }
        (FormValue::Float(text), _) => {
            let number: f64 = text.parse().ok()?;
            FormValue::Float(format!(
                "{:?}",
                if number == 0.0 { 1.0 } else { number / 2.0 }
            ))
        }
        (FormValue::String(text), FieldControl::Toggle) => {
            FormValue::String(if text == "true" { "false" } else { "true" }.into())
        }
        (FormValue::String(text), FieldControl::Number { .. }) => {
            FormValue::String(if text == "1" { "2" } else { "1" }.into())
        }
        (FormValue::String(text), _) => FormValue::String(format!("{text}w")),
        (FormValue::Datetime(_) | FormValue::List(_), _) => return None,
    })
}

#[test]
fn ca_t12_02_mudar_um_valor_num_toml_do_forge_altera_exatamente_uma_linha() {
    for name in ["02-forge-client.toml", "04-YungsBridges-forge-1_18.toml"] {
        let text = corpus_text(ConfigFormat::Toml, name);
        let form = build_form(&format!("config/{name}"), text.as_bytes(), None);
        assert_eq!(form.availability, FormAvailability::Ready, "{name}");
        let mut edited = 0;
        for item in &form.items {
            let FormItem::Field(field) = item else {
                continue;
            };
            let Some(value) = other_value(field) else {
                continue;
            };
            let out = apply_form_edits(
                &format!("config/{name}"),
                &text,
                &[FormEdit {
                    path: field.path.clone(),
                    value,
                }],
            )
            .unwrap_or_else(|error| panic!("{name} {}: {error}", field.key));
            assert_eq!(
                changed_lines(&text, &out),
                [field.line as usize],
                "{name} {}",
                field.key
            );
            edited += 1;
        }
        assert!(edited >= 4, "{name}: só {edited} campos editados");
    }
}

#[test]
fn todo_o_corpus_abre_no_formulario_ou_diz_por_que_nao() {
    let mut ready = 0;
    let mut total = 0;
    for format in ConfigFormat::ALL {
        for sample in samples(format) {
            total += 1;
            let path = format!("config/{}", sample.name);
            let path = if format == ConfigFormat::OptionsTxt {
                "options.txt".to_owned()
            } else {
                path
            };
            let form = build_form(&path, &sample.bytes, None);
            let FormAvailability::Ready = form.availability else {
                assert!(form.items.is_empty(), "{}", sample.name);
                continue;
            };
            ready += 1;
            let text = String::from_utf8(sample.bytes.clone()).unwrap();
            // Cada campo, editado sozinho, devolve o arquivo idêntico fora do valor.
            for item in form.items.iter().take(80) {
                let FormItem::Field(field) = item else {
                    continue;
                };
                let same = apply_form_edits(
                    &path,
                    &text,
                    &[FormEdit {
                        path: field.path.clone(),
                        value: field.value.clone(),
                    }],
                )
                .unwrap_or_else(|error| panic!("{} {}: {error}", sample.name, field.key));
                assert_eq!(same, text, "{} {}", sample.name, field.key);
                let Some(value) = other_value(field) else {
                    continue;
                };
                if matches!(field.control, FieldControl::List { .. }) {
                    continue;
                }
                let Ok(out) = apply_form_edits(
                    &path,
                    &text,
                    &[FormEdit {
                        path: field.path.clone(),
                        value,
                    }],
                ) else {
                    continue;
                };
                assert_eq!(
                    changed_lines(&text, &out),
                    [field.line as usize],
                    "{} {}",
                    sample.name,
                    field.key
                );
            }
        }
    }
    // A maioria dos arquivos reais tem formulário; os outros dizem o motivo.
    assert!(
        ready * 10 >= total * 8,
        "só {ready} de {total} com formulário"
    );
}

/// Jar de teste: só os arquivos de idioma, com as chaves que o `AppleSkin` (Fabric, `AutoConfig`)
/// usa no `en_us.json` e uma tradução para o português.
fn appleskin_jar() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(&mut out);
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file("fabric.mod.json", options).unwrap();
    writer
        .write_all(br#"{"schemaVersion": 1, "id": "appleskin", "version": "2.4.0"}"#)
        .unwrap();
    writer
        .start_file("assets/appleskin/lang/en_us.json", options)
        .unwrap();
    writer
        .write_all(
            br#"{
  "text.autoconfig.appleskin.title": "AppleSkin",
  "text.autoconfig.appleskin.option.showFoodValuesInTooltip": "Show Food Values in Tooltip",
  "text.autoconfig.appleskin.option.showFoodValuesInTooltip.@Tooltip": "Hold SHIFT to see",
  "text.autoconfig.appleskin.option.maxHudOverlayFlashAlpha": "Max HUD Flash Alpha"
}"#,
        )
        .unwrap();
    writer
        .start_file("assets/appleskin/lang/pt_br.json", options)
        .unwrap();
    writer
        .write_all(
            "{\"text.autoconfig.appleskin.option.showFoodValuesInTooltip\": \"§aMostrar valores da comida na dica\"}"
                .as_bytes(),
        )
        .unwrap();
    writer.finish().unwrap();
    out.into_inner()
}

#[test]
fn ca_t12_08_controle_faixa_padrao_e_rotulo_do_jar() {
    // TOML do NeoForge 1.21.1 com `# Default:`, `# Range:` e `#Allowed Values`.
    let neoforge = corpus_text(ConfigFormat::Toml, "27-supplementaries-client.toml");
    let form = build_form(
        "config/supplementaries-client.toml",
        neoforge.as_bytes(),
        None,
    );
    assert_eq!(form.availability, FormAvailability::Ready);
    let increment = field(&form, "blocks.flag.intensity_increment");
    assert_eq!(
        increment.control,
        FieldControl::Number {
            integer: false,
            min: Some(0.0),
            max: Some(10.0)
        }
    );
    assert_eq!(
        increment.default,
        Some(FieldDefault {
            value: "0.3".into(),
            inferred: false
        })
    );
    assert_eq!(
        increment.range.as_ref().map(|r| r.text.as_str()),
        Some("0.0 ~ 10.0")
    );
    let fanciness = field(&form, "blocks.flag.fanciness");
    assert_eq!(
        fanciness.control,
        FieldControl::Options {
            values: vec!["FAST".into(), "FANCY".into(), "FABULOUS".into()]
        }
    );
    assert_eq!(fanciness.default, None);

    // `.cfg` do Forge antigo com `[range: …, default: …]`.
    let cfg = corpus_text(ConfigFormat::LegacyCfg, "16-AntiqueAtlasOverlay.cfg");
    let form = build_form("config/AntiqueAtlasOverlay.cfg", cfg.as_bytes(), None);
    assert_eq!(form.availability, FormAvailability::Ready);
    let border = form
        .items
        .iter()
        .find_map(|item| match item {
            FormItem::Field(field) if field.key.ends_with(".BORDER_X") => Some(field.as_ref()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        border.control,
        FieldControl::Number {
            integer: false,
            min: Some(0.0),
            max: Some(0.5)
        }
    );
    assert_eq!(
        border.default.as_ref().map(|d| d.value.as_str()),
        Some("0.0")
    );

    // JSON do AutoConfig (Jankson, `.json5`) com a chave de tradução no jar.
    let lang: ConfigLang = read_config_lang_bytes(&appleskin_jar(), &Limits::default()).unwrap();
    let json = corpus_text(ConfigFormat::Json5, "01-appleskin.json5");
    let form = build_form("config/appleskin.json5", json.as_bytes(), Some(&lang));
    let tooltip = field(&form, "showFoodValuesInTooltip");
    assert_eq!(tooltip.control, FieldControl::Toggle);
    let label = tooltip.label.as_ref().unwrap();
    assert_eq!(label.text, "Mostrar valores da comida na dica");
    assert_eq!(label.locale, LangLocale::PtBr);
    assert_eq!(
        label.translation_key,
        "text.autoconfig.appleskin.option.showFoodValuesInTooltip"
    );
    assert_eq!(label.tooltip.as_deref(), Some("Hold SHIFT to see"));
    let alpha = field(&form, "maxHudOverlayFlashAlpha");
    assert_eq!(
        alpha.label.as_ref().map(|l| l.locale),
        Some(LangLocale::EnUs)
    );
    assert!(matches!(
        alpha.control,
        FieldControl::Number { integer: false, .. }
    ));
    // Sem rótulo no jar, o campo fica só com a chave real.
    assert_eq!(field(&form, "showFoodDebugInfo").label, None);
}
