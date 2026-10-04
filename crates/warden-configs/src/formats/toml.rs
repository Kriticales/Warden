//! TOML com `toml_edit`: só a leitura com posições; a escrita troca os bytes do valor.
//!
//! O `DocumentMut::to_string` do `toml_edit` não serve para gravar: no corpus ele trocou CRLF
//! por LF e acrescentou quebra de linha final em 28 de 30 arquivos reais (avaliação da C-01).

use toml_edit::{Array, ArrayOfTables, InlineTable, Item, Table, Value};

use super::{Builder, EditStyle, Parsed, Rendered, ValueEntry, invalid};
use crate::error::Result;
use crate::format::ConfigFormat;
use crate::path::KeyPath;
use crate::text::{DocText, strip_line_comment};
use crate::tree::ConfigValue;

pub(crate) fn parse(doc: &DocText<'_>) -> Result<Parsed> {
    let parsed = toml_edit::Document::parse(doc.body).map_err(|error| {
        let offset = error.span().map_or(0, |span| span.start);
        doc.parse_error(ConfigFormat::Toml, offset, error.message())
    })?;
    let mut walker = Walker {
        builder: Builder::new(doc),
    };
    walker.table(parsed.as_table(), &KeyPath::root());
    let mut result = walker.builder.finish();
    sort_by_position(&mut result);
    Ok(result)
}

/// Põe as entradas na ordem do arquivo. O `toml_edit` agrupa as chaves por tabela; uma tabela
/// declarada depois (`[a]`, `[b]`, `[a.c]`) apareceria fora de ordem. A ordenação é estável,
/// então uma seção implícita (sem cabeçalho) continua antes da primeira chave dela.
fn sort_by_position(parsed: &mut Parsed) {
    let mut paired: Vec<_> = std::mem::take(&mut parsed.entries)
        .into_iter()
        .zip(std::mem::take(&mut parsed.styles))
        .collect();
    paired.sort_by_key(|(entry, _)| entry.line);
    let (entries, styles) = paired.into_iter().unzip();
    parsed.entries = entries;
    parsed.styles = styles;
}

struct Walker<'d, 'a> {
    builder: Builder<'d, 'a>,
}

impl Walker<'_, '_> {
    fn line_of(&self, offset: Option<usize>) -> Option<usize> {
        offset.map(|offset| self.builder.doc.line_index_of(offset))
    }

    fn comment(&self, line_index: usize) -> Option<String> {
        self.builder
            .doc
            .leading_comment(line_index, |text| strip_line_comment(text, &['#']))
    }

    /// Linha de uma entrada: a da chave; sem chave com posição, a do valor; sem nada (tabela
    /// implícita), a da primeira entrada dentro dela.
    fn table(&mut self, table: &Table, path: &KeyPath) {
        for (key, item) in table {
            let key_start = table
                .key(key)
                .and_then(toml_edit::Key::span)
                .map(|span| span.start);
            let child = path.child_key(key);
            self.item(item, &child, key_start);
        }
    }

    fn item(&mut self, item: &Item, path: &KeyPath, key_start: Option<usize>) {
        match item {
            Item::None => {}
            Item::Value(value) => self.value(value, path, key_start),
            Item::Table(table) => {
                let header = table.span().map(|span| span.start).or(key_start);
                self.section_with_children(header, path, |walker| walker.table(table, path));
            }
            Item::ArrayOfTables(array) => self.array_of_tables(array, path),
        }
    }

    fn array_of_tables(&mut self, array: &ArrayOfTables, path: &KeyPath) {
        let first = array.iter().find_map(Table::span).map(|span| span.start);
        self.section_with_children(first, path, |walker| {
            for (index, table) in array.iter().enumerate() {
                let child = path.child_index(index);
                let start = table.span().map(|span| span.start);
                walker.section_with_children(start, &child, |inner| inner.table(table, &child));
            }
        });
    }

    /// Registra a seção e os filhos. Sem posição própria, a seção fica na linha do primeiro
    /// filho (a ordenação estável mantém a seção antes dele).
    fn section_with_children(
        &mut self,
        start: Option<usize>,
        path: &KeyPath,
        children: impl FnOnce(&mut Self),
    ) {
        let position = self.builder.parsed.entries.len();
        let own_line = self.line_of(start);
        self.builder
            .section(path.clone(), own_line.unwrap_or(0), None);
        children(self);
        let line_index = match own_line {
            Some(line) => line,
            None => self
                .builder
                .parsed
                .entries
                .get(position + 1..)
                .and_then(|rest| rest.iter().map(|entry| entry.line).min())
                .map_or(0, |line| line.saturating_sub(1)),
        };
        let comment = own_line.and_then(|line| self.comment(line));
        if let Some(entry) = self.builder.parsed.entries.get_mut(position) {
            entry.line = line_index + 1;
            entry.comment = comment;
        }
    }

    fn value(&mut self, value: &Value, path: &KeyPath, key_start: Option<usize>) {
        let value_start = value.span().map(|span| span.start);
        match value {
            Value::InlineTable(table) => {
                self.section_with_children(key_start.or(value_start), path, |walker| {
                    walker.inline_table(table, path);
                });
            }
            Value::Array(array) if !is_scalar_array(array) => {
                self.section_with_children(key_start.or(value_start), path, |walker| {
                    for (index, element) in array.iter().enumerate() {
                        walker.value(element, &path.child_index(index), None);
                    }
                });
            }
            _ => {
                let (Some(span), Some(converted)) = (value.span(), convert(value)) else {
                    return;
                };
                let line_index = self.line_of(key_start.or(value_start)).unwrap_or(0);
                let literal = self
                    .builder
                    .doc
                    .body
                    .get(span.clone())
                    .is_some_and(|raw| raw.starts_with('\'') && !raw.starts_with("'''"));
                let comment = key_start.and_then(|_| self.comment(line_index));
                self.builder.value(ValueEntry {
                    path: path.clone(),
                    value: converted,
                    line_index,
                    span,
                    style: EditStyle::Toml { literal },
                    comment,
                    cfg_type: None,
                });
            }
        }
    }

    fn inline_table(&mut self, table: &InlineTable, path: &KeyPath) {
        for (key, value) in table {
            let key_start = table
                .key(key)
                .and_then(toml_edit::Key::span)
                .map(|span| span.start);
            self.value(value, &path.child_key(key), key_start);
        }
    }
}

fn is_scalar_array(array: &Array) -> bool {
    array.iter().all(|value| match value {
        Value::InlineTable(_) => false,
        Value::Array(inner) => is_scalar_array(inner),
        _ => true,
    })
}

fn convert(value: &Value) -> Option<ConfigValue> {
    Some(match value {
        Value::String(text) => ConfigValue::String(text.value().clone()),
        Value::Integer(number) => ConfigValue::Integer(*number.value()),
        Value::Float(number) => ConfigValue::Float(*number.value()),
        Value::Boolean(flag) => ConfigValue::Bool(*flag.value()),
        Value::Datetime(moment) => ConfigValue::Datetime(moment.value().to_string()),
        Value::Array(array) => ConfigValue::List(array.iter().filter_map(convert).collect()),
        Value::InlineTable(_) => return None,
    })
}

fn to_toml(path: &KeyPath, value: &ConfigValue) -> Result<Value> {
    Ok(match value {
        ConfigValue::Bool(flag) => Value::from(*flag),
        ConfigValue::Integer(number) => Value::from(*number),
        ConfigValue::Float(number) => Value::from(*number),
        ConfigValue::String(text) => Value::from(text.as_str()),
        ConfigValue::Datetime(text) => {
            let moment: toml_edit::Datetime = text
                .parse()
                .map_err(|_| invalid(path, format!("{text:?} não é uma data TOML")))?;
            Value::from(moment)
        }
        ConfigValue::Null => return Err(invalid(path, "TOML não tem valor nulo")),
        ConfigValue::List(items) => {
            let mut array = Array::new();
            for item in items {
                array.push_formatted(to_toml(path, item)?);
            }
            array.fmt();
            Value::Array(array)
        }
    })
}

pub(crate) fn render(path: &KeyPath, value: &ConfigValue, literal: bool) -> Result<Rendered> {
    let text = match value {
        ConfigValue::String(text)
            if literal && !text.chars().any(|c| c == '\'' || c.is_control()) =>
        {
            format!("'{text}'")
        }
        other => {
            let mut converted = to_toml(path, other)?;
            converted.decor_mut().clear();
            converted.to_string()
        }
    };
    Ok(Rendered {
        text,
        expected: value.clone(),
    })
}
