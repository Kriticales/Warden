//! Leitura e escrita de cada formato.
//!
//! Todos os formatos seguem o mesmo contrato: o parser devolve as entradas com a faixa de bytes
//! de cada valor no corpo do arquivo e um [`EditStyle`] com o que é preciso para escrever um
//! valor novo no mesmo estilo (aspas, escapes, prefixo de tipo). Uma edição troca só esses bytes;
//! o resto do arquivo nunca é regravado.

use std::ops::Range;

use crate::error::Result;
use crate::format::ConfigFormat;
use crate::path::KeyPath;
use crate::text::DocText;
use crate::tree::{CfgType, ConfigEntry, ConfigValue, EntryKind, TextSpan};

pub(crate) mod json;
pub(crate) mod legacy_cfg;
pub(crate) mod options_txt;
pub(crate) mod properties;
pub(crate) mod toml;

/// Como escrever um valor novo na mesma posição, no estilo do arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditStyle {
    /// Seção: não tem valor próprio para editar.
    Section,
    /// Valor TOML; `literal` quando o texto estava entre aspas simples.
    Toml { literal: bool },
    /// Valor JSON/JSON5; `single_quote` quando o texto estava entre aspas simples (JSON5).
    Json { single_quote: bool },
    /// Valor de `.properties`.
    Properties {
        /// A linha só tinha a chave: um valor novo precisa de `=` antes.
        needs_separator: bool,
        /// O arquivo é só ASCII: caracteres de fora viram `\uXXXX`, como o `Properties.store`.
        escape_unicode: bool,
    },
    /// Valor de `options.txt`.
    Options(OptionsStyle),
    /// Propriedade simples do `.cfg`.
    Cfg { ty: CfgType },
    /// Lista do `.cfg` (`S:nome <` … `>`): a faixa do valor são as linhas dos itens.
    CfgList { ty: CfgType, item_indent: String },
}

/// Como o `options.txt` grava um valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OptionsStyle {
    /// Texto cru (`renderDistance:12`, `lang:en_us`).
    Raw,
    /// Texto em JSON entre aspas (`soundDevice:""`, versões novas).
    Quoted,
    /// Lista em JSON (`resourcePacks:["vanilla"]`).
    JsonList,
}

/// Resultado de um parser: entradas com faixas relativas ao corpo e o estilo de cada uma.
#[derive(Debug, Default)]
pub(crate) struct Parsed {
    pub entries: Vec<ConfigEntry>,
    pub styles: Vec<EditStyle>,
}

/// Monta um [`Parsed`] com as linhas e comentários do documento.
pub(crate) struct Builder<'d, 'a> {
    pub doc: &'d DocText<'a>,
    pub parsed: Parsed,
}

/// Dados de uma entrada com valor.
pub(crate) struct ValueEntry {
    pub path: KeyPath,
    pub value: ConfigValue,
    pub line_index: usize,
    pub span: Range<usize>,
    pub style: EditStyle,
    pub comment: Option<String>,
    pub cfg_type: Option<CfgType>,
}

impl<'d, 'a> Builder<'d, 'a> {
    pub(crate) fn new(doc: &'d DocText<'a>) -> Self {
        Self {
            doc,
            parsed: Parsed::default(),
        }
    }

    pub(crate) fn section(&mut self, path: KeyPath, line_index: usize, comment: Option<String>) {
        self.parsed.entries.push(ConfigEntry {
            path,
            kind: EntryKind::Section,
            value: None,
            line: line_index + 1,
            value_span: None,
            comment,
            range: None,
            allowed_values: None,
            cfg_type: None,
        });
        self.parsed.styles.push(EditStyle::Section);
    }

    pub(crate) fn value(&mut self, entry: ValueEntry) {
        self.parsed.entries.push(ConfigEntry {
            path: entry.path,
            kind: EntryKind::Value,
            value: Some(entry.value),
            line: entry.line_index + 1,
            value_span: Some(TextSpan::new(entry.span.start, entry.span.end)),
            comment: entry.comment,
            range: None,
            allowed_values: None,
            cfg_type: entry.cfg_type,
        });
        self.parsed.styles.push(entry.style);
    }

    pub(crate) fn finish(self) -> Parsed {
        self.parsed
    }
}

/// Lê o corpo no formato pedido.
pub(crate) fn parse(format: ConfigFormat, doc: &DocText<'_>) -> Result<Parsed> {
    match format {
        ConfigFormat::Toml => toml::parse(doc),
        ConfigFormat::Json | ConfigFormat::Json5 => json::parse(format, doc),
        ConfigFormat::Properties => properties::parse(doc),
        ConfigFormat::LegacyCfg => legacy_cfg::parse(doc),
        ConfigFormat::OptionsTxt => Ok(options_txt::parse(doc)),
    }
}

/// Texto novo de um valor e o valor que a releitura deve encontrar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rendered {
    pub text: String,
    pub expected: ConfigValue,
}

/// Escreve `value` no estilo de `style`. `raw` é o texto atual do valor (para manter detalhes
/// como o tipo de aspas) e `line_ending`, o fim de linha do arquivo.
pub(crate) fn render(
    format: ConfigFormat,
    style: &EditStyle,
    path: &KeyPath,
    value: &ConfigValue,
    line_ending: &str,
) -> Result<Rendered> {
    match style {
        EditStyle::Section => Err(crate::ConfigError::NotEditable {
            path: path.clone(),
            reason: "é uma seção; edite as chaves dentro dela".into(),
        }),
        EditStyle::Toml { literal } => toml::render(path, value, *literal),
        EditStyle::Json { single_quote } => json::render(format, path, value, *single_quote),
        EditStyle::Properties {
            needs_separator,
            escape_unicode,
        } => properties::render(path, value, *needs_separator, *escape_unicode),
        EditStyle::Options(style) => options_txt::render(path, value, *style),
        EditStyle::Cfg { ty } => legacy_cfg::render_scalar(path, value, *ty),
        EditStyle::CfgList { ty, item_indent } => {
            legacy_cfg::render_list(path, value, *ty, item_indent, line_ending)
        }
    }
}

/// Erro de valor inválido.
pub(crate) fn invalid(path: &KeyPath, reason: impl Into<String>) -> crate::ConfigError {
    crate::ConfigError::InvalidValue {
        path: path.clone(),
        reason: reason.into(),
    }
}
