//! Leitura de um arquivo remoto por partes, com HTTP `Range` (ARCHITECTURE §17.1; P1-17).
//!
//! Serve para ler **um** arquivo de dentro de um zip grande (o `modrinth.index.json` de um
//! `.mrpack`, o `manifest.json` de um modpack da CurseForge) sem baixar o zip inteiro: o crate
//! `zip` lê o diretório central no fim do arquivo e depois só a entrada pedida, e cada leitura
//! vira, no máximo, um pedido `Range` de [`MIN_FETCH`] bytes ou mais.
//!
//! 1. O endereço final é resolvido **antes**, sem `Range` ([`HttpClient::resolve_redirects`]):
//!    o `edge.forgecdn.net` responde 404 a qualquer pedido com `Range`, e só o
//!    `mediafilez.forgecdn.net`, para onde ele redireciona, responde 206 (R7 §9).
//! 2. O primeiro pedido traz o fim do arquivo (`Range: bytes=-<TAIL>`): ali estão o fim do
//!    diretório central e o tamanho total (`Content-Range`).
//! 3. Servidor que não aceita `Range` (responde 200, 404 ou 416 ao pedido parcial): o arquivo
//!    é baixado inteiro **para a memória**, até [`MAX_WHOLE_DOWNLOAD`] (200 MB), e
//!    [`RemoteFile::whole_download`] avisa quem chamou; acima disso, recusa com
//!    [`Error::ResponseTooLarge`] sem ler o corpo.
//!
//! Nada vai para o disco: as partes ficam num cache em memória do próprio [`RemoteFile`] (os
//! termos da CurseForge proíbem guardar o que vem dela, ARCHITECTURE §16).
//!
//! [`RangeReader`] é o `Read + Seek` síncrono que o crate `zip` pede. Cada leitura que falta no
//! cache espera o pedido com [`tokio::runtime::Handle::block_on`]: use-o dentro de
//! [`tokio::task::spawn_blocking`], nunca direto numa tarefa assíncrona.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{CONTENT_RANGE, HeaderMap, HeaderValue, RANGE};
use url::Url;
use warden_core::CancellationToken;

use crate::client::{HttpClient, Request, Response, host_of};
use crate::error::{Error, Result};

/// Quanto o primeiro pedido traz do fim do arquivo (o fim do diretório central cabe com folga).
pub const TAIL_FETCH: u64 = 64 * 1024;

/// Menor parte pedida numa leitura que falta no cache: o crate `zip` lê de poucos em poucos
/// bytes, e pedir em blocos mantém poucas requisições.
pub const MIN_FETCH: u64 = 64 * 1024;

/// Maior arquivo baixado inteiro quando o servidor não aceita `Range` (SPEC CA-T08-15).
pub const MAX_WHOLE_DOWNLOAD: u64 = 200 * 1024 * 1024;

/// Tempo de um pedido parcial (as partes são pequenas; o download inteiro não tem limite total).
const PART_TIMEOUT: Duration = Duration::from_secs(60);

/// Limites da leitura por partes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeOptions {
    /// Bytes do fim do arquivo no primeiro pedido.
    pub tail: u64,
    /// Menor parte pedida depois.
    pub min_fetch: u64,
    /// Maior arquivo aceito no download inteiro (servidor sem `Range`).
    pub max_whole: u64,
}

impl Default for RangeOptions {
    fn default() -> Self {
        Self {
            tail: TAIL_FETCH,
            min_fetch: MIN_FETCH,
            max_whole: MAX_WHOLE_DOWNLOAD,
        }
    }
}

/// Um arquivo remoto lido por partes. Barato de clonar: os clones dividem o cache e os
/// contadores.
#[derive(Clone)]
pub struct RemoteFile {
    inner: Arc<Inner>,
}

struct Inner {
    client: HttpClient,
    url: Url,
    len: u64,
    whole: bool,
    min_fetch: u64,
    cancel: Option<CancellationToken>,
    /// Partes já recebidas (início, bytes), sem sobreposição, em ordem.
    parts: Mutex<Vec<(u64, Arc<[u8]>)>>,
    requests: AtomicU64,
    received: AtomicU64,
}

impl std::fmt::Debug for RemoteFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteFile")
            .field("url", &self.inner.url.as_str())
            .field("len", &self.inner.len)
            .field("whole", &self.inner.whole)
            .finish_non_exhaustive()
    }
}

impl RemoteFile {
    /// Abre `url` para leitura por partes: resolve os redirecionamentos sem `Range` e pede o
    /// fim do arquivo. Sem suporte a `Range`, baixa o arquivo inteiro para a memória (até
    /// `options.max_whole`; acima disso, [`Error::ResponseTooLarge`]).
    pub async fn open(
        client: &HttpClient,
        url: Url,
        options: RangeOptions,
        cancel: Option<&CancellationToken>,
    ) -> Result<Self> {
        let url = client
            .resolve_redirects(url, &HeaderMap::new(), cancel)
            .await?;
        let requests = AtomicU64::new(0);
        let received = AtomicU64::new(0);
        let tail = options.tail.max(1);
        let response = part_request(client, &url, &format!("bytes=-{tail}"), cancel)
            .send_raw()
            .await?;
        requests.fetch_add(1, Ordering::Relaxed);
        let (len, whole, start, bytes) = match response.status() {
            StatusCode::PARTIAL_CONTENT => {
                let (start, total) = content_range(&response)?;
                let body = response.bytes_limited(tail).await?;
                (total, false, start, body)
            }
            StatusCode::OK => {
                let body = whole_body(response, options.max_whole).await?;
                (body.len() as u64, true, 0, body)
            }
            StatusCode::NOT_FOUND | StatusCode::RANGE_NOT_SATISFIABLE => {
                // Recusou o pedido parcial: tenta o arquivo inteiro, sem `Range`.
                drop(response);
                tracing::info!(url = %url, "servidor recusou Range; baixando o arquivo inteiro");
                let mut request = client.get(url.clone()).total_timeout(None);
                if let Some(cancel) = cancel {
                    request = request.cancel(cancel);
                }
                let response = request.send().await?;
                requests.fetch_add(1, Ordering::Relaxed);
                let body = whole_body(response, options.max_whole).await?;
                (body.len() as u64, true, 0, body)
            }
            _ => {
                response.error_for_status().await?;
                return Err(Error::Internal(
                    "resposta 2xx inesperada ao pedir parte".into(),
                ));
            }
        };
        if whole {
            tracing::info!(url = %url, bytes = len, "servidor sem Range: arquivo inteiro em memória");
        }
        received.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        if start + bytes.len() as u64 > len {
            return Err(invalid(
                &url,
                "a parte recebida passa do tamanho do arquivo",
            ));
        }
        Ok(Self {
            inner: Arc::new(Inner {
                client: client.clone(),
                url,
                len,
                whole,
                min_fetch: options.min_fetch.max(1),
                cancel: cancel.cloned(),
                parts: Mutex::new(vec![(start, Arc::from(bytes))]),
                requests,
                received,
            }),
        })
    }

    /// Tamanho do arquivo.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.len
    }

    /// Se o arquivo está vazio.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.len == 0
    }

    /// Endereço final (depois dos redirecionamentos), de onde as partes são pedidas.
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.inner.url
    }

    /// O servidor não aceitou `Range` e o arquivo foi baixado inteiro (para a memória).
    #[must_use]
    pub fn whole_download(&self) -> bool {
        self.inner.whole
    }

    /// Pedidos `GET` feitos até agora (sem contar o `HEAD` que resolve os redirecionamentos
    /// nem as novas tentativas do cliente).
    #[must_use]
    pub fn requests(&self) -> u64 {
        self.inner.requests.load(Ordering::Relaxed)
    }

    /// Bytes recebidos até agora (corpos das respostas).
    #[must_use]
    pub fn bytes_received(&self) -> u64 {
        self.inner.received.load(Ordering::Relaxed)
    }

    /// Lê a partir de `offset` até `max` bytes; pode devolver menos (o fim de uma parte do
    /// cache) e devolve vazio só no fim do arquivo. O que falta no cache é pedido com `Range`.
    pub async fn read_at(&self, offset: u64, max: usize) -> Result<Vec<u8>> {
        if offset >= self.inner.len || max == 0 {
            return Ok(Vec::new());
        }
        if let Some(bytes) = self.cached(offset, max) {
            return Ok(bytes);
        }
        let next_part = self.next_part_after(offset);
        let wanted = (max as u64).max(self.inner.min_fetch);
        let end = (offset + wanted).min(self.inner.len).min(next_part);
        self.fetch(offset, end).await?;
        Ok(self.cached(offset, max).unwrap_or_default())
    }

    /// Um leitor `Read + Seek` síncrono, para o crate `zip`. Use dentro de
    /// [`tokio::task::spawn_blocking`].
    #[must_use]
    pub fn reader(&self, handle: tokio::runtime::Handle) -> RangeReader {
        RangeReader {
            file: self.clone(),
            handle,
            position: 0,
            error: None,
        }
    }

    fn cached(&self, offset: u64, max: usize) -> Option<Vec<u8>> {
        let parts = self
            .inner
            .parts
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        parts.iter().find_map(|(start, bytes)| {
            let end = start + bytes.len() as u64;
            if offset < *start || offset >= end {
                return None;
            }
            let from = usize::try_from(offset - start).ok()?;
            let to = from.saturating_add(max).min(bytes.len());
            Some(bytes[from..to].to_vec())
        })
    }

    /// Começo da primeira parte do cache depois de `offset` (o fim do arquivo, se nenhuma).
    fn next_part_after(&self, offset: u64) -> u64 {
        let parts = self
            .inner
            .parts
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        parts
            .iter()
            .map(|(start, _)| *start)
            .filter(|start| *start > offset)
            .min()
            .unwrap_or(self.inner.len)
    }

    async fn fetch(&self, start: u64, end: u64) -> Result<()> {
        let inner = &self.inner;
        let range = format!("bytes={start}-{}", end - 1);
        let response = part_request(&inner.client, &inner.url, &range, inner.cancel.as_ref())
            .send()
            .await?;
        inner.requests.fetch_add(1, Ordering::Relaxed);
        if response.status() != StatusCode::PARTIAL_CONTENT {
            return Err(invalid(
                &inner.url,
                &format!(
                    "o servidor respondeu {} a um pedido parcial depois de aceitar Range",
                    response.status().as_u16()
                ),
            ));
        }
        let (got_start, _) = content_range(&response)?;
        if got_start != start {
            return Err(invalid(
                &inner.url,
                "Content-Range não confere com a parte pedida",
            ));
        }
        let body = response.bytes_limited(end - start).await?;
        inner
            .received
            .fetch_add(body.len() as u64, Ordering::Relaxed);
        if body.is_empty() {
            return Err(invalid(&inner.url, "parte vazia"));
        }
        let mut parts = inner.parts.lock().unwrap_or_else(PoisonError::into_inner);
        parts.push((start, Arc::from(body)));
        parts.sort_by_key(|(start, _)| *start);
        Ok(())
    }
}

/// `Read + Seek` sobre um [`RemoteFile`]. Os erros de rede viram `io::Error`; o original fica
/// em [`RangeReader::take_error`], para quem chamou devolver o código certo.
pub struct RangeReader {
    file: RemoteFile,
    handle: tokio::runtime::Handle,
    position: u64,
    error: Option<Error>,
}

impl std::fmt::Debug for RangeReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RangeReader")
            .field("file", &self.file)
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

impl RangeReader {
    /// O último erro de rede, se a leitura falhou por ele.
    pub fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }

    /// O arquivo lido.
    #[must_use]
    pub fn file(&self) -> &RemoteFile {
        &self.file
    }
}

impl Read for RangeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self
            .handle
            .block_on(self.file.read_at(self.position, buf.len()));
        match result {
            Ok(bytes) => {
                buf[..bytes.len()].copy_from_slice(&bytes);
                self.position += bytes.len() as u64;
                Ok(bytes.len())
            }
            Err(error) => {
                let message = error.to_string();
                self.error = Some(error);
                Err(io::Error::other(message))
            }
        }
    }
}

impl Seek for RangeReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let len = self.file.len();
        let next = match position {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(delta) => len.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
        };
        let next = next.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "posição antes do começo do arquivo",
            )
        })?;
        self.position = next;
        Ok(next)
    }
}

fn part_request(
    client: &HttpClient,
    url: &Url,
    range: &str,
    cancel: Option<&CancellationToken>,
) -> Request {
    let mut request = client.get(url.clone()).total_timeout(Some(PART_TIMEOUT));
    if let Ok(value) = HeaderValue::from_str(range) {
        request = request.header(RANGE, value);
    }
    if let Some(cancel) = cancel {
        request = request.cancel(cancel);
    }
    request
}

/// O arquivo inteiro, até `max` bytes. O tamanho anunciado acima do limite é recusado antes de
/// ler o corpo.
async fn whole_body(response: Response, max: u64) -> Result<Vec<u8>> {
    let response = response.error_for_status().await?;
    response.bytes_limited(max).await
}

/// Início e total em `Content-Range: bytes <início>-<fim>/<total>`.
fn content_range(response: &Response) -> Result<(u64, u64)> {
    let parsed = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_content_range);
    parsed.ok_or_else(|| invalid(response.final_url(), "Content-Range ausente ou inválido"))
}

fn parse_content_range(value: &str) -> Option<(u64, u64)> {
    let rest = value.trim().strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = end.trim().parse().ok()?;
    let total: u64 = total.trim().parse().ok()?;
    (start <= end && end < total).then_some((start, total))
}

fn invalid(url: &Url, reason: &str) -> Error {
    Error::InvalidResponse {
        host: host_of(url).unwrap_or_default(),
        url: url.to_string(),
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request as MockRequest, Respond, ResponseTemplate};

    use super::*;
    use crate::HttpConfig;

    /// Arquivo servido com ou sem `Range`, contando pedidos parciais e bytes enviados.
    #[derive(Clone)]
    struct FileServer {
        data: Arc<Vec<u8>>,
        ranges: bool,
        ranged_requests: Arc<AtomicUsize>,
        sent: Arc<AtomicUsize>,
    }

    impl FileServer {
        fn new(data: Vec<u8>, ranges: bool) -> Self {
            Self {
                data: Arc::new(data),
                ranges,
                ranged_requests: Arc::new(AtomicUsize::new(0)),
                sent: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn range(&self, header: &str) -> Option<(usize, usize)> {
            let spec = header.strip_prefix("bytes=")?;
            let len = self.data.len();
            let (start, end) = spec.split_once('-')?;
            if start.is_empty() {
                let suffix: usize = end.parse().ok()?;
                return Some((len.saturating_sub(suffix), len - 1));
            }
            let start: usize = start.parse().ok()?;
            let end: usize = if end.is_empty() {
                len - 1
            } else {
                end.parse().ok()?
            };
            (start < len).then_some((start, end.min(len - 1)))
        }
    }

    impl Respond for FileServer {
        fn respond(&self, request: &MockRequest) -> ResponseTemplate {
            let len = self.data.len();
            if request.method.as_str() == "HEAD" {
                return ResponseTemplate::new(200).insert_header("content-length", len.to_string());
            }
            let header = request
                .headers
                .get("range")
                .and_then(|value| value.to_str().ok());
            if let Some((start, end)) = header.filter(|_| self.ranges).and_then(|h| self.range(h)) {
                self.ranged_requests.fetch_add(1, Ordering::Relaxed);
                self.sent.fetch_add(end + 1 - start, Ordering::Relaxed);
                ResponseTemplate::new(206)
                    .insert_header("content-range", format!("bytes {start}-{end}/{len}"))
                    .set_body_bytes(self.data[start..=end].to_vec())
            } else {
                self.sent.fetch_add(len, Ordering::Relaxed);
                ResponseTemplate::new(200).set_body_bytes(self.data.as_slice().to_vec())
            }
        }
    }

    /// Imita o `edge.forgecdn.net`: 404 a qualquer pedido com `Range`; sem `Range`, redireciona
    /// para o outro servidor.
    struct Edge {
        target: String,
        ranged: Arc<AtomicUsize>,
    }

    impl Respond for Edge {
        fn respond(&self, request: &MockRequest) -> ResponseTemplate {
            if request.headers.contains_key("range") {
                self.ranged.fetch_add(1, Ordering::Relaxed);
                return ResponseTemplate::new(404);
            }
            ResponseTemplate::new(302).insert_header("location", self.target.as_str())
        }
    }

    fn data(len: usize) -> Vec<u8> {
        (0..len)
            .map(|i| u8::try_from(i % 251).unwrap_or(0))
            .collect()
    }

    fn client() -> HttpClient {
        HttpClient::new(HttpConfig::default()).expect("cliente")
    }

    async fn read_all(file: &RemoteFile, offset: u64, len: usize) -> Vec<u8> {
        let mut out = Vec::new();
        while out.len() < len {
            let chunk = file
                .read_at(offset + out.len() as u64, len - out.len())
                .await
                .expect("leitura");
            assert!(!chunk.is_empty(), "fim antes da hora");
            out.extend(chunk);
        }
        out
    }

    #[tokio::test]
    async fn resolve_o_redirecionamento_antes_e_le_por_partes_no_servidor_final() {
        let media = MockServer::start().await;
        let file = FileServer::new(data(1_000_000), true);
        Mock::given(path("/files/pack.zip"))
            .respond_with(file.clone())
            .mount(&media)
            .await;
        let edge = MockServer::start().await;
        let edge_ranged = Arc::new(AtomicUsize::new(0));
        Mock::given(path("/files/pack.zip"))
            .respond_with(Edge {
                target: format!("{}/files/pack.zip", media.uri()),
                ranged: Arc::clone(&edge_ranged),
            })
            .mount(&edge)
            .await;

        let url = Url::parse(&format!("{}/files/pack.zip", edge.uri())).unwrap();
        let remote = RemoteFile::open(&client(), url, RangeOptions::default(), None)
            .await
            .expect("abre");
        assert_eq!(remote.len(), 1_000_000);
        assert!(!remote.whole_download());
        assert!(remote.url().as_str().starts_with(&media.uri()));
        // O fim já veio no primeiro pedido: ler ali não pede nada.
        assert_eq!(
            read_all(&remote, 999_000, 1000).await,
            data(1_000_000)[999_000..]
        );
        assert_eq!(remote.requests(), 1);
        // O começo: um pedido de 64 KiB serve várias leituras pequenas.
        assert_eq!(read_all(&remote, 10, 30).await, data(1_000_000)[10..40]);
        assert_eq!(
            read_all(&remote, 1000, 2000).await,
            data(1_000_000)[1000..3000]
        );
        assert_eq!(remote.requests(), 2);
        assert_eq!(
            edge_ranged.load(Ordering::Relaxed),
            0,
            "nenhum Range no edge"
        );
        assert_eq!(file.ranged_requests.load(Ordering::Relaxed), 2);
        let sent = file.sent.load(Ordering::Relaxed) as u64;
        assert_eq!(remote.bytes_received(), sent);
        assert!(
            sent <= 2 * MIN_FETCH,
            "{sent} bytes: não baixou o arquivo inteiro"
        );
    }

    #[tokio::test]
    async fn sem_range_baixa_o_arquivo_inteiro_para_a_memoria_e_avisa() {
        let server = MockServer::start().await;
        let file = FileServer::new(data(300_000), false);
        Mock::given(path("/a.zip"))
            .respond_with(file.clone())
            .mount(&server)
            .await;
        let url = Url::parse(&format!("{}/a.zip", server.uri())).unwrap();
        let remote = RemoteFile::open(&client(), url, RangeOptions::default(), None)
            .await
            .expect("abre");
        assert!(remote.whole_download());
        assert_eq!(remote.len(), 300_000);
        assert_eq!(read_all(&remote, 0, 300_000).await, data(300_000));
        assert_eq!(remote.requests(), 1, "um pedido só, nada depois");
    }

    #[tokio::test]
    async fn sem_range_recusa_acima_do_limite() {
        let server = MockServer::start().await;
        Mock::given(path("/grande.zip"))
            .respond_with(FileServer::new(data(50_000), false))
            .mount(&server)
            .await;
        let url = Url::parse(&format!("{}/grande.zip", server.uri())).unwrap();
        let options = RangeOptions {
            max_whole: 10_000,
            ..RangeOptions::default()
        };
        let error = RemoteFile::open(&client(), url, options, None)
            .await
            .expect_err("recusa");
        assert!(
            matches!(error, Error::ResponseTooLarge { limit: 10_000, .. }),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn recusa_404_ao_range_cai_para_o_arquivo_inteiro() {
        let server = MockServer::start().await;
        let body = data(20_000);
        Mock::given(method("GET"))
            .and(path("/x.zip"))
            .and(wiremock::matchers::header_exists("range"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(path("/x.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;
        let url = Url::parse(&format!("{}/x.zip", server.uri())).unwrap();
        let remote = RemoteFile::open(&client(), url, RangeOptions::default(), None)
            .await
            .expect("abre");
        assert!(remote.whole_download());
        assert_eq!(read_all(&remote, 0, 20_000).await, body);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn o_leitor_sincrono_le_e_posiciona() {
        let server = MockServer::start().await;
        Mock::given(path("/r.bin"))
            .respond_with(FileServer::new(data(200_000), true))
            .mount(&server)
            .await;
        let url = Url::parse(&format!("{}/r.bin", server.uri())).unwrap();
        let remote = RemoteFile::open(&client(), url, RangeOptions::default(), None)
            .await
            .expect("abre");
        let handle = tokio::runtime::Handle::current();
        let read = tokio::task::spawn_blocking(move || {
            let mut reader = remote.reader(handle);
            reader.seek(SeekFrom::Start(5)).unwrap();
            let mut first = [0_u8; 10];
            reader.read_exact(&mut first).unwrap();
            reader.seek(SeekFrom::End(-4)).unwrap();
            let mut last = Vec::new();
            reader.read_to_end(&mut last).unwrap();
            assert!(reader.seek(SeekFrom::Current(-1_000_000)).is_err());
            (first.to_vec(), last)
        })
        .await
        .unwrap();
        let all = data(200_000);
        assert_eq!(read.0, all[5..15]);
        assert_eq!(read.1, all[199_996..]);
    }

    #[test]
    fn le_o_content_range() {
        assert_eq!(parse_content_range("bytes 100-199/200"), Some((100, 200)));
        assert_eq!(parse_content_range("bytes 0-0/1"), Some((0, 1)));
        assert_eq!(parse_content_range("bytes 5-9/*"), None);
        assert_eq!(parse_content_range("bytes 9-5/10"), None);
        assert_eq!(parse_content_range("bytes 5-10/10"), None);
        assert_eq!(parse_content_range("items 1-2/3"), None);
    }
}
