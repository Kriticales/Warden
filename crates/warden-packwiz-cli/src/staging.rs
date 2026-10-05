//! Cópia de staging do pack (ARCHITECTURE §6.1): validação, `curseforge add` e exportações
//! rodam numa cópia, nunca na pasta do pack (o packwiz sobrescreve arquivos em silêncio e as
//! exportações reescrevem o índice).
//!
//! A cópia leva todos os arquivos do pack, menos a pasta `.git` (o packwiz sempre a ignora e
//! ela pode ser grande). Não filtra pelo `.packwizignore`: assim o refresh na cópia vê
//! exatamente o que veria no original, mesmo que a leitura do ignore pelo Warden divergisse.
//! Links simbólicos e junções são recusados (ARCHITECTURE §6.2): seguir um deles copiaria
//! arquivos de fora do pack.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use warden_packwiz::HashFormat;
use warden_packwiz::hash::hash_reader;

use crate::error::{Error, Result};

/// Pasta que nunca é copiada nem comparada.
pub const SKIPPED_DIR: &str = ".git";

/// Uma cópia do pack numa pasta temporária, apagada no `Drop`.
#[derive(Debug)]
pub struct Staging {
    root: PathBuf,
    files: usize,
}

impl Staging {
    /// Copia o pack de `source` para `dest` (por exemplo `AppPaths::staging_dir(operação)`).
    /// `dest` não pode existir com conteúdo; é criada se faltar.
    ///
    /// Erros: [`Error::LinkInPack`] se o pack tiver link simbólico ou junção; [`Error::Io`]
    /// em falha de disco (a cópia parcial é apagada).
    pub fn copy_from(source: &Path, dest: &Path) -> Result<Self> {
        if fs::read_dir(dest).is_ok_and(|mut entries| entries.next().is_some()) {
            return Err(Error::io(
                "usar como staging",
                dest,
                io::Error::new(io::ErrorKind::AlreadyExists, "a pasta já tem arquivos"),
            ));
        }
        fs::create_dir_all(dest).map_err(|source| Error::io("criar", dest, source))?;
        // A partir daqui a pasta é nossa: o `Drop` apaga a cópia parcial em caso de erro.
        let mut staging = Self {
            root: dest.to_path_buf(),
            files: 0,
        };
        for file in list_files(source)? {
            let target = dest.join(&file.relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|source| Error::io("criar", parent, source))?;
            }
            match fs::copy(&file.path, &target) {
                Ok(_) => staging.files += 1,
                // Sumiu entre a listagem e a cópia (outro programa apagou): não existe mais
                // no pack, então também não existe na cópia.
                Err(error) if error.kind() == io::ErrorKind::NotFound && !file.path.exists() => {}
                Err(source) => return Err(Error::io("copiar", &file.path, source)),
            }
        }
        Ok(staging)
    }

    /// A pasta da cópia (a "pasta do pack" para o packwiz).
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Quantos arquivos foram copiados.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        // Melhor esforço: um antivírus pode segurar um arquivo por um instante; o que sobrar
        // fica na pasta de staging, que o app limpa ao iniciar.
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Um arquivo do pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile {
    /// Caminho relativo à pasta do pack, com `/`.
    pub relative: String,
    /// Caminho completo.
    pub path: PathBuf,
}

/// Todos os arquivos de `root`, recursivamente, menos `.git`, em ordem de caminho.
///
/// Erros: [`Error::LinkInPack`] em link simbólico ou junção; [`Error::Io`] se uma pasta não
/// puder ser lida.
pub fn list_files(root: &Path) -> Result<Vec<PackFile>> {
    let mut files = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            // Uma subpasta que sumiu durante a varredura não tem mais arquivos.
            Err(error) if error.kind() == io::ErrorKind::NotFound && !prefix.is_empty() => {
                continue;
            }
            Err(source) => return Err(Error::io("listar", &dir, source)),
        };
        for entry in entries {
            let entry = entry.map_err(|source| Error::io("listar", &dir, source))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let path = entry.path();
            let kind = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata.file_type(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(source) => return Err(Error::io("ler", &path, source)),
            };
            if kind.is_symlink() {
                return Err(Error::LinkInPack { path: relative });
            }
            if kind.is_dir() {
                if prefix.is_empty() && name == SKIPPED_DIR {
                    continue;
                }
                pending.push((path, relative));
            } else if kind.is_file() {
                files.push(PackFile { relative, path });
            }
        }
    }
    files.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(files)
}

/// SHA-256 de cada arquivo de `root` (menos `.git`), por caminho relativo. `filter` escolhe
/// quais arquivos entram.
pub fn snapshot(root: &Path, filter: impl Fn(&str) -> bool) -> Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for file in list_files(root)? {
        if !filter(&file.relative) {
            continue;
        }
        let mut reader = match fs::File::open(&file.path) {
            Ok(reader) => reader,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(source) => return Err(Error::io("ler", &file.path, source)),
        };
        let hash = hash_reader(HashFormat::Sha256, &mut reader)
            .map_err(|source| Error::io("ler", &file.path, source))?;
        hashes.insert(file.relative, hash);
    }
    Ok(hashes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, text: &str) {
        let full = root.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    }

    #[test]
    fn copia_tudo_menos_git_e_apaga_no_fim() {
        let temp = tempfile::tempdir().unwrap();
        let pack = temp.path().join("pack");
        write(&pack, "pack.toml", "name = \"x\"\n");
        write(&pack, "mods/a.pw.toml", "a\n");
        write(&pack, "config/sub/b.txt", "b\n");
        write(&pack, ".git/HEAD", "ref\n");
        write(&pack, "kubejs/.git/x", "dentro de subpasta é copiado\n");
        let dest = temp.path().join("staging");
        let staging = Staging::copy_from(&pack, &dest).unwrap();
        assert_eq!(staging.file_count(), 4);
        let copied: Vec<String> = list_files(staging.path())
            .unwrap()
            .into_iter()
            .map(|file| file.relative)
            .collect();
        assert_eq!(
            copied,
            [
                "config/sub/b.txt",
                "kubejs/.git/x",
                "mods/a.pw.toml",
                "pack.toml"
            ]
        );
        assert_eq!(
            fs::read_to_string(dest.join("config/sub/b.txt")).unwrap(),
            "b\n"
        );
        drop(staging);
        assert!(!dest.exists());
        assert!(pack.join(".git/HEAD").exists());
    }

    #[test]
    fn staging_ocupado_e_recusado() {
        let temp = tempfile::tempdir().unwrap();
        let pack = temp.path().join("pack");
        write(&pack, "pack.toml", "x\n");
        let dest = temp.path().join("staging");
        write(&dest, "outro.txt", "y\n");
        let error = Staging::copy_from(&pack, &dest).unwrap_err();
        assert!(matches!(error, Error::Io { .. }), "{error}");
        // O conteúdo de quem já estava lá não é apagado.
        assert!(dest.join("outro.txt").exists());
    }

    #[test]
    fn pack_inexistente_e_erro_de_disco() {
        let temp = tempfile::tempdir().unwrap();
        let error =
            Staging::copy_from(&temp.path().join("nada"), &temp.path().join("s")).unwrap_err();
        assert!(matches!(error, Error::Io { .. }), "{error}");
        assert!(!temp.path().join("s").exists());
    }

    #[test]
    fn snapshot_com_filtro() {
        let temp = tempfile::tempdir().unwrap();
        write(temp.path(), "mods/a.pw.toml", "a");
        write(temp.path(), "config/b.txt", "b");
        let all = snapshot(temp.path(), |_| true).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(
            all["config/b.txt"],
            "3e23e8160039594a33894f6564e1b1348bbd7a0088d42c4acb73eeaed59c009d"
        );
        let metafiles = snapshot(temp.path(), |path| path.ends_with(".pw.toml")).unwrap();
        assert_eq!(metafiles.keys().collect::<Vec<_>>(), ["mods/a.pw.toml"]);
    }

    #[cfg(windows)]
    #[test]
    fn juncao_no_pack_e_recusada() {
        let temp = tempfile::tempdir().unwrap();
        let pack = temp.path().join("pack");
        write(&pack, "pack.toml", "x\n");
        let outside = temp.path().join("fora");
        write(&outside, "segredo.txt", "s\n");
        junction::create(&outside, pack.join("config")).unwrap();
        let error = Staging::copy_from(&pack, &temp.path().join("s")).unwrap_err();
        assert!(
            matches!(&error, Error::LinkInPack { path } if path == "config"),
            "{error}"
        );
        assert!(!temp.path().join("s").exists());
    }

    #[cfg(unix)]
    #[test]
    fn link_simbolico_no_pack_e_recusado() {
        let temp = tempfile::tempdir().unwrap();
        let pack = temp.path().join("pack");
        write(&pack, "pack.toml", "x\n");
        write(temp.path(), "fora.txt", "s\n");
        std::os::unix::fs::symlink(temp.path().join("fora.txt"), pack.join("link.txt")).unwrap();
        let error = Staging::copy_from(&pack, &temp.path().join("s")).unwrap_err();
        assert!(matches!(&error, Error::LinkInPack { path } if path == "link.txt"));
    }
}
