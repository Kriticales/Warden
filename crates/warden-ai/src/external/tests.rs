use super::*;
use warden_diagnostics::RedactionProfile;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn redactor() -> Redactor {
    Redactor::new(RedactionProfile::new().with_username("Steve_42"))
}

fn endpoints(server: &MockServer) -> Endpoints {
    let root = Url::parse(&format!("{}/", server.uri())).unwrap();
    Endpoints {
        github: root.clone(),
        modrinth: root.clone(),
        curseforge: root,
    }
}

fn client(server: &MockServer, token: Option<SecretString>) -> ExternalClient {
    let mut config = HttpConfig::for_version("test");
    config.max_retries = 1;
    ExternalClient::new(config, endpoints(server), token).unwrap()
}

fn links() -> ModLinks {
    ModLinks {
        source_url: Some("https://github.com/example/mod".into()),
        issues_url: Some("https://github.com/example/mod/issues".into()),
    }
}

#[test]
fn repo_somente_do_dominio_exato() {
    assert!(GithubRepo::from_url("https://github.com.evil.test/a/b").is_none());
    assert!(GithubRepo::from_url("https://github.com@evil.test/a/b").is_none());
    assert!(GithubRepo::from_url("http://github.com/a/b").is_none());
    assert_eq!(
        GithubRepo::from_url("https://github.com/a/b/issues/42")
            .unwrap()
            .label(),
        "a/b"
    );
}

#[tokio::test]
async fn ca_t14_13_so_repositorio_e_termos_redigidos_saem_e_sem_permissao_nao_ha_rede() {
    let server = MockServer::start().await;
    let client = client(&server, None);
    assert!(matches!(
        client
            .search_mod_issues(&links(), "NullPointerException", &redactor(), false)
            .await,
        Err(ExternalError::PermissionDenied)
    ));
    assert!(server.received_requests().await.unwrap().is_empty());
    assert!(matches!(
        client
            .search_mod_issues(&links(), "config/../latest.log", &redactor(), true)
            .await,
        Err(ExternalError::UnsafeQuery)
    ));
    assert!(matches!(
        client
            .search_mod_issues(&links(), "Steve_42", &redactor(), true)
            .await,
        Err(ExternalError::UnsafeQuery)
    ));
    assert!(server.received_requests().await.unwrap().is_empty());

    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .and(query_param("q", "repo:example/mod is:issue NullPointerException"))
        .and(query_param("per_page", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"items": [
            {"number": 42, "title": "Falha em Steve_42", "html_url": "https://github.com/example/mod/issues/42"},
            {"number": 43, "title": "PR", "html_url": "https://github.com/example/mod/pull/43", "pull_request": {"url": "x"}}
        ]})))
        .expect(1)
        .mount(&server)
        .await;
    let first = client
        .search_mod_issues(&links(), "NullPointerException", &redactor(), true)
        .await
        .unwrap();
    let cached = client
        .search_mod_issues(&links(), "NullPointerException", &redactor(), true)
        .await
        .unwrap();
    assert_eq!(first, cached);
    assert_eq!(first.issues.len(), 1);
    assert_eq!(first.issues[0].evidence_id, "issue:example/mod#42");
    assert!(!first.issues[0].title.contains("Steve_42"));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].url.as_str().contains("Steve_42"));
}

#[tokio::test]
async fn github_usa_token_quando_disponivel_e_respeita_retry_after() {
    let server = MockServer::start().await;
    Mock::given(path("/search/issues"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/search/issues"))
        .and(header("authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"items": []})))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server, Some(SecretString::new("test-token".into())));
    let result = client
        .search_mod_issues(&links(), "NullPointerException", &redactor(), true)
        .await
        .unwrap();
    assert!(result.issues.is_empty());
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn changelog_modrinth_entre_versoes() {
    let server = MockServer::start().await;
    Mock::given(path("/project/mod/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"id": "v3", "version_number": "3", "changelog": "Fixed crash"},
            {"id": "v2", "version_number": "2", "changelog": "Changed feature"},
            {"id": "v1", "version_number": "1", "changelog": "Initial"}
        ])))
        .mount(&server)
        .await;
    let result = client(&server, None)
        .get_mod_changelog(
            &ChangelogSource::Modrinth {
                project_id: "mod".into(),
            },
            "v1",
            "v3",
            &redactor(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].text, "Fixed crash");
    assert_eq!(result[1].version, "2");
}

#[tokio::test]
async fn changelog_curseforge_converte_html_e_usa_chave() {
    let server = MockServer::start().await;
    Mock::given(path("/mods/7/files"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": [
                {"id": 3, "displayName": "3"}, {"id": 2, "displayName": "2"}
            ]})),
        )
        .mount(&server)
        .await;
    Mock::given(path("/mods/7/files/3/changelog"))
        .and(header("x-api-key", "test-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"data": "<p>Fixed &amp; improved</p>"})),
        )
        .mount(&server)
        .await;
    let key = SecretString::new("test-key".into());
    let result = client(&server, None)
        .get_mod_changelog(
            &ChangelogSource::CurseForge { mod_id: 7 },
            "2",
            "3",
            &redactor(),
            Some(&key),
        )
        .await
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "Fixed & improved");
}

#[tokio::test]
async fn changelog_github_releases_entre_tags() {
    let server = MockServer::start().await;
    Mock::given(path("/repos/example/mod/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {"tag_name": "v3", "body": "Fix"},
            {"tag_name": "v2", "body": "New"},
            {"tag_name": "v1", "body": "Old"}
        ])))
        .mount(&server)
        .await;
    let result = client(&server, None)
        .get_mod_changelog(
            &ChangelogSource::Github {
                repository: GithubRepo {
                    owner: "example".into(),
                    name: "mod".into(),
                },
            },
            "v1",
            "v3",
            &redactor(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        result
            .iter()
            .map(|v| v.version.as_str())
            .collect::<Vec<_>>(),
        vec!["v3", "v2"]
    );
}

#[tokio::test]
#[ignore = "rede"]
async fn rede_github_issues_publicas() {
    let client = ExternalClient::new(
        HttpConfig::default(),
        Endpoints::production().unwrap(),
        None,
    )
    .unwrap();
    let links = ModLinks {
        source_url: Some("https://github.com/Kriticales/Warden".into()),
        issues_url: None,
    };
    let _ = client
        .search_mod_issues(&links, "crash", &redactor(), true)
        .await
        .unwrap();
}
