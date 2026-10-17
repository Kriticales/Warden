//! Montagem do índice de um pack, com o cache e a leitura em paralelo.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::build::{index_jar_bytes_hashed, read_file_capped, sha256_hex};
use super::cache::file_identity;
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
    /// Jars lidos inteiros do disco, para calcular o hash ou montar o índice (zero quando o
    /// cache reconheceu todos os arquivos pela identidade, sem ler o conteúdo).
    pub jars_read: usize,
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
/// que precisam ser lidos são lidos em paralelo: cada leitor pega o próximo jar da fila, então
/// um mod de centenas de MB não segura os outros atrás dele.
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
    let next = AtomicUsize::new(0);
    let mut slots: Vec<Option<Result<Indexed, Error>>> = jars.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let position = next.fetch_add(1, Ordering::Relaxed);
                        let Some(jar) = jars.get(position) else {
                            break done;
                        };
                        done.push((position, index_one(jar, cache, limits)));
                    }
                })
            })
            .collect();
        for handle in handles {
            // Uma leitura não entra em pânico (QUALITY §2); se entrar, os jars daquele leitor
            // aparecem como falha (abaixo) em vez de sumir do resultado.
            for (position, result) in handle.join().unwrap_or_default() {
                if let Some(slot) = slots.get_mut(position) {
                    *slot = Some(result);
                }
            }
        }
    });
    let results = slots.into_iter().zip(jars).map(|(slot, jar)| {
        let result = slot.unwrap_or_else(|| {
            Err(Error::Read {
                path: jar.path.clone(),
                source: std::io::Error::other("a leitura do índice parou"),
            })
        });
        (result, jar)
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
                stats.jars_read += usize::from(indexed.read);
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
    /// O jar foi lido inteiro do disco.
    read: bool,
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
    let hit = |index, read| {
        Ok(Indexed {
            index,
            from_cache: true,
            read,
            write_failed: false,
        })
    };
    let Some(cache) = cache else {
        let bytes = read_file_capped(&jar.path, limits)?;
        let index = index_jar_bytes_hashed(&bytes, sha256_hex(&bytes), limits)?;
        return Ok(Indexed {
            index,
            from_cache: false,
            read: true,
            write_failed: false,
        });
    };
    if let Some(sha) = &known
        && let Some(index) = cache.load(sha)
    {
        return hit(index, false);
    }
    // Sem hash conhecido: o arquivo já visto é reconhecido pela identidade, lendo só o fim dele.
    let identity = if known.is_none() {
        file_identity(&jar.path)
    } else {
        None
    };
    if let Some(identity) = &identity
        && let Some(sha) = cache.known_sha256(identity)
        && let Some(index) = cache.load(&sha)
    {
        return hit(index, false);
    }
    // Lê o jar uma vez só: o hash e o índice saem dos mesmos bytes.
    let bytes = read_file_capped(&jar.path, limits)?;
    let sha = sha256_hex(&bytes);
    let remember = |sha: &str| {
        identity
            .as_deref()
            .is_some_and(|identity| cache.remember_sha256(identity, sha).is_err())
    };
    if let Some(index) = cache.load(&sha) {
        let write_failed = remember(&sha);
        return Ok(Indexed {
            index,
            from_cache: true,
            read: true,
            write_failed,
        });
    }
    let index = index_jar_bytes_hashed(&bytes, sha, limits)?;
    let write_failed = cache.store(&index).is_err() | remember(&index.sha256);
    Ok(Indexed {
        index,
        from_cache: false,
        read: true,
        write_failed,
    })
}
