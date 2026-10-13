//! Leitura da origem da importação: um zip (`.mrpack`, zip da CurseForge, zip de instância do
//! Prism) ou uma pasta (instância do Prism/MultiMC ou do app da CurseForge), com a mesma
//! interface.
//!
//! Segurança (ARCHITECTURE §20): todo nome de entrada do zip é conferido antes de qualquer
//! outra coisa (*zip slip*: `..`, caminho absoluto, letra de drive, barra invertida); um nome
//! perigoso recusa o arquivo inteiro. A leitura tem limites de quantidade de entradas e de
//! tamanho descompactado (proteção contra *zip bomb*), conferidos sobre os bytes lidos e não
//! só sobre o tamanho declarado. Na pasta, links simbólicos e junções nunca são seguidos.
//! Nada aqui escreve no disco.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::{Error, ImportErrorCode as Code, Result};

/// Mais entradas do que isto: recusado.
pub(crate) const MAX_ENTRIES: usize = 200_000;
/// Maior arquivo lido de uma vez (512 MiB).
pub(crate) const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
/// Soma dos tamanhos descompactados aceita (8 GiB).
pub(crate) const MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// Tipo de uma entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    /// Arquivo comum.
    File,
    /// Pasta.
    Dir,
    /// Link simbólico ou junção (nunca seguido nem copiado).
    Symlink,
}

/// Uma entrada da origem, com o caminho relativo (separado por `/`, sem `/` no fim).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    /// Caminho relativo à raiz da origem.
    pub name: String,
    /// Tamanho (descompactado) em bytes.
    pub size: u64,
    /// Tipo.
    pub kind: EntryKind,
}

/// Por que um caminho é perigoso, ou `None` se é seguro (relativo, sem `..`, sem drive).
#[must_use]
pub fn unsafe_reason(name: &str) -> Option<&'static str> {
    if name.contains('\\') {
        return Some("backslash");
    }
    if name.starts_with('/') {
        return Some("absolute");
    }
    let bytes = name.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Some("drive");
    }
    if name.split('/').any(|segment| segment == "..") {
        return Some("parent");
    }
    if name
        .chars()
        .any(|c| c == '\0' || (c.is_control() && c != '\t'))
    {
        return Some("control");
    }
    None
}

/// Recusa de caminho perigoso, com o caminho (relativo ao modpack) para a frase.
pub(crate) fn unsafe_path(name: &str, reason: &str) -> Error {
    Error::new(
        Code::UnsafePath,
        format!("caminho perigoso no modpack: {name}"),
    )
    .with_param("path", name)
    .with_param("reason", reason)
}

fn unreadable(file_name: &str, reason: impl std::fmt::Display) -> Error {
    Error::new(Code::Unreadable, format!("{file_name}: {reason}")).with_param("name", file_name)
}

/// Nome do arquivo ou da pasta, sem o caminho (o que pode aparecer em mensagens).
#[must_use]
pub(crate) fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A origem aberta.
pub(crate) enum Source {
    /// Um zip.
    Zip {
        /// O zip.
        archive: Box<zip::ZipArchive<File>>,
        /// Nome do arquivo (para mensagens).
        file_name: String,
        /// Pasta dentro do zip que é a raiz (`""` ou `"Minha instância/"`).
        prefix: String,
        /// Nome completo da entrada → índice no zip.
        index: HashMap<String, usize>,
    },
    /// Uma pasta.
    Dir {
        /// A pasta.
        root: PathBuf,
        /// Nome da pasta (para mensagens).
        file_name: String,
    },
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zip {
                file_name, prefix, ..
            } => f
                .debug_struct("Zip")
                .field("file_name", file_name)
                .field("prefix", prefix)
                .finish_non_exhaustive(),
            Self::Dir { file_name, .. } => f
                .debug_struct("Dir")
                .field("file_name", file_name)
                .finish_non_exhaustive(),
        }
    }
}

impl Source {
    /// Abre o zip e confere todos os nomes de entrada e os limites antes de qualquer leitura.
    ///
    /// Erros: [`Code::Unreadable`], [`Code::UnsafePath`], [`Code::TooLarge`].
    pub(crate) fn open_zip(path: &Path) -> Result<Self> {
        let file_name = display_name(path);
        let file = File::open(path).map_err(|error| unreadable(&file_name, error.kind()))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|error| unreadable(&file_name, format!("não é um zip: {error}")))?;
        if archive.len() > MAX_ENTRIES {
            return Err(too_large(&file_name, "entradas demais"));
        }
        let mut index = HashMap::with_capacity(archive.len());
        let mut total: u64 = 0;
        for position in 0..archive.len() {
            let entry = archive
                .by_index_raw(position)
                .map_err(|error| unreadable(&file_name, error))?;
            let name = entry
                .name()
                .map_err(|error| unreadable(&file_name, error))?
                .into_owned();
            if let Some(reason) = unsafe_reason(&name) {
                return Err(unsafe_path(&name, reason));
            }
            total = total.saturating_add(entry.size());
            if total > MAX_TOTAL_BYTES {
                return Err(too_large(
                    &file_name,
                    "conteúdo descompactado grande demais",
                ));
            }
            index.insert(name, position);
        }
        Ok(Self::Zip {
            archive: Box::new(archive),
            file_name,
            prefix: String::new(),
            index,
        })
    }

    /// Abre a pasta (só confere que existe).
    ///
    /// Erros: [`Code::Unreadable`].
    pub(crate) fn open_dir(path: &Path) -> Result<Self> {
        let file_name = display_name(path);
        if !path.is_dir() {
            return Err(unreadable(&file_name, "não é uma pasta"));
        }
        Ok(Self::Dir {
            root: path.to_path_buf(),
            file_name,
        })
    }

    /// Nome do arquivo ou da pasta de origem.
    #[must_use]
    pub(crate) fn file_name(&self) -> &str {
        match self {
            Self::Zip { file_name, .. } | Self::Dir { file_name, .. } => file_name,
        }
    }

    /// Passa a ler a partir de uma subpasta (o zip de instância do Prism tem a instância
    /// dentro de uma pasta; a instância tem o jogo em `.minecraft/` ou `minecraft/`).
    #[must_use]
    pub(crate) fn within(self, sub: &str) -> Self {
        let sub = sub.trim_matches('/');
        if sub.is_empty() {
            return self;
        }
        match self {
            Self::Zip {
                archive,
                file_name,
                prefix,
                index,
            } => Self::Zip {
                archive,
                file_name,
                prefix: format!("{prefix}{sub}/"),
                index,
            },
            Self::Dir { root, file_name } => Self::Dir {
                root: root.join(sub),
                file_name,
            },
        }
    }

    /// Se existe o arquivo `name` (relativo à raiz atual).
    #[must_use]
    pub(crate) fn has_file(&self, name: &str) -> bool {
        match self {
            Self::Zip { prefix, index, .. } => index.contains_key(&format!("{prefix}{name}")),
            Self::Dir { root, .. } => fs::symlink_metadata(root.join(name))
                .is_ok_and(|metadata| metadata.file_type().is_file()),
        }
    }

    /// Se existe a pasta `name` (relativa à raiz atual).
    #[must_use]
    pub(crate) fn has_dir(&self, name: &str) -> bool {
        match self {
            Self::Zip { prefix, index, .. } => {
                let wanted = format!("{prefix}{}/", name.trim_end_matches('/'));
                index.keys().any(|key| key.starts_with(&wanted))
            }
            Self::Dir { root, .. } => fs::symlink_metadata(root.join(name))
                .is_ok_and(|metadata| metadata.file_type().is_dir()),
        }
    }

    /// Todas as entradas abaixo da raiz atual, em ordem de nome. As pastas implícitas do zip
    /// (sem entrada própria) aparecem também.
    ///
    /// Erros: [`Code::Unreadable`], [`Code::TooLarge`].
    pub(crate) fn entries(&mut self) -> Result<Vec<Entry>> {
        let mut result: Vec<Entry> = match self {
            Self::Zip {
                archive,
                file_name,
                prefix,
                ..
            } => zip_entries(archive, file_name, prefix)?,
            Self::Dir { root, file_name } => {
                let mut entries = Vec::new();
                walk(root, "", &mut entries, file_name)?;
                entries
            }
        };
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Lê o arquivo `name` (relativo à raiz atual) inteiro, até [`MAX_FILE_BYTES`].
    ///
    /// Erros: [`Code::Unreadable`] (inexistente, link ou ilegível), [`Code::TooLarge`].
    pub(crate) fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        if let Some(reason) = unsafe_reason(name) {
            return Err(unsafe_path(name, reason));
        }
        match self {
            Self::Zip {
                archive,
                file_name,
                prefix,
                index,
            } => {
                let full = format!("{prefix}{name}");
                let position = *index
                    .get(&full)
                    .ok_or_else(|| unreadable(file_name, format!("falta {name}")))?;
                let entry = archive
                    .by_index(position)
                    .map_err(|error| unreadable(file_name, format!("{name}: {error}")))?;
                if is_zip_symlink(entry.unix_mode()) {
                    return Err(unreadable(file_name, format!("{name} é um link")));
                }
                read_limited(entry, name, file_name)
            }
            Self::Dir { root, file_name } => {
                let path = warden_core::resolve_inside(root, name)
                    .map_err(|error| unreadable(file_name, format!("{name}: {error}")))?;
                let metadata = fs::symlink_metadata(&path)
                    .map_err(|error| unreadable(file_name, format!("{name}: {}", error.kind())))?;
                if !metadata.file_type().is_file() {
                    return Err(unreadable(file_name, format!("{name} não é um arquivo")));
                }
                let file = File::open(&path)
                    .map_err(|error| unreadable(file_name, format!("{name}: {}", error.kind())))?;
                read_limited(file, name, file_name)
            }
        }
    }

    /// Lê e interpreta um JSON da origem.
    ///
    /// Erros: os de [`Self::read`] e [`Code::Unreadable`] para JSON inválido.
    pub(crate) fn read_json<T: for<'de> serde::Deserialize<'de>>(
        &mut self,
        name: &str,
    ) -> Result<T> {
        let bytes = self.read(name)?;
        let file_name = self.file_name().to_owned();
        serde_json::from_slice(&bytes)
            .map_err(|error| unreadable(&file_name, format!("{name}: {error}")))
    }
}

fn too_large(file_name: &str, reason: &str) -> Error {
    Error::new(Code::TooLarge, format!("{file_name}: {reason}")).with_param("name", file_name)
}

fn is_zip_symlink(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| mode & 0o170_000 == 0o120_000)
}

fn read_limited(reader: impl Read, name: &str, file_name: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| unreadable(file_name, format!("{name}: {error}")))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(too_large(file_name, &format!("{name} é grande demais")));
    }
    Ok(bytes)
}

fn zip_entries(
    archive: &mut zip::ZipArchive<File>,
    file_name: &str,
    prefix: &str,
) -> Result<Vec<Entry>> {
    let mut seen_dirs = std::collections::BTreeSet::new();
    let mut entries = Vec::new();
    for position in 0..archive.len() {
        let entry = archive
            .by_index_raw(position)
            .map_err(|error| unreadable(file_name, error))?;
        let full = entry
            .name()
            .map_err(|error| unreadable(file_name, error))?;
        let Some(name) = full.strip_prefix(prefix) else {
            continue;
        };
        let is_dir = entry.is_dir();
        let name = name.trim_end_matches('/').to_owned();
        if name.is_empty() {
            continue;
        }
        let mut parent = name.as_str();
        while let Some((up, _)) = parent.rsplit_once('/') {
            seen_dirs.insert(up.to_owned());
            parent = up;
        }
        if is_dir {
            seen_dirs.insert(name);
            continue;
        }
        let kind = if is_zip_symlink(entry.unix_mode()) {
            EntryKind::Symlink
        } else {
            EntryKind::File
        };
        entries.push(Entry {
            name,
            size: entry.size(),
            kind,
        });
    }
    entries.extend(seen_dirs.into_iter().map(|name| Entry {
        name,
        size: 0,
        kind: EntryKind::Dir,
    }));
    Ok(entries)
}

fn walk(root: &Path, prefix: &str, entries: &mut Vec<Entry>, file_name: &str) -> Result<()> {
    let directory = if prefix.is_empty() {
        root.to_path_buf()
    } else {
        root.join(prefix)
    };
    let reader = fs::read_dir(&directory)
        .map_err(|error| unreadable(file_name, format!("{prefix}: {}", error.kind())))?;
    for item in reader {
        let item =
            item.map_err(|error| unreadable(file_name, format!("{prefix}: {}", error.kind())))?;
        let name = item.file_name().to_string_lossy().into_owned();
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if entries.len() >= MAX_ENTRIES {
            return Err(too_large(file_name, "arquivos demais"));
        }
        let metadata = fs::symlink_metadata(item.path())
            .map_err(|error| unreadable(file_name, format!("{path}: {}", error.kind())))?;
        let link = metadata.file_type().is_symlink() || is_junction(&metadata);
        if link {
            entries.push(Entry {
                name: path,
                size: 0,
                kind: EntryKind::Symlink,
            });
        } else if metadata.is_dir() {
            entries.push(Entry {
                name: path.clone(),
                size: 0,
                kind: EntryKind::Dir,
            });
            walk(root, &path, entries, file_name)?;
        } else {
            entries.push(Entry {
                name: path,
                size: metadata.len(),
                kind: EntryKind::File,
            });
        }
    }
    Ok(())
}

#[cfg(windows)]
fn is_junction(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_junction(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::io::Write as _;

    use super::*;

    fn zip_with(path: &Path, names: &[&str]) {
        let file = File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for name in names {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"x").unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn nomes_perigosos() {
        assert_eq!(unsafe_reason("mods/a.jar"), None);
        assert_eq!(unsafe_reason("../a"), Some("parent"));
        assert_eq!(unsafe_reason("mods/../../a"), Some("parent"));
        assert_eq!(unsafe_reason("/etc/passwd"), Some("absolute"));
        assert_eq!(unsafe_reason("C:/Windows/a"), Some("drive"));
        assert_eq!(unsafe_reason("c:a"), Some("drive"));
        assert_eq!(unsafe_reason("mods\\a.jar"), Some("backslash"));
        assert_eq!(unsafe_reason("a\0b"), Some("control"));
        // `..` só como segmento inteiro.
        assert_eq!(unsafe_reason("mods/a..b.jar"), None);
    }

    #[test]
    fn zip_com_entrada_perigosa_e_recusado_inteiro() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("mau.zip");
        zip_with(
            &path,
            &["overrides/config/a.toml", "overrides/../../fora.txt"],
        );
        let error = Source::open_zip(&path).unwrap_err();
        assert_eq!(error.code(), Code::UnsafePath);
        assert_eq!(error.params()["path"], "overrides/../../fora.txt");
    }

    #[test]
    fn entradas_com_pastas_implicitas_e_prefixo() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("i.zip");
        zip_with(&path, &["Inst/instance.cfg", "Inst/.minecraft/mods/a.jar"]);
        let mut source = Source::open_zip(&path).unwrap().within("Inst");
        assert!(source.has_file("instance.cfg"));
        assert!(source.has_dir(".minecraft"));
        let names: Vec<(String, EntryKind)> = source
            .entries()
            .unwrap()
            .into_iter()
            .map(|e| (e.name, e.kind))
            .collect();
        assert_eq!(
            names,
            vec![
                (".minecraft".to_owned(), EntryKind::Dir),
                (".minecraft/mods".to_owned(), EntryKind::Dir),
                (".minecraft/mods/a.jar".to_owned(), EntryKind::File),
                ("instance.cfg".to_owned(), EntryKind::File),
            ]
        );
        assert_eq!(source.read("instance.cfg").unwrap(), b"x");
        assert_eq!(source.read("../x").unwrap_err().code(), Code::UnsafePath);
    }

    #[test]
    fn arquivo_que_nao_e_zip_e_ilegivel_sem_a_pasta_na_mensagem() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("quebrado.mrpack");
        fs::write(&path, b"nada de zip").unwrap();
        let error = Source::open_zip(&path).unwrap_err();
        assert_eq!(error.code(), Code::Unreadable);
        assert!(
            !error
                .to_string()
                .contains(&temp.path().display().to_string())
        );
        assert!(error.to_string().contains("quebrado.mrpack"));
    }

    #[cfg(unix)]
    #[test]
    fn pasta_nao_segue_links() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("inst");
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::write(root.join("mods/a.jar"), b"a").unwrap();
        fs::create_dir_all(temp.path().join("fora")).unwrap();
        fs::write(temp.path().join("fora/segredo.txt"), b"s").unwrap();
        std::os::unix::fs::symlink(temp.path().join("fora"), root.join("link")).unwrap();
        let mut source = Source::open_dir(&root).unwrap();
        let entries = source.entries().unwrap();
        assert!(
            entries
                .iter()
                .any(|e| e.name == "link" && e.kind == EntryKind::Symlink)
        );
        assert!(!entries.iter().any(|e| e.name.starts_with("link/")));
        assert!(source.read("link/segredo.txt").is_err());
    }
}
