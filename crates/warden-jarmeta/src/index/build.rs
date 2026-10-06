//! Montagem do índice de um jar.
//!
//! Os metadados (descritores e a árvore de embutidos) vêm de [`read_jar_bytes`]; daqui só sai a
//! lista de pacotes de cada jar e a presença das configs de mixin. Os embutidos seguidos são os
//! que [`read_jar_bytes`] conseguiu ler, então os mesmos [`Limits`] valem aqui.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

use sha2::{Digest, Sha256};
use zip::ZipArchive;

use super::{INDEX_FORMAT, IndexedJar, IndexedMod, JarIndex, MixinConfigEntry};
use crate::error::{Error, LimitKind};
use crate::limits::Limits;
use crate::model::{DescriptorKind, JarJarEntry, JarMetadata, Loader};
use crate::read::read_jar_bytes;

/// Todos os loaders, para saber quais leriam cada mod.
const LOADERS: [Loader; 4] = [
    Loader::Fabric,
    Loader::Quilt,
    Loader::Forge,
    Loader::NeoForge,
];

/// SHA-256 em hexadecimal minúsculo (o nome do arquivo no cache).
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub(super) fn hex(digest: &[u8]) -> String {
    digest.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// Lê o jar do disco (inteiro na memória, até [`Limits::max_file_bytes`]) e monta o índice.
///
/// # Errors
///
/// Os de [`crate::read_jar_file`]: arquivo ilegível, zip inválido ou limite passado.
pub fn index_jar_file(path: &Path, limits: &Limits) -> Result<JarIndex, Error> {
    let bytes = read_file_capped(path, limits)?;
    index_jar_bytes(&bytes, limits)
}

/// Lê o arquivo inteiro, recusando o que passa de [`Limits::max_file_bytes`].
pub(super) fn read_file_capped(path: &Path, limits: &Limits) -> Result<Vec<u8>, Error> {
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
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
    file.take(limits.max_file_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual > limits.max_file_bytes {
        return Err(Error::LimitExceeded {
            kind: LimitKind::FileSize,
            limit: limits.max_file_bytes,
            actual,
        });
    }
    Ok(bytes)
}

/// Monta o índice de um jar já na memória.
///
/// # Errors
///
/// Os de [`read_jar_bytes`]. Embutidos ilegíveis não são erro: ficam fora do índice (o aviso
/// já está em [`JarMetadata::warnings`]).
pub fn index_jar_bytes(bytes: &[u8], limits: &Limits) -> Result<JarIndex, Error> {
    let meta = read_jar_bytes(bytes, limits)?;
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| Error::InvalidArchive {
        detail: e.to_string(),
    })?;
    let mut walk = Walk {
        limits,
        budget: limits.max_total_nested_bytes,
        declared: BTreeSet::new(),
        found: BTreeSet::new(),
        index: JarIndex {
            format: INDEX_FORMAT,
            sha256: sha256_hex(bytes),
            jars: Vec::new(),
            mixin_configs: Vec::new(),
        },
    };
    walk.declared = meta
        .walk()
        .into_iter()
        .flat_map(|(_, jar)| declared_configs(jar))
        .collect();
    walk.visit(&mut zip, &meta, None, "", None);
    let Walk {
        mut index, found, ..
    } = walk;
    for entry in &mut index.mixin_configs {
        entry.present = found.contains(&entry.config);
    }
    Ok(index)
}

/// Configs de mixin declaradas pelos descritores de um jar (sem descer nos embutidos).
fn declared_configs(meta: &JarMetadata) -> impl Iterator<Item = String> + '_ {
    meta.mods
        .iter()
        .flat_map(|m| m.mixins.iter())
        .chain(meta.manifest.iter().flat_map(|m| m.mixin_configs.iter()))
        .map(|config| config.trim().to_owned())
        .filter(|config| !config.is_empty())
}

/// Estado da descida nos embutidos.
struct Walk<'l> {
    limits: &'l Limits,
    /// Bytes de embutidos que ainda podem ser carregados ([`Limits::max_total_nested_bytes`]).
    budget: u64,
    /// Configs de mixin declaradas em qualquer jar da árvore.
    declared: BTreeSet<String>,
    /// Das declaradas, as que existem em algum jar da árvore.
    found: BTreeSet<String>,
    index: JarIndex,
}

impl Walk<'_> {
    /// Acrescenta o jar e, em profundidade, os embutidos.
    fn visit<R: Read + Seek>(
        &mut self,
        zip: &mut ZipArchive<R>,
        meta: &JarMetadata,
        jarjar: Option<&JarJarEntry>,
        path: &str,
        parent: Option<u32>,
    ) {
        let position = u32::try_from(self.index.jars.len()).unwrap_or(u32::MAX);
        self.index.jars.push(IndexedJar {
            path: path.to_owned(),
            parent,
            mods: indexed_mods(meta),
            artifact: jarjar.map(|j| format!("{}:{}", j.group, j.artifact)),
            artifact_version: jarjar.and_then(|j| j.artifact_version.clone()),
            packages: packages(zip),
        });
        mixin_configs(meta, position, &mut self.index.mixin_configs);
        for config in &self.declared {
            if zip.index_for_name(config.trim_start_matches('/')).is_some() {
                self.found.insert(config.clone());
            }
        }

        for nested in &meta.nested {
            let Some(nested_meta) = nested.metadata.as_deref() else {
                continue;
            };
            let entry = nested.path.trim_start_matches('/');
            let max = self.limits.max_nested_jar_bytes.min(self.budget);
            let Some(bytes) = read_entry(zip, entry, max) else {
                continue;
            };
            self.budget = self
                .budget
                .saturating_sub(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
            let Ok(mut inner) = ZipArchive::new(Cursor::new(bytes)) else {
                continue;
            };
            let child_path = if path.is_empty() {
                nested.path.clone()
            } else {
                format!("{path}!/{}", nested.path)
            };
            self.visit(
                &mut inner,
                nested_meta,
                nested.jarjar.as_ref(),
                &child_path,
                Some(position),
            );
        }
    }
}

/// Bytes de uma entrada, até `max` (descompactados); `None` se faltar, passar ou falhar.
fn read_entry<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str, max: u64) -> Option<Vec<u8>> {
    let index = zip.index_for_name(name)?;
    let file = zip.by_index(index).ok()?;
    let mut buf = Vec::new();
    file.take(max.saturating_add(1))
        .read_to_end(&mut buf)
        .ok()?;
    (u64::try_from(buf.len()).unwrap_or(u64::MAX) <= max).then_some(buf)
}

fn indexed_mods(meta: &JarMetadata) -> Vec<IndexedMod> {
    meta.mods
        .iter()
        .map(|m| IndexedMod {
            id: m.id.clone(),
            version: m.version.clone(),
            source: m.source,
            loaders: LOADERS
                .into_iter()
                .filter(|l| {
                    meta.mods_for_loader(*l)
                        .iter()
                        .any(|other| std::ptr::eq(*other, m))
                })
                .collect(),
        })
        .collect()
}

/// Pacotes com pelo menos uma classe, em notação de ponto. `META-INF/versions/<n>/` conta no
/// pacote normal; o resto de `META-INF/` e as classes sem pacote ficam de fora.
fn packages<R: Read + Seek>(zip: &ZipArchive<R>) -> Vec<String> {
    let mut out = BTreeSet::new();
    for name in zip.file_names() {
        if let Some(package) = package_of_entry(name) {
            out.insert(package);
        }
    }
    out.into_iter().collect()
}

/// Pacote de uma entrada `.class` do zip (`a/b/C.class` → `a.b`).
pub(super) fn package_of_entry(name: &str) -> Option<String> {
    let stem = name
        .len()
        .checked_sub(".class".len())
        .filter(|&cut| name.is_char_boundary(cut))
        .filter(|&cut| name[cut..].eq_ignore_ascii_case(".class"))
        .map(|cut| &name[..cut])?;
    let stem = match stem.strip_prefix("META-INF/versions/") {
        Some(rest) => rest.split_once('/')?.1,
        None if stem.starts_with("META-INF/") => return None,
        None => stem,
    };
    let (package, class) = stem.rsplit_once('/')?;
    // Um nome de pacote Java não tem segmento vazio nem ponto dentro do segmento.
    if class.is_empty() || package.split('/').any(|s| s.is_empty() || s.contains('.')) {
        return None;
    }
    Some(package.replace('/', "."))
}

fn mixin_configs(meta: &JarMetadata, jar: u32, out: &mut Vec<MixinConfigEntry>) {
    let mut push = |config: &str, declared_in: DescriptorKind, mod_id: Option<&str>| {
        let config = config.trim();
        if config.is_empty() {
            return;
        }
        let entry = MixinConfigEntry {
            config: config.to_owned(),
            jar,
            declared_in,
            mod_id: mod_id.map(str::to_owned),
            present: false,
        };
        if !out.contains(&entry) {
            out.push(entry);
        }
    };
    for module in &meta.mods {
        for config in &module.mixins {
            push(config, module.source, Some(&module.id));
        }
    }
    if let Some(manifest) = &meta.manifest {
        for config in &manifest.mixin_configs {
            push(config, DescriptorKind::Manifest, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacote_de_cada_entrada() {
        let cases = [
            ("a/b/C.class", Some("a.b")),
            ("a/b/C$D.class", Some("a.b")),
            ("a/B.CLASS", Some("a")),
            ("C.class", None),
            ("module-info.class", None),
            ("META-INF/versions/17/a/b/C.class", Some("a.b")),
            ("META-INF/versions/17/C.class", None),
            ("META-INF/versions/", None),
            ("META-INF/x/C.class", None),
            ("a/b/c.txt", None),
            ("a//C.class", None),
            ("/C.class", None),
            ("a/.class", None),
            ("a/./B.class", None),
            ("a/../B.class", None),
            ("ç/é/Ü.class", Some("ç.é")),
            (".class", None),
            ("ass", None),
        ];
        for (name, expected) in cases {
            assert_eq!(package_of_entry(name).as_deref(), expected, "{name}");
        }
    }

    #[test]
    fn hash_em_hexadecimal() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
