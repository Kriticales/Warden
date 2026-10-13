//! Listas no texto: onde está cada item, a vírgula depois dele e se ele ocupa a linha sozinho.
//!
//! O formulário edita uma lista **item por item** (CA-T12-02): trocar um item reescreve só os
//! bytes dele; acrescentar cria a linha (ou o trecho `, item`) no estilo dos vizinhos; tirar
//! apaga a linha dele (ou o trecho `item, `). O resto da lista (quebras de linha, indentação,
//! comentários entre os itens, vírgula final) fica como estava. A posição dos itens vem de uma
//! leitura própria do trecho do valor (os parsers só dão a faixa da lista inteira): se ela não
//! encontrar exatamente os itens que o parser leu, a lista fica "só em texto" no formulário.

use crate::format::ConfigFormat;
use crate::tree::TextSpan;

/// Um item da lista no texto (posições no arquivo inteiro).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ItemSpan {
    /// Primeiro byte do item.
    pub start: usize,
    /// Byte seguinte ao último.
    pub end: usize,
    /// Posição da vírgula depois do item, se houver.
    pub comma: Option<usize>,
    /// O item ocupa a linha sozinho (com a vírgula e um comentário depois, no máximo).
    pub own_line: bool,
}

/// Como a lista está escrita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ListShape {
    /// Entre colchetes (TOML, JSON, JSON5, `options.txt`): posições logo depois do `[` e do `]`.
    Brackets { open: usize, close: usize },
    /// Lista do `.cfg`: um item por linha, sem vírgulas; `end` é o começo da linha do `>`.
    CfgLines { start: usize, end: usize },
}

/// A lista lida do texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ListLayout {
    pub shape: ListShape,
    pub items: Vec<ItemSpan>,
}

/// Como os comentários começam dentro de uma lista do formato.
#[derive(Clone, Copy)]
struct Syntax {
    hash_comments: bool,
    slash_comments: bool,
    toml_strings: bool,
}

fn syntax(format: ConfigFormat) -> Option<Syntax> {
    Some(match format {
        ConfigFormat::Toml => Syntax {
            hash_comments: true,
            slash_comments: false,
            toml_strings: true,
        },
        ConfigFormat::Json | ConfigFormat::Json5 => Syntax {
            hash_comments: false,
            slash_comments: true,
            toml_strings: false,
        },
        ConfigFormat::OptionsTxt => Syntax {
            hash_comments: false,
            slash_comments: false,
            toml_strings: false,
        },
        ConfigFormat::Properties | ConfigFormat::LegacyCfg => return None,
    })
}

/// Lê a lista no trecho `span` de `text`. `None` se o trecho não for uma lista que esta leitura
/// entende (o formulário manda a chave para o editor de texto).
pub(super) fn read(text: &str, format: ConfigFormat, span: TextSpan) -> Option<ListLayout> {
    let body = text.get(span.range())?;
    if format == ConfigFormat::LegacyCfg {
        return Some(cfg_lines(text, span));
    }
    let syntax = syntax(format)?;
    if !(body.starts_with('[') && body.ends_with(']') && body.len() >= 2) {
        return None;
    }
    let bytes = text.as_bytes();
    let open = span.start + 1;
    let close = span.end - 1;
    let mut items: Vec<ItemSpan> = Vec::new();
    // Uma vírgula depois do último item já vista (separa o próximo).
    let mut separated = true;
    let mut at = open;
    while at < close {
        let byte = *bytes.get(at)?;
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' => at += 1,
            b',' => {
                let last = items.last_mut().filter(|_| !separated)?;
                last.comma = Some(at);
                separated = true;
                at += 1;
            }
            b'#' if syntax.hash_comments => at = line_end(bytes, at),
            b'/' if syntax.slash_comments && bytes.get(at + 1) == Some(&b'/') => {
                at = line_end(bytes, at);
            }
            b'/' if syntax.slash_comments && bytes.get(at + 1) == Some(&b'*') => {
                let rest = text.get(at + 2..close)?;
                at = at + 2 + rest.find("*/")? + 2;
            }
            b'[' | b']' | b'{' | b'}' => return None,
            _ => {
                if !separated {
                    return None;
                }
                let end = item_end(text, at, close, syntax)?;
                items.push(ItemSpan {
                    start: at,
                    end,
                    comma: None,
                    own_line: false,
                });
                separated = false;
                at = end;
            }
        }
    }
    for item in &mut items {
        item.own_line = owns_line(text, item, open, close, syntax);
    }
    Some(ListLayout {
        shape: ListShape::Brackets { open, close },
        items,
    })
}

/// Fim do item que começa em `at` (texto entre aspas ou palavra solta).
fn item_end(text: &str, at: usize, close: usize, syntax: Syntax) -> Option<usize> {
    let rest = text.get(at..close)?;
    if syntax.toml_strings {
        for delimiter in ["\"\"\"", "'''"] {
            if let Some(inner) = rest.strip_prefix(delimiter) {
                let escapes = delimiter.starts_with('"');
                let length = closing(inner, delimiter, escapes)?;
                // Até duas aspas a mais no fim fazem parte do texto (`""""` fecha com uma dentro).
                let extra = inner
                    .get(length + 3..)
                    .map_or(0, |after| {
                        after.len()
                            - after
                                .trim_start_matches(delimiter.as_bytes()[0] as char)
                                .len()
                    })
                    .min(2);
                return Some(at + 3 + length + 3 + extra);
            }
        }
    }
    let first = rest.chars().next()?;
    if first == '"' || first == '\'' {
        let escapes = first == '"' || !syntax.toml_strings;
        let inner = rest.get(1..)?;
        let length = closing(inner, &first.to_string(), escapes)?;
        return Some(at + 1 + length + 1);
    }
    let length = rest
        .find(|c: char| {
            c.is_whitespace()
                || c == ','
                || (syntax.hash_comments && c == '#')
                || (syntax.slash_comments && c == '/')
        })
        .unwrap_or(rest.len());
    (length > 0).then_some(at + length)
}

/// Posição (em `inner`) do delimitador que fecha o texto; com `escapes`, `\` pula o próximo
/// caractere.
fn closing(inner: &str, delimiter: &str, escapes: bool) -> Option<usize> {
    let mut chars = inner.char_indices();
    while let Some((offset, c)) = chars.next() {
        if escapes && c == '\\' {
            chars.next();
            continue;
        }
        if inner.get(offset..)?.starts_with(delimiter) {
            return Some(offset);
        }
    }
    None
}

/// Posição do `\n` que termina a linha de `at` (ou o fim do texto).
fn line_end(bytes: &[u8], at: usize) -> usize {
    bytes
        .get(at..)
        .and_then(|rest| rest.iter().position(|byte| *byte == b'\n'))
        .map_or(bytes.len(), |offset| at + offset)
}

/// Começo da linha de `at`.
pub(super) fn line_start(text: &str, at: usize) -> usize {
    text.get(..at)
        .and_then(|before| before.rfind('\n'))
        .map_or(0, |offset| offset + 1)
}

/// Byte seguinte ao `\n` da linha de `at` (ou o fim do texto).
pub(super) fn after_line(text: &str, at: usize) -> usize {
    let end = line_end(text.as_bytes(), at);
    (end + 1).min(text.len())
}

/// Se o item está sozinho na linha: antes dele só espaços (e a linha não é a do `[`); depois,
/// no máximo a vírgula, espaços e um comentário de linha até o fim da linha.
fn owns_line(text: &str, item: &ItemSpan, open: usize, close: usize, syntax: Syntax) -> bool {
    let start = line_start(text, item.start);
    if start < open
        || !text
            .get(start..item.start)
            .is_some_and(|before| before.chars().all(|c| c == ' ' || c == '\t'))
    {
        return false;
    }
    let end = line_end(text.as_bytes(), item.end);
    if end > close {
        return false;
    }
    let Some(after) = text.get(item.end..end) else {
        return false;
    };
    let after = after.trim_end_matches('\r').trim_start_matches([' ', '\t']);
    let after = after.strip_prefix(',').unwrap_or(after);
    let after = after.trim_start_matches([' ', '\t']);
    after.is_empty()
        || (syntax.hash_comments && after.starts_with('#'))
        || (syntax.slash_comments && after.starts_with("//"))
}

/// As linhas da lista do `.cfg`: cada linha é um item, sem os espaços das pontas.
fn cfg_lines(text: &str, span: TextSpan) -> ListLayout {
    let mut items = Vec::new();
    let mut at = span.start;
    while at < span.end {
        let end = line_end(text.as_bytes(), at).min(span.end);
        let line = text.get(at..end).unwrap_or("");
        let content = line.trim();
        let offset = line.len() - line.trim_start().len();
        items.push(ItemSpan {
            start: at + offset,
            end: at + offset + content.len(),
            comma: None,
            own_line: true,
        });
        at = end + 1;
    }
    ListLayout {
        shape: ListShape::CfgLines {
            start: span.start,
            end: span.end,
        },
        items,
    }
}

/// Uma troca de bytes no texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Splice {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// O que mudar na lista: itens trocados (posição antiga → texto novo), itens novos e itens
/// tirados.
pub(super) struct ListChange<'a> {
    /// Quantos itens do começo ficam como estão.
    pub keep_head: usize,
    /// Textos dos itens que trocam de lugar com os antigos, a partir de `keep_head`.
    pub replaced: &'a [String],
    /// Textos dos itens novos, depois dos trocados.
    pub inserted: &'a [String],
    /// Quantos itens antigos saem, depois dos trocados.
    pub removed: usize,
}

/// Como escrever itens novos quando a lista não mostra o próprio estilo.
pub(super) struct NewItemStyle<'a> {
    /// Fim de linha das linhas novas numa lista ainda sem linhas.
    pub line_ending: &'a str,
    /// Indentação completa da linha de um item novo numa lista vazia.
    pub empty_indent: &'a str,
    /// Separador entre itens na mesma linha quando a lista não tem dois itens para copiar.
    pub separator: &'a str,
}

/// As trocas de bytes que fazem a mudança, no estilo da lista.
pub(super) fn plan(
    text: &str,
    layout: &ListLayout,
    change: &ListChange<'_>,
    style: &NewItemStyle<'_>,
) -> Vec<Splice> {
    let items = &layout.items;
    let mut splices = Vec::new();
    for (offset, new_text) in change.replaced.iter().enumerate() {
        if let Some(item) = items.get(change.keep_head + offset) {
            splices.push(Splice {
                start: item.start,
                end: item.end,
                text: new_text.clone(),
            });
        }
    }
    let after = change.keep_head + change.replaced.len();
    if !change.inserted.is_empty() {
        let anchor = after.checked_sub(1).and_then(|index| items.get(index));
        splices.extend(insert(
            text,
            layout,
            anchor,
            items.get(after),
            change.inserted,
            style,
        ));
    }
    if change.removed > 0 {
        splices.extend(remove(text, layout, after, change.removed));
    }
    splices
}

fn cfg(layout: &ListLayout) -> bool {
    matches!(layout.shape, ListShape::CfgLines { .. })
}

/// Fim de linha usado na linha de `at`.
fn ending_at(text: &str, at: usize, fallback: &str) -> String {
    let end = line_end(text.as_bytes(), at);
    if end >= text.len() {
        return fallback.to_owned();
    }
    if text.get(..end).is_some_and(|before| before.ends_with('\r')) {
        "\r\n".to_owned()
    } else {
        "\n".to_owned()
    }
}

/// Espaços e tabs no começo da linha de `at`, até `at`.
pub(super) fn indent_of(text: &str, at: usize) -> String {
    let start = line_start(text, at);
    text.get(start..at)
        .map(|before| {
            before
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect()
        })
        .unwrap_or_default()
}

/// Separador entre dois itens na mesma linha, copiado da lista (`, ` ou `,`).
fn inline_separator(text: &str, items: &[ItemSpan], default: &str) -> String {
    items
        .windows(2)
        .find_map(|pair| {
            let between = text.get(pair[0].end..pair[1].start)?;
            (!between.contains('\n') && between.trim() == ",").then(|| between.to_owned())
        })
        .unwrap_or_else(|| default.to_owned())
}

fn insert(
    text: &str,
    layout: &ListLayout,
    anchor: Option<&ItemSpan>,
    next: Option<&ItemSpan>,
    inserted: &[String],
    style: &NewItemStyle<'_>,
) -> Vec<Splice> {
    let line_ending = style.line_ending;
    let commas = !cfg(layout);
    let mut splices = Vec::new();
    match (anchor, next) {
        // Depois de um item que ocupa a linha: linhas novas logo abaixo, com a indentação dele.
        (Some(anchor), _) if anchor.own_line => {
            let indent = indent_of(text, anchor.start);
            let ending = ending_at(text, anchor.start, line_ending);
            let had_comma = anchor.comma.is_some();
            if commas && !had_comma {
                splices.push(Splice {
                    start: anchor.end,
                    end: anchor.end,
                    text: ",".to_owned(),
                });
            }
            let lines = item_lines(&indent, inserted, &ending, |position| {
                commas && (position + 1 < inserted.len() || had_comma)
            });
            let at = after_line(text, anchor.start);
            let at = if at > text.len() { text.len() } else { at };
            splices.push(Splice {
                start: at,
                end: at,
                text: lines,
            });
        }
        // Depois de um item na mesma linha de outros: `, item` logo depois dele.
        (Some(anchor), _) => {
            let separator = inline_separator(text, &layout.items, style.separator);
            let joined = with_separator(inserted, &separator, true);
            splices.push(Splice {
                start: anchor.end,
                end: anchor.end,
                text: joined,
            });
        }
        // Antes do primeiro item, que ocupa a linha: linhas novas logo acima.
        (None, Some(first)) if first.own_line => {
            let indent = indent_of(text, first.start);
            let ending = ending_at(text, first.start, line_ending);
            let lines = item_lines(&indent, inserted, &ending, |_| commas);
            let at = line_start(text, first.start);
            splices.push(Splice {
                start: at,
                end: at,
                text: lines,
            });
        }
        (None, Some(first)) => {
            let separator = inline_separator(text, &layout.items, style.separator);
            let joined = with_separator(inserted, &separator, false);
            splices.push(Splice {
                start: first.start,
                end: first.start,
                text: joined,
            });
        }
        // Lista vazia.
        (None, None) => splices.push(into_empty(text, layout, inserted, style)),
    }
    splices
}

/// Uma linha por item: indentação, item, vírgula quando `comma(posição)` e fim de linha.
fn item_lines(
    indent: &str,
    items: &[String],
    ending: &str,
    comma: impl Fn(usize) -> bool,
) -> String {
    let mut out = String::new();
    for (position, item) in items.iter().enumerate() {
        out.push_str(indent);
        out.push_str(item);
        if comma(position) {
            out.push(',');
        }
        out.push_str(ending);
    }
    out
}

/// Os itens com o separador antes de cada um (`, a, b`) ou depois (`a, b, `).
fn with_separator(items: &[String], separator: &str, before: bool) -> String {
    let mut out = String::new();
    for item in items {
        if before {
            out.push_str(separator);
        }
        out.push_str(item);
        if !before {
            out.push_str(separator);
        }
    }
    out
}

fn into_empty(
    text: &str,
    layout: &ListLayout,
    inserted: &[String],
    style: &NewItemStyle<'_>,
) -> Splice {
    let (line_ending, indent) = (style.line_ending, style.empty_indent);
    match layout.shape {
        ListShape::CfgLines { end, .. } => {
            let ending = ending_at(text, end.saturating_sub(1), line_ending);
            let lines = item_lines(indent, inserted, &ending, |_| false);
            Splice {
                start: end,
                end,
                text: lines,
            }
        }
        ListShape::Brackets { open, close } => {
            let inside = text.get(open..close).unwrap_or("");
            if inside.contains('\n') {
                // `[` e `]` em linhas diferentes: um item por linha, antes da linha do `]`.
                let at = line_start(text, close);
                let ending = ending_at(text, open, line_ending);
                let lines = item_lines(indent, inserted, &ending, |position| {
                    position + 1 < inserted.len()
                });
                Splice {
                    start: at,
                    end: at,
                    text: lines,
                }
            } else {
                Splice {
                    start: open,
                    end: open,
                    text: inserted.join(style.separator),
                }
            }
        }
    }
}

fn remove(text: &str, layout: &ListLayout, from: usize, count: usize) -> Vec<Splice> {
    let items = &layout.items;
    let Some(gone) = items.get(from..from + count) else {
        return Vec::new();
    };
    let previous = from.checked_sub(1).and_then(|index| items.get(index));
    let next = items.get(from + count);
    let mut splices = Vec::new();
    if gone.iter().all(|item| item.own_line) {
        // Cada item sai com a linha dele (e o comentário que estiver nela).
        for item in gone {
            splices.push(Splice {
                start: line_start(text, item.start),
                end: after_line(text, item.start),
                text: String::new(),
            });
        }
        // O último item não tinha vírgula: o que fica por último também perde a dele (o
        // inverso de acrescentar uma linha depois de um item sem vírgula).
        let last_gone = gone.last().is_some_and(|item| item.comma.is_none());
        if next.is_none()
            && last_gone
            && let Some(comma) = previous.and_then(|item| item.comma)
        {
            splices.push(Splice {
                start: comma,
                end: comma + 1,
                text: String::new(),
            });
        }
        return splices;
    }
    let (Some(first), Some(last)) = (gone.first(), gone.last()) else {
        return splices;
    };
    let (start, end) = match (previous, next) {
        // Itens no meio ou no começo: sai `item, ` até o próximo.
        (_, Some(next)) => (first.start, next.start),
        // Itens no fim: sai `, item` desde o anterior.
        (Some(previous), None) => (previous.end, last.end),
        // Todos os itens.
        (None, None) => (first.start, last.comma.map_or(last.end, |comma| comma + 1)),
    };
    splices.push(Splice {
        start,
        end,
        text: String::new(),
    });
    splices
}

/// Aplica as trocas (que não se sobrepõem) ao texto.
pub(super) fn apply(text: &str, mut splices: Vec<Splice>) -> Option<String> {
    splices.sort_by_key(|splice| (splice.start, splice.end));
    let mut out = String::with_capacity(text.len() + 64);
    let mut cursor = 0;
    for splice in splices {
        if splice.start < cursor || splice.end < splice.start {
            return None;
        }
        out.push_str(text.get(cursor..splice.start)?);
        out.push_str(&splice.text);
        cursor = splice.end;
    }
    out.push_str(text.get(cursor..)?);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(format: ConfigFormat, text: &str) -> ListLayout {
        let start = text.find('[').unwrap();
        let end = text.rfind(']').unwrap() + 1;
        read(text, format, TextSpan::new(start, end)).unwrap()
    }

    fn texts<'t>(text: &'t str, layout: &ListLayout) -> Vec<&'t str> {
        layout
            .items
            .iter()
            .map(|item| &text[item.start..item.end])
            .collect()
    }

    #[test]
    fn le_itens_virgulas_e_comentarios() {
        let text = "a = [\n  \"x, y\", # um\n  'z' ,\n  # solto\n  3\n]\n";
        let found = layout(ConfigFormat::Toml, text);
        assert_eq!(texts(text, &found), ["\"x, y\"", "'z'", "3"]);
        assert!(found.items.iter().all(|item| item.own_line));
        assert!(found.items[0].comma.is_some() && found.items[2].comma.is_none());
        let inline = "x = [1, 2,3]";
        let found = layout(ConfigFormat::Toml, inline);
        assert_eq!(texts(inline, &found), ["1", "2", "3"]);
        assert!(found.items.iter().all(|item| !item.own_line));
        let json = "{\"a\": [ \"q\\\"]\" /* c */, 'w' // d\n ]}";
        let found = layout(ConfigFormat::Json5, json);
        assert_eq!(texts(json, &found), ["\"q\\\"]\"", "'w'"]);
        let multi = "a = ['''x]''', \"\"\"y\"\"\"\"]";
        let found = layout(ConfigFormat::Toml, multi);
        assert_eq!(texts(multi, &found), ["'''x]'''", "\"\"\"y\"\"\"\""]);
    }

    #[test]
    fn recusa_o_que_nao_entende() {
        for text in [
            "a = [[1], 2]",
            "a = [1 2]",
            "a = [,1]",
            "a = [{x=1}]",
            "a = [\"x]",
        ] {
            let start = text.find('[').unwrap();
            assert_eq!(
                read(text, ConfigFormat::Toml, TextSpan::new(start, text.len())),
                None,
                "{text}"
            );
        }
        assert_eq!(
            read("a=1", ConfigFormat::Properties, TextSpan::new(2, 3)),
            None
        );
    }

    fn change(
        text: &str,
        format: ConfigFormat,
        keep_head: usize,
        replaced: &[&str],
        inserted: &[&str],
        removed: usize,
    ) -> String {
        let found = layout(format, text);
        let replaced: Vec<String> = replaced.iter().map(|s| (*s).to_owned()).collect();
        let inserted: Vec<String> = inserted.iter().map(|s| (*s).to_owned()).collect();
        let splices = plan(
            text,
            &found,
            &ListChange {
                keep_head,
                replaced: &replaced,
                inserted: &inserted,
                removed,
            },
            &NewItemStyle {
                line_ending: "\n",
                empty_indent: "    ",
                separator: ", ",
            },
        );
        apply(text, splices).unwrap()
    }

    #[test]
    fn acrescentar_e_tirar_em_varias_linhas() {
        let toml = ConfigFormat::Toml;
        let text = "a = [\r\n\t\"x\", # um\r\n\t\"y\"\r\n]\r\n";
        let more = change(text, toml, 2, &[], &["\"z\""], 0);
        assert_eq!(
            more,
            "a = [\r\n\t\"x\", # um\r\n\t\"y\",\r\n\t\"z\"\r\n]\r\n"
        );
        assert_eq!(change(&more, toml, 2, &[], &[], 1), text);
        let first = change(text, toml, 0, &[], &["\"w\""], 0);
        assert_eq!(
            first,
            "a = [\r\n\t\"w\",\r\n\t\"x\", # um\r\n\t\"y\"\r\n]\r\n"
        );
        assert_eq!(change(&first, toml, 0, &[], &[], 1), text);
        let swapped = change(text, toml, 0, &["\"q\""], &[], 0);
        assert_eq!(swapped, "a = [\r\n\t\"q\", # um\r\n\t\"y\"\r\n]\r\n");
    }

    #[test]
    fn acrescentar_e_tirar_numa_linha() {
        let json = ConfigFormat::Json;
        let text = "{\"a\": [\"x\",\"y\"]}";
        let more = change(text, json, 2, &[], &["\"z\""], 0);
        assert_eq!(more, "{\"a\": [\"x\",\"y\",\"z\"]}");
        assert_eq!(change(&more, json, 2, &[], &[], 1), text);
        assert_eq!(change(text, json, 0, &[], &[], 1), "{\"a\": [\"y\"]}");
        let empty = "{\"a\": []}";
        let one = change(empty, json, 0, &[], &["1"], 0);
        assert_eq!(one, "{\"a\": [1]}");
        assert_eq!(change(&one, json, 0, &[], &[], 1), empty);
        let open = "a = [\n]\n";
        let one = change(open, ConfigFormat::Toml, 0, &[], &["1"], 0);
        assert_eq!(one, "a = [\n    1\n]\n");
        assert_eq!(change(&one, ConfigFormat::Toml, 0, &[], &[], 1), open);
    }

    #[test]
    fn lista_do_cfg() {
        let text = "general {\n    S:l <\n\tminecraft:stone\n        b\n     >\n}\n";
        let start = text.find("\tminecraft").unwrap();
        let end = text.find("     >").unwrap();
        let found = read(text, ConfigFormat::LegacyCfg, TextSpan::new(start, end)).unwrap();
        assert_eq!(texts(text, &found), ["minecraft:stone", "b"]);
        let splices = plan(
            text,
            &found,
            &ListChange {
                keep_head: 2,
                replaced: &[],
                inserted: &["c".to_owned()],
                removed: 0,
            },
            &NewItemStyle {
                line_ending: "\n",
                empty_indent: "        ",
                separator: ", ",
            },
        );
        let more = apply(text, splices).unwrap();
        assert_eq!(more, text.replace("b\n", "b\n        c\n"));
    }
}
