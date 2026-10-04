//! Testes com o corpus de configs reais (`tests/corpus/`, origem em `FIXTURES.md`).
//!
//! Para cada arquivo de cada formato: lê sem erro; sem edição, os bytes voltam idênticos
//! (CA-T12-01); todo valor pode ser reescrito sem mudar o que o jogo lê; e cada valor, editado
//! sozinho, muda só o trecho dele (uma linha quando o valor ocupa uma linha) e só aquela chave na
//! comparação semântica.

mod common;

use common::{different_value, differing_lines, samples};
use warden_configs::{
    ConfigDocument, ConfigEdit, ConfigFormat, EntryKind, SemanticChange, compare,
};

const MINIMUM_PER_FORMAT: usize = 15;
const MAX_EDITS_PER_FILE: usize = 60;

#[test]
fn corpus_tem_pelo_menos_15_arquivos_reais_por_formato() {
    for format in ConfigFormat::ALL {
        let count = samples(format).len();
        assert!(count >= MINIMUM_PER_FORMAT, "{format}: só {count} arquivos");
    }
    let fixtures = std::fs::read_to_string(common::corpus_root().join("FIXTURES.md")).unwrap();
    for format in ConfigFormat::ALL {
        for sample in samples(format) {
            assert!(
                fixtures.contains(&format!("`{}`", sample.name)),
                "{} sem origem anotada no FIXTURES.md",
                sample.name
            );
        }
    }
}

#[test]
fn ca_t12_01_abrir_e_salvar_sem_mudancas_devolve_bytes_identicos() {
    for format in ConfigFormat::ALL {
        for sample in samples(format) {
            let document = ConfigDocument::parse(format, &sample.bytes)
                .unwrap_or_else(|error| panic!("{}: {error}", sample.name));
            assert_eq!(
                document.as_bytes(),
                sample.bytes.as_slice(),
                "{}",
                sample.name
            );
            let saved = document.apply(&[]).unwrap();
            assert_eq!(saved.as_bytes(), sample.bytes.as_slice(), "{}", sample.name);
            // Gravar cada valor com o próprio valor atual também não muda nada.
            let same: Vec<ConfigEdit> = document
                .tree()
                .values()
                .filter_map(|entry| {
                    entry
                        .value
                        .clone()
                        .map(|value| ConfigEdit::new(entry.path.clone(), value))
                })
                .rev()
                .fold(Vec::new(), |mut kept, edit| {
                    if !kept
                        .iter()
                        .any(|other: &ConfigEdit| other.path == edit.path)
                    {
                        kept.push(edit);
                    }
                    kept
                });
            let rewritten = document.apply(&same).unwrap();
            assert_eq!(
                rewritten.as_bytes(),
                sample.bytes.as_slice(),
                "{}",
                sample.name
            );
            let diff = compare(format, &sample.bytes, saved.as_bytes()).unwrap();
            assert!(
                diff.is_same_values() && !diff.text_changed,
                "{}",
                sample.name
            );
        }
    }
}

#[test]
fn todo_valor_do_corpus_pode_ser_reescrito_sem_perda() {
    for format in ConfigFormat::ALL {
        for sample in samples(format) {
            let document = ConfigDocument::parse(format, &sample.bytes).unwrap();
            let problems = document.rewrite_problems();
            assert!(problems.is_empty(), "{}: {problems:?}", sample.name);
        }
    }
}

#[test]
fn cada_valor_editado_sozinho_muda_so_o_proprio_trecho() {
    let mut edits_checked = 0usize;
    for format in ConfigFormat::ALL {
        let mut per_format = 0usize;
        for sample in samples(format) {
            let document = ConfigDocument::parse(format, &sample.bytes).unwrap();
            let entries: Vec<_> = document.tree().values().cloned().collect();
            // Arquivos grandes: amostra espalhada pelo arquivo todo (o teste em debug fica
            // em segundos); os pequenos são testados chave por chave.
            let step = entries.len().div_ceil(MAX_EDITS_PER_FILE).max(1);
            for entry in entries.into_iter().step_by(step) {
                // Chave repetida: só a última vale e só ela é editada.
                if document.get(&entry.path) != Some(&entry) {
                    continue;
                }
                let Some(new_value) = different_value(&entry) else {
                    continue;
                };
                let edit = ConfigEdit::new(entry.path.clone(), new_value);
                let edited = document
                    .apply(std::slice::from_ref(&edit))
                    .unwrap_or_else(|error| panic!("{} {}: {error}", sample.name, entry.path));
                let span = entry.value_span.unwrap();
                let (before, after) = (sample.bytes.as_slice(), edited.as_bytes());
                // Bytes antes e depois do trecho do valor ficam intactos.
                assert_eq!(
                    after.get(..span.start),
                    before.get(..span.start),
                    "{}",
                    sample.name
                );
                let tail = before.len() - span.end;
                assert_eq!(after.get(after.len() - tail..), before.get(span.end..));
                // Valor numa linha só: exatamente uma linha muda, a da chave.
                let old_value = &before[span.start..span.end];
                let new_value = &after[span.start..after.len() - tail];
                if !old_value.contains(&b'\n') && !new_value.contains(&b'\n') {
                    let lines = differing_lines(before, after).unwrap();
                    assert_eq!(lines.len(), 1, "{} {}", sample.name, entry.path);
                }
                per_format += 1;
                // `apply` já relê e confere que só aquela chave mudou; a comparação pública é
                // repetida aqui numa a cada quatro edições (cada uma relê os dois arquivos).
                if !per_format.is_multiple_of(4) {
                    continue;
                }
                // Na comparação semântica, só aquela chave mudou.
                let diff = compare(format, before, after).unwrap();
                assert_eq!(diff.changes.len(), 1, "{} {}", sample.name, entry.path);
                assert!(
                    matches!(&diff.changes[0], SemanticChange::Changed { path, .. } if *path == entry.path)
                );
                // Voltar ao valor antigo devolve o mesmo valor para o jogo.
                let back = edited
                    .apply(&[ConfigEdit::new(
                        entry.path.clone(),
                        entry.value.clone().unwrap(),
                    )])
                    .unwrap();
                assert!(
                    compare(format, before, back.as_bytes())
                        .unwrap()
                        .is_same_values()
                );
            }
        }
        assert!(
            per_format > 50,
            "{format}: só {per_format} edições testadas"
        );
        edits_checked += per_format;
    }
    assert!(edits_checked > 2500, "só {edits_checked} edições testadas");
}

#[test]
fn ca_t12_02_mudar_um_valor_num_toml_do_forge_com_comentarios_altera_uma_linha() {
    let mut checked = 0;
    for sample in samples(ConfigFormat::Toml) {
        let document = ConfigDocument::parse(ConfigFormat::Toml, &sample.bytes).unwrap();
        let ranged = document.tree().values().find(|entry| {
            entry.kind == EntryKind::Value
                && entry.comment.is_some()
                && entry
                    .range
                    .as_ref()
                    .is_some_and(|range| range.min.is_some() && range.max.is_some())
        });
        let Some(entry) = ranged.cloned() else {
            continue;
        };
        let range = entry.range.clone().unwrap();
        let new_value = match entry.value.clone().unwrap() {
            warden_configs::ConfigValue::Integer(current) => {
                #[allow(clippy::cast_possible_truncation)] // faixas do corpus cabem em i64
                let candidate = if current > range.min.unwrap() as i64 {
                    current - 1
                } else {
                    current + 1
                };
                warden_configs::ConfigValue::Integer(candidate)
            }
            warden_configs::ConfigValue::Float(current) => {
                let middle = f64::midpoint(range.min.unwrap(), range.max.unwrap());
                warden_configs::ConfigValue::Float(if (current - middle).abs() > f64::EPSILON {
                    middle
                } else {
                    range.min.unwrap()
                })
            }
            _ => continue,
        };
        let edited = document
            .apply(&[ConfigEdit::new(entry.path.clone(), new_value)])
            .unwrap();
        let lines = differing_lines(&sample.bytes, edited.as_bytes()).unwrap();
        assert_eq!(lines, vec![entry.line], "{} {}", sample.name, entry.path);
        // Comentários (inclusive o `#Range`) continuam lá, intactos.
        let edited_doc = ConfigDocument::parse(ConfigFormat::Toml, edited.as_bytes()).unwrap();
        let after = edited_doc.get(&entry.path).unwrap();
        assert_eq!(after.comment, entry.comment);
        assert_eq!(after.range, entry.range);
        checked += 1;
    }
    assert!(
        checked >= 8,
        "só {checked} TOML do Forge/NeoForge com #Range no corpus"
    );
}

#[test]
fn metadados_do_forge_no_corpus() {
    let toml_ranges = count_entries(ConfigFormat::Toml, |entry| entry.range.is_some());
    let toml_allowed = count_entries(ConfigFormat::Toml, |entry| entry.allowed_values.is_some());
    let cfg_ranges = count_entries(ConfigFormat::LegacyCfg, |entry| entry.range.is_some());
    let cfg_typed = count_entries(ConfigFormat::LegacyCfg, |entry| entry.cfg_type.is_some());
    let commented = count_entries(ConfigFormat::Json5, |entry| entry.comment.is_some());
    assert!(toml_ranges > 100, "{toml_ranges}");
    assert!(toml_allowed > 10, "{toml_allowed}");
    assert!(cfg_ranges > 50, "{cfg_ranges}");
    assert!(cfg_typed > 500, "{cfg_typed}");
    assert!(commented > 20, "{commented}");
}

#[allow(clippy::unwrap_used)] // auxiliar de teste: falhar com pânico reprova o teste
fn count_entries(
    format: ConfigFormat,
    predicate: impl Fn(&warden_configs::ConfigEntry) -> bool,
) -> usize {
    samples(format)
        .iter()
        .map(|sample| {
            let document = ConfigDocument::parse(format, &sample.bytes).unwrap();
            document
                .tree()
                .entries
                .iter()
                .filter(|entry| predicate(entry))
                .count()
        })
        .sum()
}
