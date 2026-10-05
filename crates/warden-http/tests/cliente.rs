//! Cliente HTTP contra um servidor simulado (`wiremock`): User-Agent, 429, novas tentativas,
//! limitador por servidor, redirecionamentos, erros e cancelamento. As esperas passam pelo
//! `ManualTimer`, que anota quanto o cliente esperou sem dormir de verdade.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use warden_core::{CancellationToken, CoreErrorCode, DomainCode, DomainError};
use warden_http::{
    Error, HeaderName, HostPolicy, HttpClient, HttpConfig, HttpErrorCode, ManualTimer, Url, nonzero,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn client_with(config: HttpConfig) -> (HttpClient, ManualTimer) {
    let timer = ManualTimer::new();
    let client = HttpClient::with_timer(config, Arc::new(timer.clone())).unwrap();
    (client, timer)
}

fn client() -> (HttpClient, ManualTimer) {
    client_with(HttpConfig::for_version("9.9.9"))
}

fn url(server: &MockServer, rest: &str) -> Url {
    Url::parse(&format!("{}{rest}", server.uri())).unwrap()
}

#[tokio::test]
async fn manda_o_user_agent_do_warden() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ua"))
        .and(header(
            "user-agent",
            "Kriticales/Warden/9.9.9 (+https://github.com/Kriticales/Warden)",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;
    let (client, _) = client();
    let body = client.get(url(&server, "/ua")).bytes().await.unwrap();
    assert_eq!(body, b"ok");
}

/// CA-1 da P1-03: 429 com `X-Ratelimit-Reset` é respeitado.
#[tokio::test]
async fn p1_03_ca1_429_com_x_ratelimit_reset_e_respeitado() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/projeto"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("x-ratelimit-limit", "300")
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "17")
                .set_body_json(json!({
                    "error": "ratelimit_error",
                    "description": "You are being rate-limited. Please wait 17000 milliseconds. 0/300 remaining."
                })),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/projeto"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "AANobbMI"})))
        .expect(1)
        .mount(&server)
        .await;
    let (client, timer) = client();
    let value: serde_json::Value = client.get(url(&server, "/projeto")).json().await.unwrap();
    assert_eq!(value["id"], "AANobbMI");
    // O cliente esperou exatamente o que o servidor mandou antes de repetir.
    assert_eq!(timer.sleeps(), vec![Duration::from_secs(17)]);

    // O bloqueio vale para o servidor inteiro: outra requisição logo depois não espera de novo
    // (a janela já passou no relógio), e nada foi pedido durante a espera.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
}

#[tokio::test]
async fn retry_after_vale_mais_que_o_reset_menor() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "30")
                .insert_header("x-ratelimit-reset", "4"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(path("/x"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let (client, timer) = client();
    client.get(url(&server, "/x")).send().await.unwrap();
    assert_eq!(timer.sleeps(), vec![Duration::from_secs(30)]);
}

#[tokio::test]
async fn espera_longa_demais_vira_erro_sem_repetir() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .respond_with(ResponseTemplate::new(429).insert_header("x-ratelimit-reset", "3600"))
        .expect(1)
        .mount(&server)
        .await;
    let (client, timer) = client();
    let error = client.get(url(&server, "/x")).send().await.unwrap_err();
    assert!(
        matches!(&error, Error::RateLimited { wait: Some(w), .. } if *w == Duration::from_secs(3600)),
        "{error}"
    );
    assert_eq!(error.code(), DomainCode::Domain(HttpErrorCode::RateLimited));
    assert_eq!(error.params()["seconds"], "3600");
    assert!(error.retryable());
    assert!(timer.sleeps().is_empty());
}

#[tokio::test]
async fn post_que_nao_e_consulta_nao_repete() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("x-ratelimit-reset", "2"))
        .expect(1)
        .mount(&server)
        .await;
    let (client, _) = client();
    let error = client
        .post_json(url(&server, "/escrita"), &json!({"a": 1}))
        .unwrap()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(error, Error::RateLimited { .. }), "{error}");
}

#[tokio::test]
async fn post_de_consulta_repete_e_manda_o_corpo_de_novo() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(header("content-type", "application/json"))
        .respond_with(|request: &Request| {
            ResponseTemplate::new(200).set_body_bytes(request.body.clone())
        })
        .mount(&server)
        .await;
    let (client, timer) = client();
    let echoed: serde_json::Value = client
        .post_json(url(&server, "/consulta"), &json!({"hashes": ["a"]}))
        .unwrap()
        .idempotent(true)
        .json()
        .await
        .unwrap();
    assert_eq!(echoed, json!({"hashes": ["a"]}));
    assert_eq!(timer.sleeps(), vec![Duration::from_millis(500)]);
}

#[tokio::test]
async fn erro_5xx_repete_tres_vezes_com_espera_exponencial() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .respond_with(ResponseTemplate::new(502))
        .expect(4)
        .mount(&server)
        .await;
    let (client, timer) = client();
    let error = client.get(url(&server, "/x")).send().await.unwrap_err();
    assert!(
        matches!(error, Error::ServerError { status: 502, .. }),
        "{error}"
    );
    assert_eq!(
        timer.sleeps(),
        vec![
            Duration::from_millis(500),
            Duration::from_secs(1),
            Duration::from_secs(2)
        ]
    );
}

#[tokio::test]
async fn erro_5xx_que_passa_na_segunda() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .respond_with(ResponseTemplate::new(500).insert_header("retry-after", "3"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;
    let (client, timer) = client();
    assert_eq!(client.get(url(&server, "/x")).bytes().await.unwrap(), b"ok");
    // O `Retry-After` do 5xx substitui a espera exponencial.
    assert_eq!(timer.sleeps(), vec![Duration::from_secs(3)]);
}

#[tokio::test]
async fn limitador_por_servidor_espaca_as_requisicoes() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .respond_with(ResponseTemplate::new(200))
        .expect(4)
        .mount(&server)
        .await;
    let config = HttpConfig::for_version("1")
        .with_host("127.0.0.1", HostPolicy::rate(nonzero(30), nonzero(1)));
    let (client, timer) = client_with(config);
    for _ in 0..4 {
        client.get(url(&server, "/x")).send().await.unwrap();
    }
    // 30 por minuto, sem rajada: uma a cada 2 s depois da primeira.
    assert_eq!(timer.total_slept(), Duration::from_secs(6));
}

#[tokio::test]
async fn janela_esgotada_bloqueia_a_proxima_requisicao() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "9"),
        )
        .expect(2)
        .mount(&server)
        .await;
    let (client, timer) = client();
    client.get(url(&server, "/x")).send().await.unwrap();
    assert!(timer.sleeps().is_empty());
    client.get(url(&server, "/x")).send().await.unwrap();
    assert_eq!(timer.sleeps(), vec![Duration::from_secs(9)]);
}

#[tokio::test]
async fn falha_de_conexao_repete_e_vira_rede_indisponivel() {
    // Porta fechada: reservada e solta antes do pedido (o `MockServer` não serve: o wiremock
    // reaproveita servidores e a porta continuaria aberta).
    let address = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };
    // Uma nova tentativa só: no Windows, cada conexão recusada leva cerca de 2 s.
    let mut config = HttpConfig::for_version("1");
    config.max_retries = 1;
    let (client, timer) = client_with(config);
    let error = client
        .get(Url::parse(&format!("{address}/x")).unwrap())
        .send()
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Core(CoreErrorCode::NetworkUnavailable),
        "{error}"
    );
    assert!(error.retryable());
    assert_eq!(timer.sleeps(), vec![Duration::from_millis(500)]);
}

#[tokio::test]
async fn codigos_de_erro_e_corpo_nos_detalhes() {
    let server = MockServer::start().await;
    Mock::given(path("/nao-existe"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(path("/sumiu"))
        .respond_with(ResponseTemplate::new(410))
        .mount(&server)
        .await;
    Mock::given(path("/ruim"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string(r#"{"error":"invalid_input","description":"facets"}"#),
        )
        .mount(&server)
        .await;
    Mock::given(path("/json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>"))
        .mount(&server)
        .await;
    Mock::given(path("/grande"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 2048]))
        .mount(&server)
        .await;
    let (client, timer) = client();

    let error = client
        .get(url(&server, "/nao-existe"))
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(error, Error::NotFound { status: 404, .. }),
        "{error}"
    );
    assert_eq!(error.params()["host"], "127.0.0.1");
    let error = client.get(url(&server, "/sumiu")).send().await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(HttpErrorCode::NotFound));

    let error = client.get(url(&server, "/ruim")).send().await.unwrap_err();
    assert!(
        matches!(error, Error::UnexpectedStatus { status: 400, .. }),
        "{error}"
    );
    assert!(error.detail().unwrap().contains("invalid_input"));
    assert!(!error.retryable());

    let error = client
        .get(url(&server, "/json"))
        .json::<serde_json::Value>()
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(HttpErrorCode::InvalidResponse)
    );

    let error = client
        .get(url(&server, "/grande"))
        .max_body(1024)
        .bytes()
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(HttpErrorCode::ResponseTooLarge)
    );
    // Nenhum desses erros é repetido.
    assert!(timer.sleeps().is_empty());
}

#[tokio::test]
async fn redirecionamento_seguido_e_chave_so_para_o_mesmo_site() {
    let origin = MockServer::start().await;
    let other = MockServer::start().await;
    // `localhost` é outro site para o Warden (o original é o IP 127.0.0.1).
    let other_url = other.uri().replace("127.0.0.1", "localhost");
    Mock::given(path("/mesmo"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/final"))
        .mount(&origin)
        .await;
    Mock::given(path("/final"))
        .respond_with(ResponseTemplate::new(200).set_body_string("final"))
        .mount(&origin)
        .await;
    Mock::given(path("/fora"))
        .respond_with(
            ResponseTemplate::new(301).insert_header("location", format!("{other_url}/destino")),
        )
        .mount(&origin)
        .await;
    Mock::given(path("/destino"))
        .respond_with(ResponseTemplate::new(200).set_body_string("destino"))
        .mount(&other)
        .await;
    let (client, _) = client();
    let key = secrecy::SecretString::from("$2a$10$chave-de-teste");
    let api_key = HeaderName::from_static("x-api-key");

    let response = client
        .get(url(&origin, "/mesmo"))
        .secret_header(api_key.clone(), &key)
        .unwrap()
        .send()
        .await
        .unwrap();
    assert!(response.final_url().path().ends_with("/final"));
    let requests = origin.received_requests().await.unwrap();
    assert!(requests.iter().all(|r| r.headers.contains_key("x-api-key")));

    let body = client
        .get(url(&origin, "/fora"))
        .secret_header(api_key, &key)
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(body, b"destino");
    let requests = other.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].headers.contains_key("x-api-key"));
}

#[tokio::test]
async fn redirecionamentos_demais() {
    let server = MockServer::start().await;
    Mock::given(path("/laco"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/laco"))
        .mount(&server)
        .await;
    let (client, _) = client();
    let error = client.get(url(&server, "/laco")).send().await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(HttpErrorCode::TooManyRedirects)
    );
    // 1 pedido + 10 redirecionamentos.
    assert_eq!(server.received_requests().await.unwrap().len(), 11);
}

#[tokio::test]
async fn redirecionamento_para_outro_esquema_e_recusado() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "file:///C:/Windows"))
        .mount(&server)
        .await;
    let (client, _) = client();
    let error = client.get(url(&server, "/x")).send().await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Domain(HttpErrorCode::InvalidUrl));
}

#[tokio::test]
async fn resolve_redirecionamento_com_head_e_sem_range() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/edge/a.jar"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/media/a.jar"))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/media/a.jar"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/sem-head"))
        .respond_with(ResponseTemplate::new(405))
        .mount(&server)
        .await;
    let (client, _) = client();
    let mut headers = warden_http::HeaderMap::new();
    headers.insert("range", "bytes=10-".parse().unwrap());
    let resolved = client
        .resolve_redirects(url(&server, "/edge/a.jar"), &headers, None)
        .await
        .unwrap();
    assert_eq!(resolved.path(), "/media/a.jar");
    let requests = server.received_requests().await.unwrap();
    assert!(requests.iter().all(|r| !r.headers.contains_key("range")));
    let same = client
        .resolve_redirects(url(&server, "/sem-head"), &headers, None)
        .await
        .unwrap();
    assert_eq!(same.path(), "/sem-head");
}

#[tokio::test]
async fn cancelamento_interrompe_a_espera_e_a_requisicao() {
    let server = MockServer::start().await;
    Mock::given(path("/lento"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
        .mount(&server)
        .await;
    let (client, _) = client();
    let cancel = CancellationToken::new();
    let request = client.get(url(&server, "/lento")).cancel(&cancel);
    let task = tokio::spawn(request.send());
    cancel.cancel();
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Cancelled));
    assert_eq!(error.detail(), None);
}

#[tokio::test]
async fn tempo_esgotado_vira_timeout() {
    let server = MockServer::start().await;
    Mock::given(path("/lento"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let mut config = HttpConfig::for_version("1");
    config.request_timeout = Duration::from_millis(200);
    config.max_retries = 0;
    let (client, _) = client_with(config);
    let error = client.get(url(&server, "/lento")).send().await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Core(CoreErrorCode::Timeout),
        "{error}"
    );
}
