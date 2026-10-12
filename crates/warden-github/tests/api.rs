//! A API do GitHub no repositório do Warden contra o GitHub simulado: conta, labels, branch
//! órfã dos prints, envio de arquivos, issue e cada erro (CA-UI03-05, CA-UI03-06, CA-UI03-08,
//! CA-UI03-09).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use base64::Engine as _;
use common::{access_token, capture_logs, client, endpoints};
use serde_json::{Value, json};
use warden_core::{CoreErrorCode, DomainCode, DomainError as _};
use warden_github::{Error, GithubApi, GithubErrorCode, Label, SecretString};
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const REPO: &str = "/api/repos/Kriticales/Warden";

fn api(server: &MockServer) -> GithubApi {
    GithubApi::new(
        client().0,
        endpoints(server),
        &SecretString::from(access_token(1)),
    )
}

fn label(name: &str) -> Label {
    Label {
        name: name.into(),
        color: "d73a4a".into(),
        description: "Relato enviado pelo Warden".into(),
    }
}

fn domain(error: &Error) -> GithubErrorCode {
    match error.code() {
        DomainCode::Domain(code) => code,
        DomainCode::Core(code) => panic!("código comum {code:?} para {error:?}"),
    }
}

async fn bodies(server: &MockServer) -> Vec<(String, String, Value)> {
    server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .map(|request| {
            let body = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
            (
                request.method.to_string(),
                request.url.path().to_owned(),
                body,
            )
        })
        .collect()
}

#[tokio::test]
async fn conta_do_login_com_o_token_no_cabecalho() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/user"))
        .and(header(
            "authorization",
            format!("Bearer {}", access_token(1)).as_str(),
        ))
        .and(header("accept", "application/vnd.github+json"))
        .and(header("x-github-api-version", "2022-11-28"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "login": "Kriticales", "id": 1 })),
        )
        .mount(&server)
        .await;
    let (logs, _guard) = capture_logs();
    let api = api(&server);
    assert_eq!(api.user().await.unwrap().login, "Kriticales");
    assert!(!format!("{api:?}").contains(&access_token(1)));
    assert!(!logs.text().contains(&access_token(1)), "{}", logs.text());
}

#[tokio::test]
async fn ca_ui03_05_cria_so_as_labels_que_faltam() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("{REPO}/labels/relato-do-dono")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "name": "relato-do-dono" })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("{REPO}/labels/categoria:%20sugest%C3%A3o")))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "message": "Not Found" })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{REPO}/labels")))
        .and(body_partial_json(
            json!({ "name": "categoria: sugestão", "color": "d73a4a" }),
        ))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(json!({ "name": "categoria: sugestão" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let api = api(&server);
    assert!(!api.ensure_label(&label("relato-do-dono")).await.unwrap());
    assert!(
        api.ensure_label(&label("categoria: sugestão"))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn label_criada_ao_mesmo_tempo_nao_e_erro() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "message": "Validation Failed",
            "errors": [{ "resource": "Label", "code": "already_exists", "field": "name" }]
        })))
        .mount(&server)
        .await;
    assert!(
        !api(&server)
            .ensure_label(&label("relato-do-dono"))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn ca_ui03_05_branch_dos_prints_nasce_de_um_commit_orfao() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("{REPO}/git/ref/heads/relatos-anexos")))
        .respond_with(ResponseTemplate::new(404))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("{REPO}/git/ref/heads/relatos-anexos")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ref": "refs/heads/relatos-anexos",
            "object": { "sha": "c0ffee", "type": "commit" }
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{REPO}/git/trees")))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "sha": "a1b2" })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{REPO}/git/commits")))
        .and(body_partial_json(json!({ "tree": "a1b2", "parents": [] })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "sha": "c0ffee" })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{REPO}/git/refs")))
        .and(body_partial_json(
            json!({ "ref": "refs/heads/relatos-anexos", "sha": "c0ffee" }),
        ))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(json!({ "ref": "refs/heads/relatos-anexos" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let api = api(&server);
    assert!(api.ensure_attachments_branch().await.unwrap());
    // Na segunda vez ela já existe: nada é criado.
    assert!(!api.ensure_attachments_branch().await.unwrap());
    let requests = bodies(&server).await;
    // A árvore não tem base (não sai do `main`) e nada mexe em outra referência.
    let tree = &requests
        .iter()
        .find(|(_, p, _)| p.ends_with("/git/trees"))
        .unwrap()
        .2;
    assert!(tree.get("base_tree").is_none());
    assert_eq!(tree["tree"][0]["path"], "LEIA-ME.md");
    for (verb, request_path, body) in &requests {
        assert!(!request_path.contains("main"), "{verb} {request_path}");
        assert!(!body.to_string().contains("refs/heads/main"), "{body}");
        assert!(verb == "GET" || verb == "POST", "{verb} {request_path}");
    }
}

#[tokio::test]
async fn ca_ui03_05_print_vai_para_a_branch_dos_anexos() {
    let server = MockServer::start().await;
    let png = b"\x89PNG\r\n\x1a\nconteudo".to_vec();
    Mock::given(method("PUT"))
        .and(path(format!("{REPO}/contents/anexos/20261012-abc.png")))
        .and(body_partial_json(json!({
            "branch": "relatos-anexos",
            "content": base64::engine::general_purpose::STANDARD.encode(&png),
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "content": { "path": "anexos/20261012-abc.png", "download_url": "https://raw.githubusercontent.com/x?token=SEGREDO" }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let url = api(&server)
        .upload_attachment("20261012-abc.png", &png)
        .await
        .unwrap();
    assert_eq!(
        url.as_str(),
        format!(
            "{}/site/Kriticales/Warden/blob/relatos-anexos/anexos/20261012-abc.png?raw=true",
            server.uri()
        )
    );
    assert!(!url.as_str().contains("SEGREDO"));
}

#[tokio::test]
async fn nome_de_anexo_que_sairia_da_pasta_e_recusado_sem_requisicao() {
    let server = MockServer::start().await;
    let api = api(&server);
    for name in ["", "../x.png", "a/b.png", ".png", "a b.png"] {
        let error = api.upload_attachment(name, b"x").await.unwrap_err();
        assert!(matches!(error, Error::Internal(_)), "{name}: {error:?}");
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn ca_ui03_05_issue_com_titulo_corpo_e_labels() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("{REPO}/issues")))
        .and(body_partial_json(json!({
            "title": "O Testar travou",
            "body": "corpo",
            "labels": ["relato-do-dono", "categoria: erro"],
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "number": 42,
            "html_url": "https://evil.example/Kriticales/Warden/issues/42"
        })))
        .mount(&server)
        .await;
    let issue = api(&server)
        .create_issue(
            "O Testar travou",
            "corpo",
            &["relato-do-dono".into(), "categoria: erro".into()],
        )
        .await
        .unwrap();
    assert_eq!(issue.number, 42);
    // O endereço é montado pelo Warden, não copiado da resposta.
    assert_eq!(
        issue.url.as_str(),
        format!("{}/site/Kriticales/Warden/issues/42", server.uri())
    );
}

#[tokio::test]
async fn ca_ui03_08_cada_codigo_vira_o_erro_certo() {
    let cases: Vec<(ResponseTemplate, GithubErrorCode)> = vec![
        (ResponseTemplate::new(401), GithubErrorCode::SessionExpired),
        (
            ResponseTemplate::new(403)
                .set_body_json(json!({ "message": "Resource not accessible by integration" })),
            GithubErrorCode::PermissionDenied,
        ),
        (
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "4102444800"),
            GithubErrorCode::RateLimited,
        ),
        (
            ResponseTemplate::new(403).insert_header("retry-after", "60"),
            GithubErrorCode::RateLimited,
        ),
        (
            ResponseTemplate::new(404),
            GithubErrorCode::RepositoryNotFound,
        ),
        (
            ResponseTemplate::new(422).set_body_json(json!({ "message": "Validation Failed" })),
            GithubErrorCode::RequestRejected,
        ),
        (ResponseTemplate::new(502), GithubErrorCode::Unavailable),
    ];
    for (reply, expected) in cases {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!("{REPO}/issues")))
            .respond_with(reply)
            .mount(&server)
            .await;
        let error = api(&server).create_issue("t", "b", &[]).await.unwrap_err();
        assert_eq!(domain(&error), expected, "{error:?}");
        if expected == GithubErrorCode::RateLimited {
            assert!(error.params().contains_key("seconds"), "{error:?}");
        }
        if expected == GithubErrorCode::RequestRejected {
            assert_eq!(error.params()["status"], "422");
            assert!(error.to_string().contains("Validation Failed"));
        }
        // POST não é repetido sozinho.
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        assert!(!error.to_string().contains(&access_token(1)));
    }
}

#[tokio::test]
async fn ca_ui03_08_sem_internet_e_erro_comum_de_rede() {
    // Porta fechada: ninguém escuta nela.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let base = format!("http://127.0.0.1:{port}/");
    let endpoints = warden_github::Endpoints::custom(&base, &base, common::CLIENT_ID).unwrap();
    let api = GithubApi::new(client().0, endpoints, &SecretString::from(access_token(1)));
    let error = api.create_issue("t", "b", &[]).await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Core(CoreErrorCode::NetworkUnavailable),
        "{error:?}"
    );
}

#[tokio::test]
async fn print_que_ja_esta_na_branch_nao_e_erro() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "message": "Invalid request.\n\n\"sha\" wasn't supplied."
        })))
        .mount(&server)
        .await;
    let url = api(&server)
        .upload_attachment("abc.jpg", b"\xff\xd8\xff")
        .await
        .unwrap();
    assert!(url.as_str().ends_with("/anexos/abc.jpg?raw=true"));
}
