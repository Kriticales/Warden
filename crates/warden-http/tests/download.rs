//! Downloads: hashes de uma vez, retomada com `Range` (conexão que cai no meio e `.part` de
//! outra sessão), CDN que recusa `Range` antes do redirecionamento (como o
//! `edge.forgecdn.net`), servidor que ignora `Range`, conferência de hash e tamanho e a chave
//! da CurseForge nos downloads do CDN.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpListener;
use warden_core::{DomainCode, DomainError, NoProgress, Progress, ProgressSink};
use warden_http::{
    DownloadRequest, Error, HeaderName, HttpClient, HttpConfig, HttpErrorCode, ManualTimer, Url,
    part_path,
};
use warden_packwiz::HashFormat;
use warden_packwiz::hash::{curseforge_fingerprint, hash_bytes};
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

/// 300 KB de bytes variados, com espaços e quebras de linha (o murmur2 da CurseForge os
/// descarta).
fn payload() -> Vec<u8> {
    (0..300_000_u32)
        .map(|i| match i % 97 {
            0 => b' ',
            1 => b'\n',
            2 => b'\r',
            3 => b'\t',
            _ => u8::try_from(i.wrapping_mul(2_654_435_761) >> 24).unwrap(),
        })
        .collect()
}

fn client() -> (HttpClient, ManualTimer) {
    let timer = ManualTimer::new();
    let client =
        HttpClient::with_timer(HttpConfig::for_version("1"), Arc::new(timer.clone())).unwrap();
    (client, timer)
}

fn url(base: &str, rest: &str) -> Url {
    Url::parse(&format!("{base}{rest}")).unwrap()
}

/// Sem cabeçalho `Range`.
struct NoRange;

impl Match for NoRange {
    fn matches(&self, request: &Request) -> bool {
        !request.headers.contains_key("range")
    }
}

/// Responde 206 com o pedaço pedido em `Range: bytes=<início>-`.
fn partial(data: Arc<Vec<u8>>) -> impl Fn(&Request) -> ResponseTemplate + Send + Sync {
    move |request: &Request| {
        let range = request.headers["range"].to_str().unwrap();
        let start: usize = range
            .trim_start_matches("bytes=")
            .trim_end_matches('-')
            .parse()
            .unwrap();
        ResponseTemplate::new(206)
            .insert_header(
                "content-range",
                format!("bytes {start}-{}/{}", data.len() - 1, data.len()),
            )
            .set_body_bytes(data[start..].to_vec())
    }
}

fn assert_all_hashes(hashes: &warden_http::FileHashes, data: &[u8]) {
    assert_eq!(hashes.sha1, hash_bytes(HashFormat::Sha1, data));
    assert_eq!(hashes.sha256, hash_bytes(HashFormat::Sha256, data));
    assert_eq!(hashes.sha512, hash_bytes(HashFormat::Sha512, data));
    assert_eq!(hashes.murmur2, curseforge_fingerprint(data));
}

#[derive(Default)]
struct RecordedProgress(Mutex<Vec<Progress>>);

impl ProgressSink for RecordedProgress {
    fn stage(&self, _stage: &str, _label_key: &str) {}

    fn progress(&self, progress: Progress) {
        self.0.lock().unwrap().push(progress);
    }
}

/// Gancho 1.1 (ADR-0039, item 1): sha1, sha256, sha512 e murmur2 de uma vez.
#[tokio::test]
async fn download_calcula_os_quatro_hashes_e_confere() {
    let data = payload();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/mods/a.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data.clone()))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("sub").join("a.jar");
    let (client, _) = client();
    let request = DownloadRequest::new(url(&server.uri(), "/mods/a.jar"), &destination)
        .expect_hash(HashFormat::Sha512, hash_bytes(HashFormat::Sha512, &data))
        .expect_hash(
            HashFormat::Murmur2,
            curseforge_fingerprint(&data).to_string(),
        )
        .expect_size(data.len() as u64);
    let progress = RecordedProgress::default();
    let downloaded = client.download(&request, &progress, None).await.unwrap();
    assert_eq!(downloaded.size, data.len() as u64);
    assert_eq!(downloaded.resumed_from, 0);
    assert_all_hashes(&downloaded.hashes, &data);
    assert_eq!(downloaded.hashes.md5, None);
    assert_eq!(fs::read(&destination).unwrap(), data);
    assert!(!part_path(&destination).exists());
    let progress = progress.0.lock().unwrap();
    let last = progress.last().unwrap();
    assert_eq!(last.current, data.len() as u64);
    assert_eq!(last.total, Some(data.len() as u64));
}

#[tokio::test]
async fn hash_md5_esperado_tambem_e_conferido() {
    let data = b"conteudo".to_vec();
    let server = MockServer::start().await;
    Mock::given(path("/a"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data.clone()))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = client();
    let request = DownloadRequest::new(url(&server.uri(), "/a"), dir.path().join("a"))
        .expect_hash(HashFormat::Md5, hash_bytes(HashFormat::Md5, &data));
    let downloaded = client.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(
        downloaded.hashes.md5,
        Some(hash_bytes(HashFormat::Md5, &data))
    );
}

/// Servidor HTTP mínimo que derruba a primeira conexão no meio do corpo e atende a segunda
/// com `Range`. Devolve o endereço e os pedidos recebidos (linha + cabeçalhos).
async fn flaky_server(data: Arc<Vec<u8>>, cut_at: usize) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    tokio::spawn(async move {
        let mut first = true;
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut raw = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !raw.windows(4).any(|w| w == b"\r\n\r\n") {
                let read = socket.read(&mut buffer).await.unwrap();
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..read]);
            }
            let text = String::from_utf8_lossy(&raw).to_lowercase();
            seen.lock().unwrap().push(text.clone());
            let range_start = text
                .lines()
                .find_map(|line| line.strip_prefix("range: bytes="))
                .map(|value| value.trim().trim_end_matches('-').parse::<usize>().unwrap());
            match range_start {
                None if first => {
                    first = false;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n\r\n",
                        data.len()
                    );
                    socket.write_all(head.as_bytes()).await.unwrap();
                    socket.write_all(&data[..cut_at]).await.unwrap();
                    socket.flush().await.unwrap();
                    // Derruba a conexão com o corpo pela metade.
                    drop(socket);
                }
                None => {
                    let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", data.len());
                    socket.write_all(head.as_bytes()).await.unwrap();
                    socket.write_all(&data).await.unwrap();
                }
                Some(start) => {
                    let head = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{}/{}\r\n\r\n",
                        data.len() - start,
                        data.len() - 1,
                        data.len()
                    );
                    socket.write_all(head.as_bytes()).await.unwrap();
                    socket.write_all(&data[start..]).await.unwrap();
                }
            }
        }
    });
    (format!("http://{address}"), requests)
}

/// CA-2 da P1-03: download interrompido retoma com `Range` e o hash final confere.
#[tokio::test]
async fn p1_03_ca2_download_interrompido_retoma_com_range_e_o_hash_confere() {
    let data = Arc::new(payload());
    let cut_at = 123_457;
    let (base, requests) = flaky_server(Arc::clone(&data), cut_at).await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("mod.jar");
    let (client, timer) = client();
    let request = DownloadRequest::new(url(&base, "/mod.jar"), &destination)
        .expect_hash(HashFormat::Sha1, hash_bytes(HashFormat::Sha1, &data))
        .expect_size(data.len() as u64);
    let downloaded = client.download(&request, &NoProgress, None).await.unwrap();

    assert_eq!(downloaded.resumed_from, cut_at as u64);
    assert_all_hashes(&downloaded.hashes, &data);
    assert_eq!(fs::read(&destination).unwrap(), *data);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2, "{requests:?}");
    assert!(!requests[0].contains("range:"));
    assert!(
        requests[1].contains(&format!("range: bytes={cut_at}-")),
        "{}",
        requests[1]
    );
    // Uma espera curta antes de retomar (espera exponencial da primeira tentativa).
    assert_eq!(timer.sleeps().len(), 1);
}

/// CA-2 da P1-03, `.part` que sobrou de outra sessão: o começo é relido para os hashes.
#[tokio::test]
async fn p1_03_ca2_part_de_outra_sessao_e_retomado() {
    let data = Arc::new(payload());
    let half = data.len() / 2;
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/a.jar"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/a.jar"))
        .and(header("range", format!("bytes={half}-").as_str()))
        .respond_with(partial(Arc::clone(&data)))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("a.jar");
    fs::write(part_path(&destination), &data[..half]).unwrap();
    let (client, _) = client();
    let request = DownloadRequest::new(url(&server.uri(), "/a.jar"), &destination)
        .expect_hash(HashFormat::Sha256, hash_bytes(HashFormat::Sha256, &data));
    let downloaded = client.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(downloaded.resumed_from, half as u64);
    assert_all_hashes(&downloaded.hashes, &data);
    assert_eq!(fs::read(&destination).unwrap(), *data);
}

/// Como o `edge.forgecdn.net` (R7 §9, conferido em 04/10/2026): `GET` com `Range` dá 404;
/// sem `Range`, redireciona para o servidor que aceita partes. A retomada resolve o destino
/// antes, com `HEAD` e sem `Range`, e pede a parte direto no destino.
#[tokio::test]
async fn retomada_resolve_o_redirecionamento_sem_range_como_no_cdn_da_curseforge() {
    let data = Arc::new(payload());
    let half = 100_000;
    let server = MockServer::start().await;
    let api_key = HeaderName::from_static("x-api-key");
    Mock::given(method("GET"))
        .and(path("/edge/files/1/2/jei.jar"))
        .and(header_exists("range"))
        .respond_with(ResponseTemplate::new(404))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(path("/edge/files/1/2/jei.jar"))
        .and(NoRange)
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", "/mediafilez/files/1/2/jei.jar"),
        )
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/mediafilez/files/1/2/jei.jar"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mediafilez/files/1/2/jei.jar"))
        .and(header_exists("range"))
        .and(header_exists("x-api-key"))
        .respond_with(partial(Arc::clone(&data)))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("jei.jar");
    fs::write(part_path(&destination), &data[..half]).unwrap();
    let (client, _) = client();
    let request = DownloadRequest::new(url(&server.uri(), "/edge/files/1/2/jei.jar"), &destination)
        .secret_header(api_key, &secrecy::SecretString::from("$2a$10$teste"))
        .unwrap()
        .expect_hash(
            HashFormat::Murmur2,
            curseforge_fingerprint(&data).to_string(),
        );
    let downloaded = client.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(downloaded.resumed_from, half as u64);
    assert!(downloaded.final_url.path().starts_with("/mediafilez/"));
    assert_eq!(fs::read(&destination).unwrap(), *data);
    // A chave foi para o CDN em todos os pedidos (mesmo servidor).
    let requests = server.received_requests().await.unwrap();
    assert!(requests.iter().all(|r| r.headers.contains_key("x-api-key")));
}

#[tokio::test]
async fn servidor_que_ignora_range_recomeca_do_zero() {
    let data = payload();
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/a"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data.clone()))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("a");
    // Um `.part` com lixo: se fosse aproveitado, o hash não conferiria.
    fs::write(part_path(&destination), b"lixo de outra versao").unwrap();
    let (client, _) = client();
    let request = DownloadRequest::new(url(&server.uri(), "/a"), &destination)
        .expect_hash(HashFormat::Sha1, hash_bytes(HashFormat::Sha1, &data));
    let downloaded = client.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(downloaded.resumed_from, 0);
    assert_eq!(fs::read(&destination).unwrap(), data);
}

#[tokio::test]
async fn faixa_recusada_recomeca_sem_range() {
    let data = payload();
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(header_exists("range"))
        .respond_with(ResponseTemplate::new(416))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(NoRange)
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data.clone()))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("a");
    fs::write(part_path(&destination), &data[..10]).unwrap();
    let (client, _) = client();
    let request = DownloadRequest::new(url(&server.uri(), "/a"), &destination);
    let downloaded = client.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(downloaded.resumed_from, 0);
    assert_all_hashes(&downloaded.hashes, &data);
}

#[tokio::test]
async fn part_completo_de_outra_sessao_nao_baixa_de_novo() {
    let data = payload();
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("a");
    fs::write(part_path(&destination), &data).unwrap();
    let (client, _) = client();
    let request = DownloadRequest::new(url(&server.uri(), "/a"), &destination)
        .expect_size(data.len() as u64)
        .expect_hash(HashFormat::Sha1, hash_bytes(HashFormat::Sha1, &data));
    let downloaded = client.download(&request, &NoProgress, None).await.unwrap();
    assert_all_hashes(&downloaded.hashes, &data);
    assert_eq!(fs::read(&destination).unwrap(), data);
}

fn assert_no_files(destination: &Path) {
    assert!(!destination.exists());
    assert!(!part_path(destination).exists());
}

#[tokio::test]
async fn hash_errado_apaga_o_part_e_nao_cria_o_destino() {
    let server = MockServer::start().await;
    Mock::given(path("/a"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"adulterado".to_vec()))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("a");
    let (client, _) = client();
    let request = DownloadRequest::new(url(&server.uri(), "/a"), &destination)
        .expect_hash(HashFormat::Sha1, hash_bytes(HashFormat::Sha1, b"original"));
    let error = client
        .download(&request, &NoProgress, None)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::HashMismatch { algorithm, .. } if algorithm == "sha1"),
        "{error}"
    );
    assert_eq!(
        error.code(),
        DomainCode::Domain(HttpErrorCode::HashMismatch)
    );
    assert!(
        error
            .detail()
            .unwrap()
            .contains(&hash_bytes(HashFormat::Sha1, b"original"))
    );
    assert_no_files(&destination);
}

#[tokio::test]
async fn tamanho_errado_e_tamanho_maximo() {
    let server = MockServer::start().await;
    Mock::given(path("/a"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1_u8; 5000]))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("a");
    let (client, _) = client();

    let request = DownloadRequest::new(url(&server.uri(), "/a"), &destination).expect_size(4999);
    let error = client
        .download(&request, &NoProgress, None)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            Error::SizeMismatch {
                expected: 4999,
                actual: 5000,
                ..
            }
        ),
        "{error}"
    );
    assert_no_files(&destination);

    let request = DownloadRequest::new(url(&server.uri(), "/a"), &destination).max_size(1000);
    let error = client
        .download(&request, &NoProgress, None)
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(HttpErrorCode::ResponseTooLarge)
    );
    assert!(!destination.exists());
}

#[tokio::test]
async fn erro_404_no_download() {
    let server = MockServer::start().await;
    Mock::given(path("/a"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = client();
    let request = DownloadRequest::new(url(&server.uri(), "/a"), dir.path().join("a"));
    let error = client
        .download(&request, &NoProgress, None)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::NotFound { .. }), "{error}");
}

#[tokio::test]
async fn cancelado_deixa_o_part_para_retomar() {
    let data = payload();
    let server = MockServer::start().await;
    Mock::given(path("/a"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(data)
                .set_delay(std::time::Duration::from_secs(30)),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("a");
    let (client, _) = client();
    let cancel = warden_core::CancellationToken::new();
    cancel.cancel();
    let request = DownloadRequest::new(url(&server.uri(), "/a"), &destination);
    let error = client
        .download(&request, &NoProgress, Some(&cancel))
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Cancelled), "{error}");
    assert!(!destination.exists());
}
