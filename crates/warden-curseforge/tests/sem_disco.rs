//! CA-4 da P1-04: nenhuma resposta da API da CurseForge (JSON, descrições, endereços de
//! download) é escrita em disco.
//!
//! Uma sessão inteira (busca, projetos, descrição, arquivos, `download-url`, impressões
//! digitais, distribuição bloqueada, categorias, erros registrados e o download de um jar para o
//! cache de downloads) roda com a pasta de dados de desenvolvimento numa pasta temporária
//! (ADR-0053) e os registros gravados em `logs/` como o app grava. Depois, todo arquivo da pasta
//! é lido, exceto o cache de downloads (onde fica o jar baixado), procurando marcadores que só
//! existem nas respostas simuladas.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use common::{ENTITY_CULLING, data, file, forbidden, paged, project, setup};
use serde_json::{Value, json};
use warden_core::{AppPaths, NoProgress};
use warden_curseforge::{FileRef, FilesQuery, ModLoaderType, ProjectClass, SearchQuery};
use warden_http::{HttpClient, HttpConfig};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

/// Textos que só existem nas respostas simuladas da API.
const SUMMARY: &str = "MARCADOR-RESUMO-CF";
const DESCRIPTION: &str = "MARCADOR-DESCRICAO-CF";
const AUTHOR: &str = "MARCADOR-AUTOR-CF";
const SLUG: &str = "marcador-slug-cf";
const URL_PATH: &str = "MARCADOR-URL-CF";
/// Campos do JSON da CurseForge que não aparecem em nenhum outro lugar.
const JSON_FIELDS: [&str; 3] = [
    "allowModDistribution",
    "gamePopularityRank",
    "fileFingerprint",
];

fn jar() -> Vec<u8> {
    b"PK\x03\x04 jar de teste da sessao".repeat(100)
}

/// O mod de teste, com os marcadores no resumo, no autor e no slug.
fn marked_project(id: u64, allow: bool, files: Vec<Value>) -> Value {
    let mut value = project(id, SLUG, "Mod de teste", allow, files);
    value["summary"] = json!(SUMMARY);
    value["authors"][0]["name"] = json!(AUTHOR);
    value
}

async fn mount_api(s: &common::Setup) -> String {
    let jar = jar();
    let download_url = format!("{}/files/{URL_PATH}/1/mod.jar", s.server.uri());
    let mut allowed = file(10, 1, "mod.jar", Some(&download_url));
    allowed["fileLength"] = json!(jar.len());
    allowed["hashes"] = json!([{
        "value": warden_packwiz::hash::hash_bytes(warden_packwiz::HashFormat::Sha1, &jar),
        "algo": 1
    }]);
    let blocked = file(ENTITY_CULLING, 2, "bloqueado.jar", None);
    let server = &s.server;
    Mock::given(method("GET"))
        .and(path("/v1/games/432"))
        .respond_with(data(json!({"id": 432, "name": "Minecraft"})))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/search"))
        .respond_with(paged(
            vec![marked_project(10, true, vec![allowed.clone()])],
            1,
        ))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/10"))
        .respond_with(data(marked_project(10, true, vec![allowed.clone()])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{ENTITY_CULLING}")))
        .respond_with(data(marked_project(ENTITY_CULLING, false, vec![])))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(data(json!([marked_project(ENTITY_CULLING, false, vec![])])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/10/description"))
        .respond_with(data(json!(format!("<p>{DESCRIPTION}</p>"))))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/10/files"))
        .respond_with(paged(vec![allowed.clone()], 1))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/10/files/1/download-url"))
        .respond_with(data(json!(download_url)))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/mods/{ENTITY_CULLING}/files/2/download-url"
        )))
        .respond_with(forbidden())
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/mods/{ENTITY_CULLING}/files/2")))
        .respond_with(data(blocked.clone()))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(data(json!([allowed.clone(), blocked])))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/fingerprints/432"))
        .respond_with(data(json!({
            "exactMatches": [{"id": 10, "file": allowed, "latestFiles": []}],
            "unmatchedFingerprints": null
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/categories"))
        .respond_with(data(
            json!([{"id": 6, "name": "Mods", "slug": "mc-mods", "isClass": true}]),
        ))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/404"))
        .respond_with(ResponseTemplate::new(404))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/files/{URL_PATH}/1/mod.jar")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(jar))
        .mount(server)
        .await;
    download_url
}

/// A sessão: tudo o que o app faz com a CurseForge, registrando os erros como as outras crates
/// fazem (`%error`).
async fn session(paths: &AppPaths) -> PathBuf {
    let s = setup().await;
    mount_api(&s).await;
    let c = &s.client;
    c.check_key(None).await.unwrap();
    let query = SearchQuery::new("teste")
        .class(ProjectClass::Mod)
        .game_version("1.20.1")
        .loader(ModLoaderType::Forge);
    let results = c.search(&query, None).await.unwrap();
    assert_eq!(results.mods[0].summary, SUMMARY);
    tracing::info!(resultados = results.mods.len(), "busca na CurseForge");
    c.project(10, None).await.unwrap();
    assert!(c.description(10, None).await.unwrap().contains(DESCRIPTION));
    let files = c.files(10, &FilesQuery::default(), None).await.unwrap();
    c.files_by_id(&[1, 2], None).await.unwrap();
    c.fingerprints(&[1, 99], None).await.unwrap();
    c.categories(Some(ProjectClass::Mod), None).await.unwrap();
    c.classes(None).await.unwrap();
    let report = c
        .distribution(
            &[FileRef::new(10, 1), FileRef::new(ENTITY_CULLING, 2)],
            None,
        )
        .await
        .unwrap();
    assert!(report[1].is_blocked());
    let url = c.download_url(10, 1, None).await.unwrap();
    assert!(url.as_str().contains(URL_PATH));
    for error in [
        c.download_url(ENTITY_CULLING, 2, None).await.unwrap_err(),
        c.project(404, None).await.unwrap_err(),
    ] {
        tracing::warn!(%error, ?error, "erro da CurseForge");
        tracing::error!(error = %warden_core::error_chain(&error), "cadeia do erro");
    }

    // O download vai para o cache de downloads, pelo hash (como a L-03 fará).
    let file = &files.files[0];
    let destination = paths
        .downloads_cache_dir()
        .join(format!("{}.jar", file.sha1().unwrap()));
    let request = c.download_request(file, &destination, None).await.unwrap();
    let http = HttpClient::new(HttpConfig::for_version("1")).unwrap();
    http.download(&request, &NoProgress, None).await.unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), jar());
    destination
}

/// Lê todos os arquivos abaixo de `dir`, exceto os de `skip`.
fn read_all(dir: &Path, skip: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        if current.starts_with(skip) {
            continue;
        }
        for entry in std::fs::read_dir(&current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                pending.push(path);
            } else if !path.starts_with(skip) {
                found.push((path.clone(), std::fs::read(&path).unwrap()));
            }
        }
    }
    found
}

/// Roda a sessão com os registros num arquivo, no filtro dado, e devolve o que ficou no disco
/// fora do cache de downloads.
fn run(filter: &str) -> (tempfile::TempDir, Vec<(PathBuf, Vec<u8>)>, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_dev_root(dir.path()).unwrap();
    paths.create_base_dirs().unwrap();
    let log_file = std::fs::File::create(paths.logs_dir().join("warden.teste.log")).unwrap();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_ansi(false)
        .with_writer(Mutex::new(log_file))
        .finish();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let downloaded =
        tracing::subscriber::with_default(subscriber, || runtime.block_on(session(&paths)));
    let cache = paths.downloads_cache_dir();
    assert!(downloaded.starts_with(&cache));
    let files = read_all(dir.path(), &cache);
    let log = std::fs::read_to_string(paths.logs_dir().join("warden.teste.log")).unwrap();
    assert!(
        log.contains("erro da CurseForge"),
        "registros não gravados: {log}"
    );
    // No cache de downloads, só o jar baixado.
    let cached = read_all(&cache, Path::new("|nada|"));
    assert_eq!(cached.len(), 1, "{cached:?}");
    (dir, files, downloaded)
}

fn assert_absent(files: &[(PathBuf, Vec<u8>)], markers: &[&str]) {
    for (file, bytes) in files {
        let text = String::from_utf8_lossy(bytes);
        for marker in markers {
            assert!(
                !text.contains(marker),
                "{marker} gravado em {}:\n{text}",
                file.display()
            );
        }
    }
}

/// CA-4 da P1-04, no nível de registros padrão do app (`info`): nem JSON, nem resumo, nem
/// descrição, nem autor, nem slug, nem endereço de download no disco.
#[test]
fn p1_04_ca4_nada_da_api_no_disco_nivel_normal() {
    let (_dir, files, _) = run("info");
    assert!(!files.is_empty(), "a varredura não leu nada");
    let mut markers = vec![SUMMARY, DESCRIPTION, AUTHOR, SLUG, URL_PATH];
    markers.extend(JSON_FIELDS);
    assert_absent(&files, &markers);
}

/// CA-4 da P1-04, no nível "detalhado" do app (`info,warden=debug`): nada do conteúdo das
/// respostas (JSON, resumo, descrição, autor, slug). O endereço de download fica de fora desta
/// conferência: neste nível a `warden-http` registra o endereço de cada pedido (achado
/// registrado no relatório da P1-04).
#[test]
fn p1_04_ca4_nada_da_api_no_disco_nivel_detalhado() {
    let (_dir, files, _) = run("info,warden=debug");
    let mut markers = vec![SUMMARY, DESCRIPTION, AUTHOR, SLUG];
    markers.extend(JSON_FIELDS);
    assert_absent(&files, &markers);
}
