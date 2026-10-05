//! Cliente da CurseForge contra um servidor simulado (`wiremock`), com respostas no formato real
//! (`tests/common/mod.rs`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use common::{ENTITY_CULLING, JEI, KEY, data, file, forbidden, paged, project, setup};
use serde_json::json;
use warden_core::{CoreErrorCode, DomainCode, DomainError};
use warden_curseforge::{
    CurseforgeClient, CurseforgeErrorCode, Distribution, Error, FileRef, FilesQuery, ModLoaderType,
    ProjectClass, RelationType, SearchQuery, SecretString,
};
use warden_http::{HttpClient, HttpConfig};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

async fn requests(server: &wiremock::MockServer) -> usize {
    server.received_requests().await.unwrap_or_default().len()
}

#[tokio::test]
async fn endereco_padrao_e_chave_fora_do_debug() {
    let http = HttpClient::new(HttpConfig::default()).unwrap();
    let client = CurseforgeClient::with_base_url(
        http.clone(),
        "https://api.curseforge.com/v1",
        Some(SecretString::from(KEY.to_owned())),
    )
    .unwrap();
    assert_eq!(client.base_url().as_str(), "https://api.curseforge.com/v1/");
    assert!(client.has_key());
    let text = format!("{client:?}");
    assert!(!text.contains(KEY), "{text}");
    assert!(text.contains("has_key: true"), "{text}");
    if std::env::var_os(warden_curseforge::BASE_URL_ENV).is_none() {
        let client = CurseforgeClient::new(http.clone(), None).unwrap();
        assert_eq!(
            client.base_url().as_str(),
            warden_curseforge::DEFAULT_BASE_URL
        );
        assert!(!client.has_key());
    }
    // Chave em branco é o mesmo que sem chave.
    let client = CurseforgeClient::with_base_url(
        http,
        "https://api.curseforge.com/v1",
        Some(SecretString::from("  ".to_owned())),
    )
    .unwrap();
    assert!(!client.has_key());
}

/// CA-2 da P1-04 (CA-T08-06 e CA-T21-02, parte de backend): sem chave, todo método devolve
/// `CURSEFORGE_KEY_MISSING` sem nenhuma requisição.
#[tokio::test]
async fn p1_04_ca2_sem_chave_nenhuma_requisicao() {
    let s = common::setup_with_key(None).await;
    let c = &s.client;
    let file = warden_curseforge::File {
        id: 1,
        mod_id: 2,
        ..Default::default()
    };
    let results: Vec<Error> = vec![
        c.check_key(None).await.unwrap_err(),
        c.search(&SearchQuery::new("jei"), None).await.unwrap_err(),
        c.project(JEI, None).await.unwrap_err(),
        c.projects(&[JEI], None).await.unwrap_err(),
        c.description(JEI, None).await.unwrap_err(),
        c.files(JEI, &FilesQuery::default(), None)
            .await
            .unwrap_err(),
        c.file(JEI, 1, None).await.unwrap_err(),
        c.files_by_id(&[1], None).await.unwrap_err(),
        c.download_url(JEI, 1, None).await.unwrap_err(),
        c.fingerprints(&[1], None).await.unwrap_err(),
        c.distribution(&[FileRef::new(JEI, 1)], None)
            .await
            .unwrap_err(),
        c.categories(None, None).await.unwrap_err(),
        c.classes(None).await.unwrap_err(),
    ];
    for error in results {
        assert!(matches!(error, Error::KeyMissing), "{error}");
        assert_eq!(
            error.code(),
            DomainCode::Domain(CurseforgeErrorCode::KeyMissing)
        );
    }
    // Sem chave, o download da CDN segue sem o cabeçalho (a CDN ainda não exige; ADR-0051), e
    // um arquivo sem `downloadUrl` continua bloqueado; nada disso chama a API.
    let allowed = warden_curseforge::File {
        download_url: Some("https://edge.forgecdn.net/files/0/1/a.jar".into()),
        ..file.clone()
    };
    let request = c.download_request(&allowed, "a.jar", None).await.unwrap();
    assert!(request.headers.is_empty());
    let error = c.download_request(&file, "x.jar", None).await.unwrap_err();
    assert!(
        matches!(error, Error::DistributionBlocked { page_url: None, .. }),
        "{error}"
    );
    assert_eq!(requests(&s.server).await, 0);
    assert_eq!(c.request_count(), 0);
}

/// CA-2 da P1-04 (CA-T21-02): chave recusada (403 vazio do `CloudFront`, ou 401) vira
/// `CURSEFORGE_KEY_INVALID`.
#[tokio::test]
async fn p1_04_ca2_chave_invalida() {
    let s = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/games/432"))
        .respond_with(forbidden())
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/search"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&s.server)
        .await;
    let error = s.client.check_key(None).await.unwrap_err();
    assert!(
        matches!(error, Error::KeyInvalid { status: Some(403) }),
        "{error}"
    );
    assert_eq!(
        error.code(),
        DomainCode::Domain(CurseforgeErrorCode::KeyInvalid)
    );
    assert!(error.is_key_problem());
    assert!(!error.retryable());
    let error = s
        .client
        .search(&SearchQuery::new("jei"), None)
        .await
        .unwrap_err();
    assert!(
        matches!(error, Error::KeyInvalid { status: Some(401) }),
        "{error}"
    );
    // Sem novas tentativas num 403/401.
    assert_eq!(requests(&s.server).await, 2);
}

#[tokio::test]
async fn chave_com_caractere_invalido_nao_sai() {
    let s = common::setup_with_key(Some("$2a$10$quebra\nde-linha")).await;
    let error = s.client.check_key(None).await.unwrap_err();
    assert!(
        matches!(error, Error::KeyInvalid { status: None }),
        "{error}"
    );
    assert_eq!(requests(&s.server).await, 0);
}

#[tokio::test]
async fn chave_valida() {
    let s = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/games/432"))
        .and(header("x-api-key", KEY))
        .respond_with(data(
            json!({"id": 432, "name": "Minecraft", "slug": "minecraft"}),
        ))
        .expect(1)
        .mount(&s.server)
        .await;
    s.client.check_key(None).await.unwrap();

    // Outro jogo no lugar do Minecraft: resposta que não serve.
    let s = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/games/432"))
        .respond_with(data(json!({"id": 1})))
        .mount(&s.server)
        .await;
    let error = s.client.check_key(None).await.unwrap_err();
    assert!(matches!(error, Error::InvalidResponse(_)), "{error}");
}

/// Busca: parâmetros, chave no cabeçalho e cache em memória de 5 minutos.
#[tokio::test]
async fn busca_com_filtros_e_cache_em_memoria() {
    let s = setup().await;
    let jei_file = file(
        JEI,
        9_058_038,
        "jei-1.20.1-forge-15.62.0.219.jar",
        Some("https://edge.forgecdn.net/files/9058/38/jei-1.20.1-forge-15.62.0.219.jar"),
    );
    Mock::given(method("GET"))
        .and(path("/v1/mods/search"))
        .and(header("x-api-key", KEY))
        .and(query_param("gameId", "432"))
        .and(query_param("classId", "6"))
        .and(query_param("searchFilter", "jei"))
        .and(query_param("gameVersion", "1.20.1"))
        .and(query_param("modLoaderType", "1"))
        .and(query_param("sortField", "2"))
        .and(query_param("sortOrder", "desc"))
        .and(query_param("pageSize", "20"))
        .respond_with(paged(
            vec![project(
                JEI,
                "jei",
                "Just Enough Items (JEI)",
                true,
                vec![jei_file],
            )],
            175,
        ))
        .expect(2)
        .mount(&s.server)
        .await;
    let query = SearchQuery::new("jei")
        .class(ProjectClass::Mod)
        .game_version("1.20.1")
        .loader(ModLoaderType::Forge);
    let results = s.client.search(&query, None).await.unwrap();
    assert_eq!(results.pagination.total_count, 175);
    let jei = &results.mods[0];
    assert_eq!(jei.id, JEI);
    assert_eq!(jei.class_id, Some(ProjectClass::Mod));
    assert_eq!(jei.authors[0].name, "mezz");
    assert!(!jei.is_distribution_blocked());
    let latest = &jei.latest_files[0];
    assert_eq!(latest.loaders(), vec![ModLoaderType::Forge]);
    assert_eq!(latest.minecraft_versions(), vec!["1.20.1"]);
    assert_eq!(
        latest.dependencies[0].relation_type,
        RelationType::RequiredDependency
    );
    assert_eq!(
        latest.sha1().as_deref(),
        Some("130494fd5e5e8eb5ea8008f479a34672e658e653")
    );
    // De novo: vem do cache em memória, e o projeto também.
    let again = s.client.search(&query, None).await.unwrap();
    assert_eq!(again, results);
    assert_eq!(s.client.project(JEI, None).await.unwrap().id, JEI);
    assert_eq!(requests(&s.server).await, 1);
    // Depois de 5 minutos, pede de novo.
    s.timer.advance(warden_curseforge::SEARCH_TTL);
    s.client.search(&query, None).await.unwrap();
    assert_eq!(requests(&s.server).await, 2);
    assert_eq!(s.client.request_count(), 2);
}

#[tokio::test]
async fn busca_fora_dos_limites_nao_sai() {
    let s = setup().await;
    let error = s
        .client
        .search(&SearchQuery::new("a").page(9_990, 50), None)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::InvalidQuery(_)), "{error}");
    assert_eq!(
        error.code(),
        DomainCode::Domain(CurseforgeErrorCode::InvalidQuery)
    );
    let error = s
        .client
        .files(JEI, &FilesQuery::default().page(0, 100), None)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::InvalidQuery(_)), "{error}");
    assert_eq!(requests(&s.server).await, 0);
}

/// Projetos e arquivos em lote: a API omite os inexistentes; a ordem é a do pedido; o cache
/// evita pedir de novo.
#[tokio::test]
async fn lotes_de_projetos_e_arquivos() {
    let s = setup().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .and(header("x-api-key", KEY))
        .and(body_json(json!({"modIds": [ENTITY_CULLING, JEI, 999]})))
        .respond_with(data(json!([
            project(JEI, "jei", "JEI", true, vec![]),
            project(
                ENTITY_CULLING,
                "entityculling",
                "Entity Culling",
                false,
                vec![]
            )
        ])))
        .expect(1)
        .mount(&s.server)
        .await;
    let projects = s
        .client
        .projects(&[ENTITY_CULLING, JEI, 999, JEI], None)
        .await
        .unwrap();
    let ids: Vec<u64> = projects.iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![ENTITY_CULLING, JEI]);
    assert!(projects[0].is_distribution_blocked());
    // Já no cache: nenhuma requisição nova.
    let again = s.client.projects(&[JEI], None).await.unwrap();
    assert_eq!(again[0].id, JEI);

    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .and(body_json(json!({"fileIds": [2, 1]})))
        .respond_with(data(json!([file(
            JEI,
            1,
            "a.jar",
            Some("https://edge.forgecdn.net/files/0/1/a.jar")
        )])))
        .expect(1)
        .mount(&s.server)
        .await;
    let files = s.client.files_by_id(&[2, 1], None).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, 1);
    // `file` usa o cache em memória.
    assert_eq!(
        s.client.file(JEI, 1, None).await.unwrap().file_name,
        "a.jar"
    );
    assert_eq!(requests(&s.server).await, 2);
}

/// Os lotes respondem 404 quando nenhum ID existe (API real, 05/10/2026): o resultado é vazio.
#[tokio::test]
async fn lote_sem_nenhum_encontrado() {
    let s = setup().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(404))
        .expect(3)
        .mount(&s.server)
        .await;
    assert!(
        s.client
            .projects(&[999_999_999], None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(s.client.files_by_id(&[1], None).await.unwrap().is_empty());
    let report = s
        .client
        .distribution(&[FileRef::new(2, 1)], None)
        .await
        .unwrap();
    assert_eq!(report[0].distribution, Distribution::Missing);
    // Lista vazia não chega a pedir (a API responderia 400).
    assert!(s.client.projects(&[], None).await.unwrap().is_empty());
    assert!(s.client.files_by_id(&[], None).await.unwrap().is_empty());
}

#[tokio::test]
async fn lotes_grandes_sao_divididos() {
    let s = setup().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(data(json!([])))
        .expect(3)
        .mount(&s.server)
        .await;
    let ids: Vec<u64> = (1..=(warden_curseforge::ID_BATCH as u64 * 2 + 1)).collect();
    assert!(s.client.projects(&ids, None).await.unwrap().is_empty());
}

#[tokio::test]
async fn projeto_arquivos_e_descricao() {
    let s = setup().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{JEI}")))
        .respond_with(data(project(JEI, "jei", "JEI", true, vec![])))
        .expect(1)
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/1"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{JEI}/description")))
        .respond_with(data(json!("<p>Descrição de teste</p>")))
        .expect(1)
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{JEI}/files")))
        .and(query_param("gameVersion", "1.20.1"))
        .and(query_param("modLoaderType", "1"))
        .and(query_param("pageSize", "50"))
        .respond_with(paged(
            vec![file(JEI, 2, "b.jar", None), file(JEI, 1, "a.jar", None)],
            198,
        ))
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{JEI}/files/77")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s.server)
        .await;

    assert_eq!(s.client.project(JEI, None).await.unwrap().slug, "jei");
    assert_eq!(s.client.project(JEI, None).await.unwrap().slug, "jei");
    let error = s.client.project(1, None).await.unwrap_err();
    assert!(matches!(error, Error::ModNotFound { id: 1 }), "{error}");
    assert_eq!(error.params()["modId"], "1");
    for _ in 0..2 {
        assert_eq!(
            s.client.description(JEI, None).await.unwrap(),
            "<p>Descrição de teste</p>"
        );
    }
    let page = s
        .client
        .files(
            JEI,
            &FilesQuery::default()
                .game_version("1.20.1")
                .loader(ModLoaderType::Forge),
            None,
        )
        .await
        .unwrap();
    assert_eq!(page.pagination.total_count, 198);
    assert_eq!(page.files.len(), 2);
    let error = s.client.file(JEI, 77, None).await.unwrap_err();
    assert!(
        matches!(
            error,
            Error::FileNotFound {
                mod_id: Some(JEI),
                file_id: 77
            }
        ),
        "{error}"
    );
    let error = s
        .client
        .files(1, &FilesQuery::default(), None)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::ModNotFound { id: 1 }), "{error}");
}

/// `download-url`: liberado devolve o endereço; 403 com o arquivo sem `downloadUrl` vira
/// distribuição bloqueada com a página; 403 também no arquivo vira chave recusada.
#[tokio::test]
async fn endereco_de_download_e_bloqueio() {
    let s = setup().await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{JEI}/files/1/download-url")))
        .respond_with(data(json!("https://edge.forgecdn.net/files/0/1/a.jar")))
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/mods/{ENTITY_CULLING}/files/8942326/download-url"
        )))
        .respond_with(forbidden())
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{ENTITY_CULLING}/files/8942326")))
        .respond_with(data(file(
            ENTITY_CULLING,
            8_942_326,
            "entityculling-neoforge-1.11.2-mc26.3.jar",
            None,
        )))
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{ENTITY_CULLING}")))
        .respond_with(data(project(
            ENTITY_CULLING,
            "entityculling",
            "Entity Culling",
            false,
            vec![],
        )))
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/5/files/6/download-url"))
        .respond_with(forbidden())
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/5/files/6"))
        .respond_with(forbidden())
        .mount(&s.server)
        .await;

    let url = s.client.download_url(JEI, 1, None).await.unwrap();
    assert_eq!(url.as_str(), "https://edge.forgecdn.net/files/0/1/a.jar");

    let error = s
        .client
        .download_url(ENTITY_CULLING, 8_942_326, None)
        .await
        .unwrap_err();
    let Error::DistributionBlocked {
        mod_id,
        file_id,
        file_name,
        page_url,
    } = &error
    else {
        panic!("{error}");
    };
    assert_eq!((*mod_id, *file_id), (ENTITY_CULLING, 8_942_326));
    assert_eq!(
        file_name.as_deref(),
        Some("entityculling-neoforge-1.11.2-mc26.3.jar")
    );
    assert_eq!(
        page_url.as_deref(),
        Some("https://www.curseforge.com/minecraft/mc-mods/entityculling/files/8942326")
    );
    assert_eq!(
        error.code(),
        DomainCode::Domain(CurseforgeErrorCode::DistributionBlocked)
    );
    assert_eq!(
        error.params()["file"],
        "entityculling-neoforge-1.11.2-mc26.3.jar"
    );

    let error = s.client.download_url(5, 6, None).await.unwrap_err();
    assert!(
        matches!(error, Error::KeyInvalid { status: Some(403) }),
        "{error}"
    );
}

/// Detecção de distribuição bloqueada em lote: um `POST /mods/files` e um `POST /mods` só com
/// os projetos bloqueados.
#[tokio::test]
async fn distribuicao_em_lote() {
    let s = setup().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .and(body_json(json!({"fileIds": [1, 8_942_326, 3, 4]})))
        .respond_with(data(json!([
            file(
                JEI,
                1,
                "jei.jar",
                Some("https://edge.forgecdn.net/files/0/1/jei.jar")
            ),
            file(ENTITY_CULLING, 8_942_326, "entityculling.jar", None),
            // Arquivo 4 pertence a outro projeto: não serve para o pedido.
            file(77, 4, "outro.jar", None)
        ])))
        .expect(1)
        .mount(&s.server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .and(body_json(json!({"modIds": [ENTITY_CULLING]})))
        .respond_with(data(json!([project(
            ENTITY_CULLING,
            "entityculling",
            "Entity Culling",
            false,
            vec![]
        )])))
        .expect(1)
        .mount(&s.server)
        .await;
    let report = s
        .client
        .distribution(
            &[
                FileRef::new(JEI, 1),
                FileRef::new(ENTITY_CULLING, 8_942_326),
                FileRef::new(JEI, 3),
                FileRef::new(JEI, 4),
            ],
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        report[0].distribution,
        Distribution::Allowed {
            url: "https://edge.forgecdn.net/files/0/1/jei.jar".into()
        }
    );
    assert!(report[1].is_blocked());
    assert_eq!(
        report[1].distribution,
        Distribution::Blocked {
            page_url: Some(
                "https://www.curseforge.com/minecraft/mc-mods/entityculling/files/8942326".into()
            )
        }
    );
    assert_eq!(report[1].file_name.as_deref(), Some("entityculling.jar"));
    assert_eq!(report[2].distribution, Distribution::Missing);
    assert_eq!(report[3].distribution, Distribution::Missing);
    assert_eq!(report[3].file_name, None);
}

#[tokio::test]
async fn distribuicao_sem_bloqueados_nao_pede_projetos() {
    let s = setup().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(data(json!([file(
            JEI,
            1,
            "jei.jar",
            Some("https://edge.forgecdn.net/files/0/1/jei.jar")
        )])))
        .mount(&s.server)
        .await;
    let report = s
        .client
        .distribution(&[FileRef::new(JEI, 1)], None)
        .await
        .unwrap();
    assert!(!report[0].is_blocked());
    assert_eq!(requests(&s.server).await, 1);
}

/// Impressões digitais: as reconhecidas e as que sobraram (a API devolve `null` nelas).
#[tokio::test]
async fn impressoes_digitais() {
    let s = setup().await;
    Mock::given(method("POST"))
        .and(path("/v1/fingerprints/432"))
        .and(body_json(json!({"fingerprints": [427_243_112_u32, 12_345]})))
        .respond_with(data(json!({
            "isCacheBuilt": true,
            "exactMatches": [{"id": 1_694_678, "file": file(1_694_678, 427_243_112, "cosecharia.jar", None), "latestFiles": []}],
            "exactFingerprints": [427_243_112_u32],
            "partialMatches": [],
            "partialMatchFingerprints": {},
            "installedFingerprints": [427_243_112_u32, 12_345],
            "unmatchedFingerprints": null
        })))
        .expect(1)
        .mount(&s.server)
        .await;
    let result = s
        .client
        .fingerprints(&[427_243_112, 12_345, 427_243_112], None)
        .await
        .unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[&427_243_112].id, 1_694_678);
    assert_eq!(result.unmatched, vec![12_345]);
}

#[tokio::test]
async fn categorias_e_classes() {
    let s = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/categories"))
        .and(query_param("gameId", "432"))
        .and(query_param("classesOnly", "true"))
        .respond_with(data(json!([
            {"id": 6, "gameId": 432, "name": "Mods", "slug": "mc-mods", "isClass": true},
            {"id": 6552, "gameId": 432, "name": "Shaders", "slug": "shaders", "isClass": true}
        ])))
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/categories"))
        .and(query_param("classId", "6"))
        .respond_with(data(json!([
            {"id": 421, "gameId": 432, "name": "API and Library", "slug": "library-api", "classId": 6, "parentCategoryId": 6}
        ])))
        .mount(&s.server)
        .await;
    let classes = s.client.classes(None).await.unwrap();
    assert_eq!(classes.len(), 2);
    assert_eq!(classes[1].id, ProjectClass::Shader.id());
    let categories = s
        .client
        .categories(Some(ProjectClass::Mod), None)
        .await
        .unwrap();
    assert_eq!(categories[0].class_id, Some(6));
}

/// Erros da API: 5xx depois das novas tentativas, 429 longo, 400 e JSON que não serve. O corpo
/// da CurseForge nunca entra no erro.
#[tokio::test]
async fn erros_da_api() {
    let s = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/1"))
        .respond_with(ResponseTemplate::new(503))
        .expect(4)
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/2"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "3600"))
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/3"))
        .respond_with(
            ResponseTemplate::new(400).set_body_string("{\"error\":\"texto-da-curseforge\"}"),
        )
        .mount(&s.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/4"))
        .respond_with(data(json!("texto-da-curseforge")))
        .mount(&s.server)
        .await;

    let error = s.client.project(1, None).await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(CurseforgeErrorCode::Unavailable)
    );
    assert!(error.retryable() && error.is_offline());
    // As esperas entre tentativas foram feitas no relógio manual (sem dormir de verdade).
    assert!(s.timer.total_slept() >= Duration::from_millis(500));

    let error = s.client.project(2, None).await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(CurseforgeErrorCode::RateLimited)
    );
    assert_eq!(error.params()["seconds"], "3600");

    let error = s.client.project(3, None).await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(CurseforgeErrorCode::RequestRejected)
    );
    assert!(!error.detail().unwrap().contains("texto-da-curseforge"));

    let error = s.client.project(4, None).await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(CurseforgeErrorCode::InvalidResponse)
    );
    assert!(!error.detail().unwrap().contains("texto-da-curseforge"));
}

#[tokio::test]
async fn sem_conexao_e_cancelamento() {
    // Uma porta sem ninguém escutando (o `wiremock` reaproveita servidores descartados).
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    // Sem novas tentativas: no Windows, cada conexão recusada demora ~2 s.
    let mut config = HttpConfig::for_version("1");
    config.max_retries = 0;
    let http = HttpClient::with_timer(config, std::sync::Arc::new(warden_http::ManualTimer::new()))
        .unwrap();
    let client = CurseforgeClient::with_base_url(
        http,
        &format!("http://127.0.0.1:{port}/v1"),
        Some(SecretString::from(KEY.to_owned())),
    )
    .unwrap();
    let error = client.project(9, None).await.unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Core(CoreErrorCode::NetworkUnavailable),
        "{error}"
    );
    assert!(error.is_offline());

    let s = setup().await;
    Mock::given(method("GET"))
        .respond_with(data(json!({"id": 432})).set_delay(Duration::from_secs(30)))
        .mount(&s.server)
        .await;
    let cancel = warden_core::CancellationToken::new();
    cancel.cancel();
    let error = s.client.check_key(Some(&cancel)).await.unwrap_err();
    assert_eq!(error.code(), DomainCode::Core(CoreErrorCode::Cancelled));
}
