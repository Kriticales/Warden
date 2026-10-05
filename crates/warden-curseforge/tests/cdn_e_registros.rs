//! CA-3 da P1-04: downloads da CDN da CurseForge levam o `x-api-key` (também depois do
//! redirecionamento e na retomada com `Range`), e a chave nunca aparece nos registros.
//!
//! Os registros de todo o binário de teste (inclusive os da fachada `log`, usada pelo
//! `reqwest`) vão para um buffer no nível mais detalhado (`TRACE`); no fim de cada teste o buffer
//! é conferido.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use common::{KEY, data, forbidden, setup};
use serde_json::json;
use warden_core::NoProgress;
use warden_curseforge::{CurseforgeClient, Error, File, FileHash, HashAlgo, SecretString};
use warden_http::{DownloadRequest, HttpClient, HttpConfig, Url, part_path};
use warden_packwiz::HashFormat;
use warden_packwiz::hash::hash_bytes;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// Registros capturados por todos os testes deste binário.
fn captured() -> Arc<Mutex<Vec<u8>>> {
    static BUFFER: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    BUFFER
        .get_or_init(|| {
            let buffer = Arc::new(Mutex::new(Vec::new()));
            let writer = Arc::clone(&buffer);
            // `try_init` também liga o `tracing-log`: os registros da fachada `log` entram.
            let _ = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .with_ansi(false)
                .with_writer(move || Capture(Arc::clone(&writer)))
                .try_init();
            buffer
        })
        .clone()
}

struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn logs() -> String {
    String::from_utf8_lossy(&captured().lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
}

fn assert_key_not_logged() {
    let text = logs();
    assert!(!text.is_empty(), "nenhum registro capturado");
    assert!(!text.contains(KEY), "a chave apareceu nos registros");
    // Nem pedaços longos dela.
    assert!(
        !text.contains(&KEY[7..30]),
        "parte da chave apareceu nos registros"
    );
}

/// Conteúdo do "jar" servido pela CDN simulada.
fn jar_bytes() -> Vec<u8> {
    (0..200_000_u32).map(|i| (i % 251) as u8).collect()
}

fn api_file(mod_id: u64, id: u64, url: &str, bytes: &[u8]) -> File {
    File {
        id,
        mod_id,
        file_name: "mod-teste.jar".into(),
        download_url: Some(url.into()),
        file_length: bytes.len() as u64,
        hashes: vec![FileHash {
            value: hash_bytes(HashFormat::Sha1, bytes),
            algo: HashAlgo::Sha1,
        }],
        ..File::default()
    }
}

/// Monta a CDN simulada: `edge` redireciona para `mediafilez` (302) e responde 404 a qualquer
/// pedido com `Range`, como o `edge.forgecdn.net` (R7 §9, 04/10/2026); `mediafilez` aceita
/// `Range` (206). Os dois recusam (403) pedido sem a chave.
async fn cdn(edge: &MockServer, media: &MockServer, file_path: &str, bytes: &[u8]) {
    Mock::given(header_exists("range"))
        .and(path(file_path))
        .respond_with(ResponseTemplate::new(404))
        .with_priority(1)
        .mount(edge)
        .await;
    Mock::given(path(file_path))
        .and(header("x-api-key", KEY))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}{file_path}", media.uri())),
        )
        .with_priority(2)
        .mount(edge)
        .await;
    Mock::given(path(file_path))
        .respond_with(forbidden())
        .with_priority(3)
        .mount(edge)
        .await;

    let body = bytes.to_vec();
    Mock::given(path(file_path))
        .and(header("x-api-key", KEY))
        .respond_with(move |request: &Request| partial(request, &body))
        .with_priority(1)
        .mount(media)
        .await;
    Mock::given(path(file_path))
        .respond_with(forbidden())
        .with_priority(2)
        .mount(media)
        .await;
}

/// 200 com o arquivo inteiro, ou 206 com a parte pedida em `Range: bytes=<início>-`.
fn partial(request: &Request, body: &[u8]) -> ResponseTemplate {
    let start = request
        .headers
        .get("range")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes="))
        .and_then(|value| value.strip_suffix('-'))
        .and_then(|value| value.parse::<usize>().ok());
    match start {
        Some(start) if start < body.len() => ResponseTemplate::new(206)
            .insert_header(
                "content-range",
                format!("bytes {start}-{}/{}", body.len() - 1, body.len()),
            )
            .set_body_bytes(body[start..].to_vec()),
        Some(_) => ResponseTemplate::new(416),
        None => ResponseTemplate::new(200).set_body_bytes(body.to_vec()),
    }
}

/// CA-3 da P1-04: o download pela CDN simulada (com o endereço da API) leva o `x-api-key` no
/// primeiro pedido e depois do redirecionamento; a retomada resolve o redirecionamento sem
/// `Range` e pede a parte no destino final, com a chave. Nada disso registra a chave.
#[tokio::test]
async fn p1_04_ca3_download_da_cdn_leva_a_chave_e_nao_a_registra() {
    captured();
    // O servidor da "API" faz o papel do `edge.forgecdn.net`: é o endereço em uso, então recebe
    // a chave (como o servidor de fixtures dos E2E).
    let s = setup().await;
    let media = MockServer::start().await;
    let bytes = jar_bytes();
    let file_path = "/files/8920/212/mod-teste.jar";
    cdn(&s.server, &media, file_path, &bytes).await;
    let file = api_file(
        1,
        8_920_212,
        &format!("{}{file_path}", s.server.uri()),
        &bytes,
    );
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("downloads").join("mod-teste.jar");
    let http = HttpClient::with_timer(
        HttpConfig::for_version("1"),
        Arc::new(warden_http::ManualTimer::new()),
    )
    .unwrap();

    // 1) Download inteiro.
    let request = s
        .client
        .download_request(&file, &destination, None)
        .await
        .unwrap();
    assert!(request.headers["x-api-key"].is_sensitive());
    assert_eq!(request.expected_size, Some(bytes.len() as u64));
    let text = format!("{request:?}");
    assert!(!text.contains(KEY), "{text}");
    let downloaded = http.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(downloaded.size, bytes.len() as u64);
    assert_eq!(
        downloaded.final_url.as_str(),
        format!("{}{file_path}", media.uri())
    );
    assert_eq!(std::fs::read(&destination).unwrap(), bytes);

    // 2) Retomada: sobrou um `.part` com o começo do arquivo.
    std::fs::remove_file(&destination).unwrap();
    std::fs::write(part_path(&destination), &bytes[..50_000]).unwrap();
    let downloaded = http.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(downloaded.resumed_from, 50_000);
    assert_eq!(std::fs::read(&destination).unwrap(), bytes);

    // Todo pedido à CDN levou a chave; o `edge` nunca recebeu `Range`, e a parte foi pedida ao
    // `mediafilez`.
    let edge_requests = s.server.received_requests().await.unwrap();
    let media_requests = media.received_requests().await.unwrap();
    assert!(!edge_requests.is_empty() && !media_requests.is_empty());
    for request in edge_requests.iter().chain(&media_requests) {
        assert_eq!(
            request
                .headers
                .get("x-api-key")
                .map(|v| v.to_str().unwrap()),
            Some(KEY),
            "{} {} sem a chave",
            request.method,
            request.url
        );
    }
    assert!(
        edge_requests
            .iter()
            .all(|r| !r.headers.contains_key("range"))
    );
    assert!(
        edge_requests
            .iter()
            .any(|r| r.method == wiremock::http::Method::HEAD),
        "o redirecionamento não foi resolvido antes da parte"
    );
    assert!(
        media_requests
            .iter()
            .any(|r| r.headers.get("range").is_some_and(|v| v == "bytes=50000-")),
        "a parte não foi pedida ao destino final"
    );

    // Uma chamada à API com a chave recusada também não registra a chave.
    Mock::given(method("GET"))
        .and(path("/v1/games/432"))
        .respond_with(forbidden())
        .mount(&s.server)
        .await;
    let error = s.client.check_key(None).await.unwrap_err();
    assert!(matches!(error, Error::KeyInvalid { .. }));
    tracing::warn!(%error, ?error, "erro da CurseForge registrado pelo teste");
    assert!(!format!("{error:?}").contains(KEY));

    // A captura pegou os registros detalhados das requisições e dos redirecionamentos (prova
    // de que a conferência abaixo olha o que de fato seria gravado).
    let text = logs();
    assert!(text.contains("resposta HTTP"), "{text}");
    assert!(text.contains("redirecionamento"), "{text}");
    assert!(
        text.contains("erro da CurseForge registrado pelo teste"),
        "{text}"
    );
    assert_key_not_logged();
}

/// A chave só vai para os servidores da CurseForge: `*.forgecdn.net` e `*.curseforge.com` por
/// https, ou o servidor da API em uso. Para qualquer outro, os cabeçalhos ficam vazios.
#[tokio::test]
async fn chave_so_para_os_servidores_da_curseforge() {
    captured();
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let client = CurseforgeClient::with_base_url(
        http.clone(),
        "https://api.curseforge.com/v1",
        Some(SecretString::from(KEY.to_owned())),
    )
    .unwrap();
    let url = |text: &str| Url::parse(text).unwrap();
    for allowed in [
        "https://edge.forgecdn.net/files/8920/212/a.jar",
        "https://mediafilez.forgecdn.net/files/8920/212/a.jar",
        "https://forgecdn.net/x",
        "https://EDGE.ForgeCDN.net./x",
        "https://www.curseforge.com/api/v1/mods/1/files/2/download",
        "https://api.curseforge.com/v1/x",
    ] {
        let headers = client.download_headers(&url(allowed)).unwrap();
        assert!(headers["x-api-key"].is_sensitive(), "{allowed}");
        assert_eq!(headers["x-api-key"].to_str().unwrap(), KEY, "{allowed}");
    }
    for denied in [
        "http://edge.forgecdn.net/files/1/2/a.jar",
        "https://cdn.modrinth.com/data/x.jar",
        "https://evilforgecdn.net/x",
        "https://forgecdn.net.evil.com/x",
        "https://github.com/x",
        "https://127.0.0.1/x",
    ] {
        let headers = client.download_headers(&url(denied)).unwrap();
        assert!(headers.is_empty(), "{denied}");
    }
    // Um arquivo cujo endereço não é da CurseForge baixa sem a chave.
    let file = File {
        id: 1,
        mod_id: 2,
        download_url: Some("https://example.com/a.jar".into()),
        ..File::default()
    };
    let request: DownloadRequest = client.download_request(&file, "a.jar", None).await.unwrap();
    assert!(request.headers.is_empty());
    assert!(request.expected_hashes.is_empty());

    // Sem SHA-1, o download confere o murmur2 da API.
    let file = File {
        download_url: Some("https://edge.forgecdn.net/files/0/1/a.jar".into()),
        file_fingerprint: 427_243_112,
        ..file
    };
    let request = client.download_request(&file, "a.jar", None).await.unwrap();
    assert_eq!(request.expected_hashes[0].format, HashFormat::Murmur2);
    assert_eq!(request.expected_hashes[0].value, "427243112");
    // Endereço de download quebrado vira resposta inválida.
    let file = File {
        download_url: Some("não é endereço".into()),
        ..file
    };
    let error = client
        .download_request(&file, "a.jar", None)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::InvalidResponse(_)), "{error}");
    assert_key_not_logged_or_empty();
}

/// O `download-url` em si também leva a chave e devolve o endereço sem registrá-lo nem a chave.
#[tokio::test]
async fn download_url_com_chave() {
    captured();
    let s = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/1/files/2/download-url"))
        .and(header("x-api-key", KEY))
        .respond_with(data(json!("https://edge.forgecdn.net/files/0/2/a.jar")))
        .expect(1)
        .mount(&s.server)
        .await;
    let url = s.client.download_url(1, 2, None).await.unwrap();
    assert_eq!(url.host_str(), Some("edge.forgecdn.net"));
    assert!(
        !logs().contains("files/0/2/a.jar"),
        "o endereço foi registrado"
    );
    assert_key_not_logged();
}

fn assert_key_not_logged_or_empty() {
    assert!(!logs().contains(KEY), "a chave apareceu nos registros");
}
