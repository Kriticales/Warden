#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use secrecy::SecretString;
use warden_diagnostics::Redactor;
use warden_diagnostics::redact::RedactionProfile;
use warden_http::HttpConfig;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

fn redactor() -> Redactor {
    Redactor::new(
        RedactionProfile::new()
            .with_username("Maria")
            .with_player("Steve_42"),
    )
}

fn config() -> HttpConfig {
    let mut config = HttpConfig::for_version("0.0.0");
    config.max_retries = 0;
    config
}

fn repo() -> GithubRepo {
    GithubRepo {
        owner: "MehVahdJukaar".into(),
        name: "Supplementaries".into(),
    }
}

fn issues_body() -> serde_json::Value {
    serde_json::json!({
        "total_count": 3,
        "items": [
            { "number": 3121, "title": "NPE in LivingEntity.travel with Epic Fight (Steve_42)", "state": "closed",
              "html_url": "https://github.com/MehVahdJukaar/Supplementaries/issues/3121" },
            { "number": 3200, "title": "PR que não conta", "state": "open", "pull_request": {},
              "html_url": "https://github.com/MehVahdJukaar/Supplementaries/pull/3200" },
            { "number": 2987, "title": "Outro erro", "state": "open",
              "html_url": "https://github.com/MehVahdJukaar/Supplementaries/issues/2987" }
        ]
    })
}

#[test]
fn repositorio_so_do_github_com_exato() {
    let ok =
        GithubRepo::from_url("https://github.com/MehVahdJukaar/Supplementaries/issues").unwrap();
    assert_eq!(ok.label(), "MehVahdJukaar/Supplementaries");
    assert_eq!(
        GithubRepo::from_url("https://github.com/a/b.git")
            .unwrap()
            .label(),
        "a/b"
    );
    for bad in [
        "http://github.com/a/b",
        "https://github.com.evil.example/a/b",
        "https://gitlab.com/a/b",
        "https://user:pw@github.com/a/b",
        "https://github.com:444/a/b",
        "https://github.com/a",
        "https://github.com/../b",
        "não é link",
    ] {
        assert!(GithubRepo::from_url(bad).is_none(), "{bad}");
    }
    // O issues_url vem primeiro; um source_url fora do GitHub não conta.
    let both = GithubRepo::from_links(
        Some("https://github.com/x/issues-repo/issues"),
        Some("https://github.com/x/source-repo"),
    );
    assert_eq!(both.unwrap().name, "issues-repo");
    assert!(GithubRepo::from_links(None, Some("https://codeberg.org/x/y")).is_none());
}

#[test]
fn termos_so_palavras_do_erro() {
    let r = redactor();
    assert_eq!(
        search_terms("NullPointerException  at travel at travel", &r).unwrap(),
        "NullPointerException travel",
        "palavras de 1 ou 2 letras saem e repetidas contam uma vez"
    );
    assert_eq!(
        search_terms("java.lang.NullPointerException railways$tickBogeys", &r).unwrap(),
        "java.lang.NullPointerException railways$tickBogeys"
    );
    for unsafe_query in [
        "[21:13:58] [Server thread/ERROR]: travou",
        "C:\\Users\\Maria\\AppData",
        "/home/maria/.minecraft",
        "erro com Steve_42",
        "Maria",
        "conectar 192.168.0.10",
        "maria@example.com",
        "chave $2a$10$abcdefghijklmnopqrstuv",
        "linha\noutra",
        "uma duas tres quatro cinco seis sete oito nove",
        "a b",
        "",
    ] {
        assert!(
            matches!(
                search_terms(unsafe_query, &r),
                Err(ExternalError::UnsafeQuery)
            ),
            "{unsafe_query:?}"
        );
    }
}

#[tokio::test]
async fn ca_t14_13_so_repositorio_e_termos_saem_e_o_bloco_e_o_pedido_exato() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/issues"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issues_body()))
        .expect(1)
        .mount(&server)
        .await;
    let github = GithubIssues::with_api(&config(), &server.uri()).unwrap();
    let terms = search_terms("NullPointerException travel", &redactor()).unwrap();
    let search = github
        .search(&repo(), &terms, None, &redactor(), None)
        .await
        .unwrap();

    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1);
    let query: Vec<(String, String)> = received[0]
        .url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert_eq!(
        query,
        [
            (
                "q".to_owned(),
                "repo:MehVahdJukaar/Supplementaries is:issue NullPointerException travel"
                    .to_owned()
            ),
            ("per_page".to_owned(), "5".to_owned())
        ]
    );
    // CA-T14-03: o bloco "Enviado ao GitHub" é byte a byte o pedido recebido (o simulado
    // reescreve o servidor como `localhost`; o caminho e a consulta são os bytes que chegaram).
    let sent_path = format!(
        "{}?{}",
        received[0].url.path(),
        received[0].url.query().unwrap()
    );
    assert_eq!(
        search.request.request,
        format!("GET {}{}", server.uri(), sent_path)
    );
    assert!(!search.request.cached);
    assert!(received[0].headers.get("authorization").is_none());
    // Sem pull requests; o título passa pela redação.
    assert_eq!(search.issues.len(), 2);
    assert!(!search.issues[0].title.contains("Steve_42"));
    assert!(!search.issues[0].open);
    let items = search.items();
    assert_eq!(items[0].id, "issue:MehVahdJukaar/Supplementaries#3121");
    assert!(
        items[0]
            .text
            .starts_with("#3121 (fechada) NPE in LivingEntity.travel")
    );
}

#[tokio::test]
async fn token_quando_existe_e_cache_de_uma_hora() {
    let server = MockServer::start().await;
    Mock::given(path("/search/issues"))
        .and(header("authorization", "Bearer token-de-teste"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issues_body()))
        .expect(1)
        .mount(&server)
        .await;
    let github = GithubIssues::with_api(&config(), &server.uri()).unwrap();
    let token = SecretString::from("token-de-teste");
    let first = github
        .search(
            &repo(),
            "NullPointerException",
            Some(&token),
            &redactor(),
            None,
        )
        .await
        .unwrap();
    let second = github
        .search(
            &repo(),
            "NullPointerException",
            Some(&token),
            &redactor(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(first.issues, second.issues);
    assert!(second.request.cached, "a segunda vem do cache, sem rede");
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert!(!format!("{github:?}").contains("token-de-teste"));
}

#[tokio::test]
async fn cache_vencido_busca_de_novo() {
    let server = MockServer::start().await;
    Mock::given(path("/search/issues"))
        .respond_with(ResponseTemplate::new(200).set_body_json(issues_body()))
        .expect(2)
        .mount(&server)
        .await;
    let github = GithubIssues::with_api(&config(), &server.uri())
        .unwrap()
        .with_ttl(Duration::ZERO);
    for _ in 0..2 {
        let search = github
            .search(&repo(), "NullPointerException", None, &redactor(), None)
            .await
            .unwrap();
        assert!(!search.request.cached);
    }
}

#[tokio::test]
async fn limite_do_github_e_repositorio_que_nao_existe() {
    let server = MockServer::start().await;
    Mock::given(path("/search/issues"))
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "1760000000")
                .set_body_json(serde_json::json!({ "message": "API rate limit exceeded" })),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(path("/search/issues"))
        .respond_with(
            ResponseTemplate::new(422)
                .set_body_json(serde_json::json!({ "message": "Validation Failed" })),
        )
        .mount(&server)
        .await;
    let github = GithubIssues::with_api(&config(), &server.uri()).unwrap();
    let limited = github
        .search(&repo(), "NullPointerException", None, &redactor(), None)
        .await;
    assert!(
        matches!(limited, Err(ExternalError::RateLimited)),
        "{limited:?}"
    );
    // O instante Unix do X-Ratelimit-Reset não bloqueia o cliente: a busca seguinte sai.
    let missing = tokio::time::timeout(
        Duration::from_secs(10),
        github.search(&repo(), "travel", None, &redactor(), None),
    )
    .await
    .expect("o cliente ficou bloqueado pelo X-Ratelimit-Reset");
    assert!(
        matches!(missing, Err(ExternalError::RepositoryNotFound { ref repository }) if repository == "MehVahdJukaar/Supplementaries"),
        "{missing:?}"
    );
}

fn version(id: &str, number: &str, text: &str) -> ChangelogVersion {
    ChangelogVersion {
        id: id.into(),
        number: number.into(),
        published: "2026-09-12T10:00:00Z".into(),
        text: text.into(),
    }
}

#[test]
fn changelog_entre_versoes_sem_a_de_partida() {
    let versions = [
        version("v4", "2.8.22", "nova"),
        version("v3", "2.8.21", "Corrige NPE no travel com Epic Fight"),
        version("v2", "2.8.18", "outra"),
        version("v1", "2.8.17", "instalada"),
    ];
    let (range, left_out) = changelog_range(&versions, "2.8.17", "v3").unwrap();
    let numbers: Vec<&str> = range.iter().map(|v| v.number.as_str()).collect();
    assert_eq!(numbers, ["2.8.21", "2.8.18"]);
    assert_eq!(left_out, 0);
    let items = changelog_items("supplementaries", &range, left_out, &redactor());
    assert_eq!(items[0].id, "changelog:supplementaries@2.8.21");
    assert_eq!(
        items[0].text,
        "Versão 2.8.21 (2026-09-12):\nCorrige NPE no travel com Epic Fight"
    );
    assert!(matches!(
        changelog_range(&versions, "v3", "v1"),
        Err(ExternalError::WrongOrder)
    ));
    assert!(matches!(
        changelog_range(&versions, "v1", "v1"),
        Err(ExternalError::WrongOrder)
    ));
    assert!(matches!(
        changelog_range(&versions, "9.9", "v4"),
        Err(ExternalError::VersionNotFound { ref version }) if version == "9.9"
    ));
}

#[test]
fn changelog_longo_fica_com_as_mais_proximas_da_pedida() {
    let versions: Vec<ChangelogVersion> = (0..30)
        .rev()
        .map(|n| {
            version(
                &format!("v{n}"),
                &format!("1.{n}"),
                "notas de C:\\Users\\Maria\\x",
            )
        })
        .collect();
    let (range, left_out) = changelog_range(&versions, "1.0", "1.29").unwrap();
    assert_eq!(range.len(), MAX_CHANGELOG_VERSIONS);
    assert_eq!(range[0].number, "1.29");
    assert_eq!(left_out, 29 - MAX_CHANGELOG_VERSIONS);
    let items = changelog_items("x", &range, left_out, &redactor());
    assert_eq!(items.len(), MAX_CHANGELOG_VERSIONS + 1);
    assert!(items.iter().all(|item| !item.text.contains("Maria")));
}

#[test]
fn html_da_curseforge_vira_texto() {
    assert_eq!(
        html_to_text(
            "<p>Corrige&nbsp;NPE &amp; travel</p><ul><li>Item &lt;1&gt;</li><li>Outro</li></ul><br/>fim"
        ),
        "Corrige NPE & travel\nItem <1>\nOutro\nfim"
    );
    assert_eq!(html_to_text("sem tags"), "sem tags");
    assert_eq!(html_to_text("quebrado <b"), "quebrado");
}

#[test]
fn declaracoes_no_esquema_da_d04() {
    let specs = tools();
    let names: Vec<&str> = specs.iter().map(|spec| spec.name).collect();
    assert_eq!(
        names,
        [
            GET_BISECT_RESULT,
            GET_MIXIN_REPORT,
            GET_MOD_CHANGELOG,
            SEARCH_MOD_ISSUES
        ]
    );
    let issues = specs.iter().find(|s| s.name == SEARCH_MOD_ISSUES).unwrap();
    assert_eq!(issues.needs, Needs::Github, "só com a permissão do GitHub");
    assert_eq!(issues.stage, Stage::AfterSuspect);
    assert_eq!(specs[0].stage, Stage::Initial);
}

/// Busca pública de verdade (`cargo xtask test-network`): não precisa de chave.
#[tokio::test]
#[ignore = "rede"]
async fn rede_d14_busca_de_issues_publica() {
    let github = GithubIssues::with_api(&HttpConfig::for_version("0.0.0"), GITHUB_API).unwrap();
    let sodium = GithubRepo {
        owner: "CaffeineMC".into(),
        name: "sodium".into(),
    };
    match github
        .search(&sodium, "crash", None, &redactor(), None)
        .await
    {
        Ok(search) => assert!(search.issues.len() <= MAX_ISSUES),
        Err(ExternalError::RateLimited) => {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("rede_d14: inconclusivo (limite do GitHub para buscas sem token)");
            }
        }
        Err(error) => panic!("{error}"),
    }
}
