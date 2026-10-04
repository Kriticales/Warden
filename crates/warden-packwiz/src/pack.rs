//! `pack.toml`: o manifesto do pack (R3 §1.5; struct `core.Pack` do packwiz).

use std::collections::BTreeMap;

use crate::decode::{TableReader, parse_document};
use crate::encode::{Node, encode_document, text};
use crate::error::{Error, Result};
use crate::value::Value;

/// Versão do formato que o packwiz grava (`core.CurrentPackFormat`).
pub const CURRENT_PACK_FORMAT: &str = "packwiz:1.1.0";

/// Nome padrão do índice.
pub const DEFAULT_INDEX_FILE: &str = "index.toml";

/// Loaders que o packwiz conhece em `[versions]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Loader {
    /// Fabric.
    Fabric,
    /// Forge.
    Forge,
    /// NeoForge.
    NeoForge,
    /// Quilt.
    Quilt,
    /// `LiteLoader`.
    LiteLoader,
}

impl Loader {
    /// Chave em `[versions]` e nome usado pelas APIs.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Quilt => "quilt",
            Self::LiteLoader => "liteloader",
        }
    }

    /// Todos, na ordem em que o packwiz os procura.
    pub const ALL: [Self; 5] = [
        Self::Quilt,
        Self::Fabric,
        Self::NeoForge,
        Self::Forge,
        Self::LiteLoader,
    ];
}

/// Tabela `[index]` do `pack.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRef {
    /// Caminho do índice, relativo ao `pack.toml`, com `/`.
    pub file: String,
    /// Formato do hash do índice (o packwiz sempre recalcula em `sha256`).
    pub hash_format: String,
    /// Hash do índice; vazio quando o pack usa `no-internal-hashes`.
    pub hash: String,
}

/// Conteúdo de um `pack.toml`.
///
/// Campos de texto vazios são omitidos na escrita, como no packwiz (`omitempty`).
#[derive(Debug, Clone, PartialEq)]
pub struct PackManifest {
    /// Nome do pack.
    pub name: String,
    /// Autor (opcional).
    pub author: String,
    /// Versão do pack (opcional; exigida pelo `.mrpack`).
    pub version: String,
    /// Descrição (opcional).
    pub description: String,
    /// Versão do formato (`packwiz:1.1.0`).
    pub pack_format: String,
    /// Onde está o índice e o hash dele.
    pub index: IndexRef,
    /// `[versions]`: `minecraft` e a versão do loader. `None` se a tabela não existe.
    pub versions: Option<BTreeMap<String, String>>,
    /// `[export.<formato>]`, hoje só `[export.curseforge] project-id`.
    pub export: Option<BTreeMap<String, BTreeMap<String, Value>>>,
    /// `[options]`: `acceptable-game-versions`, `datapack-folder`, `meta-folder`…
    pub options: Option<BTreeMap<String, Value>>,
}

/// Resultado da leitura de um arquivo: o modelo e o que o `refresh` do packwiz mudaria nele.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed<T> {
    /// O conteúdo lido.
    pub value: T,
    /// Chaves que o packwiz não conhece (o `refresh` as apaga), com o caminho completo.
    pub unknown_keys: Vec<String>,
    /// Ajustes que o packwiz faria ao reescrever o arquivo (formato antigo, campo faltando).
    pub migrations: Vec<Migration>,
}

/// Ajuste que o packwiz faz ao ler e regravar um arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Migration {
    /// `pack-format` ausente: assumido `packwiz:1.1.0`.
    PackFormatAssumed,
    /// `pack-format = "packwiz:1.0.0"`: migrado para `packwiz:1.1.0`.
    PackFormatUpgraded,
    /// `pack-format` com versão menor mais nova que a deste packwiz (só aviso).
    PackFormatNewer,
    /// `[index] file` ausente: assumido `index.toml`.
    IndexFileAssumed,
    /// `hash-format` do índice ausente: assumido `sha256`.
    IndexHashFormatAssumed,
}

impl PackManifest {
    /// `pack.toml` novo, no formato atual, sem hash do índice ainda.
    #[must_use]
    pub fn new(name: &str, minecraft: &str) -> Self {
        Self {
            name: name.to_owned(),
            author: String::new(),
            version: String::new(),
            description: String::new(),
            pack_format: CURRENT_PACK_FORMAT.to_owned(),
            index: IndexRef {
                file: DEFAULT_INDEX_FILE.to_owned(),
                hash_format: "sha256".to_owned(),
                hash: String::new(),
            },
            versions: Some(BTreeMap::from([(
                "minecraft".to_owned(),
                minecraft.to_owned(),
            )])),
            export: None,
            options: None,
        }
    }

    /// Lê um `pack.toml`, com as validações e ajustes do `LoadPack` do packwiz.
    pub fn parse(text: &str) -> Result<Parsed<Self>> {
        let doc = parse_document(text)?;
        let mut root = TableReader::new(doc.as_table(), "");
        let mut migrations = Vec::new();

        let name = root.string("name")?.unwrap_or_default();
        let author = root.string("author")?.unwrap_or_default();
        let version = root.string("version")?.unwrap_or_default();
        let description = root.string("description")?.unwrap_or_default();
        let mut pack_format = root.string("pack-format")?.unwrap_or_default();
        if pack_format.is_empty() {
            CURRENT_PACK_FORMAT.clone_into(&mut pack_format);
            migrations.push(Migration::PackFormatAssumed);
        }
        if pack_format == "packwiz:1.0.0" {
            CURRENT_PACK_FORMAT.clone_into(&mut pack_format);
            migrations.push(Migration::PackFormatUpgraded);
        }
        if let Some(newer) = check_pack_format(&pack_format)? {
            migrations.push(newer);
        }

        let mut unknown_keys = Vec::new();
        let mut index = IndexRef {
            file: String::new(),
            hash_format: String::new(),
            hash: String::new(),
        };
        if let Some(mut table) = root.table("index")? {
            index.file = table.string("file")?.unwrap_or_default();
            index.hash_format = table.string("hash-format")?.unwrap_or_default();
            index.hash = table.string("hash")?.unwrap_or_default();
            unknown_keys.extend(table.unknown_keys());
        }
        if index.file.is_empty() {
            DEFAULT_INDEX_FILE.clone_into(&mut index.file);
            migrations.push(Migration::IndexFileAssumed);
        }

        let versions = root.string_map("versions")?;
        let export = root.nested_value_map("export")?;
        let options = root.value_map("options")?;
        unknown_keys.extend(root.unknown_keys());
        unknown_keys.sort();

        Ok(Parsed {
            value: Self {
                name,
                author,
                version,
                description,
                pack_format,
                index,
                versions,
                export,
                options,
            },
            unknown_keys,
            migrations,
        })
    }

    /// O `pack.toml` exatamente como o packwiz o gravaria.
    #[must_use]
    pub fn to_toml_string(&self) -> String {
        let mut fields = vec![("name", text(&self.name))];
        push_text(&mut fields, "author", &self.author);
        push_text(&mut fields, "version", &self.version);
        push_text(&mut fields, "description", &self.description);
        fields.push(("pack-format", text(&self.pack_format)));
        let mut index = vec![
            ("file", text(&self.index.file)),
            ("hash-format", text(&self.index.hash_format)),
        ];
        push_text(&mut index, "hash", &self.index.hash);
        fields.push(("index", Node::Struct(index)));
        if let Some(versions) = &self.versions {
            fields.push((
                "versions",
                Node::Value(Value::Table(
                    versions
                        .iter()
                        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                        .collect(),
                )),
            ));
        }
        if let Some(export) = &self.export {
            fields.push((
                "export",
                Node::Value(Value::Table(
                    export
                        .iter()
                        .map(|(key, table)| (key.clone(), Value::Table(table.clone())))
                        .collect(),
                )),
            ));
        }
        if let Some(options) = &self.options {
            fields.push(("options", Node::Value(Value::Table(options.clone()))));
        }
        encode_document(fields)
    }

    /// Versão do Minecraft (`[versions] minecraft`).
    #[must_use]
    pub fn minecraft_version(&self) -> Option<&str> {
        self.versions.as_ref()?.get("minecraft").map(String::as_str)
    }

    /// Loaders presentes em `[versions]`, com a versão de cada um.
    #[must_use]
    pub fn loaders(&self) -> Vec<(Loader, &str)> {
        let Some(versions) = &self.versions else {
            return Vec::new();
        };
        Loader::ALL
            .into_iter()
            .filter_map(|loader| {
                versions
                    .get(loader.key())
                    .map(|version| (loader, version.as_str()))
            })
            .collect()
    }

    /// Loaders cujos mods o pack aceita (`GetCompatibleLoaders` do packwiz): Quilt aceita
    /// Fabric; NeoForge aceita Forge.
    #[must_use]
    pub fn compatible_loaders(&self) -> Vec<Loader> {
        let has = |loader: Loader| {
            self.versions
                .as_ref()
                .is_some_and(|versions| versions.contains_key(loader.key()))
        };
        let mut loaders = Vec::new();
        if has(Loader::Quilt) {
            loaders.extend([Loader::Quilt, Loader::Fabric]);
        } else if has(Loader::Fabric) {
            loaders.push(Loader::Fabric);
        }
        if has(Loader::NeoForge) {
            loaders.extend([Loader::NeoForge, Loader::Forge]);
        } else if has(Loader::Forge) {
            loaders.push(Loader::Forge);
        }
        loaders
    }

    /// Define a versão de um loader em `[versions]`, criando a tabela se faltar.
    pub fn set_loader_version(&mut self, loader: Loader, version: &str) {
        self.versions
            .get_or_insert_with(BTreeMap::new)
            .insert(loader.key().to_owned(), version.to_owned());
    }

    /// `[options] acceptable-game-versions`, na ordem gravada.
    #[must_use]
    pub fn acceptable_game_versions(&self) -> Vec<&str> {
        self.option("acceptable-game-versions")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }

    /// Um valor de `[options]`.
    #[must_use]
    pub fn option(&self, key: &str) -> Option<&Value> {
        self.options.as_ref()?.get(key)
    }

    /// Define um valor em `[options]` (o único lugar de onde o packwiz lê as opções).
    pub fn set_option(&mut self, key: &str, value: Value) {
        self.options
            .get_or_insert_with(BTreeMap::new)
            .insert(key.to_owned(), value);
    }

    /// `[export.curseforge] project-id`, se for um número válido.
    #[must_use]
    pub fn curseforge_project_id(&self) -> Option<u32> {
        let id = self.export.as_ref()?.get("curseforge")?.get("project-id")?;
        u32::try_from(id.as_integer()?).ok()
    }

    /// Define `[export.curseforge] project-id`.
    pub fn set_curseforge_project_id(&mut self, project_id: u32) {
        self.export
            .get_or_insert_with(BTreeMap::new)
            .entry("curseforge".to_owned())
            .or_default()
            .insert("project-id".to_owned(), Value::from(project_id));
    }
}

fn push_text(fields: &mut Vec<(&'static str, Node)>, name: &'static str, value: &str) {
    if !value.is_empty() {
        fields.push((name, text(value)));
    }
}

/// Confere o `pack-format` como o packwiz (`~1`; aviso se mais novo que `~1.1`).
fn check_pack_format(pack_format: &str) -> Result<Option<Migration>> {
    let invalid = |reason: &str| Error::InvalidFieldValue {
        file: None,
        key: "pack-format".to_owned(),
        reason: reason.to_owned(),
    };
    let Some(version) = pack_format.strip_prefix("packwiz:") else {
        return Err(invalid("não indica um pack do packwiz"));
    };
    let Some((major, minor, prerelease)) = parse_strict_semver(version) else {
        return Err(invalid("não é uma versão SemVer válida"));
    };
    // A restrição `~1` do packwiz (Masterminds/semver) não aceita versões de teste.
    if major != 1 || prerelease {
        return Err(invalid("o pack é de outra versão do packwiz"));
    }
    Ok((minor > 1).then_some(Migration::PackFormatNewer))
}

/// `semver.StrictNewVersion` do Go: `MAJOR.MINOR.PATCH[-pre][+build]`, sem zeros à esquerda.
/// Devolve a versão maior, a menor e se é versão de teste (`-pre`).
fn parse_strict_semver(version: &str) -> Option<(u64, u64, bool)> {
    let core_end = version.find(['-', '+']).unwrap_or(version.len());
    let (core, rest) = version.split_at(core_end);
    let parts: Vec<&str> = core.split('.').collect();
    let [major, minor, patch] = parts.as_slice() else {
        return None;
    };
    let number = |part: &str| -> Option<u64> {
        let valid = !part.is_empty()
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && !(part.len() > 1 && part.starts_with('0'));
        if valid { part.parse().ok() } else { None }
    };
    let (major, minor) = (number(major)?, number(minor)?);
    number(patch)?;
    let (pre, build) = match rest.split_once('+') {
        Some((pre, build)) => (pre, Some(build)),
        None => (rest, None),
    };
    let identifiers_ok = |text: &str, numeric_rule: bool| {
        text.split('.').all(|id| {
            !id.is_empty()
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !(numeric_rule
                    && id.len() > 1
                    && id.starts_with('0')
                    && id.bytes().all(|byte| byte.is_ascii_digit()))
        })
    };
    let prerelease = !pre.is_empty();
    if let Some(pre) = pre.strip_prefix('-') {
        if !identifiers_ok(pre, true) {
            return None;
        }
    } else if prerelease {
        return None;
    }
    if let Some(build) = build
        && !identifiers_ok(build, false)
    {
        return None;
    }
    Some((major, minor, prerelease))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INIT: &str = "name = \"T\"\nauthor = \"A\"\nversion = \"1.0.0\"\npack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\nhash = \"7b81c9e9d9e6e21a9865a2dded0dfd77e7683d679b98fe5b0114eaed9a31f17e\"\n\n[versions]\nfabric = \"0.16.14\"\nminecraft = \"1.21.1\"\n";

    #[test]
    fn ida_e_volta_do_init_real() {
        let parsed = PackManifest::parse(INIT).unwrap();
        assert!(parsed.unknown_keys.is_empty());
        assert!(parsed.migrations.is_empty());
        assert_eq!(parsed.value.to_toml_string(), INIT);
        assert_eq!(parsed.value.minecraft_version(), Some("1.21.1"));
        assert_eq!(parsed.value.loaders(), [(Loader::Fabric, "0.16.14")]);
    }

    #[test]
    fn novo_pack_e_opcoes() {
        let mut pack = PackManifest::new("Meu pack", "1.20.1");
        pack.set_loader_version(Loader::NeoForge, "47.1.106");
        pack.set_option(
            "acceptable-game-versions",
            Value::Array(vec![Value::from("1.20"), Value::from("1.20.1")]),
        );
        pack.set_option("datapack-folder", Value::from("config/openloader/data"));
        pack.set_curseforge_project_id(123_456);
        pack.description = "Descrição \"com aspas\"".to_owned();
        let text = pack.to_toml_string();
        assert_eq!(
            text,
            "name = \"Meu pack\"\ndescription = \"Descrição \\\"com aspas\\\"\"\n\
             pack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\n\
             hash-format = \"sha256\"\n\n[versions]\nminecraft = \"1.20.1\"\n\
             neoforge = \"47.1.106\"\n\n[export]\n[export.curseforge]\nproject-id = 123456\n\n\
             [options]\nacceptable-game-versions = [\"1.20\", \"1.20.1\"]\n\
             datapack-folder = \"config/openloader/data\"\n"
        );
        let back = PackManifest::parse(&text).unwrap().value;
        assert_eq!(back, pack);
        assert_eq!(back.curseforge_project_id(), Some(123_456));
        assert_eq!(back.acceptable_game_versions(), ["1.20", "1.20.1"]);
        assert_eq!(back.compatible_loaders(), [Loader::NeoForge, Loader::Forge]);
    }

    #[test]
    fn loaders_compativeis_como_no_packwiz() {
        let mut pack = PackManifest::new("x", "1.20.1");
        assert!(pack.compatible_loaders().is_empty());
        pack.set_loader_version(Loader::Quilt, "0.26.4");
        assert_eq!(pack.compatible_loaders(), [Loader::Quilt, Loader::Fabric]);
        let mut pack = PackManifest::new("x", "1.20.1");
        pack.set_loader_version(Loader::Fabric, "0.16.14");
        pack.set_loader_version(Loader::Forge, "47.4.0");
        assert_eq!(pack.compatible_loaders(), [Loader::Fabric, Loader::Forge]);
        pack.versions = None;
        assert!(pack.loaders().is_empty());
        assert!(pack.minecraft_version().is_none());
        assert_eq!(Loader::LiteLoader.key(), "liteloader");
    }

    #[test]
    fn migracoes_do_load_pack() {
        let parsed =
            PackManifest::parse("name = \"x\"\n[index]\nhash-format = \"sha256\"\n").unwrap();
        assert_eq!(
            parsed.migrations,
            [Migration::PackFormatAssumed, Migration::IndexFileAssumed]
        );
        assert_eq!(parsed.value.pack_format, CURRENT_PACK_FORMAT);
        assert_eq!(parsed.value.index.file, "index.toml");

        let parsed = PackManifest::parse("pack-format = \"packwiz:1.0.0\"\n").unwrap();
        assert!(parsed.migrations.contains(&Migration::PackFormatUpgraded));
        let parsed = PackManifest::parse("pack-format = \"packwiz:1.2.0\"\n").unwrap();
        assert!(parsed.migrations.contains(&Migration::PackFormatNewer));
    }

    #[test]
    fn pack_format_invalido() {
        for format in [
            "1.1.0",
            "packwiz:2.0.0",
            "packwiz:1.1",
            "packwiz:01.1.0",
            "packwiz:1.1.0-",
            "packwiz:1.1.0-01",
            "packwiz:1.1.0+",
            "packwiz:1.1.0x",
            "packwiz:1.1.0-rc.1",
        ] {
            let text = format!("pack-format = \"{format}\"\n");
            let error = PackManifest::parse(&text).unwrap_err();
            assert!(
                matches!(error, Error::InvalidFieldValue { ref key, .. } if key == "pack-format"),
                "{format}: {error}"
            );
        }
        for format in ["packwiz:1.1.0+build.5", "packwiz:1.0.1"] {
            let text = format!("pack-format = \"{format}\"\n");
            assert!(PackManifest::parse(&text).is_ok(), "{format}");
        }
    }

    #[test]
    fn chaves_desconhecidas_e_tipos_errados() {
        let parsed =
            PackManifest::parse("name = \"x\"\nextra = 1\n[index]\nfile = \"i.toml\"\nz = 2\n")
                .unwrap();
        assert_eq!(parsed.unknown_keys, ["extra", "index.z"]);
        assert!(PackManifest::parse("name = 1\n").is_err());
        assert!(PackManifest::parse("index = 1\n").is_err());
        assert!(PackManifest::parse("[versions]\nminecraft = 1\n").is_err());
        assert!(PackManifest::parse("[export]\ncurseforge = 1\n").is_err());
    }

    #[test]
    fn project_id_invalido() {
        let mut pack = PackManifest::new("x", "1.20.1");
        assert_eq!(pack.curseforge_project_id(), None);
        pack.export = Some(BTreeMap::from([(
            "curseforge".to_owned(),
            BTreeMap::from([("project-id".to_owned(), Value::from(-1_i64))]),
        )]));
        assert_eq!(pack.curseforge_project_id(), None);
        assert!(pack.acceptable_game_versions().is_empty());
    }
}
