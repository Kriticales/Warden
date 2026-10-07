//! Aceite das atualizações (P1-12; SPEC T10): contagem de requisições com servidor simulado,
//! canal, falha de rede, fixados e, com o packwiz real, a aplicação.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines
)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use warden_catalog::Loader;
use warden_core::CancellationToken;
use warden_curseforge::{CurseforgeClient, SecretString};
use warden_http::{HttpClient, HttpConfig};
use warden_modrinth::{MetadataCache, ModrinthClient};
use warden_packwiz::{
    CurseForgeFile, HashFormat, IndexEntry, Metafile, ModrinthFile, PackIndex, PackManifest, Side,
};
use warden_packwiz_cli::{Packwiz, RunContext};
use warden_project::ProjectErrorCode as Code;
use warden_project::create::{CreatePack, create};
use warden_project::registry::Registry;
use warden_project::updates::{
    UpdateChannel, UpdateContext, UpdateReason, UpdateSelection, UpdateStatus, apply, check, plan,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn http() -> HttpClient {
    let mut config = HttpConfig::for_version("1");
    config.max_retries = 0;
    HttpClient::new(config).unwrap()
}

fn modrinth(server: &MockServer) -> ModrinthClient {
    let cache = MetadataCache::in_memory(None).unwrap();
    ModrinthClient::with_base_url(http(), &format!("{}/v2", server.uri()), Some(cache)).unwrap()
}

fn curseforge(server: &MockServer, key: Option<&str>) -> CurseforgeClient {
    CurseforgeClient::with_base_url(
        http(),
        &format!("{}/v1", server.uri()),
        key.map(|key| SecretString::from(key.to_owned())),
    )
    .unwrap()
}

/// Endereço local em que nada escuta: simula "sem internet".
fn closed_port() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}/v2")
}

fn hash_of(n: usize) -> String {
    format!("{n:04x}").repeat(32)
}

fn version_json(
    id: &str,
    project: &str,
    number: &str,
    kind: &str,
    date: &str,
    sha512: &str,
) -> Value {
    json!({
        "id": id,
        "project_id": project,
        "name": number,
        "version_number": number,
        "version_type": kind,
        "game_versions": ["1.20.1"],
        "loaders": ["fabric"],
        "date_published": date,
        "changelog": format!("Notas da {number}"),
        "files": [{
            "hashes": { "sha1": "1".repeat(40), "sha512": sha512 },
            "url": format!("https://cdn.modrinth.com/data/{project}/versions/{id}/{number}.jar"),
            "filename": format!("{number}.jar"),
            "primary": true,
            "size": 10
        }],
        "dependencies": []
    })
}

fn write(root: &Path, relative: &str, text: &str) {
    let full = root.join(relative);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

fn modrinth_metafile(n: usize, side: Side) -> Metafile {
    Metafile::modrinth(
        &ModrinthFile {
            title: format!("Mod {n:03}"),
            project_id: format!("proj{n:04}"),
            version_id: format!("old{n:05}"),
            filename: format!("mod{n:03}-1.0.jar"),
            url: format!("https://cdn.modrinth.com/data/proj{n:04}/old{n:05}.jar"),
            hash_format: HashFormat::Sha512,
            hash: hash_of(n),
        },
        side,
    )
}

fn curseforge_metafile(project: u32, file: u32) -> Metafile {
    Metafile::curseforge(
        &CurseForgeFile {
            name: "Mod CF".into(),
            project_id: project,
            file_id: file,
            filename: format!("modcf-{file}.jar"),
            hash_format: HashFormat::Sha1,
            hash: "0123456789abcdef0123456789abcdef01234567".into(),
        },
        Side::Both,
    )
}

/// Pack escrito à mão (sem packwiz) para o Minecraft 1.20.1 com Fabric.
fn manual_pack(root: &Path, files: &[(String, String)]) {
    let mut manifest = PackManifest::new("Manual", "1.20.1");
    manifest.set_loader_version(warden_packwiz::Loader::Fabric, "0.16.14");
    write(root, "pack.toml", &manifest.to_toml_string());
    let index = PackIndex {
        hash_format: "sha256".into(),
        files: files
            .iter()
            .map(|(path, _)| IndexEntry::new(path, "00"))
            .collect(),
    };
    write(root, "index.toml", &index.to_toml_string());
    for (path, text) in files {
        write(root, path, text);
    }
}

fn json_response(body: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_string(body.to_string())
}

/// Um pack com `count` mods do Modrinth. Instalada: `old<n>`; mais nova: `new<n>`, só para os
/// que `has_update` aceita.
async fn mock_modrinth(
    server: &MockServer,
    count: usize,
    has_update: impl Fn(usize) -> bool,
    kind: &str,
) {
    let mut installed = serde_json::Map::new();
    let mut newest = serde_json::Map::new();
    for n in 0..count {
        let project = format!("proj{n:04}");
        installed.insert(
            hash_of(n),
            version_json(
                &format!("old{n:05}"),
                &project,
                "1.0",
                "release",
                "2026-01-01T00:00:00Z",
                &hash_of(n),
            ),
        );
        if has_update(n) {
            newest.insert(
                hash_of(n),
                version_json(
                    &format!("new{n:05}"),
                    &project,
                    "1.1",
                    kind,
                    "2026-03-01T00:00:00Z",
                    &hash_of(n + 10_000),
                ),
            );
        } else {
            newest.insert(hash_of(n), installed[&hash_of(n)].clone());
        }
    }
    Mock::given(method("POST"))
        .and(path("/v2/version_files/update"))
        .respond_with(json_response(&Value::Object(newest)))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(json_response(&Value::Object(installed)))
        .mount(server)
        .await;
}

fn modrinth_files(count: usize) -> Vec<(String, String)> {
    (0..count)
        .map(|n| {
            (
                format!("mods/mod{n:03}.pw.toml"),
                modrinth_metafile(n, Side::Both).to_toml_string(),
            )
        })
        .collect()
}

fn ctx<'a>(
    modrinth: &'a ModrinthClient,
    curseforge: Option<&'a CurseforgeClient>,
    prereleases: bool,
) -> UpdateContext<'a> {
    UpdateContext {
        modrinth,
        curseforge,
        prereleases,
    }
}

#[tokio::test]
async fn ca_t10_01_duzentos_mods_do_modrinth_em_no_maximo_tres_requisicoes() {
    let temp = tempfile::tempdir().unwrap();
    manual_pack(temp.path(), &modrinth_files(200));
    let server = MockServer::start().await;
    mock_modrinth(&server, 200, |n| n % 40 == 0, "release").await;
    let client = modrinth(&server);
    let report = check(temp.path(), &ctx(&client, None, false), None)
        .await
        .unwrap();
    let requests = server.received_requests().await.unwrap().len();
    assert!(requests <= 3, "{requests} requisições ao Modrinth");
    assert_eq!(client.request_count(), requests as u64);
    assert_eq!(report.items.len(), 200);
    assert_eq!(report.available, 5);
    let first = report.item("mods/mod000.pw.toml").unwrap();
    assert_eq!(first.status, UpdateStatus::Available);
    assert_eq!(first.current.as_deref(), Some("1.0"));
    assert_eq!(first.new_version.as_ref().unwrap().number, "1.1");
    assert_eq!(
        report.item("mods/mod001.pw.toml").unwrap().status,
        UpdateStatus::UpToDate
    );
}

#[tokio::test]
async fn ca_t10_03_so_estaveis_nao_oferece_beta_e_pede_so_release_ao_modrinth() {
    let temp = tempfile::tempdir().unwrap();
    manual_pack(temp.path(), &modrinth_files(2));
    let server = MockServer::start().await;
    // Mesmo que o servidor devolva um beta, ele não é oferecido.
    mock_modrinth(&server, 2, |_| true, "beta").await;
    let client = modrinth(&server);
    let report = check(temp.path(), &ctx(&client, None, false), None)
        .await
        .unwrap();
    assert_eq!(report.available, 0);
    assert!(
        report
            .items
            .iter()
            .all(|item| item.status == UpdateStatus::UpToDate)
    );
    let requests = server.received_requests().await.unwrap();
    let update = requests
        .iter()
        .find(|request| request.url.path() == "/v2/version_files/update")
        .unwrap();
    let body: Value = serde_json::from_slice(&update.body).unwrap();
    assert_eq!(body["version_types"], json!(["release"]));
    assert_eq!(body["game_versions"], json!(["1.20.1"]));
    assert_eq!(body["loaders"], json!(["fabric"]));

    // Com beta e alfa liberados, o beta é oferecido e o pedido não filtra o tipo.
    let server = MockServer::start().await;
    mock_modrinth(&server, 2, |_| true, "beta").await;
    let client = modrinth(&server);
    let report = check(temp.path(), &ctx(&client, None, true), None)
        .await
        .unwrap();
    assert_eq!(report.available, 2);
    let offered = report.items[0].new_version.as_ref().unwrap();
    assert_eq!(offered.channel, UpdateChannel::Beta);
    let requests = server.received_requests().await.unwrap();
    let update = requests
        .iter()
        .find(|request| request.url.path() == "/v2/version_files/update")
        .unwrap();
    let body: Value = serde_json::from_slice(&update.body).unwrap();
    assert!(body.get("version_types").is_none());
}

#[tokio::test]
async fn versao_mais_antiga_que_a_instalada_nunca_e_oferecida() {
    let temp = tempfile::tempdir().unwrap();
    manual_pack(temp.path(), &modrinth_files(1));
    let server = MockServer::start().await;
    let hash = hash_of(0);
    // A instalada é de março; o melhor "estável" é de janeiro.
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(json_response(&json!({ hash.clone(): version_json(
            "old00000", "proj0000", "2.0-beta", "beta", "2026-03-01T00:00:00Z", &hash) })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v2/version_files/update"))
        .respond_with(json_response(&json!({ hash.clone(): version_json(
            "stable01", "proj0000", "1.0", "release", "2026-01-01T00:00:00Z", &hash_of(9)) })))
        .mount(&server)
        .await;
    let client = modrinth(&server);
    let report = check(temp.path(), &ctx(&client, None, false), None)
        .await
        .unwrap();
    assert_eq!(report.items[0].status, UpdateStatus::UpToDate);
}

#[tokio::test]
async fn ca_t10_04_falha_de_rede_nunca_vira_em_dia() {
    let temp = tempfile::tempdir().unwrap();
    let mut files = modrinth_files(3);
    files.push((
        "mods/cf.pw.toml".into(),
        curseforge_metafile(100, 500).to_toml_string(),
    ));
    manual_pack(temp.path(), &files);
    let cache = MetadataCache::in_memory(None).unwrap();
    let offline = ModrinthClient::with_base_url(http(), &closed_port(), Some(cache)).unwrap();
    let cf_closed = CurseforgeClient::with_base_url(
        http(),
        &closed_port().replace("/v2", "/v1"),
        Some(SecretString::from("chave".to_owned())),
    )
    .unwrap();
    let report = check(temp.path(), &ctx(&offline, Some(&cf_closed), false), None)
        .await
        .unwrap();
    assert_eq!(report.items.len(), 4);
    for item in &report.items {
        assert_eq!(item.status, UpdateStatus::Failed, "{}", item.path);
        assert_eq!(item.reason, Some(UpdateReason::Offline), "{}", item.path);
        assert!(item.detail.is_some());
    }
    assert_eq!(report.available, 0);

    // Servidor respondendo 500: também falha, nunca "em dia".
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let client = modrinth(&server);
    let report = check(temp.path(), &ctx(&client, None, false), None)
        .await
        .unwrap();
    let modrinth_items = report
        .items
        .iter()
        .filter(|item| item.path.starts_with("mods/mod"));
    for item in modrinth_items {
        assert_eq!(item.status, UpdateStatus::Failed);
    }
}

#[tokio::test]
async fn fixados_locais_e_links_ficam_fora_e_curseforge_sem_chave_nao_e_verificada() {
    let temp = tempfile::tempdir().unwrap();
    let mut pinned = modrinth_metafile(1, Side::Both);
    pinned.pin = true;
    let files = vec![
        (
            "mods/mod000.pw.toml".to_owned(),
            modrinth_metafile(0, Side::Both).to_toml_string(),
        ),
        ("mods/mod001.pw.toml".to_owned(), pinned.to_toml_string()),
        (
            "mods/link.pw.toml".to_owned(),
            Metafile::url("Link", "https://example.org/link.jar", "00")
                .unwrap()
                .to_toml_string(),
        ),
        (
            "mods/cf.pw.toml".to_owned(),
            curseforge_metafile(100, 500).to_toml_string(),
        ),
        ("mods/local.jar".to_owned(), "jar".to_owned()),
    ];
    manual_pack(temp.path(), &files);
    let server = MockServer::start().await;
    mock_modrinth(&server, 2, |_| true, "release").await;
    let client = modrinth(&server);
    let report = check(temp.path(), &ctx(&client, None, false), None)
        .await
        .unwrap();
    let status = |path: &str| report.item(path).unwrap().status;
    assert_eq!(status("mods/mod000.pw.toml"), UpdateStatus::Available);
    assert_eq!(status("mods/mod001.pw.toml"), UpdateStatus::Pinned);
    assert_eq!(status("mods/link.pw.toml"), UpdateStatus::NotApplicable);
    assert_eq!(status("mods/local.jar"), UpdateStatus::NotApplicable);
    let cf = report.item("mods/cf.pw.toml").unwrap();
    assert_eq!(cf.status, UpdateStatus::NotChecked);
    assert_eq!(cf.reason, Some(UpdateReason::KeyMissing));
    // O fixado não foi pedido ao Modrinth.
    let requests = server.received_requests().await.unwrap();
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["hashes"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn arquivo_que_o_modrinth_nao_conhece_nao_e_verificavel() {
    let temp = tempfile::tempdir().unwrap();
    manual_pack(temp.path(), &modrinth_files(1));
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(json_response(&json!({})))
        .mount(&server)
        .await;
    let client = modrinth(&server);
    let report = check(temp.path(), &ctx(&client, None, false), None)
        .await
        .unwrap();
    assert_eq!(report.items[0].status, UpdateStatus::Failed);
    assert_eq!(report.items[0].reason, Some(UpdateReason::FileUnknown));
}

fn cf_mod(id: u64, indexes: &Value) -> Value {
    json!({
        "id": id, "gameId": 432, "name": format!("Mod {id}"), "slug": format!("mod-{id}"),
        "links": {}, "summary": "", "status": 4, "downloadCount": 1,
        "isFeatured": false, "primaryCategoryId": 0, "categories": [], "classId": 6,
        "authors": [], "logo": null, "screenshots": [], "mainFileId": 0,
        "latestFiles": [], "latestFilesIndexes": indexes, "isAvailable": true
    })
}

fn cf_file(id: u64, mod_id: u64, name: &str, kind: u32, deps: &Value) -> Value {
    json!({
        "id": id, "gameId": 432, "modId": mod_id, "isAvailable": true,
        "displayName": name, "fileName": name, "releaseType": kind, "fileStatus": 4,
        "hashes": [{"value": "ab".repeat(20), "algo": 1}],
        "fileDate": "2026-05-01T10:00:00Z", "fileLength": 10, "downloadCount": 1,
        "downloadUrl": null, "gameVersions": ["1.20.1", "Fabric"],
        "sortableGameVersions": [], "dependencies": deps, "fileFingerprint": 1
    })
}

#[tokio::test]
async fn curseforge_em_dois_lotes_respeita_versao_loader_e_canal() {
    let temp = tempfile::tempdir().unwrap();
    manual_pack(
        temp.path(),
        &[
            (
                "mods/a.pw.toml".into(),
                curseforge_metafile(100, 500).to_toml_string(),
            ),
            (
                "mods/b.pw.toml".into(),
                curseforge_metafile(200, 900).to_toml_string(),
            ),
            (
                "mods/c.pw.toml".into(),
                curseforge_metafile(300, 700).to_toml_string(),
            ),
        ],
    );
    let server = MockServer::start().await;
    // a: arquivo novo estável para 1.20.1/Fabric (600) e um beta ainda mais novo (650);
    // b: o instalado já é o mais novo; c: só há arquivo novo para Forge.
    let mods = json!({ "data": [
        cf_mod(100, &json!([
            {"gameVersion": "1.20.1", "fileId": 600, "filename": "a-600.jar", "releaseType": 1, "gameVersionTypeId": 1, "modLoader": 4},
            {"gameVersion": "1.20.1", "fileId": 650, "filename": "a-650.jar", "releaseType": 2, "gameVersionTypeId": 1, "modLoader": 4},
            {"gameVersion": "1.19.2", "fileId": 800, "filename": "a-800.jar", "releaseType": 1, "gameVersionTypeId": 1, "modLoader": 4}
        ])),
        cf_mod(200, &json!([{"gameVersion": "1.20.1", "fileId": 900, "filename": "b.jar", "releaseType": 1, "gameVersionTypeId": 1, "modLoader": 4}])),
        cf_mod(300, &json!([{"gameVersion": "1.20.1", "fileId": 800, "filename": "c.jar", "releaseType": 1, "gameVersionTypeId": 1, "modLoader": 1}]))
    ]});
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(json_response(&mods))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(json_response(&json!({ "data": [
            cf_file(600, 100, "a-600.jar", 1, &json!([]))
        ]})))
        .mount(&server)
        .await;
    let cf = curseforge(&server, Some("chave"));
    let mr_server = MockServer::start().await;
    let mr = modrinth(&mr_server);
    let report = check(temp.path(), &ctx(&mr, Some(&cf), false), None)
        .await
        .unwrap();
    assert_eq!(cf.request_count(), 2);
    assert_eq!(mr.request_count(), 0);
    let a = report.item("mods/a.pw.toml").unwrap();
    assert_eq!(a.status, UpdateStatus::Available);
    let new = a.new_version.as_ref().unwrap();
    assert_eq!(new.id, "600");
    assert_eq!(new.number, "a-600");
    assert_eq!(new.channel, UpdateChannel::Release);
    assert_eq!(
        report.item("mods/b.pw.toml").unwrap().status,
        UpdateStatus::UpToDate
    );
    assert_eq!(
        report.item("mods/c.pw.toml").unwrap().status,
        UpdateStatus::UpToDate
    );
    assert_eq!(report.available, 1);
}

#[tokio::test]
async fn curseforge_removida_e_chave_recusada() {
    let temp = tempfile::tempdir().unwrap();
    manual_pack(
        temp.path(),
        &[(
            "mods/a.pw.toml".into(),
            curseforge_metafile(100, 500).to_toml_string(),
        )],
    );
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(json_response(&json!({ "data": [] })))
        .mount(&server)
        .await;
    let cf = curseforge(&server, Some("chave"));
    let mr = modrinth(&MockServer::start().await);
    let report = check(temp.path(), &ctx(&mr, Some(&cf), false), None)
        .await
        .unwrap();
    assert_eq!(report.items[0].status, UpdateStatus::Failed);
    assert_eq!(report.items[0].reason, Some(UpdateReason::Removed));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let cf = curseforge(&server, Some("errada"));
    let report = check(temp.path(), &ctx(&mr, Some(&cf), false), None)
        .await
        .unwrap();
    assert_eq!(report.items[0].status, UpdateStatus::Failed);
    assert_eq!(report.items[0].reason, Some(UpdateReason::KeyInvalid));
}

#[tokio::test]
async fn revisao_mostra_novidades_do_meio_dependencias_novas_e_incompatibilidades() {
    let temp = tempfile::tempdir().unwrap();
    manual_pack(temp.path(), &modrinth_files(2));
    let server = MockServer::start().await;
    let mut newest = version_json(
        "new00000",
        "proj0000",
        "1.2",
        "release",
        "2026-03-01T00:00:00Z",
        &hash_of(10_000),
    );
    newest["dependencies"] = json!([
        {"project_id": "libX", "dependency_type": "required"},
        {"project_id": "proj0001", "dependency_type": "incompatible"},
        {"project_id": "opt", "dependency_type": "optional"}
    ]);
    let middle = version_json(
        "mid00000",
        "proj0000",
        "1.1",
        "release",
        "2026-02-01T00:00:00Z",
        &hash_of(9),
    );
    let installed = version_json(
        "old00000",
        "proj0000",
        "1.0",
        "release",
        "2026-01-01T00:00:00Z",
        &hash_of(0),
    );
    let beta = version_json(
        "beta0000",
        "proj0000",
        "1.3-beta",
        "beta",
        "2026-02-15T00:00:00Z",
        &hash_of(8),
    );
    Mock::given(method("POST"))
        .and(path("/v2/version_files/update"))
        .respond_with(json_response(&json!({ hash_of(0): newest.clone() })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(json_response(
            &json!({ hash_of(0): installed.clone(), hash_of(1): version_json(
            "old00001", "proj0001", "1.0", "release", "2026-01-01T00:00:00Z", &hash_of(1)) }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/versions"))
        .respond_with(json_response(&json!([newest.clone()])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/project/proj0000/version"))
        .respond_with(json_response(&json!([newest, beta, middle, installed])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/projects"))
        .respond_with(json_response(&json!([{
            "id": "libX", "slug": "lib-x", "title": "Biblioteca X", "description": "",
            "project_type": "mod", "client_side": "required", "server_side": "required",
            "categories": [], "loaders": [], "game_versions": [], "downloads": 1, "followers": 1,
            "status": "approved"
        }])))
        .mount(&server)
        .await;
    let client = modrinth(&server);
    let context = ctx(&client, None, false);
    let report = check(temp.path(), &context, None).await.unwrap();
    assert_eq!(report.available, 1);
    let result = plan(
        temp.path(),
        &report,
        &["mods/mod000.pw.toml".into()],
        &context,
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.items.len(), 1);
    let item = &result.items[0];
    assert_eq!(item.current.as_deref(), Some("1.0"));
    assert_eq!(item.new_version.number, "1.2");
    // Do meio e a nova, da mais nova para a mais antiga; o beta fica de fora no canal estável.
    let versions: Vec<&str> = item.changelog.iter().map(|c| c.version.as_str()).collect();
    assert_eq!(versions, ["1.2", "1.1"]);
    assert_eq!(item.changelog[0].text.as_deref(), Some("Notas da 1.2"));
    assert_eq!(result.new_dependencies.len(), 1);
    assert_eq!(result.new_dependencies[0].project_id, "libX");
    assert_eq!(result.new_dependencies[0].name, "Biblioteca X");
    assert_eq!(result.new_dependencies[0].needed_by, ["Mod 000"]);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].item, "Mod 000");
    assert_eq!(result.conflicts[0].with, "Mod 001");

    // Item sem atualização, ou fora do pack, não entra na revisão.
    let error = plan(
        temp.path(),
        &report,
        &["mods/mod001.pw.toml".into()],
        &context,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::InvalidInput);
    let error = plan(
        temp.path(),
        &report,
        &["mods/x.pw.toml".into()],
        &context,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::ItemNotFound);
}

#[tokio::test]
async fn revisao_da_curseforge_traz_notas_do_arquivo_novo_e_avisa_do_download_manual() {
    let temp = tempfile::tempdir().unwrap();
    manual_pack(
        temp.path(),
        &[(
            "mods/a.pw.toml".into(),
            curseforge_metafile(100, 500).to_toml_string(),
        )],
    );
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(json_response(&json!({ "data": [cf_mod(100, &json!([
            {"gameVersion": "1.20.1", "fileId": 600, "filename": "a-600.jar", "releaseType": 1, "gameVersionTypeId": 1, "modLoader": 4}
        ]))]})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(json_response(&json!({ "data": [
            cf_file(600, 100, "a-600.jar", 1, &json!([{"modId": 777, "relationType": 3}]))
        ]})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/100/files/600/changelog"))
        .respond_with(json_response(
            &json!({ "data": "<p>Corrige o travamento</p>" }),
        ))
        .mount(&server)
        .await;
    let cf = curseforge(&server, Some("chave"));
    let mr = modrinth(&MockServer::start().await);
    let context = ctx(&mr, Some(&cf), false);
    let report = check(temp.path(), &context, None).await.unwrap();
    let result = plan(
        temp.path(),
        &report,
        &["mods/a.pw.toml".into()],
        &context,
        None,
    )
    .await
    .unwrap();
    let item = &result.items[0];
    assert!(item.manual_download);
    assert_eq!(item.changelog.len(), 1);
    assert_eq!(
        item.changelog[0].text.as_deref(),
        Some("<p>Corrige o travamento</p>")
    );
    assert_eq!(result.new_dependencies.len(), 1);
    assert_eq!(result.new_dependencies[0].project_id, "777");
}

// ---------------------------------------------------------------------------------------------
// Com o packwiz real.

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

async fn real_pack(root: &Path, binary: PathBuf, files: &[(String, String)]) -> (PathBuf, Packwiz) {
    fs::create_dir_all(root.join("dados")).unwrap();
    let registry = Registry::open(root.join("dados/packs.json")).unwrap();
    let cli = Packwiz::new(
        binary,
        root.join("dados/cache"),
        root.join("dados/config.toml"),
    );
    let made = create(
        &CreatePack {
            name: "Pack Updates".into(),
            author: "Autor".into(),
            description: String::new(),
            destination: Some(root.join("pack")),
            minecraft: "1.20.1".into(),
            loader: Some(Loader::Fabric),
            loader_version: Some("0.16.14".into()),
        },
        root,
        &registry,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    for (path, text) in files {
        write(&made.path, path, text);
    }
    cli.refresh(
        &made.path,
        false,
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    (made.path, cli)
}

#[tokio::test]
async fn aplicar_grava_a_referencia_nova_mantem_lado_e_opcional_e_cria_ponto_de_seguranca() {
    let Some(binary) = binary() else { return };
    let temp = tempfile::tempdir().unwrap();
    let mut with_option = modrinth_metafile(0, Side::Client);
    with_option.option = Some(warden_packwiz::ModOption {
        optional: true,
        description: "Extra".into(),
        default: true,
    });
    let files = vec![
        (
            "mods/mod000.pw.toml".to_owned(),
            with_option.to_toml_string(),
        ),
        (
            "mods/mod001.pw.toml".to_owned(),
            modrinth_metafile(1, Side::Both).to_toml_string(),
        ),
    ];
    let (root, cli) = real_pack(temp.path(), binary, &files).await;
    let server = MockServer::start().await;
    mock_modrinth(&server, 2, |_| true, "release").await;
    Mock::given(method("GET"))
        .and(path("/v2/versions"))
        .respond_with(json_response(&json!([
            version_json(
                "new00000",
                "proj0000",
                "1.1",
                "release",
                "2026-03-01T00:00:00Z",
                &hash_of(10_000)
            ),
            version_json(
                "new00001",
                "proj0001",
                "1.1",
                "release",
                "2026-03-01T00:00:00Z",
                &hash_of(10_001)
            ),
        ])))
        .mount(&server)
        .await;
    let client = modrinth(&server);
    let context = ctx(&client, None, false);
    let report = check(&root, &context, None).await.unwrap();
    assert_eq!(report.available, 2);

    let selections = vec![
        UpdateSelection {
            path: "mods/mod000.pw.toml".into(),
            to_version_id: "new00000".into(),
        },
        UpdateSelection {
            path: "mods/mod001.pw.toml".into(),
            to_version_id: "new00001".into(),
        },
    ];
    // Versão diferente da revisada: recusado antes de gravar qualquer coisa.
    let wrong = vec![UpdateSelection {
        path: "mods/mod000.pw.toml".into(),
        to_version_id: "outra".into(),
    }];
    let before = fs::read_to_string(root.join("mods/mod000.pw.toml")).unwrap();
    let error = apply(
        &root,
        &report,
        &wrong,
        &context,
        None,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::InvalidInput);
    assert_eq!(
        fs::read_to_string(root.join("mods/mod000.pw.toml")).unwrap(),
        before
    );

    let applied = apply(
        &root,
        &report,
        &selections,
        &context,
        Some("Autor"),
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(applied.updated.len(), 2);
    assert_eq!(applied.updated[0].from.as_deref(), Some("1.0"));
    assert_eq!(applied.updated[0].to, "1.1");
    assert!(applied.safety_point.is_some());

    let text = fs::read_to_string(root.join("mods/mod000.pw.toml")).unwrap();
    assert!(text.contains("version = \"new00000\""), "{text}");
    assert!(text.contains("side = \"client\""), "{text}");
    assert!(text.contains("[option]"), "{text}");
    assert!(text.contains(&hash_of(10_000)), "{text}");
    assert!(text.contains("filename = \"1.1.jar\""), "{text}");
    // O índice foi atualizado: um refresh seguinte não muda nada.
    let index = fs::read(root.join("index.toml")).unwrap();
    cli.refresh(&root, false, RunContext::new(&CancellationToken::new()))
        .await
        .unwrap();
    assert_eq!(fs::read(root.join("index.toml")).unwrap(), index);
    // O ponto de segurança guarda o estado de antes.
    let repo = warden_versioning::PackRepo::open(&root).unwrap();
    let points = repo.safety_points().unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(Some(&points[0].name), applied.safety_point.as_ref());

    // Aplicar de novo o mesmo relatório recusa: o item mudou depois da verificação.
    let error = apply(
        &root,
        &report,
        &selections,
        &context,
        None,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::PackChangedExternally);
}
