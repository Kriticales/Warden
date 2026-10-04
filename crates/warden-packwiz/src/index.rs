//! `index.toml`: a lista de arquivos do pack com os hashes (R3 §1.6; `core/index.go` e
//! `core/indexfiles.go` do packwiz).

use crate::decode::{TableReader, parse_document};
use crate::encode::{Node, boolean, encode_document, text};
use crate::error::{Error, Result};
use crate::pack::{Migration, Parsed};

/// Sufixo dos metafiles.
pub const METAFILE_SUFFIX: &str = ".pw.toml";

/// Uma entrada `[[files]]` do índice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// Caminho relativo à pasta do índice, sempre com `/`.
    pub file: String,
    /// Hash do arquivo em disco (para metafiles, o hash do `.pw.toml`). Vazio com
    /// `no-internal-hashes`.
    pub hash: String,
    /// Formato do hash, só quando difere do formato do índice.
    pub hash_format: String,
    /// `alias` (o Warden nunca grava; issues #228 e #52 do packwiz).
    pub alias: String,
    /// Se é um metafile `.pw.toml`.
    pub metafile: bool,
    /// Se o packwiz-installer deve manter o arquivo que o jogador já tem.
    pub preserve: bool,
}

impl IndexEntry {
    /// Entrada com hash `sha256` no formato do índice.
    #[must_use]
    pub fn new(file: &str, hash: &str) -> Self {
        Self {
            file: file.to_owned(),
            hash: hash.to_owned(),
            hash_format: String::new(),
            alias: String::new(),
            metafile: file.ends_with(METAFILE_SUFFIX),
            preserve: false,
        }
    }

    fn to_node(&self) -> Node {
        let mut fields = vec![("file", text(&self.file))];
        for (name, value) in [
            ("hash", &self.hash),
            ("hash-format", &self.hash_format),
            ("alias", &self.alias),
        ] {
            if !value.is_empty() {
                fields.push((name, text(value)));
            }
        }
        if self.metafile {
            fields.push(("metafile", boolean(true)));
        }
        if self.preserve {
            fields.push(("preserve", boolean(true)));
        }
        Node::Struct(fields)
    }
}

/// Conteúdo de um `index.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackIndex {
    /// Formato dos hashes (o packwiz usa `sha256`).
    pub hash_format: String,
    /// Entradas, na ordem lida. A escrita ordena e remove repetidas como o packwiz.
    pub files: Vec<IndexEntry>,
}

impl Default for PackIndex {
    fn default() -> Self {
        Self {
            hash_format: "sha256".to_owned(),
            files: Vec::new(),
        }
    }
}

impl PackIndex {
    /// Lê um `index.toml` como o `LoadIndex` do packwiz.
    pub fn parse(text: &str) -> Result<Parsed<Self>> {
        let doc = parse_document(text)?;
        let mut root = TableReader::new(doc.as_table(), "");
        let mut migrations = Vec::new();
        let mut hash_format = root.string("hash-format")?.unwrap_or_default();
        if hash_format.is_empty() {
            "sha256".clone_into(&mut hash_format);
            migrations.push(Migration::IndexHashFormatAssumed);
        }
        let mut unknown_keys = Vec::new();
        let mut files = Vec::new();
        for (position, mut table) in root.table_array("files")?.into_iter().flatten().enumerate() {
            let entry = IndexEntry {
                file: table.string("file")?.unwrap_or_default(),
                hash: table.string("hash")?.unwrap_or_default(),
                hash_format: table.string("hash-format")?.unwrap_or_default(),
                alias: table.string("alias")?.unwrap_or_default(),
                metafile: table.boolean("metafile")?.unwrap_or_default(),
                preserve: table.boolean("preserve")?.unwrap_or_default(),
            };
            unknown_keys.extend(
                table
                    .unknown_keys()
                    .into_iter()
                    .map(|key| key.replacen("files.", &format!("files[{position}]."), 1)),
            );
            files.push(entry);
        }
        unknown_keys.extend(root.unknown_keys());
        Ok(Parsed {
            value: Self { hash_format, files },
            unknown_keys,
            migrations,
        })
    }

    /// O `index.toml` exatamente como o packwiz o gravaria: caminhos limpos, entradas
    /// repetidas (mesmo arquivo e `alias`) reduzidas à última e ordem por caminho.
    #[must_use]
    pub fn to_toml_string(&self) -> String {
        let entries = self.normalized_entries();
        encode_document(vec![
            ("hash-format", text(&self.hash_format)),
            (
                "files",
                Node::StructArray(entries.iter().map(IndexEntry::to_node).collect()),
            ),
        ])
    }

    /// Entradas como o packwiz as guarda em memória e grava (`toMemoryRep` + `toTomlRep`).
    #[must_use]
    pub fn normalized_entries(&self) -> Vec<IndexEntry> {
        let mut entries: Vec<IndexEntry> = Vec::with_capacity(self.files.len());
        for entry in &self.files {
            let mut entry = entry.clone();
            entry.file = clean_path(&entry.file);
            entry.alias = clean_path(&entry.alias);
            if entry.alias == "." {
                entry.alias.clear();
            }
            match entries
                .iter_mut()
                .find(|existing| existing.file == entry.file && existing.alias == entry.alias)
            {
                Some(existing) => *existing = entry,
                None => entries.push(entry),
            }
        }
        entries.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.alias.cmp(&b.alias)));
        entries
    }

    /// A entrada de um caminho (comparado depois de limpo).
    #[must_use]
    pub fn entry(&self, file: &str) -> Option<&IndexEntry> {
        let file = clean_path(file);
        self.files
            .iter()
            .rev()
            .find(|entry| clean_path(&entry.file) == file)
    }

    /// Caminhos dos metafiles do índice, limpos e sem repetição.
    #[must_use]
    pub fn metafiles(&self) -> Vec<String> {
        self.normalized_entries()
            .into_iter()
            .filter(|entry| entry.metafile)
            .map(|entry| entry.file)
            .fold(Vec::new(), |mut paths, file| {
                if !paths.contains(&file) {
                    paths.push(file);
                }
                paths
            })
    }

    /// Liga ou desliga `preserve` numa entrada existente. Devolve se a entrada existe.
    pub fn set_preserve(&mut self, file: &str, preserve: bool) -> bool {
        let file = clean_path(file);
        let mut found = false;
        for entry in &mut self.files {
            if clean_path(&entry.file) == file {
                entry.preserve = preserve;
                found = true;
            }
        }
        found
    }
}

/// `path.Clean` do Go: junta barras repetidas, tira `.`, resolve `..` e a barra final.
#[must_use]
pub fn clean_path(path: &str) -> String {
    if path.is_empty() {
        return ".".to_owned();
    }
    let rooted = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else if !rooted {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    match (rooted, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".to_owned(),
        (false, false) => joined,
    }
}

/// Confere que um caminho do índice (ou o `filename` de um metafile) fica dentro da pasta
/// do pack: relativo, sem `..` depois de limpo e sem letra de unidade.
pub fn check_relative_path(path: &str) -> Result<()> {
    let unsafe_path = |reason| Error::UnsafePath {
        path: path.chars().take(260).collect(),
        reason,
    };
    if path.is_empty() {
        return Err(unsafe_path("caminho vazio"));
    }
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/') {
        return Err(unsafe_path("caminho absoluto"));
    }
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err(unsafe_path("caminho com letra de unidade"));
    }
    if normalized.contains(':') {
        return Err(unsafe_path(
            "caminho com dois-pontos (fluxo alternativo do Windows)",
        ));
    }
    if normalized.chars().any(char::is_control) {
        return Err(unsafe_path("caminho com caractere de controle"));
    }
    if normalized.split('/').any(is_windows_device_name) {
        return Err(unsafe_path("caminho com nome reservado do Windows"));
    }
    let cleaned = clean_path(&normalized);
    if cleaned == ".." || cleaned.starts_with("../") {
        return Err(unsafe_path("caminho sai da pasta do pack"));
    }
    if cleaned == "." {
        return Err(unsafe_path("caminho aponta para a própria pasta do pack"));
    }
    Ok(())
}

/// `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, com ou sem extensão: no Windows
/// abrem um dispositivo em vez de um arquivo.
fn is_windows_device_name(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let numbered = |prefix: &str| {
        stem.strip_prefix(prefix)
            .is_some_and(|rest| rest.len() == 1 && matches!(rest.as_bytes()[0], b'1'..=b'9'))
    };
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || numbered("COM") || numbered("LPT")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_como_no_go() {
        let cases = [
            ("", "."),
            ("a//b/./c/", "a/b/c"),
            ("a/../../b", "../b"),
            ("/../a", "/a"),
            ("/", "/"),
            ("./", "."),
            ("a/b/..", "a"),
            ("../../x/..", "../.."),
        ];
        for (input, expected) in cases {
            assert_eq!(clean_path(input), expected, "{input}");
        }
    }

    #[test]
    fn caminhos_inseguros() {
        for path in [
            "",
            "/etc/x",
            "C:/x",
            "c:\\x",
            "../x",
            "a/../../x",
            "a/..",
            ".",
            "a\0b",
            "0:",
            "mods/a.jar:fluxo",
            "a\u{1}b",
            "config/NUL",
            "con.txt",
            "x/com1.log",
            "LPT9",
        ] {
            assert!(check_relative_path(path).is_err(), "{path:?}");
        }
        for path in [
            "mods/a.pw.toml",
            "config/../config/x",
            "a..b/c",
            "..a/b",
            "console.txt",
            "com0",
            "com10",
            "nulo/x",
        ] {
            assert!(check_relative_path(path).is_ok(), "{path:?}");
        }
        let long = format!("/{}", "a".repeat(500));
        let Error::UnsafePath { path, .. } = check_relative_path(&long).unwrap_err() else {
            unreachable!();
        };
        assert_eq!(path.chars().count(), 260);
    }

    #[test]
    fn indice_vazio_como_o_init() {
        let index = PackIndex::default();
        assert_eq!(
            index.to_toml_string(),
            "hash-format = \"sha256\"\nfiles = []\n"
        );
        let parsed = PackIndex::parse("").unwrap();
        assert_eq!(parsed.migrations, [Migration::IndexHashFormatAssumed]);
        assert_eq!(parsed.value, index);
    }

    #[test]
    fn normaliza_ordena_e_remove_repetidas() {
        let mut index = PackIndex::default();
        index.files.push(IndexEntry::new("mods/b.pw.toml", "2"));
        index.files.push(IndexEntry::new("./config//a.txt", "1"));
        index.files.push(IndexEntry::new("mods/b.pw.toml", "3"));
        let mut aliased = IndexEntry::new("config/a.txt", "4");
        aliased.alias = "outro/a.txt".to_owned();
        aliased.hash_format = "sha1".to_owned();
        index.files.push(aliased);
        assert!(index.set_preserve("config/a.txt", true));
        assert!(!index.set_preserve("nada", true));
        let text = index.to_toml_string();
        assert_eq!(
            text,
            "hash-format = \"sha256\"\n\n[[files]]\nfile = \"config/a.txt\"\nhash = \"1\"\n\
             preserve = true\n\n[[files]]\nfile = \"config/a.txt\"\nhash = \"4\"\n\
             hash-format = \"sha1\"\nalias = \"outro/a.txt\"\npreserve = true\n\n[[files]]\n\
             file = \"mods/b.pw.toml\"\nhash = \"3\"\nmetafile = true\n"
        );
        assert_eq!(index.metafiles(), ["mods/b.pw.toml"]);
        assert_eq!(
            index.entry("mods//b.pw.toml").map(|e| e.hash.as_str()),
            Some("3")
        );
        assert!(index.entry("nada").is_none());
        let back = PackIndex::parse(&text).unwrap();
        assert!(back.unknown_keys.is_empty());
        assert_eq!(back.value.to_toml_string(), text);
    }

    #[test]
    fn chaves_desconhecidas_e_tipos() {
        let parsed =
            PackIndex::parse("hash-format = \"sha256\"\nx = 1\n[[files]]\nfile = \"a\"\ny = 2\n")
                .unwrap();
        assert_eq!(parsed.unknown_keys, ["files[0].y", "x"]);
        assert!(PackIndex::parse("[[files]]\nfile = 1\n").is_err());
        assert!(PackIndex::parse("[[files]]\nmetafile = \"sim\"\n").is_err());
        assert!(PackIndex::parse("files = 3\n").is_err());
    }
}
