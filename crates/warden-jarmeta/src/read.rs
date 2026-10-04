//! Leitura de um jar sem extrair (R3 §4.5 item 1).
//!
//! Abre o zip, lê só os arquivos de metadados (com limite de tamanho contado nos bytes
//! descompactados), o cabeçalho de algumas classes para a versão do Java e, recursivamente, os
//! jars embutidos declarados (`jars[]` do Fabric e do Quilt, `META-INF/jarjar/metadata.json`,
//! `ContainedDeps` do manifesto do Forge 1.12.2). Nada vai para o disco.

use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

use zip::ZipArchive;
use zip::result::ZipError;

use crate::descriptor::{self, Sink};
use crate::error::{Error, LimitKind};
use crate::limits::Limits;
use crate::model::{ClassVersion, DescriptorKind, JarMetadata, NestedJar, Warning, WarningCode};
use crate::text;

/// Lê os metadados de um jar no disco.
///
/// # Errors
///
/// [`Error::Read`] se o arquivo não abre, [`Error::LimitExceeded`] se passa dos limites e
/// [`Error::InvalidArchive`] se não é um zip. Problemas nos descritores e nos jars embutidos
/// não são erros: ficam em [`JarMetadata::warnings`].
pub fn read_jar_file(path: &Path, limits: &Limits) -> Result<JarMetadata, Error> {
    let read_error = |source| Error::Read {
        path: path.to_owned(),
        source,
    };
    let file = File::open(path).map_err(read_error)?;
    let size = file.metadata().map_err(read_error)?.len();
    if size > limits.max_file_bytes {
        return Err(Error::LimitExceeded {
            kind: LimitKind::FileSize,
            limit: limits.max_file_bytes,
            actual: size,
        });
    }
    read_jar(BufReader::new(file), limits).map_err(|e| match e {
        OpenError::Io(source) => read_error(source),
        other => other.into_error(),
    })
}

/// Lê os metadados de um jar já na memória.
///
/// # Errors
///
/// Os mesmos de [`read_jar_file`], exceto a leitura do disco.
pub fn read_jar_bytes(bytes: &[u8], limits: &Limits) -> Result<JarMetadata, Error> {
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size > limits.max_file_bytes {
        return Err(Error::LimitExceeded {
            kind: LimitKind::FileSize,
            limit: limits.max_file_bytes,
            actual: size,
        });
    }
    read_jar(Cursor::new(bytes), limits).map_err(OpenError::into_error)
}

/// Falha ao abrir um zip (interna; vira [`Error`] no jar de cima e aviso nos embutidos).
#[derive(Debug)]
enum OpenError {
    Io(std::io::Error),
    Invalid(String),
    TooManyEntries { limit: usize, actual: usize },
}

impl OpenError {
    fn into_error(self) -> Error {
        match self {
            Self::Io(e) => Error::InvalidArchive {
                detail: e.to_string(),
            },
            Self::Invalid(detail) => Error::InvalidArchive { detail },
            Self::TooManyEntries { limit, actual } => Error::LimitExceeded {
                kind: LimitKind::EntryCount,
                limit: u64::try_from(limit).unwrap_or(u64::MAX),
                actual: u64::try_from(actual).unwrap_or(u64::MAX),
            },
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Io(e) => e.to_string(),
            Self::Invalid(detail) => detail.clone(),
            Self::TooManyEntries { limit, actual } => {
                format!("{actual} entradas passam do limite de {limit}")
            }
        }
    }
}

impl From<ZipError> for OpenError {
    fn from(e: ZipError) -> Self {
        match e {
            ZipError::Io(io) => Self::Io(io),
            other => Self::Invalid(other.to_string()),
        }
    }
}

/// Orçamento compartilhado por todos os jars embutidos de uma leitura.
struct Budget {
    nested_bytes_left: u64,
}

fn read_jar<R: Read + Seek>(reader: R, limits: &Limits) -> Result<JarMetadata, OpenError> {
    let mut budget = Budget {
        nested_bytes_left: limits.max_total_nested_bytes,
    };
    read_archive(reader, limits, 0, &mut budget)
}

/// Resultado de ler uma entrada do zip.
enum Entry {
    Missing,
    Bytes(Vec<u8>),
    TooLarge,
    Unreadable(String),
}

fn read_entry<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str, max: u64) -> Entry {
    let Some(index) = zip.index_for_name(name) else {
        return Entry::Missing;
    };
    let file = match zip.by_index(index) {
        Ok(f) => f,
        Err(e) => return Entry::Unreadable(e.to_string()),
    };
    let mut buf = Vec::new();
    match file.take(max.saturating_add(1)).read_to_end(&mut buf) {
        Ok(_) if u64::try_from(buf.len()).unwrap_or(u64::MAX) > max => Entry::TooLarge,
        Ok(_) => Entry::Bytes(buf),
        Err(e) => Entry::Unreadable(e.to_string()),
    }
}

/// Lê um descritor de texto; avisa se estiver grande demais, ilegível ou fora do UTF-8.
fn read_text<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    kind: DescriptorKind,
    limits: &Limits,
    warnings: &mut Vec<Warning>,
) -> Option<String> {
    let path = kind.path();
    let mut warn = |code, detail: String| {
        warnings.push(Warning {
            code,
            path: path.to_owned(),
            detail,
        });
    };
    match read_entry(zip, path, limits.max_descriptor_bytes) {
        Entry::Missing => None,
        Entry::TooLarge => {
            warn(
                WarningCode::EntryTooLarge,
                format!(
                    "passa de {} bytes descompactado",
                    limits.max_descriptor_bytes
                ),
            );
            None
        }
        Entry::Unreadable(e) => {
            warn(WarningCode::UnreadableEntry, e);
            None
        }
        Entry::Bytes(bytes) => {
            let decoded = text::decode(&bytes);
            if !decoded.was_utf8 {
                warn(
                    WarningCode::NotUtf8,
                    "não é UTF-8; lido como Windows-1252".to_owned(),
                );
            }
            Some(decoded.text)
        }
    }
}

fn read_archive<R: Read + Seek>(
    reader: R,
    limits: &Limits,
    depth: u32,
    budget: &mut Budget,
) -> Result<JarMetadata, OpenError> {
    let mut zip = ZipArchive::new(reader)?;
    if zip.len() > limits.max_entries {
        return Err(OpenError::TooManyEntries {
            limit: limits.max_entries,
            actual: zip.len(),
        });
    }
    let mut meta = JarMetadata::default();
    let mut warnings = Vec::new();
    let mut nested: Vec<(String, DescriptorKind, Option<crate::model::JarJarEntry>)> = Vec::new();

    let manifest_text = read_text(&mut zip, DescriptorKind::Manifest, limits, &mut warnings);
    let manifest = manifest_text.as_deref().map(descriptor::manifest::parse);
    let jar_version = manifest
        .as_ref()
        .and_then(|m| m.implementation_version.clone());

    for kind in [
        DescriptorKind::QuiltModJson,
        DescriptorKind::FabricModJson,
        DescriptorKind::NeoForgeModsToml,
        DescriptorKind::ModsToml,
        DescriptorKind::McmodInfo,
        DescriptorKind::JarJarMetadata,
    ] {
        if zip.index_for_name(kind.path()).is_none() {
            continue;
        }
        meta.descriptors.push(kind);
        let Some(content) = read_text(&mut zip, kind, limits, &mut warnings) else {
            continue;
        };
        let mut sink = Sink {
            path: kind.path(),
            warnings: &mut warnings,
        };
        match kind {
            DescriptorKind::QuiltModJson => {
                if let Some(q) = descriptor::quilt::parse(&content, &mut sink) {
                    meta.mods.push(q.module);
                    nested.extend(q.jars.into_iter().map(|p| (p, kind, None)));
                }
            }
            DescriptorKind::FabricModJson => {
                if let Some(f) = descriptor::fabric::parse(&content, &mut sink) {
                    meta.mods.push(f.module);
                    nested.extend(f.jars.into_iter().map(|p| (p, kind, None)));
                }
            }
            DescriptorKind::NeoForgeModsToml | DescriptorKind::ModsToml => {
                if let Some(t) =
                    descriptor::forge_toml::parse(&content, kind, jar_version.as_deref(), &mut sink)
                {
                    meta.mods.extend(t.mods);
                    if meta.language_loader.is_none() {
                        meta.language_loader = t.language_loader;
                    }
                }
            }
            DescriptorKind::McmodInfo => {
                meta.mods
                    .extend(descriptor::mcmod_info::parse(&content, &mut sink));
            }
            DescriptorKind::JarJarMetadata => {
                for item in descriptor::jarjar::parse(&content, &mut sink) {
                    nested.push((item.path, kind, Some(item.entry)));
                }
            }
            DescriptorKind::Manifest => {}
        }
    }
    if let Some(m) = &manifest {
        meta.descriptors.push(DescriptorKind::Manifest);
        for dep in &m.contained_deps {
            nested.push((format!("META-INF/{dep}"), DescriptorKind::Manifest, None));
        }
    }
    meta.manifest = manifest;
    meta.class_version = class_version(&mut zip, limits.max_class_headers);

    let mut seen: Vec<String> = Vec::new();
    for (path, declared_by, jarjar) in nested {
        let lookup = path.trim_start_matches('/').to_owned();
        if seen.contains(&lookup) {
            continue;
        }
        seen.push(lookup.clone());
        let metadata = read_nested(&mut zip, &lookup, limits, depth, budget, &mut warnings);
        meta.nested.push(NestedJar {
            path,
            declared_by,
            jarjar,
            metadata: metadata.map(Box::new),
        });
    }
    meta.warnings = warnings;
    Ok(meta)
}

fn read_nested<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    path: &str,
    limits: &Limits,
    depth: u32,
    budget: &mut Budget,
    warnings: &mut Vec<Warning>,
) -> Option<JarMetadata> {
    let mut warn = |code, detail: String| {
        warnings.push(Warning {
            code,
            path: path.to_owned(),
            detail,
        });
    };
    if depth + 1 > limits.max_depth {
        warn(
            WarningCode::UnreadableNestedJar,
            format!(
                "jar embutido além da profundidade máxima ({})",
                limits.max_depth
            ),
        );
        return None;
    }
    let max = limits.max_nested_jar_bytes.min(budget.nested_bytes_left);
    match read_entry(zip, path, max) {
        Entry::Missing => {
            warn(
                WarningCode::MissingNestedJar,
                "declarado, mas ausente no jar".to_owned(),
            );
            None
        }
        Entry::TooLarge => {
            warn(
                WarningCode::UnreadableNestedJar,
                format!("passa do limite de {max} bytes para jars embutidos"),
            );
            None
        }
        Entry::Unreadable(e) => {
            warn(WarningCode::UnreadableNestedJar, e);
            None
        }
        Entry::Bytes(bytes) => {
            let used = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            budget.nested_bytes_left = budget.nested_bytes_left.saturating_sub(used);
            match read_archive(Cursor::new(bytes), limits, depth + 1, budget) {
                Ok(m) => Some(m),
                Err(e) => {
                    warn(WarningCode::UnreadableNestedJar, e.describe());
                    None
                }
            }
        }
    }
}

/// Maior versão de classe entre as primeiras `max` classes fora de `META-INF/versions/`.
fn class_version<R: Read + Seek>(zip: &mut ZipArchive<R>, max: usize) -> Option<ClassVersion> {
    let candidates: Vec<usize> = (0..zip.len())
        .filter(|&i| {
            zip.name_for_index(i).is_some_and(|name| {
                Path::new(name)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("class"))
                    && !name.starts_with("META-INF/versions/")
            })
        })
        .take(max)
        .collect();
    let mut best: Option<ClassVersion> = None;
    for index in candidates {
        let Ok(mut file) = zip.by_index(index) else {
            continue;
        };
        let mut header = [0u8; 8];
        if file.read_exact(&mut header).is_err() {
            continue;
        }
        if let Some(version) = parse_class_header(header) {
            best = Some(best.map_or(version, |b| b.max(version)));
        }
    }
    best
}

/// Versão de um arquivo `.class` pelo cabeçalho (`CAFEBABE`, minor, major).
pub(crate) fn parse_class_header(header: [u8; 8]) -> Option<ClassVersion> {
    if header[..4] != [0xCA, 0xFE, 0xBA, 0xBE] {
        return None;
    }
    Some(ClassVersion {
        minor: u16::from_be_bytes([header[4], header[5]]),
        major: u16::from_be_bytes([header[6], header[7]]),
    })
}
