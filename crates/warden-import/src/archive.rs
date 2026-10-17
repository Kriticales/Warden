//! Leitura da origem da importação: um zip (`.mrpack`, zip da CurseForge, zip de instância do
//! Prism) ou uma pasta (instância do Prism/MultiMC ou do app da CurseForge), com a mesma
//! interface.
//!
//! Segurança (ARCHITECTURE §20): todo nome de entrada do zip é conferido antes de qualquer
//! outra coisa (*zip slip*: `..`, caminho absoluto, letra de drive, barra invertida); um nome
//! perigoso recusa o arquivo inteiro. A leitura tem limites de quantidade de entradas, de
//! tamanho descompactado e de razão de compressão (proteção contra *zip bomb*). O tamanho que o
//! zip declara não é confiável: cada leitura para no tamanho declarado (uma entrada que rende
//! mais do que declara é recusada) e o limite total soma os bytes realmente lidos. O zip que
//! outro programa extrai (o `packwiz curseforge import`) é conferido antes por
//! [`Source::verify_sizes`]. Zips dentro do zip não são abertos: viram arquivos comuns. Na
//! pasta, links simbólicos e junções nunca são seguidos. Nada aqui escreve no disco.

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
/// Maior razão entre o tamanho descompactado e o compactado de uma entrada. O deflate não passa
/// de ~1032:1 (bytes repetidos); arquivos de verdade de um modpack ficam muito abaixo de 100:1.
pub(crate) const MAX_RATIO: u64 = 500;
/// Entradas até este tamanho não passam pela conferência da razão (um arquivo pequeno e
/// repetitivo não ameaça o disco).
pub(crate) const RATIO_MIN_BYTES: u64 = 1024 * 1024;

/// Os limites da leitura ([`Limits::default`] são os de produção; os testes usam menores).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    /// Mais entradas do que isto: recusado.
    pub entries: usize,
    /// Maior arquivo lido de uma vez.
    pub file_bytes: u64,
    /// Soma dos tamanhos (declarados e lidos) aceita.
    pub total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: MAX_ENTRIES,
            file_bytes: MAX_FILE_BYTES,
            total_bytes: MAX_TOTAL_BYTES,
        }
    }
}

/// Os limites e os bytes já lidos desta origem.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Budget {
    /// Limites.
    limits: Limits,
    /// Bytes realmente lidos até agora (somados a cada leitura).
    read: u64,
}

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
        /// Limites e bytes lidos.
        budget: Budget,
    },
    /// Uma pasta.
    Dir {
        /// A pasta.
        root: PathBuf,
        /// Nome da pasta (para mensagens).
        file_name: String,
        /// Limites e bytes lidos.
        budget: Budget,
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
        Self::open_zip_with(path, Limits::default())
    }

    /// [`Self::open_zip`] com outros limites.
    pub(crate) fn open_zip_with(path: &Path, limits: Limits) -> Result<Self> {
        let file_name = display_name(path);
        let file = File::open(path).map_err(|error| unreadable(&file_name, error.kind()))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|error| unreadable(&file_name, format!("não é um zip: {error}")))?;
        if archive.len() > limits.entries {
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
            if total > limits.total_bytes {
                return Err(too_large(
                    &file_name,
                    "conteúdo descompactado grande demais",
                ));
            }
            if entry.size() > RATIO_MIN_BYTES
                && entry.size() / entry.compressed_size().max(1) > MAX_RATIO
            {
                return Err(too_large(
                    &file_name,
                    &format!("{name} tem compressão grande demais"),
                ));
            }
            index.insert(name, position);
        }
        Ok(Self::Zip {
            archive: Box::new(archive),
            file_name,
            prefix: String::new(),
            index,
            budget: Budget { limits, read: 0 },
        })
    }

    /// Descompacta o zip inteiro sem gravar nada, conferindo que cada entrada rende no máximo o
    /// que declara e que a soma não passa do limite. Usado antes de entregar o zip a outro
    /// programa que o extrai sozinho (o `packwiz curseforge import`). Na pasta, não faz nada.
    ///
    /// Erros: [`Code::Unreadable`], [`Code::TooLarge`].
    pub(crate) fn verify_sizes(&mut self) -> Result<()> {
        let Self::Zip {
            archive,
            file_name,
            budget,
            ..
        } = self
        else {
            return Ok(());
        };
        let mut total: u64 = 0;
        for position in 0..archive.len() {
            let entry = archive
                .by_index(position)
                .map_err(|error| unreadable(file_name, error))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry
                .name()
                .map_err(|error| unreadable(file_name, error))?
                .into_owned();
            let declared = entry.size();
            let read = std::io::copy(&mut entry.take(declared + 1), &mut std::io::sink())
                .map_err(|error| read_error(&error, &name, file_name))?;
            if read > declared {
                return Err(too_large(
                    file_name,
                    &format!("{name} rende mais do que declara"),
                ));
            }
            total = total.saturating_add(read);
            if total > budget.limits.total_bytes {
                return Err(too_large(file_name, "conteúdo descompactado grande demais"));
            }
        }
        Ok(())
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
            budget: Budget::default(),
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
                budget,
            } => Self::Zip {
                archive,
                file_name,
                prefix: format!("{prefix}{sub}/"),
                index,
                budget,
            },
            Self::Dir {
                root,
                file_name,
                budget,
            } => Self::Dir {
                root: root.join(sub),
                file_name,
                budget,
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
            Self::Dir {
                root,
                file_name,
                budget,
            } => {
                let mut entries = Vec::new();
                walk(root, "", &mut entries, file_name, budget.limits.entries)?;
                entries
            }
        };
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Lê o arquivo `name` (relativo à raiz atual) inteiro, até o tamanho declarado e o
    /// [`Limits::file_bytes`]; os bytes lidos somam no limite total da origem.
    ///
    /// Erros: [`Code::Unreadable`] (inexistente, link ou ilegível), [`Code::TooLarge`] (maior
    /// que o limite, rende mais do que declara ou a soma lida passa do limite total).
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
                budget,
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
                let declared = entry.size();
                read_limited(entry, declared, budget, name, file_name)
            }
            Self::Dir {
                root,
                file_name,
                budget,
            } => {
                let path = warden_core::resolve_inside(root, name)
                    .map_err(|error| unreadable(file_name, format!("{name}: {error}")))?;
                let metadata = fs::symlink_metadata(&path)
                    .map_err(|error| unreadable(file_name, format!("{name}: {}", error.kind())))?;
                if !metadata.file_type().is_file() {
                    return Err(unreadable(file_name, format!("{name} não é um arquivo")));
                }
                let file = File::open(&path)
                    .map_err(|error| unreadable(file_name, format!("{name}: {}", error.kind())))?;
                read_limited(file, metadata.len(), budget, name, file_name)
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

/// Erro de leitura de uma entrada. O `zip` 9 também confere o tamanho declarado e falha ao
/// passar dele, com `InvalidData` e esta frase: é a mesma recusa de [`read_limited`] (*zip bomb*
/// com tamanho falso), não um zip ilegível.
fn read_error(error: &std::io::Error, name: &str, file_name: &str) -> Error {
    if error.kind() == std::io::ErrorKind::InvalidData
        && error
            .to_string()
            .contains("larger than its declared uncompressed size")
    {
        return too_large(file_name, &format!("{name} rende mais do que declara"));
    }
    unreadable(file_name, format!("{name}: {error}"))
}

/// Lê no máximo `declared + 1` bytes (e nunca mais que o limite por arquivo + 1): passar do
/// declarado é sinal de *zip bomb* com tamanho falso e recusa a origem.
fn read_limited(
    reader: impl Read,
    declared: u64,
    budget: &mut Budget,
    name: &str,
    file_name: &str,
) -> Result<Vec<u8>> {
    let cap = declared.min(budget.limits.file_bytes);
    let mut bytes = Vec::new();
    reader
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| read_error(&error, name, file_name))?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > budget.limits.file_bytes {
        return Err(too_large(file_name, &format!("{name} é grande demais")));
    }
    if read > declared {
        return Err(too_large(
            file_name,
            &format!("{name} rende mais do que declara"),
        ));
    }
    budget.read = budget.read.saturating_add(read);
    if budget.read > budget.limits.total_bytes {
        return Err(too_large(file_name, "conteúdo descompactado grande demais"));
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
        let full = entry.name().map_err(|error| unreadable(file_name, error))?;
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

fn walk(
    root: &Path,
    prefix: &str,
    entries: &mut Vec<Entry>,
    file_name: &str,
    max_entries: usize,
) -> Result<()> {
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
        if entries.len() >= max_entries {
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
            walk(root, &path, entries, file_name, max_entries)?;
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

    /// Zip com entradas deflate (nome, bytes reais).
    fn zip_bytes(path: &Path, entries: &[(&str, Vec<u8>)]) {
        let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    /// Troca o tamanho descompactado declarado (diretório central e cabeçalho local) das
    /// entradas que começam com `prefix`: o truque da *zip bomb* com tamanho falso.
    fn lie_sizes(path: &Path, prefix: &str, declared: u32) {
        let mut bytes = fs::read(path).unwrap();
        let u16_at = |b: &[u8], at: usize| usize::from(u16::from_le_bytes([b[at], b[at + 1]]));
        let u32_at = |b: &[u8], at: usize| {
            usize::try_from(u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])).unwrap()
        };
        let end = bytes.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
        let count = u16_at(&bytes, end + 10);
        let mut at = u32_at(&bytes, end + 16);
        for _ in 0..count {
            assert_eq!(&bytes[at..at + 4], b"PK\x01\x02");
            let name_len = u16_at(&bytes, at + 28);
            let extra_len = u16_at(&bytes, at + 30);
            let comment_len = u16_at(&bytes, at + 32);
            let local = u32_at(&bytes, at + 42);
            let name = String::from_utf8(bytes[at + 46..at + 46 + name_len].to_vec()).unwrap();
            if name.starts_with(prefix) {
                bytes[at + 24..at + 28].copy_from_slice(&declared.to_le_bytes());
                bytes[local + 22..local + 26].copy_from_slice(&declared.to_le_bytes());
            }
            at += 46 + name_len + extra_len + comment_len;
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn entrada_que_rende_mais_do_que_declara_e_recusada() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("bomba.mrpack");
        let real = vec![7_u8; 4 * 1024 * 1024];
        zip_bytes(&path, &[("ok.txt", b"ok".to_vec()), ("bomba.bin", real)]);
        lie_sizes(&path, "bomba", 1);
        let mut source = Source::open_zip(&path).unwrap();
        assert_eq!(source.read("ok.txt").unwrap(), b"ok");
        let error = source.read("bomba.bin").unwrap_err();
        assert_eq!(error.code(), Code::TooLarge);
        assert!(error.to_string().contains("rende mais do que declara"));
        // A conferência por streaming (antes do packwiz) também recusa.
        let error = Source::open_zip(&path).unwrap().verify_sizes().unwrap_err();
        assert_eq!(error.code(), Code::TooLarge);
    }

    #[test]
    fn verify_sizes_aceita_zip_honesto_e_ignora_pasta() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("honesto.zip");
        zip_bytes(
            &path,
            &[("a/b.txt", b"abc".to_vec()), ("c.txt", Vec::new())],
        );
        Source::open_zip(&path).unwrap().verify_sizes().unwrap();
        Source::open_dir(temp.path())
            .unwrap()
            .verify_sizes()
            .unwrap();
    }

    #[test]
    fn razao_de_compressao_absurda_e_recusada_na_abertura() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("zeros.zip");
        // 8 MiB de zeros viram ~8 KiB: razão perto de 1000:1.
        zip_bytes(
            &path,
            &[
                ("pequeno.txt", vec![0_u8; 1024]),
                ("zeros.bin", vec![0_u8; 8 * 1024 * 1024]),
            ],
        );
        let error = Source::open_zip(&path).unwrap_err();
        assert_eq!(error.code(), Code::TooLarge);
        assert!(error.to_string().contains("zeros.bin"));
        // Arquivo pequeno e repetitivo não é recusado.
        let path = temp.path().join("pequeno.zip");
        zip_bytes(&path, &[("pequeno.txt", vec![0_u8; 1024 * 1024])]);
        assert!(Source::open_zip(&path).is_ok());
    }

    #[test]
    fn muitas_entradas_pequenas_que_somam_demais() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("muitas.zip");
        let entries: Vec<(String, Vec<u8>)> = (0..50)
            .map(|n| (format!("config/{n}.txt"), vec![b'a'; 100]))
            .collect();
        let borrowed: Vec<(&str, Vec<u8>)> = entries
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.clone()))
            .collect();
        zip_bytes(&path, &borrowed);
        // Entradas demais.
        let limits = Limits {
            entries: 49,
            ..Limits::default()
        };
        let error = Source::open_zip_with(&path, limits).unwrap_err();
        assert_eq!(error.code(), Code::TooLarge);
        // A soma declarada passa do total.
        let limits = Limits {
            total_bytes: 4_999,
            ..Limits::default()
        };
        let error = Source::open_zip_with(&path, limits).unwrap_err();
        assert_eq!(error.code(), Code::TooLarge);
        // Os bytes realmente lidos somam no total, mesmo relendo a mesma entrada.
        let limits = Limits {
            total_bytes: 5_000,
            ..Limits::default()
        };
        let mut source = Source::open_zip_with(&path, limits).unwrap();
        for _ in 0..50 {
            source.read("config/0.txt").unwrap();
        }
        let error = source.read("config/0.txt").unwrap_err();
        assert_eq!(error.code(), Code::TooLarge);
    }

    #[test]
    fn arquivo_maior_que_o_limite_por_arquivo() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("grande.zip");
        zip_bytes(&path, &[("grande.bin", vec![9_u8; 2048])]);
        let limits = Limits {
            file_bytes: 1024,
            ..Limits::default()
        };
        let error = Source::open_zip_with(&path, limits)
            .unwrap()
            .read("grande.bin")
            .unwrap_err();
        assert_eq!(error.code(), Code::TooLarge);
        assert!(error.to_string().contains("grande demais"));
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
