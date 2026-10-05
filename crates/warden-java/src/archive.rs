//! Extração dos pacotes do Temurin: `.zip` (Windows) e `.tar.gz` (Linux).
//!
//! - Os pacotes do Adoptium têm uma pasta no topo (`jdk-21.0.12.1+1-jre/`); ela é retirada, para
//!   o `bin/` ficar direto na pasta do Java.
//! - Todo caminho passa por [`warden_core::resolve_inside`]: nada é gravado fora do destino
//!   (*zip slip*), nem por `..`, caminho absoluto, nome reservado do Windows ou link.
//! - Limites contra pacotes malformados: número de entradas e tamanho total descompactado.
//! - Links simbólicos do `.tar.gz` só são criados no Linux e só se o alvo ficar dentro do
//!   destino; no Windows são ignorados (o Temurin do Windows não tem links).
//!
//! As funções são síncronas (rodam em `spawn_blocking`) e não limpam o destino em caso de
//! erro: quem chama apaga a pasta temporária inteira.

use std::fs::{self, File};
use std::io::{self, Read, Write as _};
use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

/// Mais entradas aceitas num pacote (um JRE tem ~200 a ~500).
pub const MAX_ENTRIES: usize = 20_000;

/// Maior tamanho descompactado aceito (um JRE tem 100 a 200 MB).
pub const MAX_UNPACKED_BYTES: u64 = 1024 * 1024 * 1024;

/// Formato do pacote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    /// `.zip`.
    Zip,
    /// `.tar.gz`.
    TarGz,
}

impl ArchiveKind {
    /// Formato pelo nome do arquivo.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        let ends = |suffix: &str| {
            lower.len() > suffix.len() && lower.as_bytes().ends_with(suffix.as_bytes())
        };
        if ends(".zip") {
            Some(Self::Zip)
        } else if ends(".tar.gz") || ends(".tgz") {
            Some(Self::TarGz)
        } else {
            None
        }
    }
}

fn archive_error(path: &Path, message: impl Into<String>) -> Error {
    Error::Archive {
        path: path.to_owned(),
        message: message.into(),
    }
}

/// Extrai `archive` em `destination` (que precisa existir e estar vazia).
pub fn extract(archive: &Path, kind: ArchiveKind, destination: &Path) -> Result<()> {
    match kind {
        ArchiveKind::Zip => extract_zip(archive, destination),
        ArchiveKind::TarGz => extract_tar_gz(archive, destination),
    }
}

/// Componentes normais do caminho de uma entrada, com `/` ou `\` como separador. Recusa
/// caminho absoluto, `..` e prefixos de unidade.
fn entry_components(name: &str) -> std::result::Result<Vec<String>, String> {
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') {
        return Err(format!("caminho absoluto no pacote: {name:?}"));
    }
    let mut components = Vec::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => {
                let part = part
                    .to_str()
                    .ok_or_else(|| format!("nome com caracteres inválidos: {name:?}"))?;
                components.push(part.to_owned());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("caminho que sai da pasta no pacote: {name:?}"));
            }
        }
    }
    Ok(components)
}

/// A pasta do topo comum a todas as entradas, se houver uma só e nenhuma entrada for um
/// arquivo solto no topo.
fn common_top(names: &[Vec<String>], is_dir: &[bool]) -> Option<String> {
    let mut top: Option<&str> = None;
    for (components, dir) in names.iter().zip(is_dir) {
        let Some(first) = components.first() else {
            continue;
        };
        if components.len() == 1 && !dir {
            return None;
        }
        match top {
            None => top = Some(first),
            Some(existing) if existing == first => {}
            Some(_) => return None,
        }
    }
    top.map(ToOwned::to_owned)
}

/// Caminho de destino da entrada, já sem a pasta do topo. `None` para a própria pasta do topo.
fn target(
    destination: &Path,
    components: &[String],
    strip: Option<&str>,
    archive: &Path,
) -> Result<Option<PathBuf>> {
    let rest = match strip {
        Some(top) if components.first().map(String::as_str) == Some(top) => &components[1..],
        _ => components,
    };
    if rest.is_empty() {
        return Ok(None);
    }
    let relative = rest.join("/");
    warden_core::resolve_inside(destination, &relative)
        .map(Some)
        .map_err(|error| archive_error(archive, error.to_string()))
}

fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io("criar a pasta", parent, e))?;
    }
    Ok(())
}

/// Copia no máximo `remaining` bytes; erro se a entrada passar disso.
fn copy_limited(
    reader: &mut dyn Read,
    path: &Path,
    remaining: &mut u64,
    archive: &Path,
) -> Result<()> {
    let mut file = File::create(path).map_err(|e| Error::io("criar", path, e))?;
    let mut limited = reader.take(*remaining + 1);
    let written = io::copy(&mut limited, &mut file).map_err(|error| {
        archive_error(
            archive,
            format!("falha ao descompactar {}: {error}", path.display()),
        )
    })?;
    if written > *remaining {
        return Err(archive_error(
            archive,
            format!(
                "o pacote passa de {} MB descompactado",
                MAX_UNPACKED_BYTES / (1024 * 1024)
            ),
        ));
    }
    *remaining -= written;
    file.flush().map_err(|e| Error::io("gravar", path, e))?;
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    // Só os bits de permissão; nada de setuid/setgid.
    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))
        .map_err(|e| Error::io("ajustar a permissão de", path, e))
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // mesma assinatura da versão do Linux
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

fn extract_zip(archive: &Path, destination: &Path) -> Result<()> {
    let file = File::open(archive).map_err(|e| Error::io("abrir", archive, e))?;
    let mut zip = zip::ZipArchive::new(io::BufReader::new(file))
        .map_err(|error| archive_error(archive, format!("zip ilegível: {error}")))?;
    if zip.len() > MAX_ENTRIES {
        return Err(archive_error(
            archive,
            format!("{} entradas é demais", zip.len()),
        ));
    }
    let mut names = Vec::with_capacity(zip.len());
    let mut dirs = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip.by_index_raw(index).map_err(|error| {
            archive_error(archive, format!("entrada {index} ilegível: {error}"))
        })?;
        names.push(
            entry_components(entry.name()).map_err(|message| archive_error(archive, message))?,
        );
        dirs.push(entry.is_dir());
    }
    let strip = common_top(&names, &dirs);
    let mut remaining = MAX_UNPACKED_BYTES;
    for (index, components) in names.iter().enumerate() {
        let mut entry = zip.by_index(index).map_err(|error| {
            archive_error(archive, format!("entrada {index} ilegível: {error}"))
        })?;
        let Some(path) = target(destination, components, strip.as_deref(), archive)? else {
            continue;
        };
        if entry.is_symlink() {
            // Os zips do Temurin não têm links; um link num zip é ignorado, nunca seguido.
            tracing::warn!(entry = entry.name(), "link simbólico no zip ignorado");
            continue;
        }
        if entry.is_dir() {
            fs::create_dir_all(&path).map_err(|e| Error::io("criar a pasta", &path, e))?;
            continue;
        }
        create_parent(&path)?;
        copy_limited(&mut entry, &path, &mut remaining, archive)?;
        if let Some(mode) = entry.unix_mode() {
            set_mode(&path, mode)?;
        }
        warden_core::fault::check("java.extract.entry")
            .map_err(|e| Error::io("extrair", &path, e))?;
    }
    Ok(())
}

fn open_tar(archive: &Path) -> Result<tar::Archive<flate2::read::GzDecoder<io::BufReader<File>>>> {
    let file = File::open(archive).map_err(|e| Error::io("abrir", archive, e))?;
    Ok(tar::Archive::new(flate2::read::GzDecoder::new(
        io::BufReader::new(file),
    )))
}

fn extract_tar_gz(archive: &Path, destination: &Path) -> Result<()> {
    // Primeira passada: nomes, para achar a pasta do topo (o tar não tem índice).
    let mut names = Vec::new();
    let mut dirs = Vec::new();
    {
        let mut tar = open_tar(archive)?;
        let entries = tar
            .entries()
            .map_err(|error| archive_error(archive, format!("tar ilegível: {error}")))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| archive_error(archive, format!("tar ilegível: {error}")))?;
            if names.len() >= MAX_ENTRIES {
                return Err(archive_error(archive, "entradas demais"));
            }
            let path = entry
                .path()
                .map_err(|error| archive_error(archive, format!("nome ilegível: {error}")))?;
            let name = path.to_string_lossy().into_owned();
            names.push(entry_components(&name).map_err(|message| archive_error(archive, message))?);
            dirs.push(entry.header().entry_type().is_dir());
        }
    }
    let strip = common_top(&names, &dirs);
    let mut remaining = MAX_UNPACKED_BYTES;
    let mut tar = open_tar(archive)?;
    let entries = tar
        .entries()
        .map_err(|error| archive_error(archive, format!("tar ilegível: {error}")))?;
    for (index, entry) in entries.enumerate() {
        let mut entry =
            entry.map_err(|error| archive_error(archive, format!("tar ilegível: {error}")))?;
        let Some(components) = names.get(index) else {
            return Err(archive_error(archive, "o tar mudou entre as duas leituras"));
        };
        let Some(path) = target(destination, components, strip.as_deref(), archive)? else {
            continue;
        };
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            fs::create_dir_all(&path).map_err(|e| Error::io("criar a pasta", &path, e))?;
        } else if kind.is_file() || kind == tar::EntryType::Continuous {
            create_parent(&path)?;
            copy_limited(&mut entry, &path, &mut remaining, archive)?;
            if let Ok(mode) = entry.header().mode() {
                set_mode(&path, mode)?;
            }
        } else if kind.is_symlink() || kind.is_hard_link() {
            let link = entry
                .link_name()
                .map_err(|error| archive_error(archive, format!("link ilegível: {error}")))?
                .ok_or_else(|| archive_error(archive, "link sem alvo"))?
                .to_string_lossy()
                .into_owned();
            create_link(
                destination,
                &path,
                &link,
                kind.is_hard_link(),
                strip.as_deref(),
                archive,
            )?;
        } else if kind.is_pax_global_extensions()
            || kind.is_pax_local_extensions()
            || kind.is_gnu_longname()
            || kind.is_gnu_longlink()
        {
            // Cabeçalhos de extensão já foram aplicados pela biblioteca.
        } else {
            return Err(archive_error(
                archive,
                format!(
                    "tipo de entrada não suportado ({kind:?}) em {}",
                    path.display()
                ),
            ));
        }
        warden_core::fault::check("java.extract.entry")
            .map_err(|e| Error::io("extrair", &path, e))?;
    }
    Ok(())
}

/// Cria um link do tar. Link simbólico: alvo relativo à pasta do link, que precisa ficar
/// dentro do destino. Link físico: alvo relativo à raiz do pacote; vira uma cópia.
fn create_link(
    destination: &Path,
    path: &Path,
    link: &str,
    hard: bool,
    strip: Option<&str>,
    archive: &Path,
) -> Result<()> {
    if hard {
        let components =
            entry_components(link).map_err(|message| archive_error(archive, message))?;
        let Some(source) = target(destination, &components, strip, archive)? else {
            return Err(archive_error(
                archive,
                format!("link físico para a pasta do topo: {link:?}"),
            ));
        };
        create_parent(path)?;
        fs::copy(&source, path).map_err(|e| Error::io("copiar", &source, e))?;
        return Ok(());
    }
    // O alvo do link simbólico, resolvido sem seguir nada, precisa ficar dentro do destino.
    let parent = path.parent().unwrap_or(destination);
    let relative_parent = parent.strip_prefix(destination).unwrap_or(Path::new(""));
    let mut resolved: Vec<String> = relative_parent
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => part.to_str().map(ToOwned::to_owned),
            _ => None,
        })
        .collect();
    let normalized = link.replace('\\', "/");
    if normalized.starts_with('/') {
        return Err(archive_error(archive, format!("link absoluto: {link:?}")));
    }
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => resolved.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                if resolved.pop().is_none() {
                    return Err(archive_error(
                        archive,
                        format!("link que sai da pasta: {link:?}"),
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(archive_error(archive, format!("link absoluto: {link:?}")));
            }
        }
    }
    symlink(path, &normalized, archive)
}

#[cfg(unix)]
fn symlink(path: &Path, target: &str, _archive: &Path) -> Result<()> {
    create_parent(path)?;
    std::os::unix::fs::symlink(target, path).map_err(|e| Error::io("criar o link", path, e))
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)] // mesma assinatura da versão do Linux
fn symlink(path: &Path, _target: &str, _archive: &Path) -> Result<()> {
    tracing::debug!(path = %path.display(), "link simbólico do tar ignorado no Windows");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formato_pelo_nome() {
        assert_eq!(
            ArchiveKind::from_name("OpenJDK21U-jre_x64_windows_hotspot_21.0.12.1_1.zip"),
            Some(ArchiveKind::Zip)
        );
        assert_eq!(ArchiveKind::from_name("x.TAR.GZ"), Some(ArchiveKind::TarGz));
        assert_eq!(ArchiveKind::from_name("x.msi"), None);
    }

    #[test]
    fn componentes_recusam_saida_da_pasta() {
        assert_eq!(entry_components("a/./b").unwrap(), vec!["a", "b"]);
        assert_eq!(entry_components("a\\b").unwrap(), vec!["a", "b"]);
        for bad in ["../x", "a/../../x", "/etc/passwd", "\\x"] {
            assert!(entry_components(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn pasta_do_topo_comum() {
        let names = vec![
            vec!["jdk".to_owned()],
            vec!["jdk".to_owned(), "bin".to_owned()],
            vec!["jdk".to_owned(), "bin".to_owned(), "java".to_owned()],
        ];
        assert_eq!(
            common_top(&names, &[true, true, false]).as_deref(),
            Some("jdk")
        );
        let mixed = vec![
            vec!["jdk".to_owned(), "a".to_owned()],
            vec!["outro".to_owned(), "b".to_owned()],
        ];
        assert_eq!(common_top(&mixed, &[false, false]), None);
        let loose = vec![vec!["release".to_owned()]];
        assert_eq!(common_top(&loose, &[false]), None);
    }
}
