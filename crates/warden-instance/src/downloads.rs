//! Cache de downloads em `cache/downloads/` (ARCHITECTURE §8.2 item 5, §13 e §17).
//!
//! - Cada arquivo fica uma vez só, endereçado pelo SHA-256: `cache/downloads/<aa>/<sha256>`.
//! - `cache/downloads/index.sqlite` guarda, por arquivo, sha1, sha256, sha512, o murmur2 da
//!   CurseForge (e o md5, quando foi calculado) e o tamanho, além das origens (fonte, projeto,
//!   versão, URL). É o gancho 1.1 da ADR-0039 (item 1): a conferência de segurança e a
//!   identificação de jars não precisam reler os arquivos. Os hashes vêm de uma passada só
//!   ([`warden_http::FileHashes`], calculados pela `warden-http` durante o download).
//! - Um arquivo é achado por qualquer um dos hashes ([`DownloadCache::find`]): o `.pw.toml` do
//!   Modrinth traz sha512, o da CurseForge sha1, o link direto sha256.
//! - O download em si é o da `warden-http`: streaming para `incoming/<formato>-<hash>.part`
//!   (o nome estável deixa um download interrompido ser retomado na próxima vez), hashes
//!   conferidos e só então a mudança para o lugar definitivo.
//! - Termos da CurseForge: a origem `curseforge` guarda só os IDs, nunca o endereço de
//!   download ([`FileOrigin::curseforge`]).
//!
//! Cache é descartável: um `index.sqlite` corrompido é apagado e recriado, e um arquivo do
//! índice que sumiu do disco simplesmente não é achado.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use rusqlite::{Connection, ErrorCode, params};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest as _, Sha256, Sha512};
use warden_core::{CancellationToken, ProgressSink};
use warden_http::{DownloadRequest, ExpectedHash, FileHashes, HttpClient, PART_SUFFIX};
use warden_packwiz::HashFormat;
use warden_packwiz::hash::{curseforge_fingerprint_reader, hash_matches};

use crate::error::{Error, Result};

/// Nome do índice dentro da pasta do cache.
pub const INDEX_FILE: &str = "index.sqlite";

/// Pasta dos downloads em andamento dentro do cache.
pub const INCOMING_DIR: &str = "incoming";

/// `.part` mais velhos que isto são apagados por [`DownloadCache::clean_stale_parts`].
pub const STALE_PART_AGE: Duration = Duration::from_hours(24);

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS files (
    sha256 TEXT PRIMARY KEY,
    sha1 TEXT NOT NULL,
    sha512 TEXT NOT NULL,
    murmur2 INTEGER NOT NULL,
    md5 TEXT,
    size INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS files_sha1 ON files (sha1);
CREATE INDEX IF NOT EXISTS files_sha512 ON files (sha512);
CREATE INDEX IF NOT EXISTS files_murmur2 ON files (murmur2);
CREATE TABLE IF NOT EXISTS origins (
    sha256 TEXT NOT NULL REFERENCES files (sha256) ON DELETE CASCADE,
    source TEXT NOT NULL,
    project TEXT NOT NULL DEFAULT '',
    version TEXT NOT NULL DEFAULT '',
    url TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (sha256, source, project, version, url)
);
";

/// De onde veio um arquivo do cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OriginSource {
    /// Modrinth.
    Modrinth,
    /// CurseForge (só IDs; nunca o endereço).
    Curseforge,
    /// Link direto.
    Url,
    /// Escolhido pelo usuário (download manual da CurseForge, T20).
    Manual,
}

impl OriginSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::Curseforge => "curseforge",
            Self::Url => "url",
            Self::Manual => "manual",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [Self::Modrinth, Self::Curseforge, Self::Url, Self::Manual]
            .into_iter()
            .find(|source| source.as_str() == text)
    }
}

/// Origem de um arquivo do cache (fonte, projeto, versão, URL).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileOrigin {
    /// Fonte.
    pub source: OriginSource,
    /// ID do projeto, quando conhecido.
    pub project: Option<String>,
    /// ID da versão (Modrinth) ou do arquivo (CurseForge), quando conhecido.
    pub version: Option<String>,
    /// Endereço, só para Modrinth e links diretos.
    pub url: Option<String>,
}

impl FileOrigin {
    /// Arquivo do Modrinth.
    #[must_use]
    pub fn modrinth(project: &str, version: &str, url: &str) -> Self {
        Self {
            source: OriginSource::Modrinth,
            project: Some(project.to_owned()),
            version: Some(version.to_owned()),
            url: Some(url.to_owned()),
        }
    }

    /// Arquivo da CurseForge: só os IDs (termos da API; ARCHITECTURE §17).
    #[must_use]
    pub fn curseforge(project: u64, file: u64) -> Self {
        Self {
            source: OriginSource::Curseforge,
            project: Some(project.to_string()),
            version: Some(file.to_string()),
            url: None,
        }
    }

    /// Link direto.
    #[must_use]
    pub fn url(url: &str) -> Self {
        Self {
            source: OriginSource::Url,
            project: None,
            version: None,
            url: Some(url.to_owned()),
        }
    }

    /// Arquivo que o usuário escolheu (download manual de um mod bloqueado da CurseForge).
    #[must_use]
    pub fn manual_curseforge(project: u64, file: u64) -> Self {
        Self {
            source: OriginSource::Manual,
            project: Some(project.to_string()),
            version: Some(file.to_string()),
            url: None,
        }
    }

    /// A origem como será gravada: a CurseForge nunca guarda endereço.
    fn sanitized(&self) -> Self {
        let mut origin = self.clone();
        if matches!(
            origin.source,
            OriginSource::Curseforge | OriginSource::Manual
        ) {
            origin.url = None;
        }
        origin
    }
}

/// Um arquivo no cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedFile {
    /// Caminho no cache.
    pub path: PathBuf,
    /// Tamanho em bytes.
    pub size: u64,
    /// Os hashes.
    pub hashes: FileHashes,
}

/// Resultado de [`DownloadCache::download`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// O arquivo no cache.
    pub file: CachedFile,
    /// Se veio da rede (`false`: outro download do mesmo arquivo terminou antes).
    pub from_network: bool,
}

/// Cache de downloads. Cada chamada ao SQLite é curta; o cache pode ser usado por várias
/// tarefas ao mesmo tempo.
#[derive(Debug)]
pub struct DownloadCache {
    root: PathBuf,
    connection: Mutex<Connection>,
    /// Uma trava por download em andamento (pelo `.part`): duas materializações que querem o
    /// mesmo arquivo ao mesmo tempo não gravam no mesmo `.part`; a segunda espera e acha o
    /// arquivo no cache.
    in_flight: Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>,
}

impl DownloadCache {
    /// Abre (ou cria) o cache na pasta dada (`AppPaths::downloads_cache_dir`). Um índice
    /// corrompido é apagado e recriado.
    pub fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root).map_err(|e| Error::io("criar a pasta", root, e))?;
        let path = root.join(INDEX_FILE);
        let connection = match open_connection(&path) {
            Ok(connection) => connection,
            Err(error) if is_corrupt(&error) => {
                tracing::warn!(%error, caminho = %path.display(), "índice do cache corrompido; recriando");
                for suffix in ["", "-wal", "-shm", "-journal"] {
                    let file = PathBuf::from(format!("{}{suffix}", path.display()));
                    match fs::remove_file(&file) {
                        Ok(()) => {}
                        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                        Err(e) => return Err(Error::io("apagar", file, e)),
                    }
                }
                open_connection(&path).map_err(Error::cache("abrir"))?
            }
            Err(error) => return Err(Error::cache("abrir")(error)),
        };
        Ok(Self {
            root: root.to_path_buf(),
            connection: Mutex::new(connection),
            in_flight: Mutex::new(HashMap::new()),
        })
    }

    /// Pasta do cache.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Caminho definitivo de um arquivo pelo SHA-256.
    #[must_use]
    pub fn file_path(&self, sha256: &str) -> PathBuf {
        let sha256 = sha256.to_ascii_lowercase();
        let prefix = sha256.get(..2).unwrap_or("00");
        self.root.join(prefix).join(sha256)
    }

    /// Caminho de um download em andamento, estável para o mesmo hash esperado.
    #[must_use]
    pub fn incoming_path(&self, expected: &ExpectedHash) -> PathBuf {
        let value: String = expected
            .value
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(128)
            .collect();
        self.root
            .join(INCOMING_DIR)
            .join(format!("{}-{value}", expected.format))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Procura um arquivo pelo hash esperado. Só devolve se o arquivo ainda estiver no disco
    /// com o tamanho registrado.
    pub fn find(&self, expected: &ExpectedHash) -> Result<Option<CachedFile>> {
        let value = expected.value.trim().to_ascii_lowercase();
        let (column, value): (&str, rusqlite::types::Value) = match expected.format {
            HashFormat::Sha1 => ("sha1", value.into()),
            HashFormat::Sha256 => ("sha256", value.into()),
            HashFormat::Sha512 => ("sha512", value.into()),
            HashFormat::Md5 => ("md5", value.into()),
            HashFormat::Murmur2 => match value.parse::<u32>() {
                Ok(number) => ("murmur2", i64::from(number).into()),
                Err(_) => return Ok(None),
            },
        };
        let rows: Vec<CachedRow> = {
            let connection = self.lock();
            let mut statement = connection
                .prepare_cached(&format!(
                    "SELECT sha256, sha1, sha512, murmur2, md5, size FROM files WHERE {column} = ?1"
                ))
                .map_err(Error::cache("procurar"))?;
            statement
                .query_map([value], CachedRow::from_row)
                .map_err(Error::cache("procurar"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Error::cache("procurar"))?
        };
        for row in rows {
            let path = self.file_path(&row.sha256);
            match fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() && metadata.len() == row.size => {
                    return Ok(Some(row.into_cached(path)));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(Error::io("ler", path, error)),
            }
        }
        Ok(None)
    }

    /// Origens registradas de um arquivo.
    pub fn origins(&self, sha256: &str) -> Result<Vec<FileOrigin>> {
        let connection = self.lock();
        let mut statement = connection
            .prepare_cached(
                "SELECT source, project, version, url FROM origins WHERE sha256 = ?1 \
                 ORDER BY source, project, version, url",
            )
            .map_err(Error::cache("ler as origens"))?;
        let rows = statement
            .query_map([sha256.to_ascii_lowercase()], |row| {
                let text = |index: usize| -> rusqlite::Result<Option<String>> {
                    let value: String = row.get(index)?;
                    Ok((!value.is_empty()).then_some(value))
                };
                let source: String = row.get(0)?;
                Ok((source, text(1)?, text(2)?, text(3)?))
            })
            .map_err(Error::cache("ler as origens"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Error::cache("ler as origens"))?;
        Ok(rows
            .into_iter()
            .filter_map(|(source, project, version, url)| {
                Some(FileOrigin {
                    source: OriginSource::parse(&source)?,
                    project,
                    version,
                    url,
                })
            })
            .collect())
    }

    /// Registra um arquivo já conferido que está em `staged` (dentro do cache): move para o
    /// lugar definitivo (ou apaga, se o mesmo arquivo já estiver lá) e grava hashes e origem.
    fn adopt(
        &self,
        staged: &Path,
        size: u64,
        hashes: &FileHashes,
        origin: &FileOrigin,
    ) -> Result<CachedFile> {
        let destination = self.file_path(&hashes.sha256);
        if fs::metadata(&destination).is_ok_and(|metadata| metadata.len() == size) {
            remove_if_exists(staged)?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|e| Error::io("criar a pasta", parent, e))?;
            }
            rename_with_retry(staged, &destination)
                .map_err(|e| Error::io("renomear para", &destination, e))?;
        }
        self.record(size, hashes, origin)?;
        Ok(CachedFile {
            path: destination,
            size,
            hashes: hashes.clone(),
        })
    }

    /// Grava os hashes e a origem no índice.
    fn record(&self, size: u64, hashes: &FileHashes, origin: &FileOrigin) -> Result<()> {
        let origin = origin.sanitized();
        let size =
            i64::try_from(size).map_err(|_| Error::Internal("arquivo grande demais".into()))?;
        let mut connection = self.lock();
        let transaction = connection.transaction().map_err(Error::cache("gravar"))?;
        transaction
            .execute(
                "INSERT INTO files (sha256, sha1, sha512, murmur2, md5, size) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT (sha256) DO UPDATE SET md5 = COALESCE(files.md5, excluded.md5)",
                params![
                    hashes.sha256,
                    hashes.sha1,
                    hashes.sha512,
                    i64::from(hashes.murmur2),
                    hashes.md5,
                    size
                ],
            )
            .map_err(Error::cache("gravar"))?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO origins (sha256, source, project, version, url) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    hashes.sha256,
                    origin.source.as_str(),
                    origin.project.unwrap_or_default(),
                    origin.version.unwrap_or_default(),
                    origin.url.unwrap_or_default()
                ],
            )
            .map_err(Error::cache("gravar"))?;
        transaction.commit().map_err(Error::cache("gravar"))
    }

    /// Baixa `request` para o cache (o `destination` do pedido é ignorado: o download vai para
    /// [`Self::incoming_path`] do primeiro hash esperado), confere e registra com a origem dada.
    /// O pedido precisa ter ao menos um hash esperado.
    pub async fn download(
        &self,
        http: &HttpClient,
        mut request: DownloadRequest,
        origin: &FileOrigin,
        progress: &dyn ProgressSink,
        cancel: Option<&CancellationToken>,
    ) -> Result<Fetched> {
        let Some(expected) = request.expected_hashes.first().cloned() else {
            return Err(Error::Internal(
                "download para o cache sem hash esperado".into(),
            ));
        };
        request.destination = self.incoming_path(&expected);
        let lock = {
            let mut in_flight = self
                .in_flight
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            Arc::clone(in_flight.entry(request.destination.clone()).or_default())
        };
        let result = async {
            let _guard = lock.lock().await;
            // Outro download do mesmo arquivo pode ter terminado enquanto este esperava.
            if let Some(cached) = self.find(&expected)? {
                self.record(cached.size, &cached.hashes, origin)?;
                return Ok(Fetched {
                    file: cached,
                    from_network: false,
                });
            }
            let downloaded = http.download(&request, progress, cancel).await?;
            let file = self.adopt(
                &downloaded.path,
                downloaded.size,
                &downloaded.hashes,
                origin,
            )?;
            Ok(Fetched {
                file,
                from_network: true,
            })
        }
        .await;
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if Arc::strong_count(&lock) <= 2 {
            in_flight.remove(&request.destination);
        }
        result
    }

    /// Copia para o cache um arquivo escolhido pelo usuário, se conferir com `expected`
    /// (download manual, T20). Devolve `None` se o hash não conferir; nada é copiado.
    pub fn import_file(
        &self,
        source: &Path,
        expected: &ExpectedHash,
        origin: &FileOrigin,
    ) -> Result<Option<CachedFile>> {
        let staged = self
            .root
            .join(INCOMING_DIR)
            .join(format!("manual-{}", warden_core::OperationId::new()));
        if let Some(parent) = staged.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::io("criar a pasta", parent, e))?;
        }
        let result = (|| {
            let (size, hashes) = copy_hashing(source, &staged, None)?;
            let actual = hashes.get(expected.format).unwrap_or_default();
            if !hash_matches(expected.format, &expected.value, &actual) {
                remove_if_exists(&staged)?;
                return Ok(None);
            }
            self.adopt(&staged, size, &hashes, origin).map(Some)
        })();
        if result.is_err() {
            let _ = remove_if_exists(&staged);
        }
        result
    }

    /// Apaga os `.part` de `incoming/` mais velhos que `max_age` (downloads abandonados).
    /// Devolve quantos foram apagados.
    pub fn clean_stale_parts(&self, max_age: Duration) -> Result<usize> {
        let dir = self.root.join(INCOMING_DIR);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(Error::io("ler a pasta", dir, error)),
        };
        let now = SystemTime::now();
        let mut removed = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            let is_part = path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(PART_SUFFIX));
            let old = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| now.duration_since(modified).unwrap_or_default() > max_age);
            if is_part && old && remove_if_exists(&path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

struct CachedRow {
    sha256: String,
    sha1: String,
    sha512: String,
    murmur2: i64,
    md5: Option<String>,
    size: u64,
}

impl CachedRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        let size: i64 = row.get(5)?;
        Ok(Self {
            sha256: row.get(0)?,
            sha1: row.get(1)?,
            sha512: row.get(2)?,
            murmur2: row.get(3)?,
            md5: row.get(4)?,
            size: u64::try_from(size).unwrap_or(0),
        })
    }

    fn into_cached(self, path: PathBuf) -> CachedFile {
        CachedFile {
            path,
            size: self.size,
            hashes: FileHashes {
                sha1: self.sha1,
                sha256: self.sha256,
                sha512: self.sha512,
                murmur2: u32::try_from(self.murmur2).unwrap_or(0),
                md5: self.md5,
            },
        }
    }
}

fn open_connection(path: &Path) -> rusqlite::Result<Connection> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(SCHEMA)?;
    Ok(connection)
}

fn is_corrupt(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    )
}

/// Apaga um arquivo; não existir não é erro.
pub(crate) fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::io("apagar", path, error)),
    }
}

/// sha1, sha256 e sha512 numa passada só, e o murmur2 da CurseForge (que precisa do tamanho
/// sem espaços antes do primeiro byte, então relê o arquivo).
pub(crate) struct LocalHasher {
    sha1: Sha1,
    sha256: Sha256,
    sha512: Sha512,
    md5: Option<md5::Md5>,
}

impl LocalHasher {
    pub(crate) fn new(with_md5: bool) -> Self {
        Self {
            sha1: Sha1::new(),
            sha256: Sha256::new(),
            sha512: Sha512::new(),
            md5: with_md5.then(md5::Md5::new),
        }
    }

    pub(crate) fn update(&mut self, data: &[u8]) {
        self.sha1.update(data);
        self.sha256.update(data);
        self.sha512.update(data);
        if let Some(md5) = &mut self.md5 {
            md5.update(data);
        }
    }

    pub(crate) fn finish(self, murmur2: u32) -> FileHashes {
        FileHashes {
            sha1: hex::encode(self.sha1.finalize()),
            sha256: hex::encode(self.sha256.finalize()),
            sha512: hex::encode(self.sha512.finalize()),
            murmur2,
            md5: self.md5.map(|md5| hex::encode(md5.finalize())),
        }
    }
}

/// Tamanho do pedaço lido de cada vez.
const CHUNK: usize = 256 * 1024;

/// Hashes de um arquivo local (com md5 e murmur2).
pub(crate) fn hash_file(path: &Path) -> io::Result<(u64, FileHashes)> {
    let mut file = fs::File::open(path)?;
    let mut hasher = LocalHasher::new(true);
    let mut buffer = vec![0_u8; CHUNK];
    let mut size = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    let mut file = fs::File::open(path)?;
    let murmur2 = curseforge_fingerprint_reader(&mut file)?;
    Ok((size, hasher.finish(murmur2)))
}

/// Copia `source` para `destination` (substituindo) calculando os hashes; para entre os
/// pedaços se `cancel` for acionado (o destino incompleto é apagado). Não renomeia nada: quem
/// chama decide.
pub(crate) fn copy_hashing(
    source: &Path,
    destination: &Path,
    cancel: Option<&CancellationToken>,
) -> Result<(u64, FileHashes)> {
    use std::io::Write as _;
    let mut input = fs::File::open(source).map_err(|e| Error::io("abrir", source, e))?;
    let mut output =
        fs::File::create(destination).map_err(|e| Error::io("criar", destination, e))?;
    let mut hasher = LocalHasher::new(true);
    let mut buffer = vec![0_u8; CHUNK];
    let mut size = 0_u64;
    let result = (|| {
        loop {
            if cancel.is_some_and(CancellationToken::is_cancelled) {
                return Err(Error::Cancelled);
            }
            let read = input
                .read(&mut buffer)
                .map_err(|e| Error::io("ler", source, e))?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .map_err(|e| Error::io("gravar", destination, e))?;
            hasher.update(&buffer[..read]);
            size += read as u64;
        }
        output
            .sync_all()
            .map_err(|e| Error::io("sincronizar com o disco", destination, e))?;
        drop(output);
        let mut file = fs::File::open(destination).map_err(|e| Error::io("ler", destination, e))?;
        let murmur2 = curseforge_fingerprint_reader(&mut file)
            .map_err(|e| Error::io("ler", destination, e))?;
        Ok((size, hasher.finish(murmur2)))
    })();
    if result.is_err() {
        let _ = remove_if_exists(destination);
    }
    result
}

/// No Windows, antivírus e indexadores abrem o arquivo recém-gravado por instantes e a
/// renomeação falha com "acesso negado"; algumas tentativas curtas resolvem (como no
/// `atomic_write` do `warden-core`).
pub(crate) fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    const ATTEMPTS: u32 = 8;
    let mut attempt = 1;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error)
                if cfg!(windows)
                    && error.kind() == io::ErrorKind::PermissionDenied
                    && attempt < ATTEMPTS =>
            {
                std::thread::sleep(Duration::from_millis(20 * u64::from(attempt)));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use warden_packwiz::hash::hash_bytes;

    use super::*;

    fn cache() -> (tempfile::TempDir, DownloadCache) {
        let dir = crate::test_support::temp_dir();
        let cache = DownloadCache::open(&dir.path().join("downloads")).unwrap();
        (dir, cache)
    }

    fn put(cache: &DownloadCache, dir: &Path, data: &[u8], origin: &FileOrigin) -> CachedFile {
        let source = dir.join("fonte.bin");
        fs::write(&source, data).unwrap();
        let expected = ExpectedHash::new(HashFormat::Sha256, hash_bytes(HashFormat::Sha256, data));
        cache
            .import_file(&source, &expected, origin)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn acha_por_qualquer_hash_e_guarda_os_quatro() {
        let (dir, cache) = cache();
        let data = b"conteudo de um jar";
        let stored = put(
            &cache,
            dir.path(),
            data,
            &FileOrigin::url("https://x/a.jar"),
        );
        assert_eq!(stored.path, cache.file_path(&stored.hashes.sha256));
        assert_eq!(fs::read(&stored.path).unwrap(), data);
        for format in HashFormat::ALL {
            let expected = ExpectedHash::new(format, hash_bytes(format, data).to_uppercase());
            let found = cache.find(&expected).unwrap().unwrap();
            assert_eq!(found, stored, "{format}");
        }
        assert!(
            cache
                .find(&ExpectedHash::new(HashFormat::Sha1, "00"))
                .unwrap()
                .is_none()
        );
        assert!(
            cache
                .find(&ExpectedHash::new(HashFormat::Murmur2, "x"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn curseforge_nunca_guarda_endereco() {
        let (dir, cache) = cache();
        let mut origin = FileOrigin::curseforge(238_222, 5_101_366);
        origin.url = Some("https://edge.forgecdn.net/files/5101/366/jei.jar".to_owned());
        let stored = put(&cache, dir.path(), b"jei", &origin);
        let origins = cache.origins(&stored.hashes.sha256).unwrap();
        assert_eq!(origins, vec![FileOrigin::curseforge(238_222, 5_101_366)]);
        // Nem o texto do arquivo do índice tem o endereço.
        drop(cache);
        let bytes = fs::read(dir.path().join("downloads").join(INDEX_FILE)).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("forgecdn"));
    }

    #[test]
    fn mesmo_arquivo_duas_origens_um_arquivo_so() {
        let (dir, cache) = cache();
        let a = put(
            &cache,
            dir.path(),
            b"x",
            &FileOrigin::url("https://a/x.jar"),
        );
        let b = put(
            &cache,
            dir.path(),
            b"x",
            &FileOrigin::modrinth("AANobbMI", "abc", "https://cdn.modrinth.com/x.jar"),
        );
        assert_eq!(a, b);
        assert_eq!(cache.origins(&a.hashes.sha256).unwrap().len(), 2);
        let files: Vec<_> = walk(cache.root());
        assert_eq!(files.len(), 1, "{files:?}");
    }

    #[test]
    fn hash_errado_nao_entra() {
        let (dir, cache) = cache();
        let source = dir.path().join("errado.jar");
        fs::write(&source, b"outro").unwrap();
        let expected = ExpectedHash::new(HashFormat::Sha1, hash_bytes(HashFormat::Sha1, b"certo"));
        let result = cache
            .import_file(&source, &expected, &FileOrigin::manual_curseforge(1, 2))
            .unwrap();
        assert!(result.is_none());
        assert!(walk(cache.root()).is_empty());
    }

    #[test]
    fn arquivo_apagado_do_disco_nao_e_achado() {
        let (dir, cache) = cache();
        let stored = put(
            &cache,
            dir.path(),
            b"y",
            &FileOrigin::url("https://a/y.jar"),
        );
        fs::remove_file(&stored.path).unwrap();
        let expected = ExpectedHash::new(HashFormat::Sha256, stored.hashes.sha256);
        assert!(cache.find(&expected).unwrap().is_none());
    }

    #[test]
    fn indice_corrompido_e_recriado() {
        let dir = crate::test_support::temp_dir();
        let root = dir.path().join("downloads");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(INDEX_FILE),
            b"isto nao e um banco sqlite, e lixo ".repeat(200),
        )
        .unwrap();
        let cache = DownloadCache::open(&root).unwrap();
        let stored = put(
            &cache,
            dir.path(),
            b"z",
            &FileOrigin::url("https://a/z.jar"),
        );
        assert!(
            cache
                .find(&ExpectedHash::new(HashFormat::Sha256, stored.hashes.sha256))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn limpa_so_part_velhos() {
        let (_dir, cache) = cache();
        let incoming = cache.root().join(INCOMING_DIR);
        fs::create_dir_all(&incoming).unwrap();
        fs::write(incoming.join("sha1-aa.part"), b"1").unwrap();
        fs::write(incoming.join("sha1-bb"), b"2").unwrap();
        assert_eq!(cache.clean_stale_parts(Duration::from_hours(1)).unwrap(), 0);
        assert_eq!(cache.clean_stale_parts(Duration::ZERO).unwrap(), 1);
        assert!(incoming.join("sha1-bb").exists());
    }

    #[test]
    fn nome_do_download_em_andamento_e_estavel_e_seguro() {
        let (_dir, cache) = cache();
        let path = cache.incoming_path(&ExpectedHash::new(HashFormat::Sha1, "AB/../cd"));
        assert_eq!(path, cache.root().join(INCOMING_DIR).join("sha1-abcd"));
    }

    fn walk(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if !path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(INDEX_FILE)
                {
                    files.push(path);
                }
            }
        }
        files
    }
}
