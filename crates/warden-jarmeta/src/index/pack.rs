//! Montagem do índice de um pack, com o cache e a leitura em paralelo.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::build::{hex, index_jar_bytes, read_file_capped};
use super::{JarIndex, JarIndexCache, PackIndex};
use crate::error::Error;
use crate::limits::Limits;
use crate::model::Loader;

/// Um jar do pack a indexar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackJar {
    /// Identificação do item no pack (o caminho do metafile, `mods/sodium.pw.toml`). É o que as
    /// consultas devolvem.
    pub item: String,
    /// Onde está o jar (na instância).
    pub path: PathBuf,
    /// SHA-256 do jar, se o chamador já sabe (evita ler o arquivo para calcular).
    pub sha256: Option<String>,
}

impl PackJar {
    /// Jar sem hash conhecido.
    #[must_use]
    pub fn new(item: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            item: item.into(),
            path: path.into(),
            sha256: None,
        }
    }
}

/// Números da montagem.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildStats {
    /// Jars pedidos.
    pub jars: usize,
    /// Índices que vieram do cache.
    pub cache_hits: usize,
    /// Jars abertos e lidos como zip (zero quando tudo veio do cache).
    pub jars_opened: usize,
    /// Índices que não deu para gravar no cache.
    pub cache_write_failures: usize,
}

/// Jar que não entrou no índice.
#[derive(Debug)]
pub struct JarFailure {
    /// O item.
    pub item: String,
    /// O motivo.
    pub error: Error,
}

/// Resultado de [`build_pack_index`].
#[derive(Debug)]
pub struct PackIndexBuild {
    /// O índice (sem os jars que falharam).
    pub index: PackIndex,
    /// Números da montagem.
    pub stats: BuildStats,
    /// Jars que não deu para ler. Não são erro do índice: o diagnóstico já avisa sobre jars
    /// ilegíveis.
    pub failures: Vec<JarFailure>,
}

/// Monta o índice do pack, usando o cache quando existe e gravando nele o que faltava. Os jars
/// que precisam ser lidos são lidos em paralelo.
///
/// `loader` é o loader do pack: escolhe o mod principal de jars multi-loader e desempata
/// configs de mixin declaradas em mais de um item.
#[must_use]
pub fn build_pack_index(
    jars: &[PackJar],
    loader: Option<Loader>,
    cache: Option<&JarIndexCache>,
    limits: &Limits,
) -> PackIndexBuild {
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .clamp(1, 8)
        .min(jars.len().max(1));
    let chunk = jars.len().div_ceil(workers).max(1);
    let results: Vec<(Result<Indexed, Error>, &PackJar)> = std::thread::scope(|scope| {
        let handles: Vec<_> = jars
            .chunks(chunk)
            .map(|part| {
                let handle = scope.spawn(move || {
                    part.iter()
                        .map(|jar| (index_one(jar, cache, limits), jar))
                        .collect::<Vec<_>>()
                });
                (part, handle)
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|(part, handle)| {
                // Uma leitura não entra em pânico (QUALITY §2); se entrar, os jars do bloco
                // aparecem como falha em vez de sumir do resultado.
                handle.join().unwrap_or_else(|_| {
                    part.iter()
                        .map(|jar| {
                            let error = Error::Read {
                                path: jar.path.clone(),
                                source: std::io::Error::other("a leitura do índice parou"),
                            };
                            (Err(error), jar)
                        })
                        .collect()
                })
            })
            .collect()
    });

    let mut stats = BuildStats {
        jars: jars.len(),
        ..BuildStats::default()
    };
    let mut entries = Vec::new();
    let mut failures = Vec::new();
    for (result, jar) in results {
        match result {
            Ok(indexed) => {
                stats.cache_hits += usize::from(indexed.from_cache);
                stats.jars_opened += usize::from(!indexed.from_cache);
                stats.cache_write_failures += usize::from(indexed.write_failed);
                entries.push((jar.item.clone(), indexed.index));
            }
            Err(error) => failures.push(JarFailure {
                item: jar.item.clone(),
                error,
            }),
        }
    }
    PackIndexBuild {
        index: PackIndex::new(entries, loader),
        stats,
        failures,
    }
}

struct Indexed {
    index: JarIndex,
    from_cache: bool,
    write_failed: bool,
}

fn index_one(
    jar: &PackJar,
    cache: Option<&JarIndexCache>,
    limits: &Limits,
) -> Result<Indexed, Error> {
    let known = jar
        .sha256
        .as_deref()
        .map(str::to_ascii_lowercase)
        .filter(|sha| sha.len() == 64);
    if let (Some(cache), Some(sha)) = (cache, &known)
        && let Some(index) = cache.load(sha)
    {
        return Ok(Indexed {
            index,
            from_cache: true,
            write_failed: false,
        });
    }
    // Sem hash conhecido: calcula lendo o arquivo em blocos, sem abrir o zip.
    let sha = match (known, cache) {
        (Some(sha), _) => Some(sha),
        (None, Some(_)) => Some(hash_file(&jar.path)?),
        (None, None) => None,
    };
    if let (Some(cache), Some(sha)) = (cache, &sha)
        && let Some(index) = cache.load(sha)
    {
        return Ok(Indexed {
            index,
            from_cache: true,
            write_failed: false,
        });
    }
    let bytes = read_file_capped(&jar.path, limits)?;
    let index = index_jar_bytes(&bytes, limits)?;
    let write_failed = cache.is_some_and(|c| c.store(&index).is_err());
    Ok(Indexed {
        index,
        from_cache: false,
        write_failed,
    })
}

fn hash_file(path: &Path) -> Result<String, Error> {
    let read_error = |source| Error::Read {
        path: path.to_owned(),
        source,
    };
    let mut reader = BufReader::new(File::open(path).map_err(read_error)?);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(read_error)?;
        if n == 0 {
            break;
        }
        hasher.update(buf.get(..n).unwrap_or_default());
    }
    Ok(hex(&hasher.finalize()))
}
