//! Edição mínima de arquivos existentes (ARCHITECTURE §6.2).
//!
//! Cada função recebe o texto atual e devolve o novo, mudando só a chave tocada: o resto do
//! arquivo (ordem, espaços, comentários, chaves desconhecidas) fica byte a byte igual. Chaves
//! novas entram na posição em que o packwiz as gravaria.

use toml_edit::{DocumentMut, Item, Table, TableLike};

use crate::decode::parse_document;
use crate::error::{Error, Result};
use crate::index::clean_path;
use crate::metafile::{ModOption, Side};
use crate::value::Value;

/// Ordem dos campos de primeiro nível de um metafile no packwiz.
const METAFILE_ORDER: [&str; 4] = ["name", "filename", "side", "pin"];

/// Ordem dos campos de texto de primeiro nível do `pack.toml` no packwiz.
const PACK_ORDER: [&str; 5] = ["name", "author", "version", "description", "pack-format"];

/// Campo de texto de primeiro nível do `pack.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackTextField {
    /// `name`.
    Name,
    /// `author`.
    Author,
    /// `version`.
    Version,
    /// `description`.
    Description,
}

impl PackTextField {
    fn key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Author => "author",
            Self::Version => "version",
            Self::Description => "description",
        }
    }
}

/// Muda o `side` de um metafile; [`Side::Unset`] remove o campo.
pub fn set_metafile_side(text: &str, side: &Side) -> Result<String> {
    edit(text, |doc| {
        let root = doc.as_table_mut();
        if *side == Side::Unset {
            root.remove("side");
        } else {
            set_root_value(
                root,
                "side",
                toml_edit::Value::from(side.as_str()),
                &METAFILE_ORDER,
            );
        }
        Ok(())
    })
}

/// Liga (`pin = true`) ou desliga (remove o campo) a versão fixada de um metafile.
pub fn set_metafile_pin(text: &str, pin: bool) -> Result<String> {
    edit(text, |doc| {
        let root = doc.as_table_mut();
        if pin {
            set_root_value(root, "pin", toml_edit::Value::from(true), &METAFILE_ORDER);
        } else {
            root.remove("pin");
        }
        Ok(())
    })
}

/// Define ou remove a tabela `[option]` de um metafile.
pub fn set_metafile_option(text: &str, option: Option<&ModOption>) -> Result<String> {
    edit(text, |doc| {
        let root = doc.as_table_mut();
        let Some(option) = option else {
            root.remove("option");
            return Ok(());
        };
        if !root.get("option").is_some_and(Item::is_table_like) {
            root.insert("option", Item::Table(Table::new()));
        }
        let Some(table) = root.get_mut("option").and_then(Item::as_table_like_mut) else {
            return Err(internal("tabela [option] recém-criada não encontrada"));
        };
        set_like(table, "optional", toml_edit::Value::from(option.optional));
        if option.description.is_empty() {
            table.remove("description");
        } else {
            set_like(
                table,
                "description",
                toml_edit::Value::from(option.description.as_str()),
            );
        }
        if option.default {
            set_like(table, "default", toml_edit::Value::from(true));
        } else {
            table.remove("default");
        }
        Ok(())
    })
}

/// Liga ou desliga `preserve` numa entrada do `index.toml`. `Ok(None)` se o arquivo não está
/// no índice.
pub fn set_index_preserve(text: &str, file: &str, preserve: bool) -> Result<Option<String>> {
    let mut doc = parse_document(text)?;
    let wanted = clean_path(file);
    let mut found = false;
    if let Some(files) = doc
        .as_table_mut()
        .get_mut("files")
        .and_then(Item::as_array_of_tables_mut)
    {
        for table in files.iter_mut() {
            let matches = table
                .get("file")
                .and_then(Item::as_str)
                .is_some_and(|path| clean_path(path) == wanted);
            if matches {
                found = true;
                if preserve {
                    set_like(table, "preserve", toml_edit::Value::from(true));
                } else {
                    table.remove("preserve");
                }
            }
        }
    }
    Ok(found.then(|| doc.to_string()))
}

/// Define ou remove um campo de texto do `pack.toml`. Texto vazio remove o campo (o packwiz
/// o omite), exceto `name`, que sempre existe.
pub fn set_pack_text(text: &str, field: PackTextField, value: &str) -> Result<String> {
    edit(text, |doc| {
        let root = doc.as_table_mut();
        if value.is_empty() && field != PackTextField::Name {
            root.remove(field.key());
        } else {
            set_root_value(
                root,
                field.key(),
                toml_edit::Value::from(value),
                &PACK_ORDER,
            );
        }
        Ok(())
    })
}

/// Define ou remove uma versão em `[versions]` (`minecraft`, `fabric`, `forge`…).
pub fn set_pack_version(text: &str, key: &str, version: Option<&str>) -> Result<String> {
    edit(text, |doc| {
        let table = ensure_table(doc, "versions", version.is_some())?;
        if let Some(table) = table {
            match version {
                Some(version) => set_like(table, key, toml_edit::Value::from(version)),
                None => {
                    table.remove(key);
                }
            }
        }
        Ok(())
    })
}

/// Define ou remove um valor de `[options]` do `pack.toml`.
pub fn set_pack_option(text: &str, key: &str, value: Option<&Value>) -> Result<String> {
    edit(text, |doc| {
        let table = ensure_table(doc, "options", value.is_some())?;
        if let Some(table) = table {
            match value {
                Some(value) => {
                    table.insert(key, value_to_item(value));
                }
                None => {
                    table.remove(key);
                }
            }
        }
        Ok(())
    })
}

/// Define ou remove `[export.curseforge] project-id` do `pack.toml`.
pub fn set_pack_curseforge_project_id(text: &str, project_id: Option<u32>) -> Result<String> {
    edit(text, |doc| {
        let root = doc.as_table_mut();
        match project_id {
            Some(project_id) => {
                let new_export = !root.get("export").is_some_and(Item::is_table_like);
                if new_export {
                    root.insert("export", Item::Table(Table::new()));
                }
                let Some(export) = root.get_mut("export").and_then(Item::as_table_like_mut) else {
                    return Err(internal("tabela [export] recém-criada não encontrada"));
                };
                if !export.get("curseforge").is_some_and(Item::is_table_like) {
                    let mut curseforge = Table::new();
                    if new_export {
                        // Como o packwiz: `[export.curseforge]` logo abaixo de `[export]`.
                        curseforge.decor_mut().set_prefix("");
                    }
                    export.insert("curseforge", Item::Table(curseforge));
                }
                let Some(curseforge) = export
                    .get_mut("curseforge")
                    .and_then(Item::as_table_like_mut)
                else {
                    return Err(internal("tabela [export.curseforge] não encontrada"));
                };
                set_like(
                    curseforge,
                    "project-id",
                    toml_edit::Value::from(i64::from(project_id)),
                );
            }
            None => {
                if let Some(curseforge) = root
                    .get_mut("export")
                    .and_then(Item::as_table_like_mut)
                    .and_then(|export| export.get_mut("curseforge"))
                    .and_then(Item::as_table_like_mut)
                {
                    curseforge.remove("project-id");
                }
            }
        }
        Ok(())
    })
}

fn edit(text: &str, change: impl FnOnce(&mut DocumentMut) -> Result<()>) -> Result<String> {
    let mut doc = parse_document(text)?;
    change(&mut doc)?;
    Ok(doc.to_string())
}

fn internal(reason: &str) -> Error {
    Error::InvalidFieldValue {
        file: None,
        key: String::new(),
        reason: format!("erro interno na edição: {reason}"),
    }
}

/// Troca o valor mantendo os espaços e comentários em volta, ou insere a chave nova no fim.
fn set_like(table: &mut dyn TableLike, key: &str, mut value: toml_edit::Value) {
    match table.get_mut(key) {
        Some(Item::Value(existing)) => {
            let decor = existing.decor().clone();
            *value.decor_mut() = decor;
            *existing = value;
        }
        _ => {
            table.insert(key, Item::Value(value));
        }
    }
}

/// Troca o valor de uma chave de primeiro nível ou, se ela é nova, insere logo depois da
/// última chave que o packwiz grava antes dela. As outras chaves não mudam de lugar.
fn set_root_value(root: &mut Table, key: &str, value: toml_edit::Value, order: &[&str]) {
    let is_new = !root.get(key).is_some_and(Item::is_value);
    set_like(root, key, value);
    if !is_new {
        return;
    }
    let rank = |name: &str| order.iter().position(|known| *known == name);
    let Some(own_rank) = rank(key) else {
        return;
    };
    let mut desired: Vec<String> = root
        .iter()
        .filter(|(name, item)| item.is_value() && *name != key)
        .map(|(name, _)| name.to_owned())
        .collect();
    let at = desired
        .iter()
        .rposition(|name| rank(name).is_some_and(|other| other < own_rank))
        .map_or(0, |position| position + 1);
    desired.insert(at, key.to_owned());
    let position = |name: &str| desired.iter().position(|wanted| wanted == name);
    root.sort_values_by(|a, _, b, _| position(a.get()).cmp(&position(b.get())));
}

fn ensure_table<'a>(
    doc: &'a mut DocumentMut,
    name: &str,
    create: bool,
) -> Result<Option<&'a mut dyn TableLike>> {
    let root = doc.as_table_mut();
    if !root.get(name).is_some_and(Item::is_table_like) {
        if !create {
            return Ok(None);
        }
        if root.contains_key(name) {
            return Err(Error::InvalidFieldType {
                file: None,
                key: name.to_owned(),
                expected: "tabela",
                found: "outro tipo",
            });
        }
        root.insert(name, Item::Table(Table::new()));
    }
    Ok(root.get_mut(name).and_then(Item::as_table_like_mut))
}

/// Converte um [`Value`] em item do `toml_edit`: tabelas viram `[tabela]` e listas de tabelas
/// viram `[[lista]]`, como o packwiz grava.
fn value_to_item(value: &Value) -> Item {
    match value {
        Value::Table(map) => {
            let mut table = Table::new();
            for (key, inner) in map {
                table.insert(key, value_to_item(inner));
            }
            Item::Table(table)
        }
        Value::Array(items) if value.is_array_of_tables() => {
            let mut array = toml_edit::ArrayOfTables::new();
            for item in items {
                if let Item::Table(table) = value_to_item(item) {
                    array.push(table);
                }
            }
            Item::ArrayOfTables(array)
        }
        other => Item::Value(value_to_inline(other)),
    }
}

fn value_to_inline(value: &Value) -> toml_edit::Value {
    match value {
        Value::String(text) => toml_edit::Value::from(text.as_str()),
        Value::Integer(number) => toml_edit::Value::from(*number),
        Value::Float(number) => toml_edit::Value::from(*number),
        Value::Boolean(flag) => toml_edit::Value::from(*flag),
        Value::Datetime(text) => text.parse::<toml_edit::Datetime>().map_or_else(
            |_| toml_edit::Value::from(text.as_str()),
            toml_edit::Value::from,
        ),
        Value::Array(items) => {
            let array: toml_edit::Array = items.iter().map(value_to_inline).collect();
            toml_edit::Value::Array(array)
        }
        Value::Table(map) => {
            let table: toml_edit::InlineTable = map
                .iter()
                .map(|(key, inner)| (key.as_str(), value_to_inline(inner)))
                .collect();
            toml_edit::Value::InlineTable(table)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::metafile::Metafile;
    use crate::pack::PackManifest;

    const MODMENU: &str = "name = \"Mod Menu\"\nfilename = \"modmenu-11.0.5.jar\"\nside = \"client\"\n\n[download]\nurl = \"https://cdn.modrinth.com/data/mOgUt4GM/versions/6lgOkclV/modmenu-11.0.5.jar\"\nhash-format = \"sha512\"\nhash = \"92e8\"\n\n[update]\n[update.modrinth]\nmod-id = \"mOgUt4GM\"\nversion = \"6lgOkclV\"\n";

    #[test]
    fn lado_muda_so_a_linha() {
        let edited = set_metafile_side(MODMENU, &Side::Both).unwrap();
        assert_eq!(
            edited,
            MODMENU.replace("side = \"client\"", "side = \"both\"")
        );
        let removed = set_metafile_side(MODMENU, &Side::Unset).unwrap();
        assert_eq!(removed, MODMENU.replace("side = \"client\"\n", ""));
        // Sem `side`, a linha nova entra depois de `filename`, como o packwiz grava.
        let back = set_metafile_side(&removed, &Side::Client).unwrap();
        assert_eq!(back, MODMENU);
    }

    #[test]
    fn pin_igual_ao_packwiz() {
        // `packwiz pin` grava `pin = true` logo depois de `side`.
        let pinned = set_metafile_pin(MODMENU, true).unwrap();
        assert_eq!(
            pinned,
            MODMENU.replace("side = \"client\"\n", "side = \"client\"\npin = true\n")
        );
        assert_eq!(
            Metafile::parse(&pinned).unwrap().value.to_toml_string(),
            pinned
        );
        assert_eq!(set_metafile_pin(&pinned, false).unwrap(), MODMENU);
    }

    #[test]
    fn opcao_acrescentada_e_removida() {
        let option = ModOption {
            optional: true,
            description: "Mapa".to_owned(),
            default: true,
        };
        let edited = set_metafile_option(MODMENU, Some(&option)).unwrap();
        let parsed = Metafile::parse(&edited).unwrap().value;
        assert_eq!(parsed.option, Some(option.clone()));
        assert!(edited.starts_with(MODMENU));
        // Mesma forma que o packwiz grava depois de `pin`/`unpin`.
        assert_eq!(parsed.to_toml_string(), edited);
        let less = ModOption {
            optional: true,
            description: String::new(),
            default: false,
        };
        let edited = set_metafile_option(&edited, Some(&less)).unwrap();
        assert!(edited.ends_with("[option]\noptional = true\n"), "{edited}");
        assert_eq!(set_metafile_option(&edited, None).unwrap(), MODMENU);
    }

    #[test]
    fn preserve_no_indice() {
        let index = "hash-format = \"sha256\"\n\n[[files]]\nfile = \"options.txt\"\nhash = \"ab\"\n\n[[files]]\nfile = \"config/a.txt\"\nhash = \"cd\"\n";
        let edited = set_index_preserve(index, "./options.txt", true)
            .unwrap()
            .unwrap();
        assert_eq!(
            edited,
            index.replace("hash = \"ab\"\n", "hash = \"ab\"\npreserve = true\n")
        );
        assert_eq!(
            set_index_preserve(&edited, "options.txt", false)
                .unwrap()
                .unwrap(),
            index
        );
        assert!(set_index_preserve(index, "nada", true).unwrap().is_none());
        assert!(
            set_index_preserve("hash-format = \"sha256\"\n", "x", true)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn campos_do_pack() {
        let pack = "name = \"T\"\nversion = \"1.0.0\"\npack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\nhash = \"x\"\n\n[versions]\nfabric = \"0.16.14\"\nminecraft = \"1.21.1\"\n";
        let edited = set_pack_text(pack, PackTextField::Author, "Eu").unwrap();
        assert_eq!(
            edited,
            pack.replace("name = \"T\"\n", "name = \"T\"\nauthor = \"Eu\"\n")
        );
        assert_eq!(
            set_pack_text(&edited, PackTextField::Author, "").unwrap(),
            pack
        );
        let edited = set_pack_text(pack, PackTextField::Name, "").unwrap();
        assert!(edited.starts_with("name = \"\"\n"));
        let edited = set_pack_text(pack, PackTextField::Description, "D").unwrap();
        assert!(edited.contains("version = \"1.0.0\"\ndescription = \"D\"\npack-format"));
        let edited = set_pack_text(pack, PackTextField::Version, "1.1.0").unwrap();
        assert!(edited.contains("version = \"1.1.0\""));

        let edited = set_pack_version(pack, "fabric", Some("0.17.0")).unwrap();
        assert_eq!(edited, pack.replace("0.16.14", "0.17.0"));
        let edited = set_pack_version(pack, "fabric", None).unwrap();
        assert_eq!(edited, pack.replace("fabric = \"0.16.14\"\n", ""));
        let no_versions = "name = \"T\"\n";
        assert_eq!(
            set_pack_version(no_versions, "fabric", None).unwrap(),
            no_versions
        );
        let created = set_pack_version(no_versions, "minecraft", Some("1.20.1")).unwrap();
        assert_eq!(
            PackManifest::parse(&created)
                .unwrap()
                .value
                .minecraft_version(),
            Some("1.20.1")
        );
        assert!(set_pack_version("versions = 1\n", "x", Some("1")).is_err());
    }

    #[test]
    fn opcoes_e_export_do_pack() {
        let pack = "name = \"T\"\npack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\n";
        let versions = Value::Array(vec![Value::from("1.21"), Value::from("1.21.1")]);
        let edited = set_pack_option(pack, "acceptable-game-versions", Some(&versions)).unwrap();
        let parsed = PackManifest::parse(&edited).unwrap().value;
        assert_eq!(parsed.acceptable_game_versions(), ["1.21", "1.21.1"]);
        assert!(edited.starts_with(pack));
        let nested = Value::Table(BTreeMap::from([
            ("a".to_owned(), Value::Float(1.5)),
            (
                "b".to_owned(),
                Value::Array(vec![Value::Table(BTreeMap::from([(
                    "c".to_owned(),
                    Value::Datetime("1979-05-27".to_owned()),
                )]))]),
            ),
            (
                "d".to_owned(),
                Value::Array(vec![Value::Table(BTreeMap::new()), Value::from(1_i64)]),
            ),
            ("e".to_owned(), Value::Datetime("não é data".to_owned())),
        ]));
        let edited = set_pack_option(&edited, "x", Some(&nested)).unwrap();
        let parsed = PackManifest::parse(&edited).unwrap().value;
        let x = parsed.option("x").and_then(Value::as_table).unwrap();
        assert_eq!(x["a"], Value::Float(1.5));
        assert!(x["b"].is_array_of_tables());
        assert_eq!(x["e"], Value::from("não é data"));
        let removed = set_pack_option(&edited, "x", None).unwrap();
        assert!(
            PackManifest::parse(&removed)
                .unwrap()
                .value
                .option("x")
                .is_none()
        );
        assert_eq!(set_pack_option(pack, "y", None).unwrap(), pack);

        let edited = set_pack_curseforge_project_id(pack, Some(42)).unwrap();
        let parsed = PackManifest::parse(&edited).unwrap().value;
        assert_eq!(parsed.curseforge_project_id(), Some(42));
        assert_eq!(parsed.to_toml_string(), edited);
        let edited = set_pack_curseforge_project_id(&edited, Some(43)).unwrap();
        assert!(edited.contains("project-id = 43"));
        let removed = set_pack_curseforge_project_id(&edited, None).unwrap();
        assert_eq!(
            PackManifest::parse(&removed)
                .unwrap()
                .value
                .curseforge_project_id(),
            None
        );
        assert_eq!(set_pack_curseforge_project_id(pack, None).unwrap(), pack);
    }

    #[test]
    fn comentarios_e_chaves_desconhecidas_ficam() {
        let text = "# comentário\nname = \"A\" # fim\nfilename = \"a.jar\"\nextra = 1\n\n[download]\nhash-format = \"sha1\"\nhash = \"x\"\n";
        let edited = set_metafile_side(text, &Side::Server).unwrap();
        assert!(edited.contains("# comentário\nname = \"A\" # fim\n"));
        assert!(edited.contains("extra = 1"));
        assert!(
            edited.contains("filename = \"a.jar\"\nside = \"server\"\n"),
            "{edited}"
        );
        assert!(set_metafile_side("x = ", &Side::Both).is_err());
    }
}
