//! Aceite da CurseForge em "Adicionar" (P1-10; SPEC T08): fonte da busca combinada, estados sem
//! chave e chave recusada, o mesmo mod nas duas fontes, dependências e gravação do `.pw.toml`
//! da CurseForge, e o link de arquivo lido pelo packwiz numa cópia do pack.
//!
//! O Modrinth simulado devolve respostas reais gravadas (`warden-modrinth`); a CurseForge
//! simulada é montada aqui, no formato da API (os termos da CurseForge proíbem guardar
//! respostas dela no repositório). Com o sidecar real do packwiz, o pack resultante passa na
//! conformidade (`check_conformance`: um `refresh` não mudaria nada).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use warden_core::CancellationToken;
use warden_curseforge::{CurseforgeClient, SecretString};
use warden_http::{HttpClient, HttpConfig};
use warden_modrinth::{MetadataCache, ModrinthClient};
use warden_packwiz::{Loader, PackIndex, PackManifest};
use warden_packwiz_cli::{Packwiz, RunContext, check_conformance};
use warden_project::add::curseforge::CurseforgeAdd;
use warden_project::add::modrinth::ModrinthAdd;
use warden_project::add::plan::{AddPlan, AddPlanRequest, NodeRole, add_plan};
use warden_project::add::{
    AddApplyRequest, AddChoice, AddSource, AddSources, ApplyItem, ChannelPolicy, SideNote,
    add_apply,
};
use warden_project::search::curseforge::CurseforgeSearch;
use warden_project::search::modrinth::ModrinthSearch;
use warden_project::search::{
    ActiveSource, InstalledKeys, PackTarget, ProjectKind, SearchPage, SearchRequest, SearchSort,
    SourceFilter, SourceId, SourceWarningReason, project_versions, search,
};
use warden_project::side::SideChoice;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const MODRINTH_FIXTURES: &str = "../warden-modrinth/tests/fixtures/http";
const KEY: &str = "$2a$10$chaveFalsaDoWardenParaTestes0123456789abcdefghijkl";

const MODRINTH_SODIUM: &str = "AANobbMI";
/// Sodium na CurseForge (mesmo autor e mesmo slug do Modrinth).
const CF_SODIUM: u64 = 394_468;
/// SHA-1 do arquivo estável do Sodium para Fabric 1.21.1 no Modrinth (`SMxNOGZ6`).
const SODIUM_SHA1: &str = "003c114c85ca88ef3362e018deb6aca0c682d6a1";
/// Mod que só existe na CurseForge, e a biblioteca obrigatória dele.
const CF_ONLY: u64 = 900_001;
const CF_LIB: u64 = 900_002;
/// Mod com a distribuição por terceiros bloqueada.
const CF_BLOCKED: u64 = 900_003;

fn modrinth_fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(MODRINTH_FIXTURES)
        .join(format!("2026-10-04-{name}.json"));
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn binary() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "packwiz-x86_64-pc-windows-msvc.exe"
    } else {
        "packwiz-x86_64-unknown-linux-gnu"
    };
    let path = std::env::var_os("WARDEN_PACKWIZ_BIN").map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/src-tauri/binaries")
                .join(name)
        },
        PathBuf::from,
    );
    if path.is_file() {
        Some(path)
    } else {
        assert_ne!(
            std::env::var_os("WARDEN_REQUIRE_EXTERNALS").as_deref(),
            Some(std::ffi::OsStr::new("1")),
            "sidecar packwiz ausente"
        );
        None
    }
}

fn packwiz(root: &Path) -> Option<Packwiz> {
    Some(Packwiz::new(
        binary()?,
        root.join(".packwiz-cache"),
        root.join(".packwiz-config.toml"),
    ))
}

fn write(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

/// Pack vazio do loader dado.
fn pack(root: &Path, minecraft: &str, loader: Loader) {
    let mut manifest = PackManifest::new("Teste", minecraft);
    manifest.set_loader_version(loader, "1.0.0");
    write(root, "pack.toml", &manifest.to_toml_string());
    let index = PackIndex {
        hash_format: "sha256".into(),
        files: Vec::new(),
    };
    write(root, "index.toml", &index.to_toml_string());
}

async fn refresh(root: &Path) {
    if let Some(cli) = packwiz(root) {
        cli.refresh(root, false, RunContext::new(&CancellationToken::new()))
            .await
            .unwrap();
    }
}

async fn assert_conforming(root: &Path) {
    let Some(cli) = packwiz(root) else { return };
    let staging = tempfile::tempdir().unwrap();
    let report = check_conformance(
        &cli,
        root,
        &staging.path().join("copia"),
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    assert!(report.is_conforming(), "{:#?}", report.differences);
}

fn http() -> HttpClient {
    let mut config = HttpConfig::for_version("1");
    config.max_retries = 0;
    HttpClient::new(config).unwrap()
}

// --- A CurseForge simulada --------------------------------------------------------------

/// Versão do Minecraft e loader de um arquivo simulado: nome no `gameVersions` e o tipo da
/// CurseForge (1 Forge, 4 Fabric).
#[derive(Clone, Copy)]
struct On {
    minecraft: &'static str,
    loader: &'static str,
    loader_type: u32,
}

const FABRIC_1_21_1: On = On {
    minecraft: "1.21.1",
    loader: "Fabric",
    loader_type: 4,
};
const FORGE_1_20_1: On = On {
    minecraft: "1.20.1",
    loader: "Forge",
    loader_type: 1,
};
const FORGE_1_7_10: On = On {
    minecraft: "1.7.10",
    loader: "Forge",
    loader_type: 1,
};
/// Como os arquivos antigos da CurseForge vêm de verdade (o NEI 1.7.10, por exemplo): só a
/// versão do Minecraft em `gameVersions`, sem nenhuma tag de loader.
const SEM_TAG_1_7_10: On = On {
    minecraft: "1.7.10",
    loader: "",
    loader_type: 1,
};
const FABRIC_1_7_10: On = On {
    minecraft: "1.7.10",
    loader: "Fabric",
    loader_type: 4,
};

fn cf_file(mod_id: u64, file_id: u64, name: &str, sha1: &str, deps: &[(u64, u32)]) -> Value {
    cf_file_on(FABRIC_1_21_1, mod_id, file_id, name, sha1, deps)
}

fn cf_file_on(
    on: On,
    mod_id: u64,
    file_id: u64,
    name: &str,
    sha1: &str,
    deps: &[(u64, u32)],
) -> Value {
    let (tags, sortable) = if on.loader.is_empty() {
        (
            json!([on.minecraft]),
            json!([{"gameVersionName": on.minecraft, "gameVersion": on.minecraft, "gameVersionTypeId": 5}]),
        )
    } else {
        (
            json!(["Client", on.minecraft, on.loader, "Server"]),
            json!([
                {"gameVersionName": on.minecraft, "gameVersion": on.minecraft, "gameVersionTypeId": 75125},
                {"gameVersionName": on.loader, "gameVersion": "", "gameVersionTypeId": 68441}
            ]),
        )
    };
    json!({
        "id": file_id, "gameId": 432, "modId": mod_id, "isAvailable": true,
        "displayName": name, "fileName": name, "releaseType": 1, "fileStatus": 4,
        "hashes": [{"value": sha1, "algo": 1}],
        "fileDate": "2026-09-19T11:47:57.347Z", "fileLength": 1000, "downloadCount": 7,
        "downloadUrl": format!("https://edge.forgecdn.net/files/{file_id}/{name}"),
        "gameVersions": tags,
        "sortableGameVersions": sortable,
        "dependencies": deps.iter()
            .map(|(id, relation)| json!({"modId": id, "relationType": relation}))
            .collect::<Vec<_>>(),
        "isServerPack": false, "fileFingerprint": file_id
    })
}

fn cf_project(id: u64, slug: &str, name: &str, author: &str, blocked: bool) -> Value {
    cf_project_on(FABRIC_1_21_1, id, slug, name, author, blocked)
}

fn cf_project_on(on: On, id: u64, slug: &str, name: &str, author: &str, blocked: bool) -> Value {
    json!({
        "id": id, "gameId": 432, "name": name, "slug": slug,
        "links": {"websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}")},
        "summary": format!("Resumo de {name}"), "status": 4, "downloadCount": 1_000,
        "classId": 6, "primaryCategoryId": 423, "authors": [{"id": 1, "name": author}],
        "logo": {"thumbnailUrl": "https://media.forgecdn.net/x.png", "url": "https://media.forgecdn.net/x.png"},
        "latestFilesIndexes": [{
            "gameVersion": on.minecraft, "fileId": id * 10, "filename": "a.jar",
            "releaseType": 1, "modLoader": on.loader_type
        }],
        "dateCreated": "2020-01-01T00:00:00Z", "dateModified": "2026-10-01T00:00:00Z",
        "allowModDistribution": !blocked, "isAvailable": true
    })
}

fn cf_projects() -> Vec<Value> {
    vec![
        cf_project(CF_SODIUM, "sodium", "Sodium", "jellysquid3", false),
        cf_project(
            CF_ONLY,
            "mod-so-da-curseforge",
            "Mod Só da CurseForge",
            "Autora",
            false,
        ),
        cf_project(
            CF_LIB,
            "biblioteca-curseforge",
            "Biblioteca da CurseForge",
            "Autora",
            false,
        ),
        cf_project(CF_BLOCKED, "mod-bloqueado", "Mod Bloqueado", "Outro", true),
    ]
}

fn cf_files() -> Vec<Value> {
    vec![
        cf_file(
            CF_SODIUM,
            CF_SODIUM * 10,
            "sodium-fabric-0.6.0+mc1.21.1.jar",
            SODIUM_SHA1,
            &[],
        ),
        cf_file(
            CF_ONLY,
            CF_ONLY * 10,
            "mod-so-da-curseforge-1.0.jar",
            "1111111111111111111111111111111111111111",
            &[(CF_LIB, 3), (CF_BLOCKED, 2)],
        ),
        cf_file(
            CF_LIB,
            CF_LIB * 10,
            "biblioteca-1.0.jar",
            "2222222222222222222222222222222222222222",
            &[],
        ),
        cf_file(
            CF_BLOCKED,
            CF_BLOCKED * 10,
            "mod-bloqueado-1.0.jar",
            "3333333333333333333333333333333333333333",
            &[],
        ),
    ]
}

fn ids_in_body(request: &Request, field: &str) -> Vec<u64> {
    let body: Value = serde_json::from_slice(&request.body).unwrap();
    body[field]
        .as_array()
        .map(|ids| ids.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default()
}

/// Status da CurseForge simulada.
#[derive(Clone, Copy, PartialEq)]
enum Cf {
    Ok,
    ServerError,
    KeyRefused,
}

async fn curseforge_server(behavior: Cf) -> MockServer {
    let server = MockServer::start().await;
    match behavior {
        Cf::ServerError => {
            Mock::given(path_regex("^/v1/.*"))
                .respond_with(ResponseTemplate::new(500))
                .mount(&server)
                .await;
            return server;
        }
        Cf::KeyRefused => {
            Mock::given(path_regex("^/v1/.*"))
                .respond_with(ResponseTemplate::new(403))
                .mount(&server)
                .await;
            return server;
        }
        Cf::Ok => {}
    }
    curseforge_mock(&server, cf_projects(), cf_files()).await;
    server
}

/// A CurseForge simulada com estes projetos e arquivos: a busca (filtrada pelo slug, quando há
/// um, como a API faz), os projetos e os arquivos, em lote e um a um (os do packwiz).
async fn curseforge_mock(server: &MockServer, projects: Vec<Value>, files: Vec<Value>) {
    let all = projects.clone();
    Mock::given(method("GET"))
        .and(path("/v1/mods/search"))
        .respond_with(move |request: &Request| {
            let slug = request
                .url
                .query_pairs()
                .find(|(key, _)| key == "slug")
                .map(|(_, value)| value.into_owned());
            let found: Vec<&Value> = all
                .iter()
                .filter(|project| slug.as_ref().is_none_or(|slug| project["slug"] == slug.as_str()))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({
                "data": found,
                "pagination": {"index": 0, "pageSize": 20, "resultCount": found.len(), "totalCount": found.len()}
            }))
        })
        .mount(server)
        .await;
    let all = projects.clone();
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(move |request: &Request| {
            let ids = ids_in_body(request, "modIds");
            let found: Vec<&Value> = all
                .iter()
                .filter(|project| ids.contains(&project["id"].as_u64().unwrap()))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({ "data": found }))
        })
        .mount(server)
        .await;
    let all = projects;
    Mock::given(method("GET"))
        .and(path_regex(r"^/v1/mods/\d+$"))
        .respond_with(move |request: &Request| {
            let id: u64 = request
                .url
                .path()
                .split('/')
                .nth(3)
                .unwrap()
                .parse()
                .unwrap();
            all.iter().find(|project| project["id"] == id).map_or_else(
                || ResponseTemplate::new(404),
                |project| ResponseTemplate::new(200).set_body_json(json!({ "data": project })),
            )
        })
        .mount(server)
        .await;
    let all = files.clone();
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(move |request: &Request| {
            let ids = ids_in_body(request, "fileIds");
            let found: Vec<&Value> = all
                .iter()
                .filter(|file| ids.contains(&file["id"].as_u64().unwrap()))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({ "data": found }))
        })
        .mount(server)
        .await;
    let all = files.clone();
    Mock::given(method("GET"))
        .and(path_regex(r"^/v1/mods/\d+/files$"))
        .respond_with(move |request: &Request| {
            let id: u64 = request.url.path().split('/').nth(3).unwrap().parse().unwrap();
            let found: Vec<&Value> = all.iter().filter(|file| file["modId"] == id).collect();
            ResponseTemplate::new(200).set_body_json(json!({
                "data": found,
                "pagination": {"index": 0, "pageSize": 50, "resultCount": found.len(), "totalCount": found.len()}
            }))
        })
        .mount(server)
        .await;
    let all = files;
    Mock::given(method("GET"))
        .and(path_regex(r"^/v1/mods/\d+/files/\d+$"))
        .respond_with(move |request: &Request| {
            let mut parts = request.url.path().split('/');
            let project: u64 = parts.nth(3).unwrap().parse().unwrap();
            let file: u64 = parts.nth(1).unwrap().parse().unwrap();
            all.iter()
                .find(|found| found["modId"] == project && found["id"] == file)
                .map_or_else(
                    || ResponseTemplate::new(404),
                    |found| ResponseTemplate::new(200).set_body_json(json!({ "data": found })),
                )
        })
        .mount(server)
        .await;
}

fn curseforge_client(server: &MockServer, key: Option<&str>) -> CurseforgeClient {
    CurseforgeClient::with_base_url(
        http(),
        &format!("{}/v1", server.uri()),
        key.map(|key| SecretString::from(key.to_owned())),
    )
    .unwrap()
}

// --- O Modrinth simulado ----------------------------------------------------------------

/// Um Modrinth que não acha nada (os mods de Forge dos testes só existem na CurseForge).
async fn modrinth_empty() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hits": [], "offset": 0, "limit": 20, "total_hits": 0
        })))
        .mount(&server)
        .await;
    server
}

async fn modrinth_server(down: bool) -> MockServer {
    let server = MockServer::start().await;
    if down {
        Mock::given(path_regex("^/v2/.*"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        return server;
    }
    Mock::given(method("GET"))
        .and(path("/v2/search"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(modrinth_fixture("search-sodium-fabric-1.21.1")),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(modrinth_fixture("projects-3")))
        .mount(&server)
        .await;
    let versions = modrinth_fixture("project-sodium-versions-fabric-1.21.1");
    Mock::given(method("GET"))
        .and(path(format!("/v2/project/{MODRINTH_SODIUM}/version")))
        .respond_with(ResponseTemplate::new(200).set_body_json(&versions))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&versions))
        .mount(&server)
        .await;
    server
}

fn modrinth_client(server: &MockServer) -> ModrinthClient {
    ModrinthClient::with_base_url(
        http(),
        &format!("{}/v2", server.uri()),
        Some(MetadataCache::in_memory(None).unwrap()),
    )
    .unwrap()
}

// --- Auxiliares de busca e plano --------------------------------------------------------

fn fabric_target(root: &Path) -> PackTarget {
    pack(root, "1.21.1", Loader::Fabric);
    PackTarget::read(root).unwrap()
}

async fn run_search(sources: &[ActiveSource], target: &PackTarget) -> SearchPage {
    search_for(sources, target, "sodium").await
}

async fn search_for(sources: &[ActiveSource], target: &PackTarget, query: &str) -> SearchPage {
    search(
        sources,
        target,
        &InstalledKeys::new(),
        &SearchRequest {
            query: query.into(),
            kind: ProjectKind::Mod,
            sort: SearchSort::Relevance,
            source: SourceFilter::All,
            environment: None,
            include_incompatible: false,
            cursor: None,
        },
        &CancellationToken::new(),
    )
    .await
    .unwrap()
}

fn both(modrinth: &MockServer, curseforge: &MockServer, key: Option<&str>) -> Vec<ActiveSource> {
    vec![
        ActiveSource::Ready(Arc::new(ModrinthSearch::new(modrinth_client(modrinth)))),
        ActiveSource::Ready(Arc::new(CurseforgeSearch::new(curseforge_client(
            curseforge, key,
        )))),
    ]
}

fn add_sources(modrinth: &MockServer, curseforge: &MockServer) -> AddSources {
    AddSources::new()
        .with(Arc::new(ModrinthAdd::new(modrinth_client(modrinth))))
        .with(Arc::new(CurseforgeAdd::new(curseforge_client(
            curseforge,
            Some(KEY),
        ))))
}

fn choice(source: SourceId, project: impl std::fmt::Display) -> AddChoice {
    AddChoice {
        source,
        project_id: format!("{project}"),
        version_id: None,
    }
}

async fn plan_of(sources: &AddSources, root: &Path, items: Vec<AddChoice>) -> AddPlan {
    add_plan(
        root,
        sources,
        ChannelPolicy::default(),
        &AddPlanRequest { items },
        &CancellationToken::new(),
    )
    .await
    .unwrap()
}

fn confirm_default(plan: &AddPlan) -> AddApplyRequest {
    AddApplyRequest {
        items: plan
            .nodes
            .iter()
            .filter(|node| node.role != NodeRole::Optional)
            .map(|node| ApplyItem {
                source: node.source,
                project_id: node.project_id.clone(),
                version_id: node.version_id.clone(),
                side: None,
                replaces: None,
            })
            .collect(),
    }
}

async fn curseforge_requests(server: &MockServer) -> usize {
    server.received_requests().await.unwrap_or_default().len()
}

// --- Busca combinada --------------------------------------------------------------------

/// CA-T08-06: sem chave, só o Modrinth, com o aviso, e nenhuma requisição à CurseForge.
#[tokio::test]
async fn ca_t08_06_sem_chave_so_modrinth_e_nenhuma_requisicao_a_curseforge() {
    let modrinth = modrinth_server(false).await;
    let curseforge = curseforge_server(Cf::Ok).await;
    let temp = tempfile::tempdir().unwrap();
    let target = fabric_target(temp.path());

    // O app deixa a fonte de fora (sem chave): nem o cliente chega a existir.
    let off = [
        ActiveSource::Ready(Arc::new(ModrinthSearch::new(modrinth_client(&modrinth)))),
        ActiveSource::Off(SourceId::Curseforge, SourceWarningReason::KeyMissing),
    ];
    let page = run_search(&off, &target).await;
    assert!(!page.items.is_empty());
    assert!(page.items.iter().all(|item| item.sources.len() == 1));
    assert_eq!(page.sources, [SourceId::Modrinth]);
    assert_eq!(page.warnings.len(), 1);
    assert_eq!(page.warnings[0].source, SourceId::Curseforge);
    assert_eq!(page.warnings[0].reason, SourceWarningReason::KeyMissing);
    assert_eq!(curseforge_requests(&curseforge).await, 0);

    // Mesmo que a fonte estivesse ligada, o cliente sem chave falha antes de qualquer rede.
    let sources = both(&modrinth, &curseforge, None);
    let page = run_search(&sources, &target).await;
    assert_eq!(page.warnings[0].reason, SourceWarningReason::KeyMissing);
    assert_eq!(curseforge_requests(&curseforge).await, 0);
}

/// Chave recusada: o aviso certo, os resultados do Modrinth.
#[tokio::test]
async fn chave_recusada_vira_aviso_de_chave() {
    let modrinth = modrinth_server(false).await;
    let curseforge = curseforge_server(Cf::KeyRefused).await;
    let temp = tempfile::tempdir().unwrap();
    let target = fabric_target(temp.path());
    let page = run_search(&both(&modrinth, &curseforge, Some(KEY)), &target).await;
    assert!(!page.items.is_empty());
    assert_eq!(page.warnings.len(), 1);
    assert_eq!(page.warnings[0].reason, SourceWarningReason::KeyRejected);
    let detail = page.warnings[0].detail.clone().unwrap_or_default();
    assert!(!detail.contains(KEY), "a chave nunca vai para o aviso");
}

/// CA-T08-09: uma fonte com erro não derruba a outra.
#[tokio::test]
async fn ca_t08_09_uma_fonte_com_erro_mostra_a_outra_com_aviso() {
    let temp = tempfile::tempdir().unwrap();
    let target = fabric_target(temp.path());

    let modrinth = modrinth_server(false).await;
    let curseforge = curseforge_server(Cf::ServerError).await;
    let page = run_search(&both(&modrinth, &curseforge, Some(KEY)), &target).await;
    assert!(!page.items.is_empty());
    assert!(page.items.iter().all(|item| item.sources.len() == 1));
    assert_eq!(page.warnings.len(), 1);
    assert_eq!(page.warnings[0].source, SourceId::Curseforge);
    assert_eq!(page.warnings[0].reason, SourceWarningReason::Unavailable);

    let modrinth = modrinth_server(true).await;
    let curseforge = curseforge_server(Cf::Ok).await;
    let page = run_search(&both(&modrinth, &curseforge, Some(KEY)), &target).await;
    assert!(!page.items.is_empty());
    assert!(
        page.items
            .iter()
            .all(|item| item.sources[0].source == SourceId::Curseforge)
    );
    assert_eq!(page.warnings.len(), 1);
    assert_eq!(page.warnings[0].source, SourceId::Modrinth);
    assert_eq!(page.warnings[0].reason, SourceWarningReason::Unavailable);
}

/// CA-T08-08: o mesmo mod nas duas fontes aparece uma vez; o que só a CurseForge tem vem
/// marcado; a distribuição bloqueada vira "Download manual necessário".
#[tokio::test]
async fn ca_t08_08_mesmo_mod_nas_duas_fontes_aparece_uma_vez() {
    let modrinth = modrinth_server(false).await;
    let curseforge = curseforge_server(Cf::Ok).await;
    let temp = tempfile::tempdir().unwrap();
    let target = fabric_target(temp.path());
    let page = run_search(&both(&modrinth, &curseforge, Some(KEY)), &target).await;

    let sodium: Vec<_> = page
        .items
        .iter()
        .filter(|item| item.title == "Sodium")
        .collect();
    assert_eq!(sodium.len(), 1, "{:#?}", page.items);
    let sources: Vec<SourceId> = sodium[0].sources.iter().map(|s| s.source).collect();
    assert_eq!(sources, [SourceId::Modrinth, SourceId::Curseforge]);
    assert_eq!(sodium[0].key, format!("modrinth:{MODRINTH_SODIUM}"));

    let only = page
        .items
        .iter()
        .find(|item| item.key == format!("curseforge:{CF_ONLY}"))
        .unwrap();
    assert_eq!(only.sources.len(), 1);
    assert_eq!(only.sources[0].source, SourceId::Curseforge);
    assert!(!only.manual_download);

    let blocked = page
        .items
        .iter()
        .find(|item| item.key == format!("curseforge:{CF_BLOCKED}"))
        .unwrap();
    assert!(blocked.manual_download);
    assert!(blocked.compatible);

    // A busca pediu os filtros travados do pack à CurseForge: classe, versão e loader.
    let requests = curseforge.received_requests().await.unwrap();
    let query: Vec<(String, String)> = requests
        .iter()
        .find(|request| request.url.path() == "/v1/mods/search")
        .unwrap()
        .url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let param = |name: &str| {
        query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(param("classId"), Some("6"));
    assert_eq!(param("gameVersion"), Some("1.21.1"));
    assert_eq!(param("modLoaderType"), Some("4"));
    assert!(
        requests
            .iter()
            .all(|request| request.headers.get("x-api-key").is_some_and(|k| k == KEY)),
        "toda requisição leva a chave do usuário"
    );
}

/// O seletor de versão traz o SHA-1 de cada fonte: o mesmo arquivo confere nas duas (SPEC T08).
#[tokio::test]
async fn versoes_das_duas_fontes_trazem_o_mesmo_sha1() {
    let modrinth = modrinth_server(false).await;
    let curseforge = curseforge_server(Cf::Ok).await;
    let temp = tempfile::tempdir().unwrap();
    let target = fabric_target(temp.path());
    let sources = add_sources(&modrinth, &curseforge);
    let cancel = CancellationToken::new();
    let from_modrinth = project_versions(
        &sources,
        &target,
        ChannelPolicy::default(),
        SourceId::Modrinth,
        MODRINTH_SODIUM,
        &cancel,
    )
    .await
    .unwrap();
    let from_curseforge = project_versions(
        &sources,
        &target,
        ChannelPolicy::default(),
        SourceId::Curseforge,
        &CF_SODIUM.to_string(),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(from_curseforge.versions.len(), 1);
    let default = from_curseforge.default_id.clone().unwrap();
    assert_eq!(default, (CF_SODIUM * 10).to_string());
    let modrinth_default = from_modrinth
        .versions
        .iter()
        .find(|version| Some(&version.id) == from_modrinth.default_id.as_ref())
        .unwrap();
    assert_eq!(modrinth_default.sha1.as_deref(), Some(SODIUM_SHA1));
    assert_eq!(
        from_curseforge.versions[0].sha1.as_deref(),
        Some(SODIUM_SHA1)
    );
}

// --- Plano e gravação -------------------------------------------------------------------

/// CA-T08-08: escolher a fonte CurseForge grava `mode = "metadata:curseforge"`; a Modrinth
/// grava `[update.modrinth]`.
#[tokio::test]
async fn ca_t08_08_cada_fonte_grava_o_seu_metafile() {
    let modrinth = modrinth_server(false).await;
    let curseforge = curseforge_server(Cf::Ok).await;
    let sources = add_sources(&modrinth, &curseforge);

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fabric_target(root);
    refresh(root).await;
    let plan = plan_of(
        &sources,
        root,
        vec![choice(SourceId::Curseforge, CF_SODIUM)],
    )
    .await;
    assert_eq!(plan.nodes.len(), 1);
    let node = &plan.nodes[0];
    assert_eq!(node.source, SourceId::Curseforge);
    assert_eq!(node.version_id, (CF_SODIUM * 10).to_string());
    assert_eq!(node.side, SideChoice::Both);
    assert_eq!(node.side_note, Some(SideNote::Unknown));
    if let Some(cli) = packwiz(root) {
        let result = add_apply(
            root,
            &sources,
            &confirm_default(&plan),
            &cli,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(result.added[0].path, "mods/sodium.pw.toml");
        let text = fs::read_to_string(root.join("mods/sodium.pw.toml")).unwrap();
        assert!(text.contains("mode = \"metadata:curseforge\""), "{text}");
        assert!(text.contains("hash-format = \"sha1\""), "{text}");
        assert!(
            text.contains(&format!("hash = \"{SODIUM_SHA1}\"")),
            "{text}"
        );
        assert!(
            text.contains(&format!("project-id = {CF_SODIUM}")),
            "{text}"
        );
        assert!(text.contains("side = \"both\""), "{text}");
        assert!(!text.contains("[update.modrinth]"), "{text}");
        assert!(!text.contains("https://"), "nenhum endereço da CDN: {text}");
        assert_conforming(root).await;
    }

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fabric_target(root);
    refresh(root).await;
    let plan = plan_of(
        &sources,
        root,
        vec![choice(SourceId::Modrinth, MODRINTH_SODIUM)],
    )
    .await;
    if let Some(cli) = packwiz(root) {
        add_apply(
            root,
            &sources,
            &confirm_default(&plan),
            &cli,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        let text = fs::read_to_string(root.join("mods/sodium.pw.toml")).unwrap();
        assert!(text.contains("[update.modrinth]"), "{text}");
        assert!(!text.contains("metadata:curseforge"), "{text}");
        assert_conforming(root).await;
    }
}

/// O item da CurseForge que o usuário clicou entra, com a dependência obrigatória dele (pela
/// CurseForge) e a opcional de fora; uma gravação, um `refresh`.
#[tokio::test]
async fn mod_so_da_curseforge_entra_com_a_dependencia_obrigatoria() {
    let modrinth = modrinth_server(false).await;
    let curseforge = curseforge_server(Cf::Ok).await;
    let sources = add_sources(&modrinth, &curseforge);
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fabric_target(root);
    refresh(root).await;

    let plan = plan_of(&sources, root, vec![choice(SourceId::Curseforge, CF_ONLY)]).await;
    let role = |id: u64| {
        plan.nodes
            .iter()
            .find(|node| node.project_id == id.to_string())
            .map(|node| node.role)
    };
    assert_eq!(role(CF_ONLY), Some(NodeRole::Chosen));
    assert_eq!(role(CF_LIB), Some(NodeRole::Required));
    assert_eq!(role(CF_BLOCKED), Some(NodeRole::Optional));
    let lib = plan
        .nodes
        .iter()
        .find(|node| node.project_id == CF_LIB.to_string())
        .unwrap();
    assert_eq!(lib.required_by, [format!("curseforge:{CF_ONLY}")]);

    let Some(cli) = packwiz(root) else { return };
    let result = add_apply(
        root,
        &sources,
        &confirm_default(&plan),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let paths: Vec<&str> = result.added.iter().map(|item| item.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "mods/mod-so-da-curseforge.pw.toml",
            "mods/biblioteca-curseforge.pw.toml"
        ]
    );
    let text = fs::read_to_string(root.join("mods/mod-so-da-curseforge.pw.toml")).unwrap();
    assert!(text.contains("name = \"Mod Só da CurseForge\""), "{text}");
    assert!(text.contains("mode = \"metadata:curseforge\""), "{text}");
    let index = fs::read_to_string(root.join("index.toml")).unwrap();
    assert!(
        index.contains("mods/biblioteca-curseforge.pw.toml"),
        "{index}"
    );
    assert_conforming(root).await;

    // Pedir de novo o que já entrou pela mesma fonte não grava nada.
    let again = add_apply(
        root,
        &sources,
        &confirm_default(&plan),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(again.added.is_empty());
    assert_eq!(again.skipped.len(), 2);
}

/// NEI, a biblioteca dele e dois resultados parecidos, todos de Forge 1.7.10 e só na CurseForge.
const CF_NEI: u64 = 910_001;
const CF_CCC: u64 = 910_002;
const CF_NEI_ADDONS: u64 = 910_003;
const CF_REI: u64 = 910_004;

fn nei_projects() -> Vec<Value> {
    let on = FORGE_1_7_10;
    vec![
        cf_project_on(
            on,
            CF_NEI_ADDONS,
            "nei-addons",
            "NEI Addons",
            "Outro",
            false,
        ),
        cf_project_on(
            on,
            CF_REI,
            "roughly-enough-items",
            "Roughly Enough Items",
            "Outra",
            false,
        ),
        cf_project_on(
            on,
            CF_NEI,
            "notenoughitems",
            "Not Enough Items",
            "chicken_bones",
            false,
        ),
        cf_project_on(
            on,
            CF_CCC,
            "codechickencore",
            "CodeChickenCore",
            "chicken_bones",
            false,
        ),
    ]
}

fn nei_files() -> Vec<Value> {
    let on = FORGE_1_7_10;
    let sha1 = |digit: char| digit.to_string().repeat(40);
    vec![
        // O NEI e o CodeChickenCore vêm como na API real: sem a tag "Forge".
        cf_file_on(
            SEM_TAG_1_7_10,
            CF_NEI,
            CF_NEI * 10,
            "NotEnoughItems-1.7.10-1.0.5.120-universal.jar",
            &sha1('a'),
            &[(CF_CCC, 3)],
        ),
        cf_file_on(
            SEM_TAG_1_7_10,
            CF_CCC,
            CF_CCC * 10,
            "CodeChickenCore-1.7.10-1.0.7.47-universal.jar",
            &sha1('b'),
            &[],
        ),
        cf_file_on(
            on,
            CF_NEI_ADDONS,
            CF_NEI_ADDONS * 10,
            "neiaddons-1.7.10-1.12.jar",
            &sha1('c'),
            &[(CF_NEI, 3)],
        ),
        cf_file_on(on, CF_REI, CF_REI * 10, "rei-1.7.10.jar", &sha1('d'), &[]),
    ]
}

/// CA-T08-02: num pack Forge 1.7.10, buscar "not enough items" e escolher o NEI (resultado da
/// CurseForge) adiciona o NEI, e não outro resultado; a biblioteca obrigatória entra junto.
#[tokio::test]
async fn ca_t08_02_nei_da_curseforge_num_pack_forge_1_7_10() {
    let modrinth = modrinth_empty().await;
    let curseforge = MockServer::start().await;
    curseforge_mock(&curseforge, nei_projects(), nei_files()).await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(root, "1.7.10", Loader::Forge);
    refresh(root).await;
    let target = PackTarget::read(root).unwrap();

    let page = search_for(
        &both(&modrinth, &curseforge, Some(KEY)),
        &target,
        "not enough items",
    )
    .await;
    let titles: Vec<&str> = page.items.iter().map(|item| item.title.as_str()).collect();
    assert!(titles.contains(&"Roughly Enough Items"), "{titles:?}");
    let nei = page
        .items
        .iter()
        .find(|item| item.title == "Not Enough Items")
        .unwrap();
    assert_eq!(nei.key, format!("curseforge:{CF_NEI}"));
    assert_eq!(nei.sources.len(), 1);
    assert_eq!(nei.sources[0].source, SourceId::Curseforge);
    assert!(nei.compatible);
    // A busca pediu à CurseForge os filtros do pack: Minecraft 1.7.10 e Forge.
    let requests = curseforge.received_requests().await.unwrap();
    let query = requests
        .iter()
        .find(|request| request.url.path() == "/v1/mods/search")
        .unwrap()
        .url
        .query()
        .unwrap_or_default()
        .to_owned();
    assert!(
        query.contains("gameVersion=1.7.10") && query.contains("modLoaderType=1"),
        "{query}"
    );

    // Escolher o NEI: o plano é o NEI e a biblioteca dele, nada mais.
    let sources = add_sources(&modrinth, &curseforge);
    let plan = plan_of(
        &sources,
        root,
        vec![choice(SourceId::Curseforge, &nei.sources[0].project_id)],
    )
    .await;
    let nodes: Vec<(String, NodeRole)> = plan
        .nodes
        .iter()
        .map(|node| (node.project_id.clone(), node.role))
        .collect();
    assert_eq!(
        nodes,
        [
            (CF_NEI.to_string(), NodeRole::Chosen),
            (CF_CCC.to_string(), NodeRole::Required)
        ]
    );
    assert_eq!(plan.nodes[0].version_id, (CF_NEI * 10).to_string());

    let Some(cli) = packwiz(root) else { return };
    let result = add_apply(
        root,
        &sources,
        &confirm_default(&plan),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let paths: Vec<&str> = result.added.iter().map(|item| item.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "mods/notenoughitems.pw.toml",
            "mods/codechickencore.pw.toml"
        ]
    );
    let text = fs::read_to_string(root.join("mods/notenoughitems.pw.toml")).unwrap();
    assert!(text.contains("name = \"Not Enough Items\""), "{text}");
    assert!(
        text.contains("filename = \"NotEnoughItems-1.7.10-1.0.5.120-universal.jar\""),
        "{text}"
    );
    assert!(text.contains(&format!("project-id = {CF_NEI}")), "{text}");
    assert!(
        text.contains(&format!("file-id = {}", CF_NEI * 10)),
        "{text}"
    );
    let index = fs::read_to_string(root.join("index.toml")).unwrap();
    assert!(!index.contains("roughly-enough-items"), "{index}");
    assert!(!index.contains("nei-addons"), "{index}");
    assert_conforming(root).await;
}

/// Num pack Forge 1.7.10, o arquivo da CurseForge sem nenhuma tag de loader (como os antigos
/// vêm de verdade) é compatível e é gravado; o arquivo marcado só "Fabric" continua de fora,
/// sem versão para o pack.
#[tokio::test]
async fn arquivo_sem_tag_de_loader_entra_num_pack_forge_1_7_10_e_o_so_fabric_nao() {
    let modrinth = modrinth_empty().await;
    let curseforge = MockServer::start().await;
    let sha1 = |digit: char| digit.to_string().repeat(40);
    curseforge_mock(
        &curseforge,
        vec![
            cf_project_on(
                SEM_TAG_1_7_10,
                CF_NEI,
                "notenoughitems",
                "Not Enough Items",
                "chicken_bones",
                false,
            ),
            cf_project_on(
                FABRIC_1_7_10,
                CF_REI,
                "so-fabric",
                "Só Fabric",
                "Outra",
                false,
            ),
        ],
        vec![
            cf_file_on(
                SEM_TAG_1_7_10,
                CF_NEI,
                CF_NEI * 10,
                "NotEnoughItems-1.7.10-1.0.5.120-universal.jar",
                &sha1('a'),
                &[],
            ),
            cf_file_on(
                FABRIC_1_7_10,
                CF_REI,
                CF_REI * 10,
                "so-fabric-1.7.10.jar",
                &sha1('d'),
                &[],
            ),
        ],
    )
    .await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    pack(root, "1.7.10", Loader::Forge);
    refresh(root).await;

    let sources = add_sources(&modrinth, &curseforge);
    let plan = plan_of(
        &sources,
        root,
        vec![
            choice(SourceId::Curseforge, CF_NEI),
            choice(SourceId::Curseforge, CF_REI),
        ],
    )
    .await;
    let nodes: Vec<(String, String)> = plan
        .nodes
        .iter()
        .map(|node| (node.project_id.clone(), node.version_id.clone()))
        .collect();
    assert_eq!(nodes, [(CF_NEI.to_string(), (CF_NEI * 10).to_string())]);
    let missing: Vec<&str> = plan.missing.iter().map(|item| item.key.as_str()).collect();
    assert_eq!(missing, [format!("curseforge:{CF_REI}")]);

    let Some(cli) = packwiz(root) else { return };
    let result = add_apply(
        root,
        &sources,
        &confirm_default(&plan),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let paths: Vec<&str> = result.added.iter().map(|item| item.path.as_str()).collect();
    assert_eq!(paths, ["mods/notenoughitems.pw.toml"]);
    let text = fs::read_to_string(root.join("mods/notenoughitems.pw.toml")).unwrap();
    assert!(
        text.contains(&format!("file-id = {}", CF_NEI * 10)),
        "{text}"
    );
    assert!(!root.join("mods/so-fabric.pw.toml").exists());
    assert_conforming(root).await;
}

/// Sem a CurseForge ligada (sem chave), pedir um item dela é `SEARCH_SOURCE_UNAVAILABLE` e nada
/// é gravado.
#[tokio::test]
async fn item_da_curseforge_sem_a_fonte_ligada_e_recusado() {
    let modrinth = modrinth_server(false).await;
    let sources = AddSources::new().with(Arc::new(ModrinthAdd::new(modrinth_client(&modrinth))));
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fabric_target(root);
    let error = add_plan(
        root,
        &sources,
        ChannelPolicy::default(),
        &AddPlanRequest {
            items: vec![choice(SourceId::Curseforge, CF_ONLY)],
        },
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.code,
        warden_project::ProjectErrorCode::SearchSourceUnavailable
    );
    assert_eq!(
        error.params.get("source").map(String::as_str),
        Some("CurseForge")
    );
}

/// O `.pw.toml` da CurseForge é o que o packwiz escreveria (formato do `curseforge add`).
#[tokio::test]
async fn metafile_da_curseforge_tem_o_formato_do_packwiz() {
    let curseforge = curseforge_server(Cf::Ok).await;
    let source = CurseforgeAdd::new(curseforge_client(&curseforge, Some(KEY)));
    let cancel = CancellationToken::new();
    let project = source
        .projects(&[CF_ONLY.to_string()], &cancel)
        .await
        .unwrap()
        .remove(0);
    let version = source
        .versions(&[(CF_ONLY * 10).to_string()], &cancel)
        .await
        .unwrap()
        .remove(0);
    let text = source
        .metafile(&project, &version, SideChoice::Both)
        .unwrap()
        .to_toml_string();
    assert_eq!(
        text,
        format!(
            "name = \"Mod Só da CurseForge\"\n\
             filename = \"mod-so-da-curseforge-1.0.jar\"\n\
             side = \"both\"\n\
             \n\
             [download]\n\
             hash-format = \"sha1\"\n\
             hash = \"1111111111111111111111111111111111111111\"\n\
             mode = \"metadata:curseforge\"\n\
             \n\
             [update]\n\
             [update.curseforge]\n\
             file-id = {}\n\
             project-id = {CF_ONLY}\n",
            CF_ONLY * 10
        )
    );
}

// --- Link da CurseForge -----------------------------------------------------------------

mod link {
    use warden_core::{DomainCode, DomainError as _};
    use warden_packwiz_cli::PackwizCliErrorCode;
    use warden_project::add::link_curseforge::{
        CurseforgeLink, LinkError, import_file_link, import_project_file, parse_link,
        resolve_project,
    };

    use super::*;

    /// JEI na CurseForge: duas versões de Forge 1.20.1 (a mais nova é a padrão).
    const CF_JEI: u64 = 238_222;
    const JEI_NEW: u64 = 4_712_345;
    const JEI_OLD: u64 = 4_600_001;
    const JEI_NEW_SHA1: &str = "abcdef0123456789abcdef0123456789abcdef01";

    fn jei_project() -> Value {
        let mut project = cf_project_on(
            FORGE_1_20_1,
            CF_JEI,
            "jei",
            "Just Enough Items (JEI)",
            "mezz",
            false,
        );
        project["latestFilesIndexes"][0]["fileId"] = json!(JEI_NEW);
        project
    }

    fn jei_files() -> Vec<Value> {
        let mut old = cf_file_on(
            FORGE_1_20_1,
            CF_JEI,
            JEI_OLD,
            "jei-1.20.1-forge-15.2.0.27.jar",
            "0123456789012345678901234567890123456789",
            &[],
        );
        old["fileDate"] = json!("2026-01-10T10:00:00.000Z");
        vec![
            cf_file_on(
                FORGE_1_20_1,
                CF_JEI,
                JEI_NEW,
                "jei-1.20.1-forge-15.20.0.106.jar",
                JEI_NEW_SHA1,
                &[],
            ),
            old,
        ]
    }

    async fn jei_server() -> MockServer {
        let server = MockServer::start().await;
        curseforge_mock(&server, vec![jei_project()], jei_files()).await;
        server
    }

    /// O packwiz real (sidecar), com a CurseForge apontada para o servidor simulado.
    fn real_packwiz(dir: &Path, server: &MockServer) -> Option<Packwiz> {
        Some(
            Packwiz::new(
                binary()?,
                dir.join("packwiz-cache"),
                dir.join("packwiz-config.toml"),
            )
            .with_curseforge_api(format!("{}/v1", server.uri())),
        )
    }

    fn key() -> SecretString {
        SecretString::from(KEY.to_owned())
    }

    /// Pack Forge 1.20.1 vazio e já com o `refresh` do packwiz.
    async fn forge_pack(root: &Path, cli: &Packwiz) {
        pack(root, "1.20.1", Loader::Forge);
        cli.refresh(root, false, RunContext::new(&CancellationToken::new()))
            .await
            .unwrap();
    }

    fn jei_sources(curseforge: &MockServer, modrinth: &MockServer) -> AddSources {
        add_sources(modrinth, curseforge)
    }

    /// Grava no pack, pelo caminho normal, o que o plano pede.
    async fn apply(root: &Path, sources: &AddSources, items: Vec<AddChoice>, cli: &Packwiz) {
        let plan = plan_of(sources, root, items).await;
        add_apply(
            root,
            sources,
            &confirm_default(&plan),
            cli,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    }

    fn requested(requests: &[Request], wanted: &str) -> bool {
        requests.iter().any(|request| request.url.path() == wanted)
    }

    /// CA-T08-03: colar o link do projeto do JEI num pack Forge 1.20.1 abre a pré-visualização
    /// (projeto achado pelo slug, versão padrão a mais nova); a versão escolhida é lida pelo
    /// packwiz real (`--addon-id`/`--file-id`) numa cópia do pack e entra pelo caminho normal. O
    /// resultado é idêntico ao de adicionar o JEI pela busca da CurseForge.
    #[tokio::test]
    async fn ca_t08_03_link_de_projeto_passa_pelo_packwiz_e_da_o_mesmo_que_a_busca() {
        let curseforge = jei_server().await;
        let modrinth = modrinth_empty().await;
        let temp = tempfile::tempdir().unwrap();
        let Some(cli) = real_packwiz(temp.path(), &curseforge) else {
            return;
        };
        let sources = jei_sources(&curseforge, &modrinth);
        let cancel = CancellationToken::new();

        // Pelo link.
        let by_link = temp.path().join("pelo-link");
        forge_pack(&by_link, &cli).await;
        let before = fs::read_to_string(by_link.join("index.toml")).unwrap();
        let link = parse_link("https://www.curseforge.com/minecraft/mc-mods/jei").unwrap();
        let target = resolve_project(&curseforge_client(&curseforge, Some(KEY)), &link, &cancel)
            .await
            .unwrap();
        assert_eq!(target.project_id, CF_JEI.to_string());
        assert_eq!(target.file_id, None);
        let versions = project_versions(
            &sources,
            &PackTarget::read(&by_link).unwrap(),
            ChannelPolicy::default(),
            SourceId::Curseforge,
            &target.project_id,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(versions.versions.len(), 2);
        let chosen = versions.default_id.unwrap();
        assert_eq!(chosen, JEI_NEW.to_string());
        curseforge.reset().await;
        curseforge_mock(&curseforge, vec![jei_project()], jei_files()).await;

        let staging = temp.path().join("copia");
        let imported = import_project_file(
            &by_link,
            &staging,
            &cli,
            Some(&key()),
            &target.project_id,
            &chosen,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(imported.project_id, CF_JEI.to_string());
        assert_eq!(imported.file_id, JEI_NEW.to_string());
        assert_eq!(imported.kind, ProjectKind::Mod);
        // Foi o packwiz que leu: ele pede o projeto e o arquivo um a um (o Warden pede em lote).
        let requests = curseforge.received_requests().await.unwrap();
        assert!(
            requested(&requests, &format!("/v1/mods/{CF_JEI}")),
            "{requests:?}"
        );
        assert!(requested(
            &requests,
            &format!("/v1/mods/{CF_JEI}/files/{JEI_NEW}")
        ));
        // A cópia foi apagada e o pack de verdade não foi tocado.
        assert!(!staging.join("pack.toml").exists());
        assert!(!by_link.join("mods").exists());
        assert_eq!(
            fs::read_to_string(by_link.join("index.toml")).unwrap(),
            before
        );
        apply(&by_link, &sources, vec![imported.choice()], &cli).await;
        let from_link = fs::read_to_string(by_link.join("mods/jei.pw.toml")).unwrap();

        // Pela busca da CurseForge.
        let by_search = temp.path().join("pela-busca");
        forge_pack(&by_search, &cli).await;
        let page = search_for(
            &both(&modrinth, &curseforge, Some(KEY)),
            &PackTarget::read(&by_search).unwrap(),
            "jei",
        )
        .await;
        let jei = page
            .items
            .iter()
            .find(|item| item.key == format!("curseforge:{CF_JEI}"))
            .unwrap();
        apply(
            &by_search,
            &sources,
            vec![choice(SourceId::Curseforge, &jei.sources[0].project_id)],
            &cli,
        )
        .await;
        let from_search = fs::read_to_string(by_search.join("mods/jei.pw.toml")).unwrap();

        assert_eq!(from_link, from_search);
        // E é o `.pw.toml` que o próprio packwiz escreveu na cópia (o lado padrão é o mesmo).
        assert_eq!(from_link, imported.metafile.to_toml_string());
        assert!(
            from_link.contains(&format!("file-id = {JEI_NEW}")),
            "{from_link}"
        );
        assert!(
            from_link.contains(&format!("hash = \"{JEI_NEW_SHA1}\"")),
            "{from_link}"
        );
        assert_conforming(&by_link).await;
        assert_conforming(&by_search).await;
    }

    /// O packwiz recebe a versão escolhida na pré-visualização, mesmo que não seja a mais nova:
    /// ele nunca escolhe a versão sozinho.
    #[tokio::test]
    async fn versao_escolhida_no_link_de_projeto_e_a_que_entra() {
        let curseforge = jei_server().await;
        let modrinth = modrinth_empty().await;
        let temp = tempfile::tempdir().unwrap();
        let Some(cli) = real_packwiz(temp.path(), &curseforge) else {
            return;
        };
        let root = temp.path().join("pack");
        forge_pack(&root, &cli).await;
        let imported = import_project_file(
            &root,
            &temp.path().join("copia"),
            &cli,
            Some(&key()),
            &CF_JEI.to_string(),
            &JEI_OLD.to_string(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(imported.file_id, JEI_OLD.to_string());
        assert_eq!(imported.metafile.filename, "jei-1.20.1-forge-15.2.0.27.jar");
        apply(
            &root,
            &jei_sources(&curseforge, &modrinth),
            vec![imported.choice()],
            &cli,
        )
        .await;
        let text = fs::read_to_string(root.join("mods/jei.pw.toml")).unwrap();
        assert!(text.contains(&format!("file-id = {JEI_OLD}")), "{text}");
        assert_conforming(&root).await;
    }

    /// O link de arquivo também é lido pelo packwiz real numa cópia, e dá o mesmo `.pw.toml`.
    #[tokio::test]
    async fn link_de_arquivo_e_lido_pelo_packwiz_em_staging() {
        let curseforge = jei_server().await;
        let modrinth = modrinth_empty().await;
        let temp = tempfile::tempdir().unwrap();
        let Some(cli) = real_packwiz(temp.path(), &curseforge) else {
            return;
        };
        let root = temp.path().join("pack");
        forge_pack(&root, &cli).await;
        let before = fs::read_to_string(root.join("index.toml")).unwrap();
        let staging = temp.path().join("copia");
        let imported = import_file_link(
            &root,
            &staging,
            &cli,
            Some(&key()),
            &format!("https://www.curseforge.com/minecraft/mc-mods/jei/files/{JEI_NEW}"),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(imported.project_id, CF_JEI.to_string());
        assert_eq!(imported.file_id, JEI_NEW.to_string());
        assert_eq!(imported.target().title, "Just Enough Items (JEI)");
        assert_eq!(imported.choice().version_id, Some(JEI_NEW.to_string()));
        assert!(!staging.join("pack.toml").exists());
        assert!(!root.join("mods").exists());
        assert_eq!(fs::read_to_string(root.join("index.toml")).unwrap(), before);

        apply(
            &root,
            &jei_sources(&curseforge, &modrinth),
            vec![imported.choice()],
            &cli,
        )
        .await;
        let text = fs::read_to_string(root.join("mods/jei.pw.toml")).unwrap();
        assert_eq!(text, imported.metafile.to_toml_string());
        assert_conforming(&root).await;
    }

    /// O packwiz não acha o arquivo: o erro dele chega, e nada fica no pack.
    #[tokio::test]
    async fn falha_do_packwiz_no_link_chega_como_erro_do_packwiz() {
        let curseforge = jei_server().await;
        let temp = tempfile::tempdir().unwrap();
        let Some(cli) = real_packwiz(temp.path(), &curseforge) else {
            return;
        };
        let root = temp.path().join("pack");
        forge_pack(&root, &cli).await;
        let error = import_project_file(
            &root,
            &temp.path().join("copia"),
            &cli,
            Some(&key()),
            &CF_JEI.to_string(),
            "1",
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, LinkError::Packwiz(_)), "{error:?}");
        assert!(!root.join("mods").exists());
    }

    /// Sem chave, o packwiz falha antes de sair da máquina: nenhuma requisição à CurseForge.
    #[tokio::test]
    async fn sem_chave_o_link_nao_faz_requisicao() {
        let curseforge = jei_server().await;
        let temp = tempfile::tempdir().unwrap();
        let Some(cli) = real_packwiz(temp.path(), &curseforge) else {
            return;
        };
        let root = temp.path().join("pack");
        forge_pack(&root, &cli).await;
        let error = import_project_file(
            &root,
            &temp.path().join("copia"),
            &cli,
            None,
            &CF_JEI.to_string(),
            &JEI_NEW.to_string(),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        let LinkError::Packwiz(error) = error else {
            panic!("{error:?}")
        };
        assert_eq!(
            error.code(),
            DomainCode::Domain(PackwizCliErrorCode::CurseForgeKeyMissing)
        );
        assert_eq!(curseforge_requests(&curseforge).await, 0);
    }

    /// IDs que não são da CurseForge são recusados antes do packwiz.
    #[tokio::test]
    async fn ids_invalidos_sao_recusados_antes_do_packwiz() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("pack");
        pack(&root, "1.20.1", Loader::Forge);
        let cli = Packwiz::new(
            temp.path().join("nao-existe"),
            temp.path().join("cache"),
            temp.path().join("config.toml"),
        );
        let error = import_project_file(
            &root,
            &temp.path().join("copia"),
            &cli,
            Some(&key()),
            "jei",
            "1",
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        let LinkError::Project(error) = error else {
            panic!("{error:?}")
        };
        assert_eq!(error.code, warden_project::ProjectErrorCode::InvalidInput);
    }

    /// O link de projeto é achado na API pelo slug; a versão é escolhida na pré-visualização.
    #[tokio::test]
    async fn link_de_projeto_e_achado_pelo_slug() {
        let curseforge = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/mods/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    cf_project(1, "sodium-extra", "Sodium Extra", "x", true),
                    cf_project(CF_SODIUM, "sodium", "Sodium", "jellysquid3", false)
                ],
                "pagination": {"index": 0, "pageSize": 5, "resultCount": 2, "totalCount": 2}
            })))
            .mount(&curseforge)
            .await;
        let client = curseforge_client(&curseforge, Some(KEY));
        let link = parse_link("https://www.curseforge.com/minecraft/mc-mods/sodium").unwrap();
        assert!(matches!(link, CurseforgeLink::Project { .. }));
        let target = resolve_project(&client, &link, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(target.project_id, CF_SODIUM.to_string());
        assert_eq!(target.title, "Sodium");
        assert_eq!(target.file_id, None);
        assert_eq!(target.choice().version_id, None);
        assert!(!target.manual_download);
        let query: Vec<_> = curseforge
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|request| request.url.query().unwrap_or_default().to_owned())
            .collect();
        assert!(
            query[0].contains("slug=sodium") && query[0].contains("classId=6"),
            "{query:?}"
        );

        // O autor bloqueou downloads por outros apps: a pré-visualização do link avisa.
        let blocked = parse_link("https://www.curseforge.com/minecraft/mc-mods/sodium-extra");
        let target = resolve_project(&client, &blocked.unwrap(), &CancellationToken::new())
            .await
            .unwrap();
        assert!(target.manual_download);

        // Um modpack não entra pela página Adicionar.
        let modpack = parse_link("https://www.curseforge.com/minecraft/modpacks/algum").unwrap();
        let error = resolve_project(&client, &modpack, &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error.code, warden_project::ProjectErrorCode::InvalidInput);
    }
}

// --- Rede (`cargo xtask test-network`) ----------------------------------------------------

/// As APIs reais, com a chave do `.env` (`CURSEFORGE_API_KEY`) e o sidecar real do packwiz.
mod rede {
    use warden_project::add::link_curseforge::{import_project_file, parse_link, resolve_project};

    use super::*;

    fn key() -> SecretString {
        SecretString::from(std::env::var("CURSEFORGE_API_KEY").expect("CURSEFORGE_API_KEY ausente"))
    }

    fn real_sources() -> (Vec<ActiveSource>, AddSources, CurseforgeClient) {
        let modrinth =
            ModrinthClient::new(http(), Some(MetadataCache::in_memory(None).unwrap())).unwrap();
        let curseforge = CurseforgeClient::new(http(), Some(key())).unwrap();
        let search = vec![
            ActiveSource::Ready(Arc::new(ModrinthSearch::new(modrinth.clone()))),
            ActiveSource::Ready(Arc::new(CurseforgeSearch::new(curseforge.clone()))),
        ];
        let add = AddSources::new()
            .with(Arc::new(ModrinthAdd::new(modrinth)))
            .with(Arc::new(CurseforgeAdd::new(curseforge.clone())));
        (search, add, curseforge)
    }

    fn cli(dir: &Path) -> Packwiz {
        Packwiz::new(
            binary().expect("sidecar packwiz ausente"),
            dir.join("packwiz-cache"),
            dir.join("packwiz-config.toml"),
        )
    }

    async fn forge_pack(root: &Path, minecraft: &str, cli: &Packwiz) {
        pack(root, minecraft, Loader::Forge);
        cli.refresh(root, false, RunContext::new(&CancellationToken::new()))
            .await
            .unwrap();
    }

    async fn apply(root: &Path, sources: &AddSources, items: Vec<AddChoice>, cli: &Packwiz) {
        let plan = plan_of(sources, root, items).await;
        add_apply(
            root,
            sources,
            &confirm_default(&plan),
            cli,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    }

    /// CA-T08-02 com as APIs reais: "not enough items" num pack Forge 1.7.10 acha o NEI na
    /// CurseForge, e escolhê-lo grava o NEI (`notenoughitems`), não outro resultado.
    #[tokio::test]
    #[ignore = "rede"]
    async fn rede_ca_t08_02_nei_num_pack_forge_1_7_10() {
        let (search_sources, sources, _) = real_sources();
        let temp = tempfile::tempdir().unwrap();
        let cli = cli(temp.path());
        let root = temp.path().join("pack");
        forge_pack(&root, "1.7.10", &cli).await;
        let page = search_for(
            &search_sources,
            &PackTarget::read(&root).unwrap(),
            "not enough items",
        )
        .await;
        assert!(page.warnings.is_empty(), "{:?}", page.warnings);
        let nei = page
            .items
            .iter()
            .find_map(|item| {
                item.sources.iter().find(|source| {
                    source.source == SourceId::Curseforge && source.slug == "notenoughitems"
                })
            })
            .unwrap_or_else(|| panic!("NEI não veio da CurseForge: {:#?}", page.items))
            .clone();
        apply(
            &root,
            &sources,
            vec![choice(SourceId::Curseforge, &nei.project_id)],
            &cli,
        )
        .await;
        let text = fs::read_to_string(root.join("mods/notenoughitems.pw.toml")).unwrap();
        assert!(
            text.contains(&format!("project-id = {}", nei.project_id)),
            "{text}"
        );
        assert!(text.contains("mode = \"metadata:curseforge\""), "{text}");
        assert_conforming(&root).await;
    }

    /// CA-T08-03 com as APIs reais: o link do projeto do JEI num pack Forge 1.20.1, com a versão
    /// padrão lida pelo packwiz (`--addon-id`/`--file-id`), dá o mesmo `.pw.toml` que a busca.
    #[tokio::test]
    #[ignore = "rede"]
    async fn rede_ca_t08_03_link_do_jei_igual_a_busca() {
        let (search_sources, sources, client) = real_sources();
        let temp = tempfile::tempdir().unwrap();
        let cli = cli(temp.path());
        let cancel = CancellationToken::new();

        let by_link = temp.path().join("pelo-link");
        forge_pack(&by_link, "1.20.1", &cli).await;
        let link = parse_link("https://www.curseforge.com/minecraft/mc-mods/jei").unwrap();
        let target = resolve_project(&client, &link, &cancel).await.unwrap();
        assert_eq!(target.project_id, "238222");
        let versions = project_versions(
            &sources,
            &PackTarget::read(&by_link).unwrap(),
            ChannelPolicy::default(),
            SourceId::Curseforge,
            &target.project_id,
            &cancel,
        )
        .await
        .unwrap();
        let chosen = versions.default_id.unwrap();
        let imported = import_project_file(
            &by_link,
            &temp.path().join("copia"),
            &cli,
            Some(&key()),
            &target.project_id,
            &chosen,
            &cancel,
        )
        .await
        .unwrap();
        apply(&by_link, &sources, vec![imported.choice()], &cli).await;
        let from_link = fs::read_to_string(by_link.join("mods/jei.pw.toml")).unwrap();

        let by_search = temp.path().join("pela-busca");
        forge_pack(&by_search, "1.20.1", &cli).await;
        let page = search_for(
            &search_sources,
            &PackTarget::read(&by_search).unwrap(),
            "jei",
        )
        .await;
        let found = page
            .items
            .iter()
            .flat_map(|item| &item.sources)
            .find(|source| source.source == SourceId::Curseforge && source.project_id == "238222")
            .unwrap_or_else(|| panic!("JEI não veio da CurseForge: {:#?}", page.items))
            .clone();
        apply(
            &by_search,
            &sources,
            vec![choice(SourceId::Curseforge, &found.project_id)],
            &cli,
        )
        .await;
        let from_search = fs::read_to_string(by_search.join("mods/jei.pw.toml")).unwrap();
        assert_eq!(from_link, from_search);
        assert_eq!(from_link, imported.metafile.to_toml_string());
        assert_conforming(&by_link).await;
    }
}
