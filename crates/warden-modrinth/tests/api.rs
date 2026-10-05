//! Cliente do Modrinth contra um servidor simulado que devolve respostas reais gravadas em
//! 04/10/2026 (`tests/fixtures/http/`, origem em `FIXTURES.md`).

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use warden_core::{CoreErrorCode, DomainCode, DomainError};
use warden_http::{HttpClient, HttpConfig, ManualTimer};
use warden_modrinth::{
    DependencyType, Environment, Error, Facets, HashAlgorithm, MetadataCache, ModrinthClient,
    ModrinthErrorCode, ProjectStatus, ProjectType, SearchIndex, SearchQuery, SideSupport,
    UnixClock, UpdateFilter, VersionFilter, VersionType,
};
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("fixtures/http/2026-10-04-", $name, ".json"))
    };
}

const SODIUM: &str = "AANobbMI";

fn json_fixture(text: &str) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_string(text)
}

struct Setup {
    server: MockServer,
    client: ModrinthClient,
    timer: ManualTimer,
    now: Arc<AtomicI64>,
}

async fn setup() -> Setup {
    let server = MockServer::start().await;
    let timer = ManualTimer::new();
    let http =
        HttpClient::with_timer(HttpConfig::for_version("1"), Arc::new(timer.clone())).unwrap();
    let now = Arc::new(AtomicI64::new(1_790_000_000));
    let reader = Arc::clone(&now);
    let clock: UnixClock = Arc::new(move || reader.load(Ordering::SeqCst));
    let cache = MetadataCache::in_memory(Some(clock)).unwrap();
    let client =
        ModrinthClient::with_base_url(http, &format!("{}/v2", server.uri()), Some(cache)).unwrap();
    Setup {
        server,
        client,
        timer,
        now,
    }
}

#[tokio::test]
async fn endereco_padrao_da_api() {
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let client =
        ModrinthClient::with_base_url(http.clone(), "https://api.modrinth.com/v2", None).unwrap();
    assert_eq!(client.base_url().as_str(), "https://api.modrinth.com/v2/");
    assert!(client.cache().is_none());
    // Sem a variável de ambiente, o endereço oficial.
    if std::env::var_os(warden_modrinth::BASE_URL_ENV).is_none() {
        let client = ModrinthClient::new(http, None).unwrap();
        assert_eq!(
            client.base_url().as_str(),
            warden_modrinth::DEFAULT_BASE_URL
        );
    }
}

#[tokio::test]
async fn busca_com_facets_codificados() {
    let s = setup().await;
    Mock::given(method("GET"))
        .and(path("/v2/search"))
        .and(query_param("query", "sodium"))
        .and(query_param(
            "facets",
            r#"[["categories:fabric"],["versions:1.21.1"],["project_type:mod"]]"#,
        ))
        .and(query_param("index", "relevance"))
        .and(query_param("limit", "5"))
        .respond_with(json_fixture(fixture!("search-sodium-fabric-1.21.1")))
        .expect(1)
        .mount(&s.server)
        .await;
    let query = SearchQuery::new("sodium")
        .facets(
            Facets::new()
                .loaders(["fabric"])
                .game_versions(["1.21.1"])
                .project_type(ProjectType::Mod),
        )
        .index(SearchIndex::Relevance)
        .page(0, 5);
    let results = s.client.search(&query, None).await.unwrap();
    assert_eq!(results.total_hits, 47);
    assert_eq!(results.limit, 5);
    let first = &results.hits[0];
    assert_eq!(first.project_id, SODIUM);
    assert_eq!(first.slug, "sodium");
    assert_eq!(first.environment, vec![Environment::ClientOnly]);
    assert_eq!(first.client_side, SideSupport::Required);
    assert!(first.categories.contains(&"fabric".to_owned()));
    // Colchetes codificados na URL (com colchetes crus a API responde 400).
    let requests = s.server.received_requests().await.unwrap();
    let raw_query = requests[0].url.query().unwrap();
    assert!(!raw_query.contains('['), "{raw_query}");
    assert!(raw_query.contains("%5B%5B"), "{raw_query}");
}

/// Gancho 1.1 (ADR-0039, item 7) com a resposta real do Sodium.
#[tokio::test]
async fn projeto_com_cache_de_24h_e_gancho_1_1() {
    let s = setup().await;
    Mock::given(path("/v2/project/sodium"))
        .respond_with(json_fixture(fixture!("project-sodium")))
        .expect(2)
        .mount(&s.server)
        .await;
    let project = s.client.project("sodium", None).await.unwrap();
    assert_eq!(project.id, SODIUM);
    assert_eq!(project.project_type, ProjectType::Mod);
    assert_eq!(project.status, ProjectStatus::Approved);
    assert_eq!(project.environment, vec![Environment::ClientOnly]);
    assert_eq!(project.server_side, SideSupport::Unsupported);
    assert!(project.loaders.contains(&"neoforge".to_owned()));
    assert!(project.game_versions.contains(&"1.21.1".to_owned()));
    assert_eq!(
        project.license.as_ref().unwrap().id,
        "LicenseRef-Polyform-Shield-1.0.0"
    );

    // Do cache, pelo slug e pelo ID, sem pedir de novo.
    s.client.project("sodium", None).await.unwrap();
    s.client.project(SODIUM, None).await.unwrap();
    assert_eq!(s.client.request_count(), 1);

    let summary = s
        .client
        .cache()
        .unwrap()
        .project_summary(SODIUM)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.status, ProjectStatus::Approved);
    assert_eq!(summary.updated, "2026-09-20T21:27:09.277251Z");
    assert_eq!(summary.game_versions, project.game_versions);

    // Passadas 24 h, pede de novo.
    s.now.fetch_add(24 * 60 * 60, Ordering::SeqCst);
    s.client.project("sodium", None).await.unwrap();
    assert_eq!(s.client.request_count(), 2);
}

#[tokio::test]
async fn sem_rede_usa_o_projeto_vencido_do_cache() {
    let s = setup().await;
    Mock::given(path("/v2/project/sodium"))
        .respond_with(json_fixture(fixture!("project-sodium")))
        .up_to_n_times(1)
        .mount(&s.server)
        .await;
    Mock::given(path("/v2/project/sodium"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&s.server)
        .await;
    s.client.project("sodium", None).await.unwrap();
    s.now.fetch_add(48 * 60 * 60, Ordering::SeqCst);
    let project = s.client.project("sodium", None).await.unwrap();
    assert_eq!(project.id, SODIUM);
    // Tentou de novo (3 vezes) antes de cair no cache.
    assert_eq!(s.timer.sleeps().len(), 3);
}

#[tokio::test]
async fn projeto_inexistente() {
    let s = setup().await;
    Mock::given(path("/v2/project/nao-existe"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s.server)
        .await;
    let error = s.client.project("nao-existe", None).await.unwrap_err();
    assert!(matches!(&error, Error::ProjectNotFound { id } if id == "nao-existe"));
    assert_eq!(
        error.code(),
        DomainCode::Domain(ModrinthErrorCode::ProjectNotFound)
    );
    let error = s
        .client
        .project_versions("nao-existe", &VersionFilter::default(), None)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::ProjectNotFound { .. }));
}

#[tokio::test]
async fn projetos_em_lote_omitem_os_inexistentes() {
    let s = setup().await;
    Mock::given(path("/v2/projects"))
        .respond_with(json_fixture(fixture!("projects-3")))
        .expect(1)
        .mount(&s.server)
        .await;
    let ids: Vec<String> = ["AANobbMI", "P7dR8mSH", "gvQqBUqZ", "naoExiste", "AANobbMI"]
        .map(String::from)
        .to_vec();
    let projects = s.client.projects(&ids, None).await.unwrap();
    let got: Vec<&str> = projects.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(got, vec!["AANobbMI", "P7dR8mSH", "gvQqBUqZ"]);
    let requests = s.server.received_requests().await.unwrap();
    let asked: Vec<String> =
        serde_json::from_str(&requests[0].url.query_pairs().next().unwrap().1).unwrap();
    assert_eq!(asked, vec!["AANobbMI", "P7dR8mSH", "gvQqBUqZ", "naoExiste"]);
    // Agora todos vêm do cache (o inexistente é pedido de novo, sozinho).
    Mock::given(path("/v2/projects"))
        .respond_with(json_fixture("[]"))
        .mount(&s.server)
        .await;
    let again = s.client.projects(&ids[..3], None).await.unwrap();
    assert_eq!(again.len(), 3);
    assert_eq!(s.client.request_count(), 1);
}

/// R7 §9 item 4: o Warden lê o `environment` da **versão**, não só o do projeto.
#[tokio::test]
async fn versoes_do_projeto_trazem_o_environment_da_versao() {
    let s = setup().await;
    Mock::given(path(format!("/v2/project/{SODIUM}/version")))
        .and(query_param("loaders", r#"["fabric"]"#))
        .and(query_param("game_versions", r#"["1.21.1"]"#))
        .and(query_param("include_changelog", "false"))
        .respond_with(json_fixture(fixture!(
            "project-sodium-versions-fabric-1.21.1"
        )))
        .mount(&s.server)
        .await;
    let filter = VersionFilter {
        loaders: vec!["fabric".into()],
        game_versions: vec!["1.21.1".into()],
        ..VersionFilter::default()
    };
    let versions = s
        .client
        .project_versions(SODIUM, &filter, None)
        .await
        .unwrap();
    let latest = &versions[0];
    assert_eq!(latest.id, "SMxNOGZ6");
    assert_eq!(latest.version_type, VersionType::Release);
    assert_eq!(latest.environment, Some(Environment::ClientOnly));
    let file = latest.primary_file().unwrap();
    assert_eq!(file.filename, "sodium-fabric-0.8.13+mc1.21.1.jar");
    assert_eq!(file.hashes.sha1, "003c114c85ca88ef3362e018deb6aca0c682d6a1");
    assert_eq!(file.size, 1_574_609);
    assert!(versions.iter().any(|v| v.version_type == VersionType::Beta));
    assert!(
        versions
            .iter()
            .all(|v| v.environment == Some(Environment::ClientOnly))
    );
}

#[tokio::test]
async fn dependencias_incompativeis_do_embeddium() {
    let s = setup().await;
    Mock::given(path("/v2/project/embeddium/version"))
        .respond_with(json_fixture(fixture!("project-embeddium-versions")))
        .mount(&s.server)
        .await;
    let versions = s
        .client
        .project_versions("embeddium", &VersionFilter::default(), None)
        .await
        .unwrap();
    let incompatible: Vec<&str> = versions[0]
        .dependencies_of(DependencyType::Incompatible)
        .filter_map(|d| d.project_id.as_deref())
        .collect();
    assert_eq!(incompatible, vec!["4ZqxOvjD", "S1tndFDa"]);
}

#[tokio::test]
async fn versao_avulsa_e_em_lote_com_cache() {
    let s = setup().await;
    Mock::given(path("/v2/version/SMxNOGZ6"))
        .respond_with(json_fixture(fixture!("version-SMxNOGZ6")))
        .expect(1)
        .mount(&s.server)
        .await;
    Mock::given(path("/v2/versions"))
        .respond_with(json_fixture(fixture!("versions-2")))
        .expect(1)
        .mount(&s.server)
        .await;
    Mock::given(path("/v2/version/naoExiste"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s.server)
        .await;
    let version = s.client.version("SMxNOGZ6", None).await.unwrap();
    assert_eq!(version.project_id, SODIUM);
    // A segunda vem do cache.
    assert_eq!(s.client.version("SMxNOGZ6", None).await.unwrap(), version);
    let versions = s
        .client
        .versions(&["SMxNOGZ6".into(), "QV48eyCs".into()], None)
        .await
        .unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[1].version_number, "mc1.21.1-0.8.13-beta.2-fabric");
    // Só a que faltava foi pedida.
    let requests = s.server.received_requests().await.unwrap();
    let batch = requests
        .iter()
        .find(|r| r.url.path() == "/v2/versions")
        .unwrap();
    let asked: Vec<String> =
        serde_json::from_str(&batch.url.query_pairs().next().unwrap().1).unwrap();
    assert_eq!(asked, vec!["QV48eyCs"]);
    let error = s.client.version("naoExiste", None).await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(ModrinthErrorCode::VersionNotFound)
    );
}

#[tokio::test]
async fn version_files_identifica_pelo_hash() {
    let s = setup().await;
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .and(body_partial_json(json!({"algorithm": "sha1"})))
        .respond_with(json_fixture(fixture!("version_files-sha1")))
        .expect(1)
        .mount(&s.server)
        .await;
    let hashes = vec![
        "003C114C85CA88EF3362E018DEB6ACA0C682D6A1".to_owned(),
        "2172d0e9a5fbdd6791c27769867334382ba6be02".to_owned(),
        "0000000000000000000000000000000000000000".to_owned(),
    ];
    let map = s
        .client
        .version_files(&hashes, HashAlgorithm::Sha1, None)
        .await
        .unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(
        map["003c114c85ca88ef3362e018deb6aca0c682d6a1"].id,
        "SMxNOGZ6"
    );
    assert_eq!(
        map["2172d0e9a5fbdd6791c27769867334382ba6be02"].project_id,
        SODIUM
    );
    let body: Value =
        serde_json::from_slice(&s.server.received_requests().await.unwrap()[0].body).unwrap();
    assert_eq!(
        body["hashes"][0],
        "003c114c85ca88ef3362e018deb6aca0c682d6a1"
    );
    // As versões identificadas ficam no cache (com as notas).
    s.client.version("SMxNOGZ6", None).await.unwrap();
    assert_eq!(s.client.request_count(), 1);
}

/// CA-4 da P1-03 (base de CA-T10-01): verificação de atualizações de 200 hashes usa uma
/// requisição.
#[tokio::test]
async fn p1_03_ca4_atualizacoes_de_200_hashes_numa_requisicao() {
    let s = setup().await;
    Mock::given(method("POST"))
        .and(path("/v2/version_files/update"))
        .and(body_partial_json(json!({
            "algorithm": "sha1",
            "loaders": ["fabric"],
            "game_versions": ["1.21.1"],
            "version_types": ["release"]
        })))
        .respond_with(json_fixture(fixture!("version_files-update-sha1")))
        .expect(1)
        .mount(&s.server)
        .await;
    let mut hashes: Vec<String> = (0..198).map(|i| format!("{i:040x}")).collect();
    hashes.push("2172d0e9a5fbdd6791c27769867334382ba6be02".into());
    hashes.push("d01f2112698d08b052d042c68c1206af790e660a".into());
    let filter = UpdateFilter {
        loaders: vec!["fabric".into()],
        game_versions: vec!["1.21.1".into()],
        version_types: vec![VersionType::Release],
    };
    let map = s
        .client
        .version_files_update(&hashes, HashAlgorithm::Sha1, &filter, None)
        .await
        .unwrap();
    assert_eq!(s.client.request_count(), 1);
    let requests = s.server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["hashes"].as_array().unwrap().len(), 200);
    assert_eq!(map.len(), 2);
    for version in map.values() {
        assert_eq!(version.version_number, "mc1.21.1-0.8.13-fabric");
    }
}

#[tokio::test]
async fn lotes_grandes_sao_divididos() {
    let s = setup().await;
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(json_fixture("{}"))
        .expect(2)
        .mount(&s.server)
        .await;
    let hashes: Vec<String> = (0..1500).map(|i| format!("{i:040x}")).collect();
    let map = s
        .client
        .version_files(&hashes, HashAlgorithm::Sha512, None)
        .await
        .unwrap();
    assert!(map.is_empty());
    let requests = s.server.received_requests().await.unwrap();
    let sizes: Vec<usize> = requests
        .iter()
        .map(|r| {
            serde_json::from_slice::<Value>(&r.body).unwrap()["hashes"]
                .as_array()
                .unwrap()
                .len()
        })
        .collect();
    assert_eq!(sizes, vec![1000, 500]);
}

#[tokio::test]
async fn listas_de_referencia_com_cache() {
    let s = setup().await;
    for (kind, body) in [
        ("loader", fixture!("tag-loader")),
        ("game_version", fixture!("tag-game_version")),
        ("category", fixture!("tag-category")),
    ] {
        Mock::given(path(format!("/v2/tag/{kind}")))
            .respond_with(json_fixture(body))
            .expect(1)
            .mount(&s.server)
            .await;
    }
    for _ in 0..2 {
        let loaders = s.client.loaders(None).await.unwrap();
        assert!(loaders.iter().any(|l| l.name == "neoforge"));
        assert!(loaders.iter().any(|l| l.name == "legacy-fabric"));
        let versions = s.client.game_versions(None).await.unwrap();
        assert_eq!(versions[0].version, "26.4-snapshot-2");
        assert!(versions.iter().any(|v| v.version == "1.7.10" && v.major));
        let categories = s.client.categories(None).await.unwrap();
        assert!(categories.iter().any(|c| c.name == "optimization"));
    }
    assert_eq!(s.client.request_count(), 3);
}

#[tokio::test]
async fn lista_vencida_serve_sem_rede() {
    let s = setup().await;
    Mock::given(path("/v2/tag/loader"))
        .respond_with(json_fixture(fixture!("tag-loader")))
        .up_to_n_times(1)
        .mount(&s.server)
        .await;
    s.client.loaders(None).await.unwrap();
    s.now.fetch_add(25 * 60 * 60, Ordering::SeqCst);
    // Servidor sem a rota agora: 404 não é falta de rede, então não usa o cache.
    let error = s.client.loaders(None).await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(ModrinthErrorCode::NotFound)
    );
    Mock::given(path("/v2/tag/loader"))
        .respond_with(ResponseTemplate::new(429).insert_header("x-ratelimit-reset", "999"))
        .mount(&s.server)
        .await;
    let loaders = s.client.loaders(None).await.unwrap();
    assert!(!loaders.is_empty());
}

#[tokio::test]
async fn limite_de_requisicoes_do_modrinth_e_respeitado() {
    let s = setup().await;
    Mock::given(path("/v2/project/sodium"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("x-ratelimit-limit", "300")
                .insert_header("x-ratelimit-remaining", "0")
                .insert_header("x-ratelimit-reset", "12"),
        )
        .up_to_n_times(1)
        .mount(&s.server)
        .await;
    Mock::given(path("/v2/project/sodium"))
        .respond_with(json_fixture(fixture!("project-sodium")))
        .mount(&s.server)
        .await;
    s.client.project("sodium", None).await.unwrap();
    assert_eq!(s.timer.sleeps(), vec![Duration::from_secs(12)]);
}

#[tokio::test]
async fn resposta_que_nao_e_do_modelo() {
    let s = setup().await;
    Mock::given(path("/v2/project/x"))
        .respond_with(json_fixture(r#"{"id": 5}"#))
        .mount(&s.server)
        .await;
    Mock::given(path("/v2/search"))
        .respond_with(ResponseTemplate::new(400).set_body_string(
            r#"{"error":"invalid_input","description":"Error while validating input: facets"}"#,
        ))
        .mount(&s.server)
        .await;
    let error = s.client.project("x", None).await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(ModrinthErrorCode::InvalidResponse)
    );
    let error = s
        .client
        .search(&SearchQuery::new("x"), None)
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(ModrinthErrorCode::RequestRejected)
    );
    assert!(error.detail().unwrap().contains("invalid_input"));
}

#[tokio::test]
async fn cancelamento() {
    let s = setup().await;
    Mock::given(path("/v2/search"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
        .mount(&s.server)
        .await;
    let cancel = warden_core::CancellationToken::new();
    cancel.cancel();
    let error = s
        .client
        .search(&SearchQuery::new("x"), Some(&cancel))
        .await
        .unwrap_err();
    assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Cancelled));
}

#[tokio::test]
async fn cache_em_arquivo_dentro_das_pastas_de_desenvolvimento() {
    // O cache do app fica em `AppPaths::metadata_db_file()`; nos testes, numa pasta temporária.
    let dir = tempfile::tempdir().unwrap();
    let paths = warden_core::AppPaths::from_dev_root(dir.path()).unwrap();
    let file = paths.metadata_db_file();
    assert!(file.starts_with(dir.path()));
    let server = MockServer::start().await;
    Mock::given(path("/v2/project/sodium"))
        .respond_with(json_fixture(fixture!("project-sodium")))
        .expect(1)
        .mount(&server)
        .await;
    let http = HttpClient::new(HttpConfig::for_version("1")).unwrap();
    {
        let cache = MetadataCache::open(&file).unwrap();
        let client = ModrinthClient::with_base_url(
            http.clone(),
            &format!("{}/v2/", server.uri()),
            Some(cache),
        )
        .unwrap();
        client.project("sodium", None).await.unwrap();
    }
    // Outra abertura (outra sessão do app): vem do arquivo, sem pedir de novo.
    let cache = MetadataCache::open(&file).unwrap();
    let client =
        ModrinthClient::with_base_url(http, &format!("{}/v2/", server.uri()), Some(cache)).unwrap();
    assert_eq!(client.project("sodium", None).await.unwrap().id, SODIUM);
    assert!(file.is_file());
}

/// O servidor recebe o pedido como foi montado (útil para conferir a codificação).
#[tokio::test]
async fn slug_com_caracteres_estranhos_nao_muda_de_pasta() {
    let s = setup().await;
    Mock::given(method("GET"))
        .respond_with(|request: &Request| {
            ResponseTemplate::new(404).set_body_string(request.url.path().to_owned())
        })
        .mount(&s.server)
        .await;
    let error = s.client.project("../search", None).await.unwrap_err();
    assert!(matches!(error, Error::ProjectNotFound { .. }));
    let requests = s.server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/v2/project/..%2Fsearch");
}
