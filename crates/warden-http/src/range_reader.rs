//! Leitura de um arquivo remoto por partes, com HTTP `Range` (ARCHITECTURE §17.1; P1-17).
//!
//! Serve para ler **um** arquivo de dentro de um zip grande (o `modrinth.index.json` de um
//! `.mrpack`, o `manifest.json` de um modpack da CurseForge) sem baixar o zip inteiro: o crate
//! `zip` lê o diretório central no fim do arquivo e depois só a entrada pedida, e cada leitura
//! que falta no cache vira, no máximo, um pedido `Range` de [`MIN_FETCH`] bytes ou mais.
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
//! 4. Cada pedido cabe no buraco entre as partes já baixadas e tem pelo menos [`MIN_FETCH`]: se
//!    a leitura encosta no começo de uma parte já baixada (o `zip` procura o fim do diretório
//!    central de trás para frente), o bloco é pedido **para trás**, terminando nessa parte. Ao
//!    estender uma parte vizinha, o bloco cresce com ela (até [`MAX_FETCH`]), e partes vizinhas
//!    viram uma só. Assim, ler o arquivo inteiro de trás para frente custa poucos pedidos.
//! 5. Teto por arquivo aberto: [`MAX_REQUESTS`] pedidos e [`MAX_READ_BYTES`] recebidos (fora o
//!    download inteiro, que tem o seu limite). Passou disso, a leitura para com
//!    [`Error::InvalidResponse`]: um arquivo que não é zip, ou um servidor que mente o tamanho,
//!    não vira milhares de pedidos.
//! 6. Sem tempo total: o que esgota é o tempo **sem receber dados** do cliente
//!    ([`crate::HttpConfig::read_timeout`], 30 s), em cada pedido e no download inteiro. Um
//!    download lento mas contínuo termina; um servidor parado expira. O `CancellationToken`
//!    interrompe qualquer espera.
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

use reqwest::StatusCode;
use reqwest::header::{CONTENT_RANGE, HeaderMap, HeaderValue, RANGE};
use url::Url;
use warden_core::CancellationToken;

use crate::client::{HttpClient, Request, Response, host_of};
use crate::error::{Error, Result};

/// Quanto o primeiro pedido traz do fim do arquivo: 72 KiB, mais que os 65.557 bytes onde o fim
/// do diretório central de um zip pode estar (22 bytes do registro e até 65.535 de comentário).
pub const TAIL_FETCH: u64 = 72 * 1024;

/// Menor parte pedida numa leitura que falta no cache: o crate `zip` lê de poucos em poucos
/// bytes, e pedir em blocos mantém poucas requisições.
pub const MIN_FETCH: u64 = 64 * 1024;

/// Maior arquivo baixado inteiro quando o servidor não aceita `Range` (SPEC CA-T08-15).
pub const MAX_WHOLE_DOWNLOAD: u64 = 200 * 1024 * 1024;

/// Maior bloco pedido ao estender uma parte vizinha (o bloco dobra a cada pedido até aqui).
pub const MAX_FETCH: u64 = 4 * 1024 * 1024;

/// Teto de pedidos por arquivo aberto. A leitura de um modpack de verdade faz poucos: o fim do
/// arquivo, o diretório central e a entrada (que dobra de tamanho a cada pedido).
pub const MAX_REQUESTS: u64 = 32;

/// Teto de bytes recebidos em pedidos parciais por arquivo aberto (o download inteiro, sem
/// `Range`, tem o seu limite em [`MAX_WHOLE_DOWNLOAD`]).
pub const MAX_READ_BYTES: u64 = 24 * 1024 * 1024;

/// Limites da leitura por partes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeOptions {
    /// Bytes do fim do arquivo no primeiro pedido.
    pub tail: u64,
    /// Menor parte pedida depois.
    pub min_fetch: u64,
    /// Maior arquivo aceito no download inteiro (servidor sem `Range`).
    pub max_whole: u64,
    /// Maior bloco pedido ao estender uma parte vizinha.
    pub max_fetch: u64,
    /// Teto de pedidos (o primeiro incluído).
    pub max_requests: u64,
    /// Teto de bytes recebidos em pedidos parciais.
    pub max_bytes: u64,
}

impl Default for RangeOptions {
    fn default() -> Self {
        Self {
            tail: TAIL_FETCH,
            min_fetch: MIN_FETCH,
            max_whole: MAX_WHOLE_DOWNLOAD,
            max_fetch: MAX_FETCH,
            max_requests: MAX_REQUESTS,
            max_bytes: MAX_READ_BYTES,
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
    max_fetch: u64,
    max_requests: u64,
    max_bytes: u64,
    cancel: Option<CancellationToken>,
    /// Partes já recebidas (início, bytes), sem sobreposição nem vizinhas, em ordem.
    parts: Mutex<Vec<(u64, Vec<u8>)>>,
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
                max_fetch: options.max_fetch.max(options.min_fetch).max(1),
                max_requests: options.max_requests.max(1),
                max_bytes: options.max_bytes,
                cancel: cancel.cloned(),
                parts: Mutex::new(vec![(start, bytes)]),
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
    /// cache) e devolve vazio só no fim do arquivo. O que falta no cache é pedido com `Range`
    /// (veja o passo 4 do módulo); passou do teto de pedidos ou de bytes, [`Error::InvalidResponse`].
    pub async fn read_at(&self, offset: u64, max: usize) -> Result<Vec<u8>> {
        if offset >= self.inner.len || max == 0 {
            return Ok(Vec::new());
        }
        if let Some(bytes) = self.cached(offset, max) {
            return Ok(bytes);
        }
        let (start, end) = self.plan(offset, max as u64);
        self.within_limits(end - start)?;
        self.fetch(start, end).await?;
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

    /// O bloco `[início, fim)` a pedir para ler `max` bytes em `offset`, que não está no cache.
    ///
    /// O bloco fica no buraco entre a parte anterior e a seguinte e tem pelo menos
    /// `max(max, min_fetch)` bytes (menos só se o buraco for menor). Se a leitura continua a parte
    /// anterior, o bloco cresce até o tamanho dela; se encosta na seguinte, também, e é pedido
    /// para trás, terminando nela. O crescimento para em `max_fetch`.
    fn plan(&self, offset: u64, max: u64) -> (u64, u64) {
        let inner = &self.inner;
        let parts = inner.parts.lock().unwrap_or_else(PoisonError::into_inner);
        let previous = parts
            .iter()
            .rev()
            .find(|(start, _)| *start <= offset)
            .map(|(start, bytes)| (start + bytes.len() as u64, bytes.len() as u64));
        let next = parts
            .iter()
            .find(|(start, _)| *start > offset)
            .map(|(start, bytes)| (*start, bytes.len() as u64));
        drop(parts);
        let gap_start = previous.map_or(0, |(end, _)| end);
        let gap_end = next.map_or(inner.len, |(start, _)| start);
        let mut size = max.max(inner.min_fetch);
        if let Some((end, len)) = previous
            && end == offset
        {
            size = size.max(len.min(inner.max_fetch));
        }
        if let Some((start, len)) = next
            && offset.saturating_add(max) >= start
        {
            size = size.max(len.min(inner.max_fetch));
        }
        let end = offset.saturating_add(size).min(gap_end);
        let start = end.saturating_sub(size).max(gap_start).min(offset);
        (start, end)
    }

    /// Recusa o próximo pedido se ele passar do teto de pedidos ou de bytes.
    fn within_limits(&self, bytes: u64) -> Result<()> {
        let inner = &self.inner;
        let requests = inner.requests.load(Ordering::Relaxed);
        let received = inner.received.load(Ordering::Relaxed);
        if requests >= inner.max_requests || received.saturating_add(bytes) > inner.max_bytes {
            tracing::warn!(
                url = %inner.url,
                pedidos = requests,
                bytes = received,
                "leitura por partes passou do limite"
            );
            return Err(invalid(
                &inner.url,
                &format!(
                    "a leitura por partes passou do limite ({} pedidos ou {} MB) sem achar o que \
                     procurava: o arquivo não parece um zip válido",
                    inner.max_requests,
                    inner.max_bytes / (1024 * 1024)
                ),
            ));
        }
        Ok(())
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
        parts.push((start, body));
        merge_parts(&mut parts);
        Ok(())
    }
}

/// Ordena as partes e junta as que se encostam ou se sobrepõem.
fn merge_parts(parts: &mut Vec<(u64, Vec<u8>)>) {
    parts.sort_by_key(|(start, _)| *start);
    let mut merged: Vec<(u64, Vec<u8>)> = Vec::with_capacity(parts.len());
    for (start, bytes) in parts.drain(..) {
        if let Some((last_start, last)) = merged.last_mut() {
            let last_end = *last_start + last.len() as u64;
            if start <= last_end {
                let overlap = usize::try_from(last_end - start).unwrap_or(usize::MAX);
                if overlap < bytes.len() {
                    last.extend_from_slice(&bytes[overlap..]);
                }
                continue;
            }
        }
        merged.push((start, bytes));
    }
    *parts = merged;
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
    // Sem tempo total: a resposta ao primeiro pedido pode ser o arquivo inteiro (servidor sem
    // `Range`). O que esgota é o tempo sem dados do cliente (`HttpConfig::read_timeout`).
    let mut request = client.get(url.clone()).total_timeout(None);
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
    use std::time::Duration;

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

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
            sent <= TAIL_FETCH + MIN_FETCH,
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

    /// Byte `i` do "lixo": nunca forma `PK` (sem cara de zip), igual em qualquer pedido.
    fn junk_byte(i: u64) -> u8 {
        match u8::try_from(i % 251).unwrap_or(0) {
            b'P' => b'Q',
            byte => byte,
        }
    }

    /// Um arquivo que não é zip, de qualquer tamanho declarado (até "1 GB"), servido com
    /// `Range`. Os bytes são gerados a cada pedido: nada do tamanho declarado fica em memória.
    #[derive(Clone)]
    struct Junk {
        len: u64,
        requests: Arc<AtomicUsize>,
        sent: Arc<AtomicU64>,
    }

    impl Junk {
        fn new(len: u64) -> Self {
            Self {
                len,
                requests: Arc::default(),
                sent: Arc::default(),
            }
        }
    }

    impl Respond for Junk {
        fn respond(&self, request: &MockRequest) -> ResponseTemplate {
            if request.method.as_str() == "HEAD" {
                return ResponseTemplate::new(200);
            }
            self.requests.fetch_add(1, Ordering::Relaxed);
            let spec = request
                .headers
                .get("range")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("bytes="))
                .and_then(|spec| spec.split_once('-'))
                .map(|(start, end)| (start.to_owned(), end.to_owned()));
            let Some((start, end)) = spec else {
                return ResponseTemplate::new(500);
            };
            let (start, end) = if start.is_empty() {
                let suffix: u64 = end.parse().unwrap();
                (self.len.saturating_sub(suffix), self.len - 1)
            } else {
                let end: u64 = end.parse().unwrap();
                (start.parse().unwrap(), end.min(self.len - 1))
            };
            let body: Vec<u8> = (start..=end).map(junk_byte).collect();
            self.sent.fetch_add(body.len() as u64, Ordering::Relaxed);
            ResponseTemplate::new(206)
                .insert_header("content-range", format!("bytes {start}-{end}/{}", self.len))
                .set_body_bytes(body)
        }
    }

    async fn junk_file(len: u64) -> (MockServer, Junk, RemoteFile) {
        let server = MockServer::start().await;
        let junk = Junk::new(len);
        Mock::given(path("/lixo.zip"))
            .respond_with(junk.clone())
            .mount(&server)
            .await;
        let url = Url::parse(&format!("{}/lixo.zip", server.uri())).unwrap();
        let remote = RemoteFile::open(&client(), url, RangeOptions::default(), None)
            .await
            .expect("abre");
        (server, junk, remote)
    }

    /// Lê o arquivo de trás para frente em janelas de 1 KiB, como o `zip` faz ao procurar o fim
    /// do diretório central num arquivo que não é zip. Devolve o primeiro erro, se houver.
    async fn scan_backwards(file: &RemoteFile) -> Result<()> {
        let mut end = file.len();
        while end > 0 {
            let start = end.saturating_sub(1024);
            let mut at = start;
            while at < end {
                let chunk = file.read_at(at, usize::try_from(end - at).unwrap()).await?;
                assert!(!chunk.is_empty(), "fim antes da hora");
                for (i, byte) in chunk.iter().enumerate() {
                    assert_eq!(*byte, junk_byte(at + i as u64), "byte {}", at + i as u64);
                }
                at += chunk.len() as u64;
            }
            end = start;
        }
        Ok(())
    }

    /// Defeito da revisão: cada leitura de 1 KiB de trás para frente virava um pedido (300 KB de
    /// lixo = 232 pedidos; 3 MB = 2.877). Agora o bloco é pedido para trás, cresce com a parte
    /// vizinha e as partes se juntam: poucos pedidos, e cada byte chega uma vez só.
    #[tokio::test]
    async fn ler_de_tras_para_frente_faz_poucos_pedidos() {
        for (len, limit) in [(300_000_u64, 4_usize), (3_000_000, 8)] {
            let (_server, junk, remote) = junk_file(len).await;
            scan_backwards(&remote).await.expect("lê tudo");
            let requests = junk.requests.load(Ordering::Relaxed);
            assert!(requests <= limit, "{len} bytes: {requests} pedidos");
            assert_eq!(remote.requests(), requests as u64);
            assert_eq!(
                junk.sent.load(Ordering::Relaxed),
                len,
                "{len} bytes: cada byte chega uma vez só"
            );
        }
    }

    /// Defeito da revisão: um servidor que declara 1 GB de lixo virava mais de 31 mil pedidos em
    /// 60 s, ainda rodando. Agora a leitura para no teto de pedidos e de bytes, rápido, com um
    /// erro que diz o motivo.
    #[tokio::test]
    async fn arquivo_enorme_para_no_teto_de_pedidos_e_de_bytes() {
        let started = std::time::Instant::now();
        let (_server, junk, remote) = junk_file(1024 * 1024 * 1024).await;
        let error = scan_backwards(&remote).await.expect_err("para no teto");
        let Error::InvalidResponse { reason, .. } = &error else {
            panic!("{error:?}");
        };
        assert!(reason.contains("passou do limite"), "{reason}");
        let requests = junk.requests.load(Ordering::Relaxed) as u64;
        assert!(requests <= MAX_REQUESTS, "{requests} pedidos");
        assert!(requests <= 16, "{requests} pedidos");
        assert!(junk.sent.load(Ordering::Relaxed) <= MAX_READ_BYTES);
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "{:?}",
            started.elapsed()
        );
    }

    /// O teto de pedidos vale também para leituras espalhadas (que não crescem).
    #[tokio::test]
    async fn leituras_espalhadas_param_no_teto_de_pedidos() {
        let (_server, junk, remote) = junk_file(100 * 1024 * 1024).await;
        let mut result = Ok(Vec::new());
        for i in 0..1_000_u64 {
            result = remote.read_at(i * 100_000, 10).await;
            if result.is_err() {
                break;
            }
        }
        assert!(
            matches!(result, Err(Error::InvalidResponse { .. })),
            "{result:?}"
        );
        assert_eq!(junk.requests.load(Ordering::Relaxed) as u64, MAX_REQUESTS);
    }

    /// Ler uma entrada grande para a frente, em pedaços de 8 KiB: o bloco dobra a cada pedido
    /// (a parte anterior cresce), em vez de um pedido a cada 64 KiB.
    #[tokio::test]
    async fn ler_para_a_frente_dobra_o_bloco() {
        let (_server, junk, remote) = junk_file(10 * 1024 * 1024).await;
        let from = 1024 * 1024;
        let mut at = from;
        while at < from + 8 * 1024 * 1024 {
            let chunk = remote.read_at(at, 8 * 1024).await.expect("lê");
            assert_eq!(chunk[0], junk_byte(at));
            at += chunk.len() as u64;
        }
        // O fim (1) e 8 MiB em 64 KiB, 64 KiB, 128 KiB, … 4 MiB (8).
        let requests = junk.requests.load(Ordering::Relaxed);
        assert!(requests <= 9, "{requests} pedidos");
    }

    #[test]
    fn junta_partes_vizinhas_e_sobrepostas() {
        let mut parts = vec![
            (10, vec![10, 11]),
            (0, vec![0, 1, 2]),
            (3, vec![3, 4]),
            (4, vec![4, 5, 6]),
            (20, vec![20]),
            (11, vec![11]),
        ];
        merge_parts(&mut parts);
        assert_eq!(
            parts,
            vec![
                (0, vec![0, 1, 2, 3, 4, 5, 6]),
                (10, vec![10, 11]),
                (20, vec![20]),
            ]
        );
    }

    /// Servidor HTTP mínimo que responde devagar: `HEAD` na hora; `GET` com o corpo de
    /// `body_len` bytes, `chunk` por vez a cada `gap`, ignorando `Range` (como um servidor que
    /// não aceita leitura por partes). Com `stall_after`, para de mandar depois desses bytes e
    /// espera o cliente fechar a conexão; `closed` avisa quando ele fecha.
    struct Slow {
        body_len: usize,
        chunk: usize,
        gap: Duration,
        stall_after: Option<usize>,
    }

    async fn slow_server(plan: Slow) -> (Url, oneshot::Receiver<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (closed_tx, closed_rx) = oneshot::channel();
        tokio::spawn(async move {
            let mut closed_tx = Some(closed_tx);
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut buffer = [0_u8; 4096];
                loop {
                    let mut request = Vec::new();
                    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                        match socket.read(&mut buffer).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => request.extend_from_slice(&buffer[..n]),
                        }
                    }
                    if request.is_empty() {
                        break;
                    }
                    let head = format!(
                        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n",
                        plan.body_len
                    );
                    if socket.write_all(head.as_bytes()).await.is_err() {
                        break;
                    }
                    if request.starts_with(b"HEAD") {
                        continue;
                    }
                    let mut sent = 0;
                    while sent < plan.body_len {
                        if plan.stall_after.is_some_and(|limit| sent >= limit) {
                            // Parado: só espera o cliente fechar.
                            while matches!(socket.read(&mut buffer).await, Ok(n) if n > 0) {}
                            if let Some(tx) = closed_tx.take() {
                                let _ = tx.send(());
                            }
                            return;
                        }
                        let n = plan.chunk.min(plan.body_len - sent);
                        if socket.write_all(&vec![7_u8; n]).await.is_err() {
                            return;
                        }
                        sent += n;
                        tokio::time::sleep(plan.gap).await;
                    }
                }
            }
        });
        let url = Url::parse(&format!("http://{address}/pack.zip")).unwrap();
        (url, closed_rx)
    }

    fn client_idle(idle: Duration) -> HttpClient {
        HttpClient::new(HttpConfig {
            read_timeout: idle,
            ..HttpConfig::default()
        })
        .expect("cliente")
    }

    /// Defeito da revisão: um tempo de 60 s valia para o download inteiro (servidor sem
    /// `Range`, até 200 MB), e numa conexão lenta o modpack nunca abria. Agora só conta o tempo
    /// **sem dados**: 33 KiB, 1 KiB a cada 2 s (64 s no total), terminam. Este teste leva um
    /// minuto de propósito: passa do tempo total antigo.
    #[tokio::test]
    async fn download_inteiro_lento_mas_continuo_termina() {
        let (url, _closed) = slow_server(Slow {
            body_len: 33 * 1024,
            chunk: 1024,
            gap: Duration::from_secs(2),
            stall_after: None,
        })
        .await;
        let started = std::time::Instant::now();
        let remote = RemoteFile::open(&client(), url, RangeOptions::default(), None)
            .await
            .expect("termina");
        assert!(
            started.elapsed() > Duration::from_secs(62),
            "{:?}",
            started.elapsed()
        );
        assert!(remote.whole_download());
        assert_eq!(remote.len(), 33 * 1024);
    }

    /// Servidor que para de mandar no meio do corpo: expira pelo tempo sem dados do cliente
    /// (`HttpConfig::read_timeout`; 2 s aqui, 30 s no app).
    #[tokio::test]
    async fn servidor_parado_expira_pelo_tempo_sem_dados() {
        let (url, _closed) = slow_server(Slow {
            body_len: 80 * 1024,
            chunk: 1024,
            gap: Duration::from_millis(10),
            stall_after: Some(4 * 1024),
        })
        .await;
        let started = std::time::Instant::now();
        let idle = Duration::from_secs(2);
        let error = RemoteFile::open(&client_idle(idle), url, RangeOptions::default(), None)
            .await
            .expect_err("expira");
        assert!(matches!(error, Error::Timeout { .. }), "{error:?}");
        let elapsed = started.elapsed();
        assert!(
            elapsed >= idle && elapsed < idle + Duration::from_secs(5),
            "{elapsed:?}"
        );
    }

    /// Cancelar interrompe a espera na hora e fecha a conexão: nada fica rodando.
    #[tokio::test]
    async fn cancelar_interrompe_e_fecha_a_conexao() {
        let (url, closed) = slow_server(Slow {
            body_len: 80 * 1024,
            chunk: 1024,
            gap: Duration::from_millis(10),
            stall_after: Some(2 * 1024),
        })
        .await;
        let cancel = CancellationToken::new();
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            trigger.cancel();
        });
        let started = std::time::Instant::now();
        let client = client_idle(Duration::from_secs(60));
        let error = RemoteFile::open(&client, url, RangeOptions::default(), Some(&cancel))
            .await
            .expect_err("cancelado");
        assert!(matches!(error, Error::Cancelled), "{error:?}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        tokio::time::timeout(Duration::from_secs(5), closed)
            .await
            .expect("a conexão fechou")
            .expect("aviso do servidor");
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
