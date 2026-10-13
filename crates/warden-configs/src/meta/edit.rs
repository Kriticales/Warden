//! Edição pelo formulário (C-04; CA-T12-02): só os bytes do valor mudam, e o mínimo deles.
//!
//! - Valor igual ao atual (mesmo que escrito de outro jeito, como `0.2` para `0.20`): a linha
//!   não é tocada.
//! - Número: grava exatamente o que a pessoa digitou, se o formato aceita assim e a releitura dá
//!   o número pedido; senão, a forma normal do número (`5` vira `5.0` numa chave decimal do TOML).
//! - Lista: item por item ([`super::list`]). Trocar um item reescreve só ele (com as mesmas
//!   aspas); acrescentar cria a linha no estilo dos vizinhos; tirar apaga a linha do item. O
//!   resto da lista (uma linha ou várias, indentação, comentários) fica como estava.
//! - O resto (texto, liga/desliga, data): pelo [`ConfigDocument::apply`], no estilo da chave.
//!
//! Toda edição é conferida: o texto novo é relido e só a chave pedida pode ter mudado, para o
//! valor pedido; e os bytes fora do valor são comparados com os originais. Se algo não bater,
//! nada é devolvido ([`ConfigError::EditNotConfirmed`]) e a interface cai para o editor de
//! texto.

use std::collections::HashSet;

use super::list::{self, ListChange, ListShape, NewItemStyle, Splice};
use super::{
    FormEdit, FormValue, ListItemKind, detect, entry_list_kind, from_keys, list_kind,
    same_name_list,
};
use crate::compare::{SemanticChange, diff_trees};
use crate::document::{ConfigDocument, ConfigEdit};
use crate::error::{ConfigError, Result};
use crate::format::ConfigFormat;
use crate::formats;
use crate::path::KeyPath;
use crate::tree::{CfgType, ConfigEntry, ConfigValue, TextSpan};

fn invalid(path: &KeyPath, reason: impl Into<String>) -> ConfigError {
    ConfigError::InvalidValue {
        path: path.clone(),
        reason: reason.into(),
    }
}

fn not_confirmed(detail: impl Into<String>) -> ConfigError {
    ConfigError::EditNotConfirmed {
        detail: detail.into(),
    }
}

fn parse_integer(path: &KeyPath, text: &str) -> Result<i64> {
    text.trim()
        .parse::<i64>()
        .map_err(|_| invalid(path, format!("{text:?} não é um número inteiro")))
}

fn parse_float(path: &KeyPath, text: &str) -> Result<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| invalid(path, format!("{text:?} não é um número")))
}

fn parse_bool(path: &KeyPath, text: &str) -> Result<bool> {
    match text.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(invalid(path, format!("{text:?} não é true nem false"))),
    }
}

/// Texto do valor do formulário, para guardar como texto.
fn form_text(value: &FormValue) -> Option<String> {
    match value {
        FormValue::Bool(flag) => Some(flag.to_string()),
        FormValue::Integer(text)
        | FormValue::Float(text)
        | FormValue::String(text)
        | FormValue::Datetime(text) => Some(text.clone()),
        FormValue::List(_) => None,
    }
}

/// Converte o texto de um item para o tipo do item atual.
pub(super) fn item_value(path: &KeyPath, kind: ListItemKind, text: &str) -> Result<ConfigValue> {
    Ok(match kind {
        ListItemKind::Text => ConfigValue::String(text.to_owned()),
        ListItemKind::Integer => ConfigValue::Integer(parse_integer(path, text)?),
        ListItemKind::Float => ConfigValue::Float(parse_float(path, text)?),
        ListItemKind::Bool => ConfigValue::Bool(parse_bool(path, text)?),
    })
}

/// O valor do formulário no tipo que a chave tem no arquivo: o tipo nunca muda pela edição.
pub(super) fn to_config_value(
    path: &KeyPath,
    current: &ConfigValue,
    value: &FormValue,
) -> Result<ConfigValue> {
    let text = || form_text(value).ok_or_else(|| invalid(path, "a chave não é uma lista"));
    Ok(match current {
        ConfigValue::Bool(_) => match value {
            FormValue::Bool(flag) => ConfigValue::Bool(*flag),
            other => ConfigValue::Bool(parse_bool(path, &form_text(other).unwrap_or_default())?),
        },
        ConfigValue::Integer(_) => ConfigValue::Integer(parse_integer(path, &text()?)?),
        ConfigValue::Float(_) => ConfigValue::Float(parse_float(path, &text()?)?),
        ConfigValue::String(_) => ConfigValue::String(text()?),
        ConfigValue::Datetime(_) => ConfigValue::Datetime(text()?),
        ConfigValue::List(items) => {
            let FormValue::List(new_items) = value else {
                return Err(invalid(path, "a chave é uma lista"));
            };
            let kind = list_kind(items)
                .ok_or_else(|| invalid(path, "lista com itens de tipos diferentes"))?;
            ConfigValue::List(
                new_items
                    .iter()
                    .map(|item| item_value(path, kind, item))
                    .collect::<Result<_>>()?,
            )
        }
        ConfigValue::Null => return Err(invalid(path, "valor nulo só se edita como texto")),
    })
}

/// Aplica as edições do formulário ao texto do arquivo `path` e devolve o texto novo.
///
/// # Errors
///
/// Os de [`ConfigDocument::parse`] e [`ConfigDocument::apply`];
/// [`ConfigError::InvalidValue`] para valor que não serve ao tipo da chave;
/// [`ConfigError::DuplicateEdit`] para a mesma chave duas vezes; e
/// [`ConfigError::EditNotConfirmed`] quando a releitura não confirma a mudança.
pub fn apply_form_edits(path: &str, text: &str, edits: &[FormEdit]) -> Result<String> {
    let format =
        detect(path, text.as_bytes()).map_err(|_| not_confirmed("o arquivo não tem formulário"))?;
    let mut seen = HashSet::new();
    for edit in edits {
        if !seen.insert(&edit.path) {
            return Err(ConfigError::DuplicateEdit {
                path: from_keys(&edit.path),
            });
        }
    }
    let mut current = text.to_owned();
    for edit in edits {
        current = apply_one(format, &current, edit, true)?;
    }
    Ok(current)
}

/// Uma edição: relê o texto (as posições mudam a cada edição) e troca o valor da chave. Com
/// `in_steps`, uma lista com itens iguais no meio muda um trecho de cada vez.
fn apply_one(format: ConfigFormat, text: &str, edit: &FormEdit, in_steps: bool) -> Result<String> {
    let document = ConfigDocument::parse(format, text.as_bytes())?;
    let key = from_keys(&edit.path);
    let entry = document
        .get(&key)
        .ok_or_else(|| ConfigError::PathNotFound { path: key.clone() })?;
    let not_editable = || ConfigError::NotEditable {
        path: key.clone(),
        reason: "é uma seção; edite as chaves dentro dela".into(),
    };
    let current = entry.value.as_ref().ok_or_else(not_editable)?;
    let span = entry.value_span.ok_or_else(not_editable)?;
    let edited = Edit {
        document: &document,
        text,
        entry,
        key: &key,
        span,
        in_steps,
    };
    let out = match current {
        ConfigValue::List(items) => edited.list(items, &edit.value)?,
        _ => edited.scalar(current, &edit.value)?,
    };
    // Nada fora da faixa do valor pode ter mudado.
    let tail = text.len().saturating_sub(span.end);
    let same_head = out.get(..span.start) == text.get(..span.start);
    let same_tail = out.len() >= tail && out.get(out.len() - tail..) == text.get(span.end..);
    if !(same_head && same_tail) {
        return Err(not_confirmed("bytes fora do valor mudariam"));
    }
    Ok(out)
}

struct Edit<'e> {
    document: &'e ConfigDocument,
    text: &'e str,
    entry: &'e ConfigEntry,
    key: &'e KeyPath,
    span: TextSpan,
    /// Lista com itens iguais no meio: um trecho de cada vez ([`Edit::steps`]).
    in_steps: bool,
}

impl Edit<'_> {
    fn format(&self) -> ConfigFormat {
        self.document.format()
    }

    /// Aplica as trocas e confere pela releitura que só esta chave mudou, para `expected`.
    fn verified(&self, splices: Vec<Splice>, expected: &ConfigValue) -> Option<String> {
        let out = list::apply(self.text, splices)?;
        let after = ConfigDocument::parse(self.format(), out.as_bytes()).ok()?;
        let changes = diff_trees(self.document.tree(), after.tree());
        let [SemanticChange::Changed { after: value, .. }] = changes.as_slice() else {
            return None;
        };
        (changes[0].path() == self.key && value.same_as(expected)).then_some(out)
    }

    fn scalar(&self, current: &ConfigValue, value: &FormValue) -> Result<String> {
        let wanted = to_config_value(self.key, current, value)?;
        if wanted.same_as(current) {
            return Ok(self.text.to_owned());
        }
        // Número: exatamente o que foi digitado, se o formato aceita assim.
        if matches!(current, ConfigValue::Integer(_) | ConfigValue::Float(_))
            && let Some(typed) = form_text(value)
            && let Some(out) = self.verified(
                vec![Splice {
                    start: self.span.start,
                    end: self.span.end,
                    text: typed.trim().to_owned(),
                }],
                &wanted,
            )
        {
            return Ok(out);
        }
        let edited = self
            .document
            .apply(&[ConfigEdit::new(self.key.clone(), wanted.clone())])?;
        let out = String::from_utf8(edited.into_bytes())
            .map_err(|error| not_confirmed(error.to_string()))?;
        // Os escapes que o arquivo usa sem precisar (`'` do Gson, `\:` do Java).
        let tail = self.text.len().saturating_sub(self.span.end);
        let rendered = out
            .get(self.span.start..out.len().saturating_sub(tail))
            .unwrap_or_default();
        let restyled = self.restyle(rendered);
        if restyled != rendered
            && let Some(better) = self.verified(
                vec![Splice {
                    start: self.span.start,
                    end: self.span.end,
                    text: restyled,
                }],
                &wanted,
            )
        {
            return Ok(better);
        }
        Ok(out)
    }

    fn list(&self, items: &[ConfigValue], value: &FormValue) -> Result<String> {
        let current = ConfigValue::List(items.to_vec());
        let FormValue::List(typed) = value else {
            return Err(invalid(self.key, "a chave é uma lista"));
        };
        let kind = entry_list_kind(self.document.tree(), self.entry)
            .ok_or_else(|| invalid(self.key, "lista com itens de tipos diferentes"))?;
        let wanted = typed
            .iter()
            .map(|item| item_value(self.key, kind, item))
            .collect::<Result<Vec<_>>>()?;
        if ConfigValue::List(wanted.clone()).same_as(&current) {
            return Ok(self.text.to_owned());
        }
        let layout = list::read(self.text, self.format(), self.span)
            .filter(|layout| layout.items.len() == items.len())
            .ok_or_else(|| not_confirmed("a lista não pôde ser lida item por item"))?;

        // Os itens iguais do começo e do fim ficam; os do meio trocam, entram ou saem.
        let old = items.len();
        let new = wanted.len();
        let head = items
            .iter()
            .zip(&wanted)
            .take_while(|(a, b)| a.same_as(b))
            .count();
        let room = old.min(new) - head;
        let tail = items
            .iter()
            .rev()
            .zip(wanted.iter().rev())
            .take(room)
            .take_while(|(a, b)| a.same_as(b))
            .count();
        // Itens iguais também no meio (tirar o primeiro e o terceiro de uma vez): uma mudança
        // por trecho, para cada item que fica manter a própria linha e o próprio comentário.
        if let Some(steps) = self.steps(items, &wanted, typed, &layout, head + tail) {
            return steps;
        }
        let replaced = (old - head - tail).min(new - head - tail);
        let removed = old - head - tail - replaced;

        let raw = |index: usize| {
            layout
                .items
                .get(index)
                .and_then(|item| self.text.get(item.start..item.end))
        };
        // Aspas de cada item novo: as do item que ele substitui ou as do vizinho.
        let neighbour = raw((head + replaced).saturating_sub(1)).or_else(|| raw(0));
        let render = |exact_numbers: bool| -> Result<Vec<String>> {
            (head..new - tail)
                .map(|index| {
                    let model = if index < head + replaced {
                        raw(index)
                    } else {
                        neighbour
                    };
                    let item = wanted.get(index).ok_or_else(|| not_confirmed("item"))?;
                    let typed = typed.get(index).map_or("", String::as_str);
                    self.render_item(item, model, typed, exact_numbers)
                })
                .collect()
        };
        let indent = self.empty_indent(&layout.shape);
        let style = NewItemStyle {
            line_ending: &self.document.tree().line_ending,
            empty_indent: &indent,
            separator: if self.format() == ConfigFormat::OptionsTxt {
                ","
            } else {
                ", "
            },
        };
        let attempt = |texts: &[String]| {
            let splices = list::plan(
                self.text,
                &layout,
                &ListChange {
                    keep_head: head,
                    replaced: texts.get(..replaced).unwrap_or(&[]),
                    inserted: texts.get(replaced..).unwrap_or(&[]),
                    removed,
                },
                &style,
            );
            self.verified(splices, &ConfigValue::List(wanted.clone()))
        };
        if let Some(out) = attempt(&render(true)?) {
            return Ok(out);
        }
        attempt(&render(false)?).ok_or_else(|| {
            not_confirmed("a releitura da lista não confirmou a mudança item por item")
        })
    }

    /// A mudança da lista em passos, um trecho contíguo por vez, quando a maior sequência de
    /// itens iguais (nas duas listas, na mesma ordem) passa de `kept`, os iguais do começo e do
    /// fim. `None` quando um passo só basta.
    fn steps(
        &self,
        items: &[ConfigValue],
        wanted: &[ConfigValue],
        typed: &[String],
        layout: &list::ListLayout,
        kept: usize,
    ) -> Option<Result<String>> {
        if !self.in_steps {
            return None;
        }
        let (old, new) = (items.len(), wanted.len());
        // `common[i][j]`: a maior sequência comum entre `items[i..]` e `wanted[j..]`.
        let mut common = vec![vec![0_usize; new + 1]; old + 1];
        for i in (0..old).rev() {
            for j in (0..new).rev() {
                common[i][j] = if items[i].same_as(&wanted[j]) {
                    common[i + 1][j + 1] + 1
                } else {
                    common[i + 1][j].max(common[i][j + 1])
                };
            }
        }
        if common[0][0] <= kept {
            return None;
        }
        // Os trechos entre os itens que ficam: (início, fim) na lista antiga e na nova.
        let mut gaps = Vec::new();
        let (mut i, mut j, mut from) = (0, 0, (0, 0));
        while i < old && j < new {
            if items[i].same_as(&wanted[j]) && common[i][j] == common[i + 1][j + 1] + 1 {
                gaps.push((from.0..i, from.1..j));
                i += 1;
                j += 1;
                from = (i, j);
            } else if common[i + 1][j] >= common[i][j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
        gaps.push((from.0..old, from.1..new));
        // A lista de agora como a pessoa a escreveria: o texto de cada item no arquivo.
        let mut current: Vec<String> = items
            .iter()
            .zip(&layout.items)
            .map(|(item, span)| super::plain_text(item, self.text.get(span.start..span.end)))
            .collect();
        let mut text = self.text.to_owned();
        // Do fim para o começo: as posições dos trechos anteriores não mudam.
        for (before, after) in gaps.into_iter().rev() {
            if before.is_empty() && after.is_empty() {
                continue;
            }
            current.splice(before, typed.get(after).unwrap_or(&[]).iter().cloned());
            let step = FormEdit {
                path: super::to_keys(self.key),
                value: FormValue::List(current.clone()),
            };
            text = match apply_one(self.format(), &text, &step, false) {
                Ok(text) => text,
                Err(error) => return Some(Err(error)),
            };
        }
        Some(Ok(text))
    }

    /// Texto de um item novo, no estilo de `model` (o item que ele substitui ou um vizinho).
    fn render_item(
        &self,
        value: &ConfigValue,
        model: Option<&str>,
        typed: &str,
        exact_numbers: bool,
    ) -> Result<String> {
        if exact_numbers && matches!(value, ConfigValue::Integer(_) | ConfigValue::Float(_)) {
            return Ok(typed.trim().to_owned());
        }
        let quoted_with = |quote: char| model.is_some_and(|raw| raw.starts_with(quote));
        let key = self.key;
        let text = match self.format() {
            ConfigFormat::Toml => {
                let literal = quoted_with('\'') && !model.is_some_and(|raw| raw.starts_with("'''"));
                formats::toml::render(key, value, literal)?.text
            }
            format @ (ConfigFormat::Json | ConfigFormat::Json5) => {
                formats::json::render(format, key, value, quoted_with('\''))?.text
            }
            ConfigFormat::OptionsTxt => {
                formats::json::render(ConfigFormat::Json, key, value, false)?.text
            }
            ConfigFormat::LegacyCfg => {
                let ty = self.entry.cfg_type.unwrap_or(CfgType::String);
                let line = formats::legacy_cfg::render_list(
                    key,
                    &ConfigValue::List(vec![value.clone()]),
                    ty,
                    "",
                    "\n",
                )?;
                line.text.trim_end_matches('\n').to_owned()
            }
            ConfigFormat::Properties => {
                return Err(invalid(key, ".properties não tem listas"));
            }
        };
        Ok(self.restyle(&text))
    }

    /// Indentação completa de um item novo numa lista vazia: a dos itens de outra lista do
    /// arquivo com o mesmo nome ou com a chave na mesma indentação; sem modelo, no `.cfg` a da
    /// chave mais quatro espaços (como o Forge) e entre colchetes a do `]` mais um nível do
    /// arquivo.
    fn empty_indent(&self, shape: &ListShape) -> String {
        let tree = self.document.tree();
        let key_indent = |entry: &ConfigEntry| line_indent(self.text, entry.line);
        let ours = key_indent(self.entry);
        let item_indent = |entry: &ConfigEntry| {
            let span = entry.value_span?;
            let layout = list::read(self.text, self.format(), span)?;
            let item = layout.items.iter().find(|item| item.own_line)?;
            Some(list::indent_of(self.text, item.start))
        };
        let same_name = same_name_list(tree, self.entry).and_then(item_indent);
        let same_level = || {
            tree.values()
                .filter(|other| other.path != self.entry.path && key_indent(other) == ours)
                .filter(|other| matches!(other.value, Some(ConfigValue::List(_))))
                .find_map(item_indent)
        };
        if let Some(indent) = same_name.or_else(same_level) {
            return indent;
        }
        match shape {
            ListShape::CfgLines { .. } => format!("{ours}    "),
            ListShape::Brackets { close, .. } => {
                format!(
                    "{}{}",
                    list::indent_of(self.text, *close),
                    indent_unit(self.text)
                )
            }
        }
    }

    /// Escapes que o arquivo usa sem precisar, aplicados ao texto novo para que voltar ao
    /// valor antigo devolva os mesmos bytes: o Gson do Minecraft escreve `'`, `&`, `<`, `>` e
    /// `=` como `\u0027`…; o `Properties.store` do Java escreve `:`, `=`, `#` e `!` com `\`.
    fn restyle(&self, rendered: &str) -> String {
        match self.format() {
            ConfigFormat::Json | ConfigFormat::Json5 | ConfigFormat::OptionsTxt
                if rendered.starts_with('"') && gson_html_safe(self.text) =>
            {
                rendered
                    .chars()
                    .map(|c| match c {
                        '\'' => "\\u0027".to_owned(),
                        '&' => "\\u0026".to_owned(),
                        '<' => "\\u003c".to_owned(),
                        '>' => "\\u003e".to_owned(),
                        '=' => "\\u003d".to_owned(),
                        other => other.to_string(),
                    })
                    .collect()
            }
            ConfigFormat::Properties if properties_store(self.text) => {
                let mut out = String::with_capacity(rendered.len() + 4);
                let mut chars = rendered.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => {
                            out.push(c);
                            out.extend(chars.next());
                        }
                        ':' | '=' | '#' | '!' => {
                            out.push('\\');
                            out.push(c);
                        }
                        other => out.push(other),
                    }
                }
                out
            }
            _ => rendered.to_owned(),
        }
    }
}

/// Espaços e tabs no começo da linha `line` (começa em 1).
fn line_indent(text: &str, line: usize) -> String {
    text.split('\n')
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

/// Um nível de indentação do arquivo: o começo da primeira linha indentada (um tab ou os
/// espaços dela); sem nenhuma, quatro espaços.
fn indent_unit(text: &str) -> String {
    text.split('\n')
        .filter(|line| !line.trim().is_empty())
        .find_map(|line| {
            let indent: String = line
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            match indent.chars().next() {
                None => None,
                Some('\t') => Some("\t".to_owned()),
                Some(_) => Some(indent),
            }
        })
        .unwrap_or_else(|| "    ".to_owned())
}

/// O arquivo foi escrito pelo Gson com os escapes de HTML (o padrão dele): `\u0027` e afins.
fn gson_html_safe(text: &str) -> bool {
    ["\\u0027", "\\u0026", "\\u003c", "\\u003e", "\\u003d"]
        .iter()
        .any(|escape| text.contains(escape))
}

/// O arquivo foi escrito pelo `Properties.store` do Java: tem `\:` ou `\=`, ou a linha de data
/// que ele põe no topo (`#Thu Aug 07 11:12:14 BST 2025`).
fn properties_store(text: &str) -> bool {
    text.contains("\\:")
        || text.contains("\\=")
        || text.lines().take(3).any(|line| {
            let words: Vec<&str> = line.trim_start_matches('#').split(' ').collect();
            line.starts_with('#')
                && words.len() == 6
                && words[3].matches(':').count() == 2
                && words[5].trim().parse::<u16>().is_ok()
        })
}
