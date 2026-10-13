//! Formulário com o corpus de configs reais e jars de teste (C-04; SPEC T12, CA-T12-02 e
//! CA-T12-08; ADR-0054).

// Falhar com pânico é o jeito de reprovar o teste.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::io::Write as _;

use common::{corpus_root, folder, samples};
use warden_configs::meta::{
    ConfigField, ConfigForm, FieldControl, FieldDefault, FieldKey, FormAvailability, FormEdit,
    FormItem, FormValue, ListItemKind, apply_form_edits, build_form,
};
use warden_configs::{ConfigDocument, ConfigFormat, KeyPath, PathSegment};
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

/// Item novo de uma lista, diferente de `current`, no tipo dos itens.
fn other_item(item: ListItemKind, current: Option<&str>) -> String {
    let candidates: &[&str] = match item {
        ListItemKind::Text => &["warden:item", "warden:outro"],
        ListItemKind::Integer => &["7", "8"],
        ListItemKind::Float => &["2.5", "3.5"],
        ListItemKind::Bool => &["true", "false"],
    };
    candidates
        .iter()
        .find(|candidate| Some(**candidate) != current)
        .map(|candidate| (*candidate).to_owned())
        .unwrap_or_default()
}

/// As edições de teste de um campo: um valor diferente para os simples; para as listas, trocar
/// o primeiro e o último item, acrescentar um item no fim e tirar o último.
fn test_edits(field: &ConfigField) -> Vec<(&'static str, FormValue)> {
    let (FormValue::List(items), FieldControl::List { item }) = (&field.value, &field.control)
    else {
        return other_value(field)
            .map(|value| vec![("valor", value)])
            .unwrap_or_default();
    };
    let mut edits = Vec::new();
    let mut appended = items.clone();
    appended.push(other_item(*item, None));
    edits.push(("acrescentar", FormValue::List(appended)));
    if let Some(first) = items.first() {
        let mut changed = items.clone();
        changed[0] = other_item(*item, Some(first));
        edits.push(("trocar o primeiro", FormValue::List(changed)));
        let mut removed = items.clone();
        removed.pop();
        edits.push(("tirar o último", FormValue::List(removed)));
    }
    if items.len() > 1
        && let Some(last) = items.last()
    {
        let mut changed = items.clone();
        let end = changed.len() - 1;
        changed[end] = other_item(*item, Some(last));
        edits.push(("trocar o último", FormValue::List(changed)));
    }
    edits
}

/// Linhas do arquivo original que mudaram (de 1 em diante), comparando o começo e o fim
/// iguais; vazio quando só se acrescentaram linhas.
fn changed_region(before: &str, after: &str) -> std::ops::RangeInclusive<usize> {
    let old: Vec<&str> = before.split('\n').collect();
    let new: Vec<&str> = after.split('\n').collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let room = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(room)
        .take_while(|(a, b)| a == b)
        .count();
    prefix + 1..=old.len() - suffix
}

fn key_path(keys: &[FieldKey]) -> KeyPath {
    KeyPath(
        keys.iter()
            .map(|key| match key {
                FieldKey::Name(name) => PathSegment::Key(name.clone()),
                FieldKey::Index(index) => PathSegment::Index(*index as usize),
            })
            .collect(),
    )
}

/// Os dois textos diferem numa linha só, e só nos espaços do começo e na vírgula do fim.
fn only_layout_differs(before: &str, after: &str) -> bool {
    let region = changed_region(before, after);
    let old: Vec<&str> = before.split('\n').collect();
    let new: Vec<&str> = after.split('\n').collect();
    let bare = |line: &str| line.trim().trim_end_matches(',').to_owned();
    old.len() == new.len()
        && region.start() == region.end()
        && bare(old[*region.start() - 1]) == bare(new[*region.start() - 1])
}

/// Edita o campo e volta ao valor original. `Ok(true)` quando a volta só difere na linha
/// recriada de um item tirado (veja abaixo); `Err` com o problema.
fn edit_and_restore(
    path: &str,
    text: &str,
    field: &ConfigField,
    lines: &std::ops::RangeInclusive<usize>,
    what: &str,
    value: FormValue,
) -> Result<bool, String> {
    let edit = |text: &str, value: FormValue| {
        apply_form_edits(
            path,
            text,
            &[FormEdit {
                path: field.path.clone(),
                value,
            }],
        )
    };
    let out = edit(text, value).map_err(|error| format!("a edição falhou: {error}"))?;
    let region = changed_region(text, &out);
    if !region.is_empty() && (region.start() < lines.start() || region.end() > lines.end()) {
        return Err(format!("mudou as linhas {region:?}, fora de {lines:?}"));
    }
    let back =
        edit(&out, field.value.clone()).map_err(|error| format!("a volta falhou: {error}"))?;
    if back == text {
        return Ok(false);
    }
    // Tirar o último item e pô-lo de volta recria a linha dele no estilo das vizinhas: se só
    // ele tinha outra indentação ou uma vírgula final, isso se perde junto com a linha. Única
    // diferença aceita: essa linha.
    if what == "tirar o último" && only_layout_differs(text, &back) {
        return Ok(true);
    }
    let region = changed_region(text, &back);
    let original = text.split('\n').skip(region.start() - 1);
    let restored = back.split('\n').skip(region.start() - 1);
    Err(format!(
        "não voltou idêntico; linha {}: {:?} virou {:?}",
        region.start(),
        original.take(3).collect::<Vec<_>>(),
        restored.take(3).collect::<Vec<_>>(),
    ))
}

/// Ida e volta no corpus real: cada campo do formulário de cada arquivo, editado e depois
/// devolvido ao valor original, deixa o arquivo **byte a byte igual**; e a edição muda só as
/// linhas daquele campo (a linha da chave até o fim do valor). Vale para listas de uma linha ou
/// de várias, com comentários dentro, e para números como `0.20` (CA-T12-02).
fn round_trip(format: ConfigFormat) {
    let mut files = 0;
    let mut edits = 0;
    let mut tolerated = 0;
    let mut broken_files = std::collections::BTreeSet::new();
    let mut problems = Vec::new();
    for sample in samples(format) {
        let path = if format == ConfigFormat::OptionsTxt {
            "options.txt".to_owned()
        } else {
            format!("config/{}", sample.name)
        };
        let form = build_form(&path, &sample.bytes, None);
        if form.availability != FormAvailability::Ready {
            continue;
        }
        files += 1;
        let text = String::from_utf8(sample.bytes.clone()).unwrap();
        let document = ConfigDocument::parse(format, &sample.bytes).unwrap();
        // Arquivo enorme (1.477 chaves iguais no `allowed_sounds.properties`): uma amostra
        // espalhada de até 300 campos, para o teste caber no tempo da CI.
        let fields: Vec<&ConfigField> = form
            .items
            .iter()
            .filter_map(|item| match item {
                FormItem::Field(field) => Some(field.as_ref()),
                _ => None,
            })
            .collect();
        let step = fields.len().div_ceil(300).max(1);
        for field in fields.into_iter().step_by(step) {
            let span = document
                .get(&key_path(&field.path))
                .unwrap()
                .value_span
                .unwrap();
            let last_byte = span.end.max(span.start + 1) - 1;
            let last_line = text[..last_byte.min(text.len())].matches('\n').count() + 1;
            let lines = field.line as usize..=last_line.max(field.line as usize);
            for (what, value) in test_edits(field) {
                edits += 1;
                match edit_and_restore(&path, &text, field, &lines, what, value) {
                    Ok(false) => {}
                    Ok(true) => tolerated += 1,
                    Err(detail) => {
                        broken_files.insert(sample.name.clone());
                        problems.push(format!("{} {} ({what}): {detail}", sample.name, field.key));
                    }
                }
            }
        }
    }
    assert!(files >= 5, "só {files} arquivos com formulário");
    assert!(
        tolerated <= 1,
        "{tolerated} itens voltaram com outra indentação ou vírgula"
    );
    assert!(
        broken_files.is_empty(),
        "{format:?}: {} de {files} arquivos não voltaram idênticos ({} de {edits} edições):\n{}",
        broken_files.len(),
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn ida_e_volta_no_corpus_toml() {
    round_trip(ConfigFormat::Toml);
}

#[test]
fn ida_e_volta_no_corpus_json() {
    round_trip(ConfigFormat::Json);
}

#[test]
fn ida_e_volta_no_corpus_json5() {
    round_trip(ConfigFormat::Json5);
}

#[test]
fn ida_e_volta_no_corpus_properties() {
    round_trip(ConfigFormat::Properties);
}

#[test]
fn ida_e_volta_no_corpus_cfg() {
    round_trip(ConfigFormat::LegacyCfg);
}

#[test]
fn ida_e_volta_no_corpus_options_txt() {
    round_trip(ConfigFormat::OptionsTxt);
}
