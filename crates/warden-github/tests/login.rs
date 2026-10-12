//! Login pelo device flow e renovação contra o GitHub simulado (CA-UI03-03, CA-UI03-04,
//! CA-UI03-09). As esperas passam pelo `ManualTimer`, que anota quanto se esperou sem dormir.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{
    CLIENT_ID, DEVICE_CODE, access_token, capture_logs, client, endpoints, refresh_token,
};
use serde_json::json;
use warden_core::CancellationToken;
use warden_github::device::{refresh, request_code, wait_for_token};
use warden_github::{DeviceCode, Endpoints, Error, ExposeSecret, SecretString, WaitEvent};
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn code(interval: u64, expires_in: u64) -> DeviceCode {
    DeviceCode {
        user_code: "WDJB-MJHT".into(),
        device_code: SecretString::from(DEVICE_CODE.to_owned()),
        expires_in: Duration::from_secs(expires_in),
        interval: Duration::from_secs(interval),
    }
}

async fn mount_poll(server: &MockServer, reply: serde_json::Value, times: Option<u64>) {
    let mock = Mock::given(method("POST"))
        .and(path("/site/login/oauth/access_token"))
        .and(body_string_contains(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(reply));
    match times {
        Some(times) => mock.up_to_n_times(times).mount(server).await,
        None => mock.mount(server).await,
    }
}

fn token_reply(n: u32) -> serde_json::Value {
    json!({
        "access_token": access_token(n),
        "expires_in": 28800,
        "refresh_token": refresh_token(n),
        "refresh_token_expires_in": 15_897_600,
        "token_type": "bearer",
        "scope": ""
    })
}

#[tokio::test]
async fn pede_o_codigo_com_o_client_id_no_corpo() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/site/login/device/code"))
        .and(header("accept", "application/json"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_code": DEVICE_CODE,
            "user_code": "WDJB-MJHT",
            "verification_uri": "https://github.com/login/device",
            "expires_in": 899,
            "interval": 5
        })))
        .expect(1)
        .mount(&server)
        .await;
    let (http, _) = client();
    let code = request_code(&http, &endpoints(&server)).await.unwrap();
    assert_eq!(code.user_code, "WDJB-MJHT");
    assert_eq!(code.device_code.expose_secret(), DEVICE_CODE);
    assert_eq!(code.expires_in, Duration::from_secs(899));
    assert_eq!(code.interval, Duration::from_secs(5));
    // O segredo não aparece no `Debug`.
    assert!(!format!("{code:?}").contains(DEVICE_CODE));
    let received = server.received_requests().await.unwrap();
    assert_eq!(
        String::from_utf8(received[0].body.clone()).unwrap(),
        format!("client_id={CLIENT_ID}")
    );
    assert_eq!(received[0].url.query(), None);
}

#[tokio::test]
async fn ca_ui03_10_sem_client_id_nada_sai() {
    let server = MockServer::start().await;
    let (http, timer) = client();
    let empty = Endpoints::custom(&server.uri(), &server.uri(), "").unwrap();
    assert!(matches!(
        request_code(&http, &empty).await,
        Err(Error::LoginUnavailable)
    ));
    let refreshed = refresh(&http, &empty, &SecretString::from(refresh_token(1))).await;
    assert!(matches!(refreshed, Err(Error::LoginUnavailable)));
    assert!(server.received_requests().await.unwrap().is_empty());
    assert!(timer.sleeps().is_empty());
}

#[tokio::test]
async fn app_recusado_pelo_github() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/site/login/device/code"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": "device_flow_disabled",
            "error_description": "Device Flow must be explicitly enabled for this App"
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/site/login/device/code"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let (http, _) = client();
    let error = request_code(&http, &endpoints(&server)).await.unwrap_err();
    assert!(
        matches!(&error, Error::LoginAppRejected { reason } if reason == "device_flow_disabled"),
        "{error:?}"
    );
    let error = request_code(&http, &endpoints(&server)).await.unwrap_err();
    assert!(
        matches!(&error, Error::LoginAppRejected { reason } if reason == "HTTP 404"),
        "{error:?}"
    );
}

#[tokio::test]
async fn ca_ui03_03_espera_respeita_interval_e_slow_down() {
    let server = MockServer::start().await;
    mount_poll(
        &server,
        json!({ "error": "authorization_pending" }),
        Some(2),
    )
    .await;
    mount_poll(
        &server,
        json!({ "error": "slow_down", "interval": 10 }),
        Some(1),
    )
    .await;
    mount_poll(&server, json!({ "error": "slow_down" }), Some(1)).await;
    mount_poll(&server, token_reply(1), None).await;
    let (http, timer) = client();
    let (logs, _guard) = capture_logs();
    let mut events = Vec::new();
    let tokens = wait_for_token(
        &http,
        &endpoints(&server),
        &code(5, 900),
        &timer,
        &CancellationToken::new(),
        |event| events.push(event),
    )
    .await
    .unwrap();
    assert_eq!(tokens.access_token.expose_secret(), access_token(1));
    assert_eq!(
        tokens.refresh_token.as_ref().unwrap().expose_secret(),
        refresh_token(1)
    );
    assert_eq!(tokens.expires_in, Some(Duration::from_hours(8)));
    assert_eq!(
        tokens.refresh_token_expires_in,
        Some(Duration::from_hours(4_416))
    );
    // 5, 5 (pendentes), 5 (slow_down para 10), 10 (slow_down sem intervalo: +5), 15.
    let seconds: Vec<u64> = timer.sleeps().iter().map(Duration::as_secs).collect();
    assert_eq!(seconds, [5, 5, 5, 10, 15]);
    assert_eq!(
        events,
        [
            WaitEvent::Pending,
            WaitEvent::Pending,
            WaitEvent::SlowDown(Duration::from_secs(10)),
            WaitEvent::SlowDown(Duration::from_secs(15)),
        ]
    );
    // CA-UI03-09: nem o device_code nem os tokens aparecem nos registros.
    let text = logs.text();
    assert!(text.contains("consultar mais devagar"), "{text}");
    for secret in [DEVICE_CODE.to_owned(), access_token(1), refresh_token(1)] {
        assert!(!text.contains(&secret), "segredo nos registros:\n{text}");
    }
    let format = format!("{tokens:?}");
    assert!(!format.contains(&access_token(1)) && !format.contains(&refresh_token(1)));
}

#[tokio::test]
async fn ca_ui03_03_codigo_vencido_e_acesso_recusado() {
    for (reply, expected) in [
        (json!({ "error": "expired_token" }), "vencido"),
        (json!({ "error": "access_denied" }), "recusado"),
        (json!({ "error": "unsupported_grant_type" }), "app"),
    ] {
        let server = MockServer::start().await;
        mount_poll(&server, reply, None).await;
        let (http, timer) = client();
        let error = wait_for_token(
            &http,
            &endpoints(&server),
            &code(5, 900),
            &timer,
            &CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();
        match expected {
            "vencido" => assert!(matches!(error, Error::LoginCodeExpired), "{error:?}"),
            "recusado" => assert!(matches!(error, Error::LoginDenied), "{error:?}"),
            _ => assert!(
                matches!(&error, Error::LoginAppRejected { reason } if reason == "unsupported_grant_type"),
                "{error:?}"
            ),
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn ca_ui03_03_prazo_do_codigo_acaba_sem_esperar_o_github() {
    let server = MockServer::start().await;
    mount_poll(&server, json!({ "error": "authorization_pending" }), None).await;
    let (http, timer) = client();
    let error = wait_for_token(
        &http,
        &endpoints(&server),
        &code(5, 12),
        &timer,
        &CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap_err();
    assert!(matches!(error, Error::LoginCodeExpired), "{error:?}");
    // Consultou em 5 s e 10 s; em 15 s o código já tinha vencido.
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
    assert_eq!(timer.total_slept(), Duration::from_secs(15));
}

#[tokio::test]
async fn ca_ui03_03_cancelar_para_sem_nova_requisicao() {
    let server = MockServer::start().await;
    mount_poll(&server, json!({ "error": "authorization_pending" }), None).await;
    let (http, timer) = client();
    let cancel = CancellationToken::new();
    let mut polls = 0;
    let error = wait_for_token(
        &http,
        &endpoints(&server),
        &code(5, 900),
        &timer,
        &cancel,
        |_| {
            polls += 1;
            if polls == 2 {
                cancel.cancel();
            }
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(error, Error::Cancelled), "{error:?}");
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn ca_ui03_04_renovacao_troca_os_dois_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/site/login/oauth/access_token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains(refresh_token(1).as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_reply(2)))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/site/login/oauth/access_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": "bad_refresh_token",
            "error_description": "The refresh token passed is incorrect or expired."
        })))
        .mount(&server)
        .await;
    let (http, _) = client();
    let (logs, _guard) = capture_logs();
    let tokens = refresh(
        &http,
        &endpoints(&server),
        &SecretString::from(refresh_token(1)),
    )
    .await
    .unwrap();
    assert_eq!(tokens.access_token.expose_secret(), access_token(2));
    assert_eq!(
        tokens.refresh_token.unwrap().expose_secret(),
        refresh_token(2)
    );
    let error = refresh(
        &http,
        &endpoints(&server),
        &SecretString::from(refresh_token(9)),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&error, Error::SessionExpired { reason } if reason == "bad_refresh_token"),
        "{error:?}"
    );
    assert!(error.needs_login());
    // O token de renovação vai no corpo, nunca no endereço nem nos registros.
    for request in server.received_requests().await.unwrap() {
        assert_eq!(request.url.query(), None);
    }
    let text = logs.text();
    for secret in [
        refresh_token(1),
        refresh_token(2),
        refresh_token(9),
        access_token(2),
    ] {
        assert!(!text.contains(&secret), "segredo nos registros:\n{text}");
        assert!(!error.to_string().contains(&secret));
    }
}

#[tokio::test]
async fn resposta_sem_token_e_invalida() {
    let server = MockServer::start().await;
    mount_poll(&server, json!({ "token_type": "bearer" }), None).await;
    let (http, timer) = client();
    let error = wait_for_token(
        &http,
        &endpoints(&server),
        &code(5, 900),
        &timer,
        &CancellationToken::new(),
        |_| {},
    )
    .await
    .unwrap_err();
    assert!(matches!(error, Error::InvalidResponse(_)), "{error:?}");
}
