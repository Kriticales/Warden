//! De um arquivo para as entradas do índice: uma por chave nos formatos que a crate lê, uma
//! por linha nos outros (YAML, SNBT, scripts, `.cfg` que não é do Forge, texto).

use std::path::Path;

use crate::format::ConfigFormat;
use crate::tree::{ConfigValue, EntryKind};
use crate::{ConfigDocument, MAX_STRUCTURED_BYTES};

/// Maior arquivo lido pela busca (8 MB, o mesmo limite de leitura do editor). Entre 2 MB e
/// 8 MB o arquivo é indexado por linha, como no editor de texto.
pub(crate) const MAX_INDEXED_BYTES: u64 = 8 * 1024 * 1024;

/// Texto guardado de um valor ou de uma linha (o resto é cortado com "…").
const MAX_FIELD_CHARS: usize = 300;

/// Texto guardado de um comentário.
const MAX_COMMENT_CHARS: usize = 1_000;

/// Um texto pronto para o *matcher*: o texto e, se não for ASCII, os caracteres já separados.
#[derive(Debug, Clone)]
pub(crate) struct Haystack {
    pub(crate) text: Box<str>,
    pub(crate) chars: Option<Box<[char]>>,
}

impl Haystack {
    pub(crate) fn new(text: &str) -> Self {
        let chars = (!text.is_ascii()).then(|| text.chars().collect());
        Self {
            text: text.into(),
            chars,
        }
    }

    pub(crate) fn utf32(&self) -> nucleo_matcher::Utf32Str<'_> {
        match &self.chars {
            Some(chars) => nucleo_matcher::Utf32Str::Unicode(chars),
            None => nucleo_matcher::Utf32Str::Ascii(self.text.as_bytes()),
        }
    }
}

/// Uma entrada do índice: uma chave (com valor ou seção) ou uma linha de texto.
#[derive(Debug, Clone)]
pub(crate) struct IndexEntry {
    /// Linha no arquivo (começa em 1).
    pub(crate) line: u32,
    /// Caminho da chave para exibição (`geral.spawnRate`); `None` numa linha de texto.
    pub(crate) key: Option<Haystack>,
    /// Rótulo traduzido do mod (P1, C-04): ainda vazio.
    pub(crate) label: Option<Haystack>,
    /// Comentário numa linha só.
    pub(crate) comment: Option<Haystack>,
    /// Valor como texto.
    pub(crate) value: Option<Haystack>,
    /// A linha, nos arquivos sem leitor estruturado.
    pub(crate) text: Option<Haystack>,
}

/// Como o arquivo foi lido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Extraction {
    /// Pelo leitor do formato: uma entrada por chave.
    Keys(ConfigFormat),
    /// Por linha.
    Lines,
}

/// As entradas de um arquivo. Arquivo que o leitor do formato recusa (ou grande demais para
/// ele) cai na leitura por linha; texto fora de UTF-8 é lido trocando os bytes inválidos.
pub(crate) fn extract(path: &str, bytes: &[u8]) -> (Extraction, Vec<IndexEntry>) {
    if bytes.len() <= MAX_STRUCTURED_BYTES
        && let Some(format) = ConfigFormat::detect(Path::new(path), bytes)
        && let Ok(document) = ConfigDocument::parse(format, bytes)
    {
        return (Extraction::Keys(format), keys(&document));
    }
    (Extraction::Lines, lines(bytes))
}

fn keys(document: &ConfigDocument) -> Vec<IndexEntry> {
    document
        .tree()
        .entries
        .iter()
        .filter(|entry| !entry.path.is_empty())
        .map(|entry| {
            let value = match (&entry.kind, &entry.value) {
                (EntryKind::Value, Some(value)) => Some(value_text(value)),
                _ => None,
            };
            IndexEntry {
                line: u32::try_from(entry.line).unwrap_or(u32::MAX),
                key: Some(Haystack::new(&entry.path.to_string())),
                label: None,
                comment: entry
                    .comment
                    .as_deref()
                    .map(one_line)
                    .filter(|text| !text.is_empty())
                    .map(|text| Haystack::new(&cut(&text, MAX_COMMENT_CHARS))),
                value: value.map(|text| Haystack::new(&cut(&text, MAX_FIELD_CHARS))),
                text: None,
            }
        })
        .collect()
}

fn value_text(value: &ConfigValue) -> String {
    value.as_plain_text().unwrap_or_else(|| value.to_string())
}

fn lines(bytes: &[u8]) -> Vec<IndexEntry> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then(|| IndexEntry {
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                key: None,
                label: None,
                comment: None,
                value: None,
                text: Some(Haystack::new(&cut(trimmed, MAX_FIELD_CHARS))),
            })
        })
        .collect()
}

/// Junta as linhas do comentário com um espaço e tira os espaços repetidos.
fn one_line(comment: &str) -> String {
    comment.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Corta em `max` caracteres, com "…" no fim.
fn cut(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(entries: &[IndexEntry]) -> Vec<(u32, String, Option<String>, Option<String>)> {
        entries
            .iter()
            .map(|entry| {
                (
                    entry.line,
                    entry
                        .key
                        .as_ref()
                        .or(entry.text.as_ref())
                        .map(|h| h.text.to_string())
                        .unwrap_or_default(),
                    entry.value.as_ref().map(|h| h.text.to_string()),
                    entry.comment.as_ref().map(|h| h.text.to_string()),
                )
            })
            .collect()
    }

    #[test]
    fn toml_vira_uma_entrada_por_chave() {
        let toml =
            "[mobs]\n\t#Chance de\n\t#  aparecer\n\tspawnRate = 5\n\tnomes = [\"a\", \"b\"]\n";
        let (how, entries) = extract("config/x-common.toml", toml.as_bytes());
        assert_eq!(how, Extraction::Keys(ConfigFormat::Toml));
        assert_eq!(
            texts(&entries),
            vec![
                (1, "mobs".into(), None, None),
                (
                    4,
                    "mobs.spawnRate".into(),
                    Some("5".into()),
                    Some("Chance de aparecer".into())
                ),
                (5, "mobs.nomes".into(), Some("[\"a\", \"b\"]".into()), None),
            ]
        );
    }

    #[test]
    fn sem_leitor_vira_uma_entrada_por_linha() {
        let yaml = "\u{feff}spawn:\n\n  rate: 3\r\n";
        let (how, entries) = extract("config/x.yml", yaml.as_bytes());
        assert_eq!(how, Extraction::Lines);
        assert_eq!(
            texts(&entries),
            vec![
                (1, "spawn:".into(), None, None),
                (3, "rate: 3".into(), None, None)
            ]
        );
        // `.cfg` que não é do Forge, TOML quebrado e texto fora de UTF-8 também.
        let ini = extract("config/a.cfg", b"[s]\nk = v\n");
        assert_eq!(ini.0, Extraction::Lines);
        let broken = extract("config/a.toml", b"a = [\n");
        assert_eq!(broken.0, Extraction::Lines);
        let latin1 = extract("options.txt", b"nome:Jos\xe9\n");
        assert_eq!(latin1.0, Extraction::Lines);
        assert_eq!(texts(&latin1.1)[0].1, "nome:Jos\u{fffd}");
    }

    #[test]
    fn textos_longos_sao_cortados() {
        let long = "x".repeat(400);
        let (_, entries) = extract("notas.txt", long.as_bytes());
        let text = entries[0].text.as_ref().unwrap();
        assert_eq!(text.text.chars().count(), MAX_FIELD_CHARS + 1);
        assert!(text.text.ends_with('…'));
        assert_eq!(cut("ação", 10), "ação");
        assert_eq!(cut("ação", 2), "aç…");
    }

    #[test]
    fn haystack_separa_os_caracteres_so_fora_do_ascii() {
        assert!(Haystack::new("spawn").chars.is_none());
        let accented = Haystack::new("geração");
        assert_eq!(accented.chars.as_deref().map(<[char]>::len), Some(7));
        assert!(matches!(
            accented.utf32(),
            nucleo_matcher::Utf32Str::Unicode(_)
        ));
    }
}
