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

fn cf_file(mod_id: u64, file_id: u64, name: &str, sha1: &str, deps: &[(u64, u32)]) -> Value {
    json!({
        "id": file_id, "gameId": 432, "modId": mod_id, "isAvailable": true,
        "displayName": name, "fileName": name, "releaseType": 1, "fileStatus": 4,
        "hashes": [{"value": sha1, "algo": 1}],
        "fileDate": "2026-09-19T11:47:57.347Z", "fileLength": 1000, "downloadCount": 7,
        "downloadUrl": format!("https://edge.forgecdn.net/files/{file_id}/{name}"),
        "gameVersions": ["Client", "1.21.1", "Fabric", "Server"],
        "sortableGameVersions": [
            {"gameVersionName": "1.21.1", "gameVersion": "1.21.1", "gameVersionTypeId": 75125},
            {"gameVersionName": "Fabric", "gameVersion": "", "gameVersionTypeId": 68441}
        ],
        "dependencies": deps.iter()
            .map(|(id, relation)| json!({"modId": id, "relationType": relation}))
            .collect::<Vec<_>>(),
        "isServerPack": false, "fileFingerprint": file_id
    })
}

fn cf_project(id: u64, slug: &str, name: &str, author: &str, blocked: bool) -> Value {
    json!({
        "id": id, "gameId": 432, "name": name, "slug": slug,
        "links": {"websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}")},
        "summary": format!("Resumo de {name}"), "status": 4, "downloadCount": 1_000,
        "classId": 6, "authors": [{"id": 1, "name": author}],
        "logo": {"thumbnailUrl": "https://media.forgecdn.net/x.png", "url": "https://media.forgecdn.net/x.png"},
        "latestFilesIndexes": [{
            "gameVersion": "1.21.1", "fileId": id * 10, "filename": "a.jar",
            "releaseType": 1, "modLoader": 4
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
    let projects = cf_projects();
    let files = cf_files();
    Mock::given(method("GET"))
        .and(path("/v1/mods/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": projects,
            "pagination": {"index": 0, "pageSize": 20, "resultCount": 4, "totalCount": 4}
        })))
        .mount(&server)
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
        .mount(&server)
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
        .mount(&server)
        .await;
    let all = files;
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
        .mount(&server)
        .await;
    server
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
    search(
        sources,
        target,
        &InstalledKeys::new(),
        &SearchRequest {
            query: "sodium".into(),
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

fn choice(source: SourceId, project: impl ToString) -> AddChoice {
    AddChoice {
        source,
        project_id: project.to_string(),
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

/// CA-T08-02: o item da CurseForge que o usuário clicou entra, com a dependência obrigatória
/// dele (pela CurseForge) e a opcional de fora; uma gravação, um `refresh`.
#[tokio::test]
async fn ca_t08_02_mod_so_da_curseforge_entra_com_a_dependencia_obrigatoria() {
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

#[cfg(unix)]
mod link {
    use std::os::unix::fs::PermissionsExt as _;

    use warden_project::add::link_curseforge::{
        CurseforgeLink, LinkError, import_file_link, parse_link, resolve_project,
    };

    use super::*;

    /// Um "packwiz" de mentira que se comporta como o `curseforge add` do JEI: confere a
    /// resposta `n` da entrada padrão, escreve o `.pw.toml` e imprime uma linha.
    fn fake_packwiz(dir: &Path, answer_ok: bool) -> Packwiz {
        let script = dir.join("packwiz-falso");
        let metafile = r#"name = "Just Enough Items (JEI)"
filename = "jei-1.20.1-forge-15.20.0.106.jar"
side = "both"

[download]
hash-format = "sha1"
hash = "abcdef0123456789abcdef0123456789abcdef01"
mode = "metadata:curseforge"

[update]
[update.curseforge]
file-id = 4712345
project-id = 238222
"#;
        let text = format!(
            "#!/bin/sh\nread answer\nif [ \"$answer\" != \"n\" ]; then echo \"resposta $answer\" >&2; exit 3; fi\n\
             {}mkdir -p mods\ncat > mods/jei.pw.toml <<'FIM'\n{metafile}FIM\necho 'Project added!'\n",
            if answer_ok { "" } else { "exit 4\n" }
        );
        fs::write(&script, text).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        Packwiz::new(script, dir.join("cache"), dir.join("config.toml"))
    }

    /// O link de arquivo passa pelo packwiz numa cópia (o pack de verdade fica como estava), com
    /// a resposta `n` às dependências; o que sai vai pelo caminho normal e dá o mesmo `.pw.toml`.
    #[tokio::test]
    async fn ca_t08_03_link_de_arquivo_e_lido_pelo_packwiz_em_staging() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("pack");
        pack(&root, "1.20.1", Loader::Forge);
        let before = fs::read_to_string(root.join("index.toml")).unwrap();
        let cli = fake_packwiz(temp.path(), true);
        let staging = temp.path().join("staging");
        let url = "https://www.curseforge.com/minecraft/mc-mods/jei/files/4712345";

        let imported = import_file_link(
            &root,
            &staging,
            &cli,
            Some(&SecretString::from(KEY.to_owned())),
            url,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(imported.project_id, "238222");
        assert_eq!(imported.file_id, "4712345");
        assert_eq!(imported.kind, ProjectKind::Mod);
        assert_eq!(imported.target().title, "Just Enough Items (JEI)");
        let choice = imported.choice();
        assert_eq!(choice.source, SourceId::Curseforge);
        assert_eq!(choice.version_id.as_deref(), Some("4712345"));

        // O pack de verdade não foi tocado, e a cópia foi apagada.
        assert!(!root.join("mods").exists());
        assert_eq!(fs::read_to_string(root.join("index.toml")).unwrap(), before);
        assert!(!staging.join("pack.toml").exists());

        // Pelo caminho normal sai o mesmo `.pw.toml` (o lado é do Warden: ambos).
        let project = warden_project::add::SourceProject {
            source: SourceId::Curseforge,
            id: "238222".into(),
            slug: "jei".into(),
            title: "Just Enough Items (JEI)".into(),
            kind: Some(ProjectKind::Mod),
            icon_url: None,
            side: None,
        };
        let version = warden_project::add::SourceVersion {
            source: SourceId::Curseforge,
            id: "4712345".into(),
            project_id: "238222".into(),
            number: "jei".into(),
            channel: warden_project::add::VersionChannel::Release,
            published: String::new(),
            game_versions: vec!["1.20.1".into()],
            loaders: vec!["forge".into()],
            file: Some(warden_project::add::SourceFile {
                name: "jei-1.20.1-forge-15.20.0.106.jar".into(),
                url: String::new(),
                sha1: Some("abcdef0123456789abcdef0123456789abcdef01".into()),
                sha512: None,
            }),
            side: SideChoice::Both,
            side_note: None,
            dependencies: Vec::new(),
        };
        let client = curseforge_client(&curseforge_server(Cf::Ok).await, Some(KEY));
        let ours = CurseforgeAdd::new(client)
            .metafile(&project, &version, SideChoice::Both)
            .unwrap();
        assert_eq!(ours.to_toml_string(), imported.metafile.to_toml_string());
    }

    /// O packwiz recusa o link: o erro dele chega, e nada fica no pack.
    #[tokio::test]
    async fn falha_do_packwiz_no_link_chega_como_erro_do_packwiz() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("pack");
        pack(&root, "1.20.1", Loader::Forge);
        let cli = fake_packwiz(temp.path(), false);
        let error = import_file_link(
            &root,
            &temp.path().join("staging"),
            &cli,
            Some(&SecretString::from(KEY.to_owned())),
            "https://www.curseforge.com/minecraft/mc-mods/jei/files/4712345",
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, LinkError::Packwiz(_)), "{error:?}");
        assert!(!root.join("mods").exists());
    }

    /// O link de projeto é achado na API pelo slug; a versão é escolhida na pré-visualização.
    #[tokio::test]
    async fn link_de_projeto_e_achado_pelo_slug() {
        let curseforge = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/mods/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [
                    cf_project(1, "sodium-extra", "Sodium Extra", "x", false),
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

        // Um modpack não entra pela página Adicionar.
        let modpack = parse_link("https://www.curseforge.com/minecraft/modpacks/algum").unwrap();
        let error = resolve_project(&client, &modpack, &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error.code, warden_project::ProjectErrorCode::InvalidInput);
    }
}
