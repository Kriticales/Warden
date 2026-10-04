//! Escrita de TOML byte a byte igual à do packwiz.
//!
//! O packwiz grava `pack.toml`, `index.toml` e os metafiles com o codificador da biblioteca
//! Go `github.com/BurntSushi/toml` v1.5.0 (MIT), com `Indent = ""`. Este módulo porta as regras
//! dele que importam para o formato (arquivo `encode.go` daquela versão):
//!
//! - dentro de cada tabela, primeiro as chaves simples e depois as subtabelas; nas structs do
//!   Go a ordem é a da declaração, nos mapas é a alfabética (por bytes);
//! - uma linha em branco antes de cada tabela de primeiro nível e de cada `[[lista]]`, e nenhuma
//!   antes de subtabelas (`[update]` seguido direto de `[update.modrinth]`);
//! - textos sempre entre aspas duplas, com os escapes da tabela `dblQuotedReplacer`;
//! - chaves sem aspas só com `A-Za-z0-9_-`;
//! - decimais no formato mais curto, sem expoente, sempre com ponto (`1.0`);
//! - listas de valores numa linha (`["a", "b"]`) e listas não vazias só de tabelas como
//!   `[[chave]]`; lista vazia vira `[]`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::value::Value;

/// Nó a gravar: um valor livre ou uma struct do Go (campos em ordem fixa).
#[derive(Debug, Clone)]
pub(crate) enum Node {
    /// Valor livre; tabelas são mapas (chaves em ordem alfabética).
    Value(Value),
    /// Struct: campos na ordem da declaração no packwiz (os omitidos já ficaram de fora).
    Struct(Vec<(&'static str, Node)>),
    /// Lista de structs (as entradas `[[files]]` do índice).
    StructArray(Vec<Node>),
}

impl Node {
    fn is_table(&self) -> bool {
        match self {
            Self::Struct(_) => true,
            Self::Value(value) => matches!(value, Value::Table(_)) || value.is_array_of_tables(),
            Self::StructArray(items) => !items.is_empty(),
        }
    }
}

/// Grava uma struct de primeiro nível (o documento inteiro).
pub(crate) fn encode_document(fields: Vec<(&'static str, Node)>) -> String {
    let mut encoder = Encoder::default();
    encoder.struct_body(&[], fields);
    encoder.out
}

#[derive(Default)]
struct Encoder {
    out: String,
    has_written: bool,
}

impl Encoder {
    fn write(&mut self, text: &str) {
        self.out.push_str(text);
        self.has_written = true;
    }

    fn newline(&mut self) {
        if self.has_written {
            self.write("\n");
        }
    }

    fn encode(&mut self, key: &[String], node: Node) {
        match node {
            Node::Struct(fields) => {
                self.table_header(key);
                self.struct_body(key, fields);
            }
            Node::StructArray(items) if !items.is_empty() => {
                for item in items {
                    self.array_table_header(key);
                    match item {
                        Node::Struct(fields) => self.struct_body(key, fields),
                        Node::Value(Value::Table(map)) => self.map_body(key, map),
                        // Lista de structs só contém structs (garantido pelos chamadores).
                        Node::Value(_) | Node::StructArray(_) => {}
                    }
                }
            }
            Node::StructArray(_) => self.key_value(key, &Value::Array(Vec::new())),
            Node::Value(Value::Table(map)) => {
                self.table_header(key);
                self.map_body(key, map);
            }
            Node::Value(value) if value.is_array_of_tables() => {
                if let Value::Array(items) = value {
                    for item in items {
                        self.array_table_header(key);
                        if let Value::Table(map) = item {
                            self.map_body(key, map);
                        }
                    }
                }
            }
            Node::Value(value) => self.key_value(key, &value),
        }
    }

    fn table_header(&mut self, key: &[String]) {
        if key.len() == 1 {
            // Linha em branco entre tabelas de primeiro nível (nenhuma no começo do arquivo).
            self.newline();
        }
        if !key.is_empty() {
            let header = format!("[{}]", key_path(key));
            self.write(&header);
            self.newline();
        }
    }

    fn array_table_header(&mut self, key: &[String]) {
        self.newline();
        let header = format!("[[{}]]", key_path(key));
        self.write(&header);
        self.newline();
    }

    fn struct_body(&mut self, key: &[String], fields: Vec<(&'static str, Node)>) {
        let (sub, direct): (Vec<_>, Vec<_>) = fields.into_iter().partition(|(_, n)| n.is_table());
        for (name, node) in direct.into_iter().chain(sub) {
            self.encode(&child(key, name), node);
        }
    }

    fn map_body(&mut self, key: &[String], map: BTreeMap<String, Value>) {
        let (sub, direct): (Vec<_>, Vec<_>) = map
            .into_iter()
            .partition(|(_, value)| Node::Value(value.clone()).is_table());
        for (name, value) in direct.into_iter().chain(sub) {
            self.encode(&child(key, &name), Node::Value(value));
        }
    }

    fn key_value(&mut self, key: &[String], value: &Value) {
        let Some(last) = key.last() else {
            return;
        };
        let mut line = maybe_quoted(last);
        line.push_str(" = ");
        write_element(&mut line, value);
        self.write(&line);
        self.newline();
    }
}

fn child(key: &[String], name: &str) -> Vec<String> {
    let mut path = key.to_vec();
    path.push(name.to_owned());
    path
}

fn is_bare_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Chave como o packwiz a grava: sem aspas se for "nua", senão entre aspas duplas.
pub(crate) fn maybe_quoted(key: &str) -> String {
    if is_bare_key(key) {
        key.to_owned()
    } else {
        quoted(key)
    }
}

fn key_path(key: &[String]) -> String {
    key.iter()
        .map(|part| maybe_quoted(part))
        .collect::<Vec<_>>()
        .join(".")
}

/// Texto entre aspas duplas com os escapes do `dblQuotedReplacer` do BurntSushi/toml.
pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if u32::from(c) < 0x20 || c == '\u{7f}' => {
                // Escrever numa `String` não falha.
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Decimal como o Go o grava (`strconv.FormatFloat(f, 'f', -1, 64)` + ".0" se faltar o ponto).
pub(crate) fn format_float(number: f64) -> String {
    if number.is_nan() {
        return if number.is_sign_negative() {
            "-nan"
        } else {
            "nan"
        }
        .to_owned();
    }
    if number.is_infinite() {
        return if number.is_sign_negative() {
            "-inf"
        } else {
            "inf"
        }
        .to_owned();
    }
    // O `Display` do Rust também dá a menor forma que volta ao mesmo número, sem expoente.
    let text = number.to_string();
    if text.contains('.') {
        text
    } else {
        text + ".0"
    }
}

fn write_element(out: &mut String, value: &Value) {
    match value {
        Value::String(text) => out.push_str(&quoted(text)),
        Value::Integer(number) => out.push_str(&number.to_string()),
        Value::Float(number) => out.push_str(&format_float(*number)),
        Value::Boolean(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Datetime(text) => out.push_str(text),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_element(out, item);
            }
            out.push(']');
        }
        Value::Table(map) => write_inline_table(out, map),
    }
}

fn write_inline_table(out: &mut String, map: &BTreeMap<String, Value>) {
    let (sub, direct): (Vec<_>, Vec<_>) = map
        .iter()
        .partition(|(_, value)| matches!(value, Value::Table(_)) || value.is_array_of_tables());
    out.push('{');
    let trailing_comma = !sub.is_empty();
    for (index, (name, value)) in direct.iter().enumerate() {
        inline_pair(out, name, value);
        if trailing_comma || index + 1 != direct.len() {
            out.push_str(", ");
        }
    }
    for (index, (name, value)) in sub.iter().enumerate() {
        inline_pair(out, name, value);
        if index + 1 != sub.len() {
            out.push_str(", ");
        }
    }
    out.push('}');
}

fn inline_pair(out: &mut String, name: &str, value: &Value) {
    out.push_str(&maybe_quoted(name));
    out.push_str(" = ");
    write_element(out, value);
}

/// Atalho para campos de texto.
pub(crate) fn text(value: &str) -> Node {
    Node::Value(Value::String(value.to_owned()))
}

/// Atalho para campos lógicos.
pub(crate) fn boolean(value: bool) -> Node {
    Node::Value(Value::Boolean(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(pairs: &[(&str, Value)]) -> Value {
        Value::Table(
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect(),
        )
    }

    #[test]
    fn aspas_e_escapes_como_no_go() {
        assert_eq!(quoted("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(quoted("\u{8}\t\n\u{c}\r"), r#""\b\t\n\f\r""#);
        assert_eq!(quoted("\u{0}\u{1b}\u{7f}"), r#""\u0000\u001b\u007f""#);
        assert_eq!(quoted("Jade 🔍 ç"), "\"Jade 🔍 ç\"");
    }

    #[test]
    fn chaves_nuas_e_com_aspas() {
        assert_eq!(maybe_quoted("mod-id"), "mod-id");
        assert_eq!(maybe_quoted("a_B9"), "a_B9");
        assert_eq!(maybe_quoted("a.b"), "\"a.b\"");
        assert_eq!(maybe_quoted(""), "\"\"");
        assert_eq!(maybe_quoted("ç"), "\"ç\"");
    }

    #[test]
    fn decimais_como_no_go() {
        assert_eq!(format_float(1.0), "1.0");
        assert_eq!(format_float(-0.0), "-0.0");
        assert_eq!(format_float(0.1), "0.1");
        assert_eq!(format_float(1e21), "1000000000000000000000.0");
        assert_eq!(format_float(1.5e-7), "0.00000015");
        assert_eq!(format_float(f64::NAN), "nan");
        assert_eq!(format_float(-f64::NAN), "-nan");
        assert_eq!(format_float(f64::INFINITY), "inf");
        assert_eq!(format_float(f64::NEG_INFINITY), "-inf");
    }

    #[test]
    fn chaves_simples_antes_das_tabelas_e_linhas_em_branco() {
        let doc = encode_document(vec![
            (
                "options",
                Node::Value(table(&[
                    ("z", Value::from(1_i64)),
                    ("sub", table(&[("k", Value::from("v"))])),
                    (
                        "a",
                        Value::Array(vec![Value::from("x"), Value::from(2_i64)]),
                    ),
                ])),
            ),
            ("name", text("T")),
            ("flag", boolean(true)),
        ]);
        assert_eq!(
            doc,
            "name = \"T\"\nflag = true\n\n[options]\na = [\"x\", 2]\nz = 1\n[options.sub]\nk = \"v\"\n"
        );
    }

    #[test]
    fn listas_de_tabelas_e_tabelas_em_linha() {
        let doc = encode_document(vec![
            (
                "files",
                Node::StructArray(vec![
                    Node::Struct(vec![("file", text("a"))]),
                    Node::Value(table(&[("file", Value::from("b"))])),
                ]),
            ),
            (
                "list",
                Node::Value(Value::Array(vec![
                    table(&[("x", Value::from(1_i64))]),
                    table(&[("y", Value::from(2_i64))]),
                ])),
            ),
            (
                "mixed",
                Node::Value(Value::Array(vec![
                    table(&[
                        ("s", table(&[("t", Value::from(true))])),
                        ("u", Value::from(1_i64)),
                        ("v", Value::from(2_i64)),
                    ]),
                    Value::from(3_i64),
                ])),
            ),
            ("empty", Node::StructArray(vec![])),
        ]);
        assert_eq!(
            doc,
            "mixed = [{u = 1, v = 2, s = {t = true}}, 3]\nempty = []\n\n[[files]]\nfile = \"a\"\n\n\
             [[files]]\nfile = \"b\"\n\n[[list]]\nx = 1\n\n[[list]]\ny = 2\n"
        );
    }

    #[test]
    fn tabela_vazia_e_tabela_aninhada_com_aspas() {
        let doc = encode_document(vec![
            ("versions", Node::Value(Value::Table(BTreeMap::new()))),
            (
                "export",
                Node::Value(table(&[("a b", table(&[("c", Value::from(1_i64))]))])),
            ),
        ]);
        assert_eq!(doc, "[versions]\n\n[export]\n[export.\"a b\"]\nc = 1\n");
    }

    #[test]
    fn valores_soltos() {
        let mut out = String::new();
        write_element(
            &mut out,
            &Value::Datetime("1979-05-27T07:32:00Z".to_owned()),
        );
        write_element(&mut out, &Value::Float(2.5));
        write_element(&mut out, &Value::Boolean(false));
        write_element(&mut out, &table(&[]));
        assert_eq!(out, "1979-05-27T07:32:00Z2.5false{}");
    }
}
