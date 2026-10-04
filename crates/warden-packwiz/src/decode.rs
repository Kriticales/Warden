//! Leitura de TOML para os modelos, com as mesmas exigências de tipo do packwiz.
//!
//! O packwiz lê com `BurntSushi/toml`: um campo de texto com um número é erro, e chaves
//! desconhecidas são ignoradas. Aqui é igual, mas as chaves desconhecidas são anotadas
//! (`unknown_keys`) para a interface poder avisar que o `refresh` vai apagá-las.

use std::collections::BTreeMap;

use toml_edit::{DocumentMut, Item, TableLike};

use crate::error::{Error, Result};
use crate::value::Value;

/// Lê o texto como documento TOML.
pub(crate) fn parse_document(text: &str) -> Result<DocumentMut> {
    text.parse::<DocumentMut>()
        .map_err(|error| Error::InvalidToml {
            file: None,
            message: error.to_string().trim_end().to_owned(),
        })
}

/// Leitor de uma tabela, que lembra quais chaves foram usadas.
pub(crate) struct TableReader<'a> {
    table: &'a dyn TableLike,
    prefix: String,
    used: Vec<String>,
}

impl std::fmt::Debug for TableReader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableReader")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

impl<'a> TableReader<'a> {
    pub(crate) fn new(table: &'a dyn TableLike, prefix: &str) -> Self {
        Self {
            table,
            prefix: prefix.to_owned(),
            used: Vec::new(),
        }
    }

    fn full_key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{}.{key}", self.prefix)
        }
    }

    fn item(&mut self, key: &str) -> Option<&'a Item> {
        let item = self.table.get(key).filter(|item| !item.is_none())?;
        self.used.push(key.to_owned());
        Some(item)
    }

    fn wrong_type(&self, key: &str, expected: &'static str, item: &Item) -> Error {
        Error::InvalidFieldType {
            file: None,
            key: self.full_key(key),
            expected,
            found: item_type_name(item),
        }
    }

    /// Campo de texto; ausente vira `None`.
    pub(crate) fn string(&mut self, key: &str) -> Result<Option<String>> {
        match self.item(key) {
            None => Ok(None),
            Some(item) => match item.as_str() {
                Some(text) => Ok(Some(text.to_owned())),
                None => Err(self.wrong_type(key, "texto", item)),
            },
        }
    }

    /// Campo lógico; ausente vira `None`.
    pub(crate) fn boolean(&mut self, key: &str) -> Result<Option<bool>> {
        match self.item(key) {
            None => Ok(None),
            Some(item) => match item.as_bool() {
                Some(flag) => Ok(Some(flag)),
                None => Err(self.wrong_type(key, "verdadeiro ou falso", item)),
            },
        }
    }

    /// Subtabela (normal ou em linha); ausente vira `None`.
    pub(crate) fn table(&mut self, key: &str) -> Result<Option<TableReader<'a>>> {
        let prefix = self.full_key(key);
        match self.item(key) {
            None => Ok(None),
            Some(item) => match item.as_table_like() {
                Some(table) => Ok(Some(TableReader::new(table, &prefix))),
                None => Err(self.wrong_type(key, "tabela", item)),
            },
        }
    }

    /// Lista de tabelas (`[[chave]]` ou lista de tabelas em linha); ausente vira `None`.
    pub(crate) fn table_array(&mut self, key: &str) -> Result<Option<Vec<TableReader<'a>>>> {
        let prefix = self.full_key(key);
        let Some(item) = self.item(key) else {
            return Ok(None);
        };
        if let Some(array) = item.as_array_of_tables() {
            return Ok(Some(
                array
                    .iter()
                    .map(|table| TableReader::new(table, &prefix))
                    .collect(),
            ));
        }
        if let Some(array) = item.as_array() {
            let mut tables = Vec::with_capacity(array.len());
            for value in array {
                match value.as_inline_table() {
                    Some(table) => tables.push(TableReader::new(table, &prefix)),
                    None => {
                        return Err(Error::InvalidFieldType {
                            file: None,
                            key: prefix,
                            expected: "lista de tabelas",
                            found: "lista com valores que não são tabelas",
                        });
                    }
                }
            }
            return Ok(Some(tables));
        }
        Err(self.wrong_type(key, "lista de tabelas", item))
    }

    /// Tabela de textos (como `[versions]`); ausente vira `None`.
    pub(crate) fn string_map(&mut self, key: &str) -> Result<Option<BTreeMap<String, String>>> {
        let Some(mut table) = self.table(key)? else {
            return Ok(None);
        };
        let mut map = BTreeMap::new();
        for name in table.keys() {
            if let Some(text) = table.string(&name)? {
                map.insert(name, text);
            }
        }
        Ok(Some(map))
    }

    /// Tabela de conteúdo livre (como `[options]`); ausente vira `None`.
    pub(crate) fn value_map(&mut self, key: &str) -> Result<Option<BTreeMap<String, Value>>> {
        let Some(mut table) = self.table(key)? else {
            return Ok(None);
        };
        Ok(Some(table.all_values()))
    }

    /// Tabela de tabelas de conteúdo livre (como `[update]` e `[export]`); ausente vira `None`.
    pub(crate) fn nested_value_map(
        &mut self,
        key: &str,
    ) -> Result<Option<BTreeMap<String, BTreeMap<String, Value>>>> {
        let Some(mut table) = self.table(key)? else {
            return Ok(None);
        };
        let mut map = BTreeMap::new();
        for name in table.keys() {
            if let Some(inner) = table.value_map(&name)? {
                map.insert(name, inner);
            }
        }
        Ok(Some(map))
    }

    fn keys(&self) -> Vec<String> {
        self.table
            .iter()
            .filter(|(_, item)| !item.is_none())
            .map(|(key, _)| key.to_owned())
            .collect()
    }

    fn all_values(&mut self) -> BTreeMap<String, Value> {
        let mut map = BTreeMap::new();
        for (key, item) in self.table.iter() {
            if let Some(value) = item_to_value(item) {
                map.insert(key.to_owned(), value);
            }
        }
        self.used = map.keys().cloned().collect();
        map
    }

    /// Chaves que nenhum campo leu, com o caminho completo.
    pub(crate) fn unknown_keys(&self) -> Vec<String> {
        self.keys()
            .into_iter()
            .filter(|key| !self.used.contains(key))
            .map(|key| self.full_key(&key))
            .collect()
    }
}

/// Converte um item do `toml_edit` num [`Value`]; `None` para item vazio.
pub(crate) fn item_to_value(item: &Item) -> Option<Value> {
    match item {
        Item::None => None,
        Item::Value(value) => Some(edit_value_to_value(value)),
        Item::Table(table) => Some(Value::Table(
            table
                .iter()
                .filter_map(|(key, item)| item_to_value(item).map(|value| (key.to_owned(), value)))
                .collect(),
        )),
        Item::ArrayOfTables(array) => Some(Value::Array(
            array
                .iter()
                .map(|table| {
                    Value::Table(
                        table
                            .iter()
                            .filter_map(|(key, item)| {
                                item_to_value(item).map(|value| (key.to_owned(), value))
                            })
                            .collect(),
                    )
                })
                .collect(),
        )),
    }
}

fn edit_value_to_value(value: &toml_edit::Value) -> Value {
    match value {
        toml_edit::Value::String(text) => Value::String(text.value().clone()),
        toml_edit::Value::Integer(number) => Value::Integer(*number.value()),
        toml_edit::Value::Float(number) => Value::Float(*number.value()),
        toml_edit::Value::Boolean(flag) => Value::Boolean(*flag.value()),
        toml_edit::Value::Datetime(date) => Value::Datetime(date.value().to_string()),
        toml_edit::Value::Array(array) => {
            Value::Array(array.iter().map(edit_value_to_value).collect())
        }
        toml_edit::Value::InlineTable(table) => Value::Table(
            table
                .iter()
                .map(|(key, value)| (key.to_owned(), edit_value_to_value(value)))
                .collect(),
        ),
    }
}

fn item_type_name(item: &Item) -> &'static str {
    item_to_value(item).map_or("vazio", |value| value.type_name())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tipos_errados_citam_a_chave_completa() {
        let doc = parse_document("a = 1\n[t]\nb = \"x\"\n").unwrap();
        let mut root = TableReader::new(doc.as_table(), "");
        let error = root.string("a").unwrap_err();
        assert_eq!(
            error.to_string(),
            "o campo `a` deveria ser texto, mas é número inteiro"
        );
        let mut table = root.table("t").unwrap().unwrap();
        let error = table.boolean("b").unwrap_err();
        assert!(error.to_string().contains("`t.b`"), "{error}");
        assert!(root.table("a").is_err());
        assert!(root.table_array("a").is_err());
        assert!(root.string("nada").unwrap().is_none());
    }

    #[test]
    fn listas_de_tabelas_em_linha_e_erros() {
        let doc = parse_document("x = [{a = 1}, {a = 2}]\ny = [1]\n").unwrap();
        let mut root = TableReader::new(doc.as_table(), "");
        assert_eq!(root.table_array("x").unwrap().unwrap().len(), 2);
        let error = root.table_array("y").unwrap_err();
        assert!(error.to_string().contains("lista de tabelas"), "{error}");
        assert!(root.table_array("z").unwrap().is_none());
    }

    #[test]
    fn mapas_e_chaves_desconhecidas() {
        let text = "[v]\nminecraft = \"1.21.1\"\n[o]\nn = 1.5\nd = 1979-05-27\nl = [1, \"a\"]\n\
                    [o.sub]\nk = true\n[[o.arr]]\nq = 1\n[u.modrinth]\nmod-id = \"x\"\n";
        let doc = parse_document(&format!("extra = 1\n{text}")).unwrap();
        let mut root = TableReader::new(doc.as_table(), "");
        let versions = root.string_map("v").unwrap().unwrap();
        assert_eq!(versions["minecraft"], "1.21.1");
        let options = root.value_map("o").unwrap().unwrap();
        assert_eq!(options["n"], Value::Float(1.5));
        assert_eq!(options["d"], Value::Datetime("1979-05-27".to_owned()));
        assert!(options["arr"].is_array_of_tables());
        assert!(options["sub"].as_table().is_some());
        let update = root.nested_value_map("u").unwrap().unwrap();
        assert_eq!(update["modrinth"]["mod-id"], Value::from("x"));
        assert_eq!(root.unknown_keys(), ["extra"]);
        assert!(root.string_map("nada").unwrap().is_none());
        assert!(root.value_map("nada").unwrap().is_none());
        assert!(root.nested_value_map("nada").unwrap().is_none());

        let doc = parse_document("[v]\nforge = 1\n").unwrap();
        let mut root = TableReader::new(doc.as_table(), "");
        let error = root.string_map("v").unwrap_err();
        assert!(error.to_string().contains("`v.forge`"), "{error}");
    }

    #[test]
    fn sintaxe_invalida() {
        let error = parse_document("a = \n").unwrap_err();
        assert!(matches!(error, Error::InvalidToml { .. }));
    }
}
