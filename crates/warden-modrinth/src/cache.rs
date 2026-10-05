//! Cache persistente do Modrinth em `cache/metadata.sqlite` (ARCHITECTURE §13 e §17).
//!
//! - Projetos: valem 24 h. Além do JSON cru, `status`, `updated` e `game_versions` ficam em
//!   colunas próprias (gancho 1.1 da ADR-0039, item 7: a manutenção dos mods da W-04 lê esses
//!   campos sem pedir de novo à API; ver [`MetadataCache::project_summary`]).
//! - Versões: imutáveis, não vencem. Uma versão vinda de uma lista sem notas
//!   (`include_changelog=false`) fica marcada como incompleta e não atende quem pede a versão
//!   avulsa (que traz as notas).
//! - Listas de referência (loaders, versões do Minecraft, categorias): valem 24 h.
//! - Sem rede, o cliente usa o que houver, mesmo vencido.
//!
//! O arquivo é dividido com o catálogo (P1-05): as tabelas desta crate têm o prefixo
//! `modrinth_`. Cache é descartável: um arquivo corrompido é apagado e recriado. O caminho vem
//! de `AppPaths::metadata_db_file`, e os testes usam uma pasta temporária.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, ErrorCode, OptionalExtension as _, params};

use crate::error::{Error, Result};
use crate::model::{Project, ProjectStatus, Version};

/// Validade dos projetos.
pub const PROJECT_TTL: Duration = Duration::from_hours(24);
/// Validade das listas de referência.
pub const TAG_TTL: Duration = Duration::from_hours(24);

/// Versão do esquema das tabelas `modrinth_*`.
const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS modrinth_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS modrinth_projects (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL,
    status TEXT NOT NULL,
    updated TEXT NOT NULL,
    game_versions TEXT NOT NULL,
    json TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS modrinth_projects_slug ON modrinth_projects (slug);
CREATE TABLE IF NOT EXISTS modrinth_versions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    complete INTEGER NOT NULL,
    json TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS modrinth_versions_project ON modrinth_versions (project_id);
CREATE TABLE IF NOT EXISTS modrinth_tags (
    kind TEXT PRIMARY KEY,
    json TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
);
";

/// Relógio de parede em segundos Unix (injetável nos testes).
pub type UnixClock = Arc<dyn Fn() -> i64 + Send + Sync>;

fn system_clock() -> UnixClock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
            })
    })
}

/// Um valor do cache e se ainda está na validade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cached<T> {
    /// O valor.
    pub value: T,
    /// Quando foi buscado (segundos Unix).
    pub fetched_at: i64,
    /// Se ainda vale.
    pub fresh: bool,
}

/// Campos do projeto guardados em colunas (gancho 1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    /// ID.
    pub id: String,
    /// Situação.
    pub status: ProjectStatus,
    /// Última atualização (RFC 3339).
    pub updated: String,
    /// Versões do Minecraft.
    pub game_versions: Vec<String>,
    /// Quando foi buscado (segundos Unix).
    pub fetched_at: i64,
}

/// Cache do Modrinth. Barato de clonar.
#[derive(Clone)]
pub struct MetadataCache {
    inner: Arc<Inner>,
}

struct Inner {
    connection: Mutex<Connection>,
    clock: UnixClock,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for MetadataCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetadataCache")
            .field("path", &self.inner.path)
            .finish_non_exhaustive()
    }
}

impl MetadataCache {
    /// Abre (ou cria) o cache em `path`, criando a pasta. Um arquivo corrompido é apagado e
    /// recriado.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_clock(path, system_clock())
    }

    /// Como [`Self::open`], com o relógio dado.
    pub fn open_with_clock(path: &Path, clock: UnixClock) -> Result<Self> {
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
                tracing::warn!(caminho = %path.display(), %error, "cache do Modrinth corrompido; recriando");
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
    pub fn in_memory(clock: Option<UnixClock>) -> Result<Self> {
        let connection = Connection::open_in_memory().map_err(Error::cache("abrir"))?;
        prepare(&connection).map_err(Error::cache("criar as tabelas"))?;
        Ok(Self::from_connection(
            connection,
            clock.unwrap_or_else(system_clock),
            None,
        ))
    }

    fn from_connection(connection: Connection, clock: UnixClock, path: Option<PathBuf>) -> Self {
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

    fn now(&self) -> i64 {
        (self.inner.clock)()
    }

    /// Roda `work` com a conexão fora do executor assíncrono.
    async fn run<T, F>(&self, action: &'static str, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection, i64) -> rusqlite::Result<T> + Send + 'static,
    {
        let inner = Arc::clone(&self.inner);
        let now = self.now();
        tokio::task::spawn_blocking(move || {
            // Um pânico em outra consulta não deixa a conexão inconsistente: cada consulta é
            // uma transação do SQLite.
            let mut connection = inner
                .connection
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            work(&mut connection, now)
        })
        .await
        .map_err(|e| Error::Internal(format!("tarefa do cache: {e}")))?
        .map_err(Error::cache(action))
    }

    /// Projeto pelo ID ou slug.
    pub async fn project(&self, id_or_slug: &str) -> Result<Option<Cached<Project>>> {
        let key = id_or_slug.to_owned();
        let row = self
            .run("ler um projeto", move |connection, _| {
                connection
                    .query_row(
                        "SELECT json, fetched_at FROM modrinth_projects
                         WHERE id = ?1 OR slug = lower(?1) LIMIT 1",
                        [key],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                    )
                    .optional()
            })
            .await?;
        Ok(row.and_then(|(json, fetched_at)| {
            parse_row::<Project>(&json).map(|value| Cached {
                value,
                fetched_at,
                fresh: is_fresh(fetched_at, self.now(), PROJECT_TTL),
            })
        }))
    }

    /// Projetos pelos IDs (os que estiverem no cache).
    pub async fn projects(&self, ids: &[String]) -> Result<Vec<Cached<Project>>> {
        let mut found = Vec::new();
        for id in ids {
            if let Some(project) = self.project(id).await? {
                found.push(project);
            }
        }
        Ok(found)
    }

    /// Grava projetos (JSON cru de cada um).
    pub async fn put_projects(&self, projects: Vec<(Project, String)>) -> Result<()> {
        self.run("gravar projetos", move |connection, now| {
            let transaction = connection.transaction()?;
            {
                let mut statement = transaction.prepare(
                    "INSERT INTO modrinth_projects
                         (id, slug, status, updated, game_versions, json, fetched_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT (id) DO UPDATE SET
                         slug = excluded.slug, status = excluded.status,
                         updated = excluded.updated, game_versions = excluded.game_versions,
                         json = excluded.json, fetched_at = excluded.fetched_at",
                )?;
                for (project, json) in &projects {
                    statement.execute(params![
                        project.id,
                        project.slug.to_lowercase(),
                        status_text(project.status),
                        project.updated,
                        serde_json::to_string(&project.game_versions).unwrap_or_default(),
                        json,
                        now,
                    ])?;
                }
            }
            transaction.commit()
        })
        .await
    }

    /// `status`, `updated` e `game_versions` de um projeto, lidos das colunas (gancho 1.1).
    pub async fn project_summary(&self, id: &str) -> Result<Option<ProjectSummary>> {
        let id = id.to_owned();
        let row = self
            .run("ler um projeto", move |connection, _| {
                connection
                    .query_row(
                        "SELECT id, status, updated, game_versions, fetched_at
                         FROM modrinth_projects WHERE id = ?1",
                        [id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, i64>(4)?,
                            ))
                        },
                    )
                    .optional()
            })
            .await?;
        Ok(row.map(
            |(id, status, updated, game_versions, fetched_at)| ProjectSummary {
                id,
                status: serde_json::from_value(serde_json::Value::String(status))
                    .unwrap_or(ProjectStatus::Unknown),
                updated,
                game_versions: serde_json::from_str(&game_versions).unwrap_or_default(),
                fetched_at,
            },
        ))
    }

    /// Versões pelos IDs. Com `need_changelog`, só as gravadas com as notas.
    pub async fn versions(&self, ids: &[String], need_changelog: bool) -> Result<Vec<Version>> {
        let ids = ids.to_vec();
        let rows = self
            .run("ler versões", move |connection, _| {
                let mut statement = connection
                    .prepare("SELECT json, complete FROM modrinth_versions WHERE id = ?1")?;
                let mut rows = Vec::new();
                for id in &ids {
                    let row = statement
                        .query_row([id], |row| {
                            Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
                        })
                        .optional()?;
                    rows.extend(row);
                }
                Ok(rows)
            })
            .await?;
        Ok(rows
            .into_iter()
            .filter(|(_, complete)| *complete || !need_changelog)
            .filter_map(|(json, _)| parse_row::<Version>(&json))
            .collect())
    }

    /// Grava versões. `complete` diz se vieram com as notas; uma versão completa nunca é
    /// trocada por uma incompleta.
    pub async fn put_versions(
        &self,
        versions: Vec<(Version, String)>,
        complete: bool,
    ) -> Result<()> {
        self.run("gravar versões", move |connection, now| {
            let transaction = connection.transaction()?;
            {
                let mut statement = transaction.prepare(
                    "INSERT INTO modrinth_versions (id, project_id, complete, json, fetched_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT (id) DO UPDATE SET
                         project_id = excluded.project_id, complete = excluded.complete,
                         json = excluded.json, fetched_at = excluded.fetched_at
                     WHERE excluded.complete >= modrinth_versions.complete",
                )?;
                for (version, json) in &versions {
                    statement.execute(params![
                        version.id,
                        version.project_id,
                        complete,
                        json,
                        now
                    ])?;
                }
            }
            transaction.commit()
        })
        .await
    }

    /// Lista de referência (`loader`, `game_version`, `category`), como JSON cru.
    pub async fn tag(&self, kind: &str) -> Result<Option<Cached<String>>> {
        let kind = kind.to_owned();
        let row = self
            .run("ler uma lista", move |connection, _| {
                connection
                    .query_row(
                        "SELECT json, fetched_at FROM modrinth_tags WHERE kind = ?1",
                        [kind],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                    )
                    .optional()
            })
            .await?;
        Ok(row.map(|(value, fetched_at)| Cached {
            value,
            fetched_at,
            fresh: is_fresh(fetched_at, self.now(), TAG_TTL),
        }))
    }

    /// Grava uma lista de referência.
    pub async fn put_tag(&self, kind: &str, json: String) -> Result<()> {
        let kind = kind.to_owned();
        self.run("gravar uma lista", move |connection, now| {
            connection.execute(
                "INSERT INTO modrinth_tags (kind, json, fetched_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (kind) DO UPDATE SET json = excluded.json,
                     fetched_at = excluded.fetched_at",
                params![kind, json, now],
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
            "SELECT value FROM modrinth_meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .and_then(|value| value.parse().ok());
    if version != Some(SCHEMA_VERSION) {
        // Esquema desconhecido (de outra versão do Warden): o cache é descartável.
        connection.execute_batch(
            "DELETE FROM modrinth_projects; DELETE FROM modrinth_versions;
             DELETE FROM modrinth_tags;",
        )?;
        connection.execute(
            "INSERT INTO modrinth_meta (key, value) VALUES ('schema_version', ?1)
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

fn is_fresh(fetched_at: i64, now: i64, ttl: Duration) -> bool {
    let ttl = i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX);
    now >= fetched_at && now - fetched_at < ttl
}

fn status_text(status: ProjectStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

/// Lê uma linha; JSON que não lê mais (versão antiga do modelo) conta como ausente.
fn parse_row<T: serde::de::DeserializeOwned>(json: &str) -> Option<T> {
    match serde_json::from_str(json) {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::warn!(%error, "linha do cache do Modrinth ilegível; ignorada");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    fn clock() -> (UnixClock, Arc<AtomicI64>) {
        let now = Arc::new(AtomicI64::new(1_000_000));
        let reader = Arc::clone(&now);
        (Arc::new(move || reader.load(Ordering::SeqCst)), now)
    }

    fn project(id: &str, slug: &str) -> (Project, String) {
        let json = serde_json::json!({
            "id": id, "slug": slug, "project_type": "mod", "title": slug,
            "status": "archived", "updated": "2026-09-20T21:27:09Z",
            "game_versions": ["1.20.1", "1.21.1"], "campo_novo": 1
        })
        .to_string();
        (serde_json::from_str(&json).unwrap(), json)
    }

    fn version(id: &str, changelog: Option<&str>) -> (Version, String) {
        let json = serde_json::json!({
            "id": id, "project_id": "P", "version_number": "1", "version_type": "release",
            "changelog": changelog
        })
        .to_string();
        (serde_json::from_str(&json).unwrap(), json)
    }

    #[tokio::test]
    async fn projeto_vale_24_horas_e_e_achado_pelo_slug() {
        let (clock, now) = clock();
        let cache = MetadataCache::in_memory(Some(clock)).unwrap();
        assert!(cache.path().is_none());
        assert!(cache.project("AAA").await.unwrap().is_none());
        cache
            .put_projects(vec![project("AAA", "Sodium")])
            .await
            .unwrap();
        let cached = cache.project("sodium").await.unwrap().unwrap();
        assert!(cached.fresh);
        assert_eq!(cached.value.id, "AAA");
        assert_eq!(
            cache.project("SODIUM").await.unwrap().unwrap().value.id,
            "AAA"
        );
        now.fetch_add(24 * 60 * 60 - 1, Ordering::SeqCst);
        assert!(cache.project("AAA").await.unwrap().unwrap().fresh);
        now.fetch_add(1, Ordering::SeqCst);
        let stale = cache.project("AAA").await.unwrap().unwrap();
        assert!(!stale.fresh);
        assert_eq!(stale.fetched_at, 1_000_000);
        assert_eq!(
            cache
                .projects(&["AAA".into(), "BBB".into()])
                .await
                .unwrap()
                .len(),
            1
        );
    }

    /// Gancho 1.1 (ADR-0039, item 7).
    #[tokio::test]
    async fn gancho_1_1_status_updated_e_game_versions_em_colunas() {
        let cache = MetadataCache::in_memory(None).unwrap();
        cache
            .put_projects(vec![project("AAA", "sodium")])
            .await
            .unwrap();
        let summary = cache.project_summary("AAA").await.unwrap().unwrap();
        assert_eq!(summary.status, ProjectStatus::Archived);
        assert_eq!(summary.updated, "2026-09-20T21:27:09Z");
        assert_eq!(summary.game_versions, vec!["1.20.1", "1.21.1"]);
        assert!(cache.project_summary("ZZZ").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn versao_completa_nao_e_trocada_por_incompleta() {
        let cache = MetadataCache::in_memory(None).unwrap();
        cache
            .put_versions(vec![version("V1", None)], false)
            .await
            .unwrap();
        let ids = vec!["V1".to_owned()];
        assert_eq!(cache.versions(&ids, false).await.unwrap().len(), 1);
        assert!(cache.versions(&ids, true).await.unwrap().is_empty());
        cache
            .put_versions(vec![version("V1", Some("notas"))], true)
            .await
            .unwrap();
        cache
            .put_versions(vec![version("V1", None)], false)
            .await
            .unwrap();
        let versions = cache.versions(&ids, true).await.unwrap();
        assert_eq!(versions[0].changelog.as_deref(), Some("notas"));
    }

    #[tokio::test]
    async fn listas_de_referencia_vencem() {
        let (clock, now) = clock();
        let cache = MetadataCache::in_memory(Some(clock)).unwrap();
        assert!(cache.tag("loader").await.unwrap().is_none());
        cache.put_tag("loader", "[]".into()).await.unwrap();
        assert!(cache.tag("loader").await.unwrap().unwrap().fresh);
        now.fetch_add(TAG_TTL.as_secs().try_into().unwrap(), Ordering::SeqCst);
        let stale = cache.tag("loader").await.unwrap().unwrap();
        assert!(!stale.fresh);
        assert_eq!(stale.value, "[]");
        // Relógio que voltou no tempo: não confia no cache.
        now.store(0, Ordering::SeqCst);
        assert!(!cache.tag("loader").await.unwrap().unwrap().fresh);
    }

    #[tokio::test]
    async fn arquivo_persiste_entre_aberturas() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache").join("metadata.sqlite");
        {
            let cache = MetadataCache::open(&path).unwrap();
            assert_eq!(cache.path(), Some(path.as_path()));
            cache
                .put_projects(vec![project("AAA", "sodium")])
                .await
                .unwrap();
        }
        let cache = MetadataCache::open(&path).unwrap();
        assert!(cache.project("AAA").await.unwrap().is_some());
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
        let cache = MetadataCache::open(&path).unwrap();
        cache.put_tag("loader", "[]".into()).await.unwrap();
        assert!(cache.tag("loader").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn esquema_de_outra_versao_e_descartado() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.sqlite");
        {
            let cache = MetadataCache::open(&path).unwrap();
            cache.put_tag("loader", "[]".into()).await.unwrap();
        }
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute(
                    "UPDATE modrinth_meta SET value = '0' WHERE key = 'schema_version'",
                    [],
                )
                .unwrap();
            // Tabela de outra crate no mesmo arquivo: fica intacta.
            connection
                .execute_batch("CREATE TABLE catalog_x (a); INSERT INTO catalog_x VALUES (1);")
                .unwrap();
        }
        let cache = MetadataCache::open(&path).unwrap();
        assert!(cache.tag("loader").await.unwrap().is_none());
        drop(cache);
        let connection = Connection::open(&path).unwrap();
        let count: i64 = connection
            .query_row("SELECT count(*) FROM catalog_x", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn linha_ilegivel_conta_como_ausente() {
        let cache = MetadataCache::in_memory(None).unwrap();
        cache.put_projects(vec![project("AAA", "s")]).await.unwrap();
        cache
            .run("estragar", |connection, _| {
                connection.execute("UPDATE modrinth_projects SET json = '{'", [])
            })
            .await
            .unwrap();
        assert!(cache.project("AAA").await.unwrap().is_none());
    }

    #[test]
    fn texto_da_situacao() {
        assert_eq!(status_text(ProjectStatus::Approved), "approved");
        assert_eq!(status_text(ProjectStatus::Unknown), "unknown");
    }
}
