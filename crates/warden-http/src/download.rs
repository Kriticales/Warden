//! Download em streaming para `.part`, com hashes, retomada por `Range` e conferência
//! (ARCHITECTURE §17).
//!
//! 1. Os bytes vão para `<destino>.part` à medida que chegam, e sha1, sha256 e sha512 são
//!    calculados na mesma passada ([`crate::hashes`]).
//! 2. Se a conexão cair no meio, o cliente tenta de novo (até 3 vezes, com espera exponencial)
//!    pedindo só o que falta (`Range: bytes=<já baixado>-`). Um `.part` que sobrou de outra
//!    sessão também é retomado: o começo é relido do disco para os hashes.
//! 3. Antes de pedir uma parte, o endereço final é resolvido **sem** `Range`
//!    ([`HttpClient::resolve_redirects`]); o `edge.forgecdn.net` responde 404 a um `GET` com
//!    `Range`. Servidor que ignora o `Range` (200), recusa a faixa (416) ou responde 404 a ela
//!    faz o download recomeçar do zero.
//! 4. No fim: murmur2 da CurseForge sobre o `.part`, conferência do tamanho e dos hashes
//!    esperados e só então a renomeação para o destino. Se não conferir, o `.part` é apagado.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use reqwest::StatusCode;
use reqwest::header::{CONTENT_RANGE, HeaderMap, HeaderName, HeaderValue, RANGE};
use secrecy::{ExposeSecret as _, SecretString};
use tokio::io::AsyncWriteExt as _;
use url::Url;
use warden_core::{CancellationToken, Progress, ProgressSink};
use warden_packwiz::HashFormat;

use crate::client::{HttpClient, Response, backoff, host_of, is_transient_error};
use crate::error::{Error, Result};
use crate::hashes::{ExpectedHash, FileHashes, StreamHasher};
use crate::limiter::cancellable;

/// Sufixo do arquivo em andamento.
pub const PART_SUFFIX: &str = ".part";

/// `<destino>.part`.
#[must_use]
pub fn part_path(destination: &Path) -> PathBuf {
    let mut name = destination.as_os_str().to_os_string();
    name.push(PART_SUFFIX);
    PathBuf::from(name)
}

/// O que baixar e como conferir.
#[derive(Clone)]
pub struct DownloadRequest {
    /// Endereço.
    pub url: Url,
    /// Caminho final do arquivo (a pasta é criada se faltar).
    pub destination: PathBuf,
    /// Hashes que o arquivo precisa ter.
    pub expected_hashes: Vec<ExpectedHash>,
    /// Tamanho que o arquivo precisa ter.
    pub expected_size: Option<u64>,
    /// Maior tamanho aceito.
    pub max_size: Option<u64>,
    /// Cabeçalhos extras (o `x-api-key` da CurseForge, marcado como sensível).
    pub headers: HeaderMap,
}

impl std::fmt::Debug for DownloadRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Sem os cabeçalhos: podem ter chave.
        f.debug_struct("DownloadRequest")
            .field("url", &self.url.as_str())
            .field("destination", &self.destination)
            .field("expected_hashes", &self.expected_hashes)
            .field("expected_size", &self.expected_size)
            .field("max_size", &self.max_size)
            .finish_non_exhaustive()
    }
}

impl DownloadRequest {
    /// Download de `url` para `destination`, sem conferência.
    #[must_use]
    pub fn new(url: Url, destination: impl Into<PathBuf>) -> Self {
        Self {
            url,
            destination: destination.into(),
            expected_hashes: Vec::new(),
            expected_size: None,
            max_size: None,
            headers: HeaderMap::new(),
        }
    }

    /// Exige um hash.
    #[must_use]
    pub fn expect_hash(mut self, format: HashFormat, value: impl Into<String>) -> Self {
        self.expected_hashes.push(ExpectedHash::new(format, value));
        self
    }

    /// Exige um tamanho.
    #[must_use]
    pub fn expect_size(mut self, size: u64) -> Self {
        self.expected_size = Some(size);
        self
    }

    /// Recusa arquivos maiores que `max`.
    #[must_use]
    pub fn max_size(mut self, max: u64) -> Self {
        self.max_size = Some(max);
        self
    }

    /// Acrescenta um cabeçalho.
    #[must_use]
    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.insert(name, value);
        self
    }

    /// Acrescenta um cabeçalho com segredo (marcado como sensível; só segue redirecionamentos
    /// para o mesmo site). É assim que a P1-04 manda o `x-api-key` também nos downloads do CDN
    /// da CurseForge (R7 §9).
    pub fn secret_header(mut self, name: HeaderName, secret: &SecretString) -> Result<Self> {
        let mut value = HeaderValue::from_str(secret.expose_secret())
            .map_err(|_| Error::Internal(format!("valor inválido no cabeçalho {name}")))?;
        value.set_sensitive(true);
        self.headers.insert(name, value);
        Ok(self)
    }
}

/// Resultado de um download conferido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    /// Caminho final.
    pub path: PathBuf,
    /// Tamanho em bytes.
    pub size: u64,
    /// sha1, sha256, sha512 e murmur2 (e md5, se pedido).
    pub hashes: FileHashes,
    /// Endereço que entregou o arquivo, depois dos redirecionamentos.
    pub final_url: Url,
    /// Bytes que já estavam no disco quando a última parte foi pedida (0 se não houve
    /// retomada).
    pub resumed_from: u64,
}

/// Estado do `.part` durante o download.
struct PartState {
    hasher: StreamHasher,
    written: u64,
}

impl HttpClient {
    /// Baixa, confere e grava `request.destination` (ver o módulo). Erros: os da requisição,
    /// [`Error::HashMismatch`], [`Error::SizeMismatch`], [`Error::ResponseTooLarge`] e
    /// [`Error::Io`]. Cancelado, o `.part` fica no disco para retomar depois.
    pub async fn download(
        &self,
        request: &DownloadRequest,
        progress: &dyn ProgressSink,
        cancel: Option<&CancellationToken>,
    ) -> Result<Downloaded> {
        let destination = &request.destination;
        let part = part_path(destination);
        if let Some(parent) = destination.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| Error::io("criar a pasta", parent, e))?;
        }
        let with_md5 = request
            .expected_hashes
            .iter()
            .any(|hash| hash.format == HashFormat::Md5);
        let mut state = read_existing_part(&part, with_md5).await?;
        let mut resolved: Option<Url> = None;
        let mut attempt: u32 = 0;
        let mut resumed_from = 0;
        let mut ranges_refused = false;
        let final_url = loop {
            let offset = state.written;
            if offset > 0 && request.expected_size.is_some_and(|size| offset >= size) {
                if request.expected_size == Some(offset) {
                    // O `.part` já está completo (sobrou de uma sessão interrompida no fim).
                    break resolved.clone().unwrap_or_else(|| request.url.clone());
                }
                state = restart(&part, with_md5)?;
                continue;
            }
            let ranged = offset > 0 && !ranges_refused;
            if offset > 0 && !ranged {
                state = restart(&part, with_md5)?;
            }
            let url = if !ranged {
                request.url.clone()
            } else if let Some(url) = &resolved {
                url.clone()
            } else {
                let url = self
                    .resolve_redirects(request.url.clone(), &request.headers, cancel)
                    .await?;
                resolved = Some(url.clone());
                url
            };
            let mut http = self.get(url);
            http.headers.extend(request.headers.clone());
            http.timeout = None;
            http.cancel = cancel.cloned();
            if ranged {
                let range = HeaderValue::from_str(&format!("bytes={offset}-"))
                    .map_err(|e| Error::Internal(format!("cabeçalho Range: {e}")))?;
                http.headers.insert(RANGE, range);
            }
            let response = http.send_raw().await?;
            let status = response.status();
            let restart_needed = ranged
                && match status {
                    StatusCode::PARTIAL_CONTENT => {
                        content_range_start(response.headers()) != Some(offset)
                    }
                    // Ignorou o `Range` (200), recusou a faixa (416) ou não aceita `Range` (404).
                    StatusCode::OK | StatusCode::RANGE_NOT_SATISFIABLE | StatusCode::NOT_FOUND => {
                        true
                    }
                    _ => false,
                };
            if restart_needed {
                tracing::info!(url = %request.url, status = status.as_u16(), "retomada recusada; recomeçando");
                ranges_refused = true;
                if status == StatusCode::OK {
                    state = restart(&part, with_md5)?;
                } else {
                    drop(response);
                    continue;
                }
            }
            let mut response = response.error_for_status().await?;
            if resolved.is_none() {
                resolved = Some(response.final_url().clone());
            }
            if state.written > 0 {
                resumed_from = state.written;
            }
            match self
                .stream_body(&mut response, &part, &mut state, request, progress)
                .await
            {
                Ok(()) => break response.final_url().clone(),
                Err(error) if is_transient_error(&error) && attempt < self.config().max_retries => {
                    tracing::info!(url = %request.url, baixado = state.written, %error, "download interrompido; retomando");
                    drop(response);
                    let wait = backoff(self.config(), attempt);
                    cancellable(cancel, self.timer().sleep(wait)).await?;
                    attempt += 1;
                }
                Err(error) => return Err(error),
            }
        };
        finish(request, &part, state, final_url, resumed_from).await
    }

    async fn stream_body(
        &self,
        response: &mut Response,
        part: &Path,
        state: &mut PartState,
        request: &DownloadRequest,
        progress: &dyn ProgressSink,
    ) -> Result<()> {
        let total = response
            .content_length()
            .map(|length| length + state.written);
        let host = response.host().to_owned();
        let too_large = |limit: u64| Error::ResponseTooLarge {
            host: host.clone(),
            url: request.url.to_string(),
            limit,
        };
        if let (Some(max), Some(total)) = (request.max_size, total)
            && total > max
        {
            return Err(too_large(max));
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(part)
            .await
            .map_err(|e| Error::io("abrir", part, e))?;
        progress.progress(Progress::bytes(state.written, total));
        let result = async {
            while let Some(chunk) = response.chunk().await? {
                if let Some(max) = request.max_size
                    && state.written + chunk.len() as u64 > max
                {
                    return Err(too_large(max));
                }
                file.write_all(&chunk)
                    .await
                    .map_err(|e| Error::io("gravar", part, e))?;
                state.hasher.update(&chunk);
                state.written += chunk.len() as u64;
                progress.progress(Progress::bytes(state.written, total));
            }
            Ok(())
        }
        .await;
        // O que já foi gravado precisa chegar ao arquivo antes de retomar ou conferir.
        file.flush()
            .await
            .map_err(|e| Error::io("gravar", part, e))?;
        if result.is_ok() {
            file.sync_all()
                .await
                .map_err(|e| Error::io("sincronizar com o disco", part, e))?;
        }
        result
    }
}

/// Murmur2, conferência do tamanho e dos hashes e renomeação do `.part` para o destino.
async fn finish(
    request: &DownloadRequest,
    part: &Path,
    state: PartState,
    final_url: Url,
    resumed_from: u64,
) -> Result<Downloaded> {
    let size = state.written;
    let part_for_hash = part.to_path_buf();
    let murmur2 = tokio::task::spawn_blocking(move || {
        let mut file = fs::File::open(&part_for_hash)?;
        warden_packwiz::hash::curseforge_fingerprint_reader(&mut file)
    })
    .await
    .map_err(|e| Error::Internal(format!("cálculo do murmur2: {e}")))?
    .map_err(|e| Error::io("ler", part, e))?;
    let hashes = state.hasher.finish(murmur2);
    let host = host_of(&request.url)?;
    if let Some(expected) = request.expected_size
        && expected != size
    {
        remove_part(part);
        return Err(Error::SizeMismatch {
            host,
            url: request.url.to_string(),
            expected,
            actual: size,
        });
    }
    for expected in &request.expected_hashes {
        if let Err(actual) = expected.check(&hashes) {
            remove_part(part);
            return Err(Error::HashMismatch {
                host,
                url: request.url.to_string(),
                algorithm: expected.format.to_string(),
                expected: expected.value.clone(),
                actual,
            });
        }
    }
    let destination = &request.destination;
    rename_with_retry(part, destination).map_err(|e| Error::io("renomear para", destination, e))?;
    Ok(Downloaded {
        path: destination.clone(),
        size,
        hashes,
        final_url,
        resumed_from,
    })
}

/// Lê o `.part` que sobrou (se houver) para os hashes do começo.
async fn read_existing_part(part: &Path, with_md5: bool) -> Result<PartState> {
    let path = part.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut hasher = StreamHasher::new(with_md5);
        match fs::File::open(&path) {
            Ok(mut file) => {
                let written = hasher
                    .update_from(&mut file)
                    .map_err(|e| Error::io("ler", &path, e))?;
                Ok(PartState { hasher, written })
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(PartState { hasher, written: 0 })
            }
            Err(error) => Err(Error::io("abrir", &path, error)),
        }
    })
    .await
    .map_err(|e| Error::Internal(format!("leitura do .part: {e}")))?
}

/// Esvazia o `.part` para recomeçar.
fn restart(part: &Path, with_md5: bool) -> Result<PartState> {
    match fs::remove_file(part) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::io("apagar", part, error)),
    }
    Ok(PartState {
        hasher: StreamHasher::new(with_md5),
        written: 0,
    })
}

fn remove_part(part: &Path) {
    if let Err(error) = fs::remove_file(part)
        && error.kind() != io::ErrorKind::NotFound
    {
        // Não impede o erro principal; o próximo download recomeça ou apaga o arquivo.
        tracing::warn!(caminho = %part.display(), %error, "não foi possível apagar o .part");
    }
}

/// Começo da faixa em `Content-Range: bytes <início>-<fim>/<total>`.
fn content_range_start(headers: &HeaderMap) -> Option<u64> {
    let value = headers.get(CONTENT_RANGE)?.to_str().ok()?.trim();
    let range = value.strip_prefix("bytes ")?;
    let (start, _) = range.split_once('-')?;
    start.trim().parse().ok()
}

/// No Windows, antivírus e indexadores abrem o arquivo recém-gravado por instantes e a
/// renomeação falha com "acesso negado"; algumas tentativas curtas resolvem (como no
/// `atomic_write` do `warden-core`).
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    const ATTEMPTS: u32 = 5;
    let mut attempt = 1;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error)
                if cfg!(windows)
                    && error.kind() == io::ErrorKind::PermissionDenied
                    && attempt < ATTEMPTS =>
            {
                std::thread::sleep(std::time::Duration::from_millis(20 * u64::from(attempt)));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caminho_do_part() {
        assert_eq!(
            part_path(Path::new("cache/x.jar")),
            PathBuf::from("cache/x.jar.part")
        );
    }

    #[test]
    fn inicio_do_content_range() {
        let mut headers = HeaderMap::new();
        assert_eq!(content_range_start(&headers), None);
        headers.insert(CONTENT_RANGE, HeaderValue::from_static("bytes 100-199/200"));
        assert_eq!(content_range_start(&headers), Some(100));
        headers.insert(CONTENT_RANGE, HeaderValue::from_static("bytes 5-9/*"));
        assert_eq!(content_range_start(&headers), Some(5));
        headers.insert(CONTENT_RANGE, HeaderValue::from_static("items 1-2/3"));
        assert_eq!(content_range_start(&headers), None);
    }

    #[test]
    fn debug_sem_cabecalhos() {
        let request = DownloadRequest::new(
            Url::parse("https://edge.forgecdn.net/files/1/2/a.jar").unwrap(),
            "a.jar",
        )
        .secret_header(
            HeaderName::from_static("x-api-key"),
            &SecretString::from("$2a$segredo"),
        )
        .unwrap();
        let text = format!("{request:?}");
        assert!(!text.contains("segredo"), "{text}");
        assert!(request.headers["x-api-key"].is_sensitive());
    }
}
