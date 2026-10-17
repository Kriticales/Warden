//! Como uma entrada casa com a busca e quanto vale (ARCHITECTURE §10.1).
//!
//! A busca é por palavras: cada palavra precisa aparecer em algum campo da entrada. Sem
//! diferenciar maiúsculas nem acentos (a palavra perde os acentos, e o `nucleo-matcher`
//! normaliza o texto do arquivo). No nome da chave a comparação é aproximada (letras podem
//! faltar no meio: "spwn" acha `spawnRate`), mas com no máximo um salto a cada quatro letras,
//! para "spawn" não casar com qualquer chave que tenha s, p, a, w e n espalhados. No
//! comentário, no valor e nas linhas de texto a palavra precisa aparecer inteira.
//!
//! Pontuação: o campo vale mais que a qualidade do casamento (chave > rótulo > comentário >
//! valor; linha de texto vale como comentário); dentro do campo, a do `nucleo-matcher`.

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher};

use super::extract::{Haystack, IndexEntry};
use super::model::{HitField, MarkedPart, MarkedText};

/// Palavras levadas em conta (o resto da busca é ignorado).
const MAX_WORDS: usize = 8;

/// Caracteres de contexto antes do primeiro destaque num trecho cortado.
const SNIPPET_BEFORE: usize = 40;

/// Tamanho máximo de um trecho.
const SNIPPET_CHARS: usize = 160;

/// Peso de cada campo, bem acima de qualquer pontuação do `nucleo-matcher` (que fica na casa
/// das centenas para palavras curtas).
fn tier(field: HitField) -> u32 {
    match field {
        HitField::Key => 40_000,
        HitField::Label => 30_000,
        HitField::Comment | HitField::Line => 20_000,
        HitField::Value => 10_000,
    }
}

/// Uma palavra da busca, pronta para os dois jeitos de comparar.
struct Word {
    fuzzy: Atom,
    exact: Atom,
    /// Saltos aceitos no nome da chave.
    max_gaps: usize,
}

/// A busca compilada.
pub(crate) struct CompiledQuery {
    words: Vec<Word>,
}

/// Como uma entrada casou.
pub(crate) struct EntryMatch {
    pub(crate) score: u32,
    pub(crate) field: HitField,
    /// Posições (em caracteres) destacadas em cada campo.
    pub(crate) key: Vec<u32>,
    pub(crate) label: Vec<u32>,
    pub(crate) comment: Vec<u32>,
    pub(crate) value: Vec<u32>,
    pub(crate) text: Vec<u32>,
}

impl EntryMatch {
    /// Uma chave mostrada sem busca por texto (o filtro "Só o que mudou do padrão" sozinho).
    pub(crate) fn whole_key() -> Self {
        Self {
            score: 0,
            field: HitField::Key,
            key: Vec::new(),
            label: Vec::new(),
            comment: Vec::new(),
            value: Vec::new(),
            text: Vec::new(),
        }
    }
}

/// Tira os acentos (á → a, ç → c) e passa para minúsculas, como o `nucleo-matcher` faz com o
/// texto do arquivo.
fn fold(word: &str) -> String {
    word.chars()
        .map(nucleo_matcher::chars::normalize)
        .flat_map(char::to_lowercase)
        .collect()
}

impl CompiledQuery {
    /// Compila a busca. `None` quando não sobra nenhuma palavra.
    pub(crate) fn new(text: &str) -> Option<Self> {
        let words: Vec<Word> = text
            .split_whitespace()
            .take(MAX_WORDS)
            .map(|word| {
                let folded = fold(word);
                let atom = |kind| {
                    Atom::new(
                        &folded,
                        CaseMatching::Ignore,
                        Normalization::Smart,
                        kind,
                        false,
                    )
                };
                Word {
                    fuzzy: atom(AtomKind::Fuzzy),
                    exact: atom(AtomKind::Substring),
                    max_gaps: (folded.chars().count() / 4).max(1),
                }
            })
            .collect();
        (!words.is_empty()).then_some(Self { words })
    }

    /// Confere uma entrada. `None` se alguma palavra não aparece em nenhum campo.
    pub(crate) fn matches(&self, entry: &IndexEntry, matcher: &mut Matcher) -> Option<EntryMatch> {
        let mut found = EntryMatch {
            score: 0,
            field: HitField::Value,
            key: Vec::new(),
            label: Vec::new(),
            comment: Vec::new(),
            value: Vec::new(),
            text: Vec::new(),
        };
        let mut best_field: Option<(u32, HitField)> = None;
        let mut scratch = Vec::new();
        for word in &self.words {
            let mut best: Option<(u32, HitField)> = None;
            let consider = |field: HitField, score: u16, best: &mut Option<(u32, HitField)>| {
                let total = tier(field) + u32::from(score);
                if best.is_none_or(|(current, _)| total > current) {
                    *best = Some((total, field));
                }
            };
            if let Some(key) = &entry.key {
                scratch.clear();
                // `score` é bem mais barato que `indices`: as posições só de quem casou.
                if word.fuzzy.score(key.utf32(), matcher).is_some()
                    && let Some(score) = word.fuzzy.indices(key.utf32(), matcher, &mut scratch)
                    && gaps(&scratch) <= word.max_gaps
                {
                    consider(HitField::Key, score, &mut best);
                    found.key.extend_from_slice(&scratch);
                }
            }
            for (field, haystack, marks) in [
                (HitField::Label, &entry.label, &mut found.label),
                (HitField::Comment, &entry.comment, &mut found.comment),
                (HitField::Value, &entry.value, &mut found.value),
                (HitField::Line, &entry.text, &mut found.text),
            ] {
                if let Some(haystack) = haystack {
                    scratch.clear();
                    if word.exact.score(haystack.utf32(), matcher).is_some()
                        && let Some(score) =
                            word.exact.indices(haystack.utf32(), matcher, &mut scratch)
                    {
                        consider(field, score, &mut best);
                        marks.extend_from_slice(&scratch);
                    }
                }
            }
            let (score, field) = best?;
            found.score += score;
            if best_field.is_none_or(|(current, _)| score > current) {
                best_field = Some((score, field));
            }
        }
        found.field = best_field.map_or(HitField::Value, |(_, field)| field);
        Some(found)
    }
}

/// Um `Matcher` com a configuração da busca.
pub(crate) fn new_matcher() -> Matcher {
    Matcher::new(Config::DEFAULT)
}

/// Quantos saltos há entre as posições casadas (as posições vêm em ordem).
fn gaps(indices: &[u32]) -> usize {
    indices
        .windows(2)
        .filter(|pair| pair[1] != pair[0] + 1)
        .count()
}

/// Texto com as posições destacadas. `snippet` corta textos longos em volta do primeiro
/// destaque.
pub(crate) fn marked(haystack: &Haystack, indices: &[u32], snippet: bool) -> MarkedText {
    let chars: Vec<char> = haystack.text.chars().collect();
    let mut marks = vec![false; chars.len()];
    for &index in indices {
        if let Some(mark) = usize::try_from(index)
            .ok()
            .and_then(|index| marks.get_mut(index))
        {
            *mark = true;
        }
    }
    let (mut start, mut end) = (0, chars.len());
    if snippet && chars.len() > SNIPPET_CHARS {
        let first = marks.iter().position(|&mark| mark).unwrap_or(0);
        start = first.saturating_sub(SNIPPET_BEFORE);
        // Começa numa palavra inteira.
        while start > 0 && start < first && !chars[start - 1].is_whitespace() {
            start += 1;
        }
        end = (start + SNIPPET_CHARS).min(chars.len());
        if end < chars.len() {
            while end > first + 1 && !chars[end].is_whitespace() {
                end -= 1;
            }
        }
    }
    let mut parts: Vec<MarkedPart> = Vec::new();
    if start > 0 {
        parts.push(MarkedPart {
            text: "…".into(),
            marked: false,
        });
    }
    for index in start..end {
        let mark = marks[index];
        match parts.last_mut() {
            Some(last) if last.marked == mark && !(index == start && start > 0) => {
                last.text.push(chars[index]);
            }
            _ => parts.push(MarkedPart {
                text: chars[index].to_string(),
                marked: mark,
            }),
        }
    }
    if end < chars.len() {
        match parts.last_mut() {
            Some(last) if !last.marked => {
                let trimmed = last.text.trim_end().len();
                last.text.truncate(trimmed);
                last.text.push('…');
            }
            _ => parts.push(MarkedPart {
                text: "…".into(),
                marked: false,
            }),
        }
    }
    MarkedText { parts }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: Option<&str>, comment: Option<&str>, value: Option<&str>) -> IndexEntry {
        IndexEntry {
            line: 1,
            key: key.map(Haystack::new),
            label: None,
            comment: comment.map(Haystack::new),
            value: value.map(Haystack::new),
            text: None,
            changed: false,
        }
    }

    fn check(query: &str, entry: &IndexEntry) -> Option<EntryMatch> {
        CompiledQuery::new(query)
            .unwrap()
            .matches(entry, &mut new_matcher())
    }

    #[test]
    fn sem_maiusculas_nem_acentos() {
        for key in ["Spawn", "spawnRate", "mobs.SPAWN_chance"] {
            assert!(
                check("spawn", &entry(Some(key), None, None)).is_some(),
                "{key}"
            );
        }
        let comment = entry(Some("x"), Some("Geração de minérios perto do início"), None);
        assert!(check("geracao", &comment).is_some());
        assert!(check("GERAÇÃO minerio", &comment).is_some());
        let plain = entry(Some("x"), Some("geracao"), None);
        assert!(check("geração", &plain).is_some());
        assert!(check("geração", &entry(Some("x"), Some("gerar"), None)).is_none());
    }

    #[test]
    fn todas_as_palavras_precisam_aparecer() {
        let item = entry(Some("mobs.spawnRate"), Some("Quantos aparecem"), Some("5"));
        assert!(check("spawn aparecem", &item).is_some());
        assert!(check("spawn 5", &item).is_some());
        assert!(check("spawn ausente", &item).is_none());
        assert!(CompiledQuery::new("   ").is_none());
    }

    #[test]
    fn chave_aproximada_mas_sem_letras_espalhadas() {
        assert!(check("spwn", &entry(Some("spawnRate"), None, None)).is_some());
        assert!(check("spawnrate", &entry(Some("spawn_rate"), None, None)).is_some());
        let scattered = entry(Some("showParticlesAndWarningsNow"), None, None);
        assert!(check("spawn", &scattered).is_none());
        // Fora da chave, a palavra precisa estar inteira.
        assert!(check("spwn", &entry(Some("x"), Some("spawn"), None)).is_none());
    }

    #[test]
    fn chave_vale_mais_que_rotulo_comentario_e_valor() {
        let key = check("spawn", &entry(Some("spawn"), None, None)).unwrap();
        let comment = check("spawn", &entry(Some("x"), Some("spawn"), None)).unwrap();
        let value = check("spawn", &entry(Some("x"), None, Some("spawn"))).unwrap();
        assert_eq!(key.field, HitField::Key);
        assert_eq!(comment.field, HitField::Comment);
        assert_eq!(value.field, HitField::Value);
        let mut labeled = entry(Some("x"), Some("spawn"), None);
        labeled.label = Some(Haystack::new("Spawn nas vilas"));
        let label = check("spawn", &labeled).unwrap();
        assert_eq!(label.field, HitField::Label);
        assert!(key.score > label.score && label.score > comment.score);
        assert!(key.score > comment.score && comment.score > value.score);
        assert!(tier(HitField::Key) > tier(HitField::Label));
        assert!(tier(HitField::Label) > tier(HitField::Comment));
        // Uma chave longa ainda vence um comentário que casa perfeitamente.
        let long = check(
            "spawn",
            &entry(
                Some("a.very.long.section.with.many.parts.spawnInVillagesOnly"),
                None,
                None,
            ),
        )
        .unwrap();
        assert!(long.score > comment.score);
    }

    #[test]
    fn destaque_e_trecho() {
        let haystack = Haystack::new("abc spawn def");
        let found = marked(&haystack, &[4, 5, 6, 7, 8], false);
        assert_eq!(found.plain(), "abc spawn def");
        assert_eq!(found.marked(), vec!["spawn"]);

        let long = format!("{}spawn {}", "antes ".repeat(30), "depois ".repeat(40));
        let snippet = marked(&Haystack::new(&long), &[180, 181, 182, 183, 184], true);
        let text = snippet.plain();
        assert!(text.starts_with('…') && text.ends_with('…'), "{text}");
        assert!(text.chars().count() <= SNIPPET_CHARS + 2);
        assert_eq!(snippet.marked(), vec!["spawn"]);
        assert!(text.contains("antes spawn depois"), "{text}");

        let accented = marked(&Haystack::new("geração"), &[0, 1], false);
        assert_eq!(accented.marked(), vec!["ge"]);
        assert_eq!(gaps(&[0, 1, 3, 4]), 1);
    }
}
