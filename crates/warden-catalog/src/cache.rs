//! Cache persistente do catálogo em `cache/metadata.sqlite` (ARCHITECTURE §13 e §17).
//!
//! - Respostas das fontes (manifesto, listas do Fabric, Forge e NeoForge): guardadas como
//!   vieram, com a hora em que foram baixadas. Valem [`LIST_TTL`] (6 h, a revalidação do
//!   manifesto da ARCHITECTURE §17.1); sem rede, servem mesmo vencidas, e a interface mostra
//!   a data.
//! - JSON das versões do Minecraft: pelo `sha1` do manifesto (o conteúdo é imutável), sem
//!   vencimento.
//!
//! O arquivo é dividido com a `warden-modrinth`: as tabelas desta crate têm o prefixo
//! `catalog_` e a versão do esquema fica em `catalog_meta`. Cache é descartável: um arquivo
//! corrompido é apagado e recriado, e um esquema de outra versão do Warden só limpa as tabelas
//! `catalog_*`. O caminho vem de `AppPaths::metadata_db_file`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, ErrorCode, OptionalExtension as _, params};

use crate::error::{Error, Result};

/// Validade das respostas das fontes.
pub const LIST_TTL: Duration = Duration::from_hours(6);

/// Versão do esquema das tabelas `catalog_*`.
const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS catalog_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS catalog_documents (
    key TEXT PRIMARY KEY,
    body BLOB NOT NULL,
    fetched_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS catalog_version_json (
    sha1 TEXT PRIMARY KEY,
    id TEXT NOT NULL,
    body BLOB NOT NULL,
    fetched_at_ms INTEGER NOT NULL
);
";

/// Relógio de parede em milissegundos Unix (injetável nos testes).
pub type WallClock = Arc<dyn Fn() -> u64 + Send + Sync>;

fn system_clock() -> WallClock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            })
    })
}

/// Uma resposta guardada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedDocument {
    /// O corpo, como veio.
    pub body: Vec<u8>,
    /// Quando foi baixado (milissegundos Unix).
    pub fetched_at_ms: u64,
    /// Se ainda vale.
    pub fresh: bool,
}

/// Cache do catálogo. Barato de clonar.
#[derive(Clone)]
pub struct CatalogCache {
    inner: Arc<Inner>,
}

struct Inner {
    connection: Mutex<Connection>,
    clock: WallClock,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for CatalogCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CatalogCache")
            .field("path", &self.inner.path)
            .finish_non_exhaustive()
    }
}

impl CatalogCache {
    /// Abre (ou cria) o cache em `path`, criando a pasta. Um arquivo corrompido é apagado e
    /// recriado.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_clock(path, system_clock())
    }

    /// Como [`Self::open`], com o relógio dado.
    pub fn open_with_clock(path: &Path, clock: WallClock) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|source| Error::CacheIo {
                action: "criar a pasta",
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let connection = match open_connection(path) {
            Ok(connection) => connection,
            Err(error) if is_corruption(&error) => {
                tracing::warn!(caminho = %path.display(), %error, "cache do catálogo corrompido; recriando");
                remove_database(path)?;
                open_connection(path).map_err(Error::cache("abrir"))?
            }
            Err(error) => return Err(Error::cache("abrir")(error)),
        };
        Ok(Self::from_connection(
            connection,
            clock,
            Some(path.to_path_buf()),
        ))
    }

    /// Cache só em memória (testes e uso sem disco).
    pub fn in_memory(clock: Option<WallClock>) -> Result<Self> {
        let connection = Connection::open_in_memory().map_err(Error::cache("abrir"))?;
        prepare(&connection).map_err(Error::cache("criar as tabelas"))?;
        Ok(Self::from_connection(
            connection,
            clock.unwrap_or_else(system_clock),
            None,
        ))
    }

    fn from_connection(connection: Connection, clock: WallClock, path: Option<PathBuf>) -> Self {
        Self {
            inner: Arc::new(Inner {
                connection: Mutex::new(connection),
                clock,
                path,
            }),
        }
    }

    /// Caminho do arquivo (`None` em memória).
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.inner.path.as_deref()
    }

    /// Agora, pelo relógio do cache (milissegundos Unix).
    #[must_use]
    pub fn now_ms(&self) -> u64 {
        (self.inner.clock)()
    }

    /// Roda `work` com a conexão fora do executor assíncrono.
    async fn run<T, F>(&self, action: &'static str, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> rusqlite::Result<T> + Send + 'static,
    {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            // Um pânico em outra consulta não deixa a conexão inconsistente: cada consulta é
            // uma transação do SQLite.
            let mut connection = inner
                .connection
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            work(&mut connection)
        })
        .await
        .map_err(|e| Error::Internal(format!("tarefa do cache: {e}")))?
        .map_err(Error::cache(action))
    }

    /// Uma resposta guardada pela chave.
    pub async fn document(&self, key: &str) -> Result<Option<CachedDocument>> {
        let key = key.to_owned();
        let row = self
            .run("ler uma lista", move |connection| {
                connection
                    .query_row(
                        "SELECT body, fetched_at_ms FROM catalog_documents WHERE key = ?1",
                        [key],
                        |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)),
                    )
                    .optional()
            })
            .await?;
        let now = self.now_ms();
        Ok(row.map(|(body, fetched_at)| {
            let fetched_at_ms = u64::try_from(fetched_at).unwrap_or(0);
            CachedDocument {
                body,
                fetched_at_ms,
                fresh: is_fresh(fetched_at_ms, now, LIST_TTL),
            }
        }))
    }

    /// Grava uma resposta com a hora de agora; devolve essa hora.
    pub async fn put_document(&self, key: &str, body: Vec<u8>) -> Result<u64> {
        let key = key.to_owned();
        let now = self.now_ms();
        let stored = i64::try_from(now).unwrap_or(i64::MAX);
        self.run("gravar uma lista", move |connection| {
            connection.execute(
                "INSERT INTO catalog_documents (key, body, fetched_at_ms) VALUES (?1, ?2, ?3)
                 ON CONFLICT (key) DO UPDATE SET body = excluded.body,
                     fetched_at_ms = excluded.fetched_at_ms",
                params![key, body, stored],
            )?;
            Ok(())
        })
        .await?;
        Ok(now)
    }

    /// O JSON de uma versão pelo `sha1`.
    pub async fn version_json(&self, sha1: &str) -> Result<Option<Vec<u8>>> {
        let sha1 = sha1.to_ascii_lowercase();
        self.run("ler o JSON de uma versão", move |connection| {
            connection
                .query_row(
                    "SELECT body FROM catalog_version_json WHERE sha1 = ?1",
                    [sha1],
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .optional()
        })
        .await
    }

    /// Grava o JSON de uma versão (o `sha1` já conferido).
    pub async fn put_version_json(&self, sha1: &str, id: &str, body: Vec<u8>) -> Result<()> {
        let sha1 = sha1.to_ascii_lowercase();
        let id = id.to_owned();
        let now = i64::try_from(self.now_ms()).unwrap_or(i64::MAX);
        self.run("gravar o JSON de uma versão", move |connection| {
            connection.execute(
                "INSERT INTO catalog_version_json (sha1, id, body, fetched_at_ms)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (sha1) DO NOTHING",
                params![sha1, id, body, now],
            )?;
            Ok(())
        })
        .await
    }
}

fn open_connection(path: &Path) -> rusqlite::Result<Connection> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    prepare(&connection)?;
    Ok(connection)
}

fn prepare(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(SCHEMA)?;
    let version: Option<i64> = connection
        .query_row(
            "SELECT value FROM catalog_meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .and_then(|value| value.parse().ok());
    if version != Some(SCHEMA_VERSION) {
        // Esquema desconhecido (de outra versão do Warden): o cache é descartável.
        connection
            .execute_batch("DELETE FROM catalog_documents; DELETE FROM catalog_version_json;")?;
        connection.execute(
            "INSERT INTO catalog_meta (key, value) VALUES ('schema_version', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [SCHEMA_VERSION.to_string()],
        )?;
    }
    Ok(())
}

fn is_corruption(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt)
    )
}

fn remove_database(path: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let file = PathBuf::from(name);
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::CacheIo {
                    action: "apagar",
                    path: file,
                    source,
                });
            }
        }
    }
    Ok(())
}

fn is_fresh(fetched_at_ms: u64, now_ms: u64, ttl: Duration) -> bool {
    let ttl = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX);
    now_ms >= fetched_at_ms && now_ms - fetched_at_ms < ttl
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn clock() -> (WallClock, Arc<AtomicU64>) {
        let now = Arc::new(AtomicU64::new(1_000_000_000));
        let reader = Arc::clone(&now);
        (Arc::new(move || reader.load(Ordering::SeqCst)), now)
    }

    #[tokio::test]
    async fn lista_vale_6_horas() {
        let (clock, now) = clock();
        let cache = CatalogCache::in_memory(Some(clock)).unwrap();
        assert!(cache.path().is_none());
        assert!(cache.document("mojang").await.unwrap().is_none());
        let at = cache.put_document("mojang", b"{}".to_vec()).await.unwrap();
        assert_eq!(at, 1_000_000_000);
        let cached = cache.document("mojang").await.unwrap().unwrap();
        assert!(cached.fresh);
        assert_eq!(cached.body, b"{}");
        now.fetch_add(6 * 60 * 60 * 1000 - 1, Ordering::SeqCst);
        assert!(cache.document("mojang").await.unwrap().unwrap().fresh);
        now.fetch_add(1, Ordering::SeqCst);
        let stale = cache.document("mojang").await.unwrap().unwrap();
        assert!(!stale.fresh);
        assert_eq!(stale.fetched_at_ms, 1_000_000_000);
        // Relógio que voltou no tempo: não confia no cache.
        now.store(0, Ordering::SeqCst);
        assert!(!cache.document("mojang").await.unwrap().unwrap().fresh);
    }

    #[tokio::test]
    async fn json_da_versao_pelo_sha1() {
        let cache = CatalogCache::in_memory(None).unwrap();
        let sha = "A".repeat(40);
        assert!(cache.version_json(&sha).await.unwrap().is_none());
        cache
            .put_version_json(&sha, "1.7.10", b"um".to_vec())
            .await
            .unwrap();
        cache
            .put_version_json(&sha, "1.7.10", b"dois".to_vec())
            .await
            .unwrap();
        assert_eq!(
            cache.version_json(&"a".repeat(40)).await.unwrap().unwrap(),
            b"um"
        );
    }

    #[tokio::test]
    async fn arquivo_persiste_entre_aberturas() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache").join("metadata.sqlite");
        {
            let cache = CatalogCache::open(&path).unwrap();
            assert_eq!(cache.path(), Some(path.as_path()));
            cache.put_document("k", b"v".to_vec()).await.unwrap();
        }
        let cache = CatalogCache::open(&path).unwrap();
        assert_eq!(cache.document("k").await.unwrap().unwrap().body, b"v");
        assert!(format!("{cache:?}").contains("metadata.sqlite"));
    }

    #[tokio::test]
    async fn arquivo_corrompido_e_recriado() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.sqlite");
        fs::write(
            &path,
            b"isto nao e um banco sqlite, so texto qualquer para ocupar espaco",
        )
        .unwrap();
        let cache = CatalogCache::open(&path).unwrap();
        cache.put_document("k", b"v".to_vec()).await.unwrap();
        assert!(cache.document("k").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn esquema_de_outra_versao_so_limpa_as_tabelas_do_catalogo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.sqlite");
        {
            let cache = CatalogCache::open(&path).unwrap();
            cache.put_document("k", b"v".to_vec()).await.unwrap();
        }
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "UPDATE catalog_meta SET value = '0' WHERE key = 'schema_version'",
                    [],
                )
                .unwrap();
            connection
                .execute_batch("CREATE TABLE modrinth_x (a); INSERT INTO modrinth_x VALUES (1);")
                .unwrap();
        }
        let cache = CatalogCache::open(&path).unwrap();
        assert!(cache.document("k").await.unwrap().is_none());
        drop(cache);
        let connection = Connection::open(&path).unwrap();
        let count: i64 = connection
            .query_row("SELECT count(*) FROM modrinth_x", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn pasta_que_nao_pode_ser_criada() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("arquivo");
        fs::write(&file, b"x").unwrap();
        let error = CatalogCache::open(&file.join("sub").join("metadata.sqlite")).unwrap_err();
        assert!(matches!(error, Error::CacheIo { .. }), "{error}");
    }
}
