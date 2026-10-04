//! Valor TOML dinâmico, do jeito que o packwiz o guarda (o `interface{}` do Go).
//!
//! Usado onde o formato aceita conteúdo livre: `[options]` e `[export.<formato>]` do
//! `pack.toml` e as tabelas `[update.<fonte>]` dos metafiles. Números inteiros são `i64` e
//! decimais `f64`, como no leitor de TOML do packwiz (BurntSushi/toml).

use std::collections::BTreeMap;

/// Valor TOML de conteúdo livre.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Texto.
    String(String),
    /// Número inteiro.
    Integer(i64),
    /// Número decimal.
    Float(f64),
    /// Verdadeiro ou falso.
    Boolean(bool),
    /// Data e hora, guardada como o texto lido (o Warden não interpreta datas do packwiz).
    Datetime(String),
    /// Lista. Uma lista não vazia só de tabelas é gravada como `[[chave]]`.
    Array(Vec<Value>),
    /// Tabela, com as chaves em ordem alfabética (a ordem em que o packwiz grava).
    Table(BTreeMap<String, Value>),
}

impl Value {
    /// O texto, se o valor for texto.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// O número, se o valor for inteiro.
    #[must_use]
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(number) => Some(*number),
            _ => None,
        }
    }

    /// O valor lógico, se for um.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(flag) => Some(*flag),
            _ => None,
        }
    }

    /// A lista, se o valor for uma.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// A tabela, se o valor for uma.
    #[must_use]
    pub fn as_table(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Self::Table(table) => Some(table),
            _ => None,
        }
    }

    /// Nome do tipo em português, para mensagens de erro.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::String(_) => "texto",
            Self::Integer(_) => "número inteiro",
            Self::Float(_) => "número decimal",
            Self::Boolean(_) => "verdadeiro ou falso",
            Self::Datetime(_) => "data",
            Self::Array(_) => "lista",
            Self::Table(_) => "tabela",
        }
    }

    /// Se é uma lista não vazia só de tabelas (gravada como `[[chave]]`).
    pub(crate) fn is_array_of_tables(&self) -> bool {
        matches!(self, Self::Array(items)
            if !items.is_empty() && items.iter().all(|item| matches!(item, Self::Table(_))))
    }
}

impl From<&str> for Value {
    fn from(text: &str) -> Self {
        Self::String(text.to_owned())
    }
}

impl From<String> for Value {
    fn from(text: String) -> Self {
        Self::String(text)
    }
}

impl From<i64> for Value {
    fn from(number: i64) -> Self {
        Self::Integer(number)
    }
}

impl From<u32> for Value {
    fn from(number: u32) -> Self {
        Self::Integer(i64::from(number))
    }
}

impl From<bool> for Value {
    fn from(flag: bool) -> Self {
        Self::Boolean(flag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acessores_e_nomes_de_tipo() {
        let table = Value::Table(BTreeMap::from([("a".to_owned(), Value::from(1_i64))]));
        let cases: [(Value, &str); 7] = [
            (Value::from("x"), "texto"),
            (Value::from(3_u32), "número inteiro"),
            (Value::Float(1.5), "número decimal"),
            (Value::from(true), "verdadeiro ou falso"),
            (Value::Datetime("1979-05-27".to_owned()), "data"),
            (Value::Array(vec![]), "lista"),
            (table.clone(), "tabela"),
        ];
        for (value, name) in &cases {
            assert_eq!(value.type_name(), *name);
        }
        assert_eq!(Value::from(String::from("y")).as_str(), Some("y"));
        assert_eq!(Value::from(7_i64).as_integer(), Some(7));
        assert_eq!(Value::from(false).as_bool(), Some(false));
        assert_eq!(Value::Array(vec![]).as_array().map(<[Value]>::len), Some(0));
        assert!(table.as_table().is_some());
        assert_eq!(Value::from(1_i64).as_str(), None);
        assert_eq!(Value::from("1").as_integer(), None);
        assert_eq!(Value::from("1").as_bool(), None);
        assert_eq!(Value::from("1").as_array(), None);
        assert_eq!(Value::from("1").as_table(), None);
    }

    #[test]
    fn lista_de_tabelas_so_quando_nao_vazia_e_homogenea() {
        let table = Value::Table(BTreeMap::new());
        assert!(Value::Array(vec![table.clone()]).is_array_of_tables());
        assert!(!Value::Array(vec![]).is_array_of_tables());
        assert!(!Value::Array(vec![table, Value::from(1_i64)]).is_array_of_tables());
        assert!(!Value::from(1_i64).is_array_of_tables());
    }
}
