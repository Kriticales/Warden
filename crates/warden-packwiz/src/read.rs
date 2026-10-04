//! Leitura tolerante da pasta de um pack: um arquivo com problema vira erro só dele.
//!
//! O `pack.toml` é indispensável (sem ele não há pack). O índice e cada metafile são lidos
//! separadamente; o resultado guarda o erro de cada um, para a interface mostrar o pack com os
//! itens problemáticos marcados em vez de recusar tudo.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::index::{PackIndex, check_relative_path};
use crate::metafile::Metafile;
use crate::pack::{PackManifest, Parsed};

/// Nome do manifesto na pasta do pack.
pub const PACK_FILE: &str = "pack.toml";

/// Nome do arquivo de ignorar do packwiz.
pub const PACKWIZIGNORE_FILE: &str = ".packwizignore";

/// Um metafile lido (ou o erro dele).
#[derive(Debug)]
pub struct MetafileRead {
    /// Caminho relativo à pasta do índice, com `/`.
    pub path: String,
    /// O conteúdo ou o erro só deste arquivo.
    pub result: Result<Parsed<Metafile>>,
}

/// O pack lido do disco.
#[derive(Debug)]
pub struct PackRead {
    /// Pasta do pack.
    pub root: PathBuf,
    /// O `pack.toml`.
    pub pack: Parsed<PackManifest>,
    /// O índice (ou o erro dele).
    pub index: Result<Parsed<PackIndex>>,
    /// Os metafiles listados no índice, na ordem do índice.
    pub metafiles: Vec<MetafileRead>,
    /// O texto do `.packwizignore`, se existir.
    pub packwizignore: Option<String>,
}

impl PackRead {
    /// Os metafiles lidos sem erro.
    pub fn valid_metafiles(&self) -> impl Iterator<Item = (&str, &Metafile)> {
        self.metafiles.iter().filter_map(|read| {
            read.result
                .as_ref()
                .ok()
                .map(|parsed| (read.path.as_str(), &parsed.value))
        })
    }

    /// Os erros de todos os arquivos (índice e metafiles).
    pub fn errors(&self) -> impl Iterator<Item = &Error> {
        self.index.as_ref().err().into_iter().chain(
            self.metafiles
                .iter()
                .filter_map(|read| read.result.as_ref().err()),
        )
    }
}

/// Lê a pasta de um pack. Falha só se o `pack.toml` não puder ser lido.
pub fn read_pack(root: &Path) -> Result<PackRead> {
    let pack_path = root.join(PACK_FILE);
    let pack_text = read_text(&pack_path)?;
    let pack = PackManifest::parse(&pack_text).map_err(|error| error.in_file(PACK_FILE))?;

    let index_file = pack.value.index.file.clone();
    let index = check_relative_path(&index_file)
        .and_then(|()| read_text(&root.join(&index_file)).map_err(|e| e.in_file(&index_file)))
        .and_then(|text| PackIndex::parse(&text).map_err(|error| error.in_file(&index_file)));

    // Os caminhos do índice são relativos à pasta do índice.
    let index_dir = Path::new(&index_file)
        .parent()
        .map_or_else(|| root.to_path_buf(), |parent| root.join(parent));
    let metafiles = match &index {
        Ok(parsed) => parsed
            .value
            .metafiles()
            .into_iter()
            .map(|path| {
                let result = read_metafile(&index_dir, &path);
                MetafileRead { path, result }
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    let packwizignore = match fs::read_to_string(root.join(PACKWIZIGNORE_FILE)) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(Error::Io {
                action: "ler",
                path: root.join(PACKWIZIGNORE_FILE),
                source,
            });
        }
    };

    Ok(PackRead {
        root: root.to_path_buf(),
        pack,
        index,
        metafiles,
        packwizignore,
    })
}

/// Lê um metafile pelo caminho relativo à pasta do índice.
pub fn read_metafile(index_dir: &Path, path: &str) -> Result<Parsed<Metafile>> {
    check_relative_path(path)?;
    let text = read_text(&index_dir.join(path)).map_err(|error| error.in_file(path))?;
    let parsed = Metafile::parse(&text).map_err(|error| error.in_file(path))?;
    check_relative_path(&parsed.value.filename).map_err(|error| match error {
        Error::UnsafePath { reason, .. } => Error::InvalidFieldValue {
            file: Some(path.to_owned()),
            key: "filename".to_owned(),
            reason: format!("o nome do arquivo sai da pasta do metafile ({reason})"),
        },
        other => other,
    })?;
    Ok(parsed)
}

fn read_text(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| Error::Io {
        action: "ler",
        path: path.to_path_buf(),
        source,
    })?;
    String::from_utf8(bytes).map_err(|_| Error::InvalidToml {
        file: None,
        message: format!("{} não está em UTF-8", path.display()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, text: &str) {
        let full = root.join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full, text).unwrap();
    }

    const PACK: &str = "name = \"T\"\npack-format = \"packwiz:1.1.0\"\n\n[index]\nfile = \"index.toml\"\nhash-format = \"sha256\"\n\n[versions]\nminecraft = \"1.21.1\"\n";
    const GOOD: &str = "name = \"A\"\nfilename = \"a.jar\"\nside = \"both\"\n\n[download]\nurl = \"https://x/a.jar\"\nhash-format = \"sha256\"\nhash = \"00\"\n";

    #[test]
    fn ca_5_metafile_invalido_so_falha_ele() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "pack.toml", PACK);
        write(
            root,
            "index.toml",
            "hash-format = \"sha256\"\n\n[[files]]\nfile = \"mods/a.pw.toml\"\nhash = \"1\"\nmetafile = true\n\n\
             [[files]]\nfile = \"mods/b.pw.toml\"\nhash = \"2\"\nmetafile = true\n\n\
             [[files]]\nfile = \"mods/c.pw.toml\"\nhash = \"3\"\nmetafile = true\n\n\
             [[files]]\nfile = \"mods/d.pw.toml\"\nhash = \"4\"\nmetafile = true\n\n\
             [[files]]\nfile = \"../fora.pw.toml\"\nhash = \"5\"\nmetafile = true\n\n\
             [[files]]\nfile = \"mods/e.pw.toml\"\nhash = \"6\"\nmetafile = true\n\n\
             [[files]]\nfile = \"config/x.txt\"\nhash = \"7\"\n",
        );
        write(root, "mods/a.pw.toml", GOOD);
        write(root, "mods/b.pw.toml", "name = \"B\"\nfilename = \n");
        write(
            root,
            "mods/c.pw.toml",
            &GOOD.replace("hash = \"00\"", "hash = 5"),
        );
        write(
            root,
            "mods/e.pw.toml",
            &GOOD.replace("a.jar\"\nside", "../a.jar\"\nside"),
        );
        let read = read_pack(root).unwrap();
        assert!(read.index.is_ok());
        assert_eq!(read.metafiles.len(), 6);
        let valid: Vec<&str> = read.valid_metafiles().map(|(path, _)| path).collect();
        assert_eq!(valid, ["mods/a.pw.toml"]);
        let errors: Vec<String> = read.errors().map(ToString::to_string).collect();
        assert_eq!(errors.len(), 5, "{errors:?}");
        let by_path = |path: &str| {
            read.metafiles
                .iter()
                .find(|m| m.path == path)
                .and_then(|m| m.result.as_ref().err())
                .unwrap()
        };
        assert!(matches!(
            by_path("../fora.pw.toml"),
            Error::UnsafePath { .. }
        ));
        assert!(
            matches!(by_path("mods/b.pw.toml"), Error::InvalidToml { file: Some(f), .. } if f == "mods/b.pw.toml")
        );
        assert!(
            matches!(by_path("mods/c.pw.toml"), Error::InvalidFieldType { key, .. } if key == "download.hash")
        );
        assert!(matches!(by_path("mods/d.pw.toml"), Error::Io { .. }));
        assert!(
            matches!(by_path("mods/e.pw.toml"), Error::InvalidFieldValue { key, .. } if key == "filename")
        );
        assert!(read.packwizignore.is_none());
    }

    #[test]
    fn indice_com_erro_e_pack_sem_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(matches!(read_pack(root), Err(Error::Io { .. })));
        write(root, "pack.toml", "name = 1\n");
        assert!(matches!(
            read_pack(root),
            Err(Error::InvalidFieldType { file: Some(f), .. }) if f == "pack.toml"
        ));
        write(
            root,
            "pack.toml",
            &PACK.replace("index.toml", "../index.toml"),
        );
        let read = read_pack(root).unwrap();
        assert!(matches!(read.index, Err(Error::UnsafePath { .. })));
        assert!(read.metafiles.is_empty());
        write(root, "pack.toml", PACK);
        write(root, "index.toml", "files = 1\n");
        write(root, ".packwizignore", "/logs/\n");
        let read = read_pack(root).unwrap();
        assert!(
            matches!(&read.index, Err(Error::InvalidFieldType { file: Some(f), .. }) if f == "index.toml")
        );
        assert_eq!(read.packwizignore.as_deref(), Some("/logs/\n"));
        assert_eq!(read.errors().count(), 1);
    }

    #[test]
    fn indice_em_subpasta_e_texto_nao_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "pack.toml",
            &PACK.replace("\"index.toml\"", "\"meta/index.toml\""),
        );
        write(
            root,
            "meta/index.toml",
            "hash-format = \"sha256\"\n\n[[files]]\nfile = \"a.pw.toml\"\nmetafile = true\n",
        );
        write(root, "meta/a.pw.toml", GOOD);
        let read = read_pack(root).unwrap();
        assert_eq!(read.valid_metafiles().count(), 1);
        fs::write(root.join("meta/a.pw.toml"), [0xff, 0xfe]).unwrap();
        let read = read_pack(root).unwrap();
        assert!(matches!(
            &read.metafiles[0].result,
            Err(Error::InvalidToml { .. })
        ));
    }
}
