//! Aceite de "Trocar de versão" e da versão do loader (P1-15; SPEC T07 e T11).
//!
//! O Modrinth simulado devolve respostas reais gravadas (`tests/fixtures/http/FIXTURES.md`); a
//! CurseForge é um servidor simulado (a nuvem não tem a chave). Com o sidecar real do packwiz,
//! o pack resultante passa na conformidade (`check_conformance`: um `refresh` não mudaria nada).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use warden_core::CancellationToken;
use warden_curseforge::{CurseforgeClient, SecretString};
use warden_http::{HttpClient, HttpConfig};
use warden_modrinth::{MetadataCache, ModrinthClient};
use warden_packwiz::{
    CurseForgeFile, HashFormat, Loader, Metafile, ModOption, ModrinthFile, PackIndex, PackManifest,
    Side,
};
use warden_packwiz_cli::{Packwiz, RunContext, check_conformance};
use warden_project::ProjectErrorCode as Code;
use warden_project::add::AddSources;
use warden_project::add::curseforge::CurseforgeAdd;
use warden_project::add::modrinth::ModrinthAdd;
use warden_project::change_version::{apply, options, plan};
use warden_project::search::SourceId;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const FIXTURES: &str = "tests/fixtures/http";
const MODMENU: &str = "mOgUt4GM";
const FABRIC_API: &str = "P7dR8mSH";
const PLACEHOLDER: &str = "eXts2L7r";
const APPLESKIN: &str = "EsAfCjCV";
/// Mod Menu 11.0.5 e 11.0.4 (respostas gravadas).
const MODMENU_NEW: &str = "6lgOkclV";
const MODMENU_OLD: &str = "v6Xx3fbU";

fn recorded(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURES)
        .join(format!("2026-10-07-{name}.json"));
    serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap()
}

fn as_list(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap()
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
    binary().map(|binary| {
        Packwiz::new(
            binary,
            root.join("../dados/cache"),
            root.join("../dados/config.toml"),
        )
    })
}

fn http() -> HttpClient {
    let mut config = HttpConfig::for_version("1");
    config.max_retries = 0;
    HttpClient::new(config).unwrap()
}

/// `GET /projects?ids=[…]` e `GET /versions?ids=[…]`: devolve os conhecidos pelo ID.
struct ByIds(Vec<Value>);

impl Respond for ByIds {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let ids: Vec<String> = request
            .url
            .query_pairs()
            .find(|(key, _)| key == "ids")
            .map(|(_, value)| serde_json::from_str(&value).unwrap())
            .unwrap_or_default();
        let found: Vec<&Value> = self
            .0
            .iter()
            .filter(|item| ids.iter().any(|id| item["id"] == id.as_str()))
            .collect();
        ResponseTemplate::new(200).set_body_json(found)
    }
}

/// O Modrinth simulado com as respostas reais do Mod Menu, da `AppleSkin`, da Fabric API e da
/// Placeholder API para o Fabric 1.21.1.
async fn modrinth_server() -> (MockServer, ModrinthClient) {
    let server = MockServer::start().await;
    let mut versions = Vec::new();
    for (id, name) in [
        (MODMENU, "versions-modmenu-fabric-1.21.1"),
        (APPLESKIN, "versions-appleskin-fabric-1.21.1"),
        (FABRIC_API, "versions-fabric-api-fabric-1.21.1"),
        (PLACEHOLDER, "versions-placeholder-api-fabric-1.21.1"),
    ] {
        let list = recorded(name);
        versions.extend(as_list(&list));
        Mock::given(method("GET"))
            .and(path(format!("/v2/project/{id}/version")))
            .respond_with(ResponseTemplate::new(200).set_body_json(list))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v2/projects"))
        .respond_with(ByIds(as_list(&recorded("projects-add"))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/versions"))
        .respond_with(ByIds(versions))
        .mount(&server)
        .await;
    let cache = MetadataCache::in_memory(None).unwrap();
    let client =
        ModrinthClient::with_base_url(http(), &format!("{}/v2", server.uri()), Some(cache))
            .unwrap();
    (server, client)
}

fn write(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

/// Pack Fabric 1.21.1 com os metafiles dados, indexado pelo packwiz real quando ele existe.
async fn pack(root: &Path, files: &[(&str, Metafile)]) {
    let mut manifest = PackManifest::new("Trocar versão", "1.21.1");
    manifest.set_loader_version(Loader::Fabric, "0.16.14");
    write(root, "pack.toml", &manifest.to_toml_string());
    let index = PackIndex {
        hash_format: "sha256".into(),
        files: files
            .iter()
            .map(|(path, _)| warden_packwiz::IndexEntry::new(path, "00"))
            .collect(),
    };
    write(root, "index.toml", &index.to_toml_string());
    for (path, metafile) in files {
        write(root, path, &metafile.to_toml_string());
    }
    if let Some(cli) = packwiz(root) {
        cli.refresh(root, false, RunContext::new(&CancellationToken::new()))
            .await
            .unwrap();
    }
}

/// O metafile de uma versão gravada do Modrinth, como o "Adicionar" gravaria.
fn modrinth_metafile(title: &str, list: &str, version_id: &str, side: Side) -> Metafile {
    let version = as_list(&recorded(list))
        .into_iter()
        .find(|version| version["id"] == version_id)
        .unwrap();
    let file = &version["files"][0];
    Metafile::modrinth(
        &ModrinthFile {
            title: title.into(),
            project_id: version["project_id"].as_str().unwrap().into(),
            version_id: version_id.into(),
            filename: file["filename"].as_str().unwrap().into(),
            url: file["url"].as_str().unwrap().into(),
            hash_format: HashFormat::Sha512,
            hash: file["hashes"]["sha512"].as_str().unwrap().into(),
        },
        side,
    )
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

fn root_of(temp: &tempfile::TempDir) -> PathBuf {
    let root = temp.path().join("pack");
    fs::create_dir_all(temp.path().join("dados")).unwrap();
    fs::create_dir_all(&root).unwrap();
    root
}

#[tokio::test]
async fn modrinth_lista_compativeis_marca_a_instalada_e_as_mais_antigas() {
    let temp = tempfile::tempdir().unwrap();
    let root = root_of(&temp);
    let (_server, client) = modrinth_server().await;
    let sources = AddSources::new().with(Arc::new(ModrinthAdd::new(client)));
    pack(
        &root,
        &[(
            "mods/modmenu.pw.toml",
            modrinth_metafile(
                "Mod Menu",
                "versions-modmenu-fabric-1.21.1",
                MODMENU_OLD,
                Side::Client,
            ),
        )],
    )
    .await;
    let cancel = CancellationToken::new();
    let found = options(&root, &sources, "mods/modmenu.pw.toml", &cancel)
        .await
        .unwrap();
    assert_eq!(found.name, "Mod Menu");
    assert_eq!(found.source, SourceId::Modrinth);
    assert_eq!(found.current_id.as_deref(), Some(MODMENU_OLD));
    assert_eq!(found.current.as_deref(), Some("11.0.4"));
    assert!(!found.current_incompatible);
    let rows: Vec<(&str, bool, bool)> = found
        .versions
        .iter()
        .map(|v| (v.number.as_str(), v.installed, v.older))
        .collect();
    // Todas as compatíveis com o pack, da mais nova para a mais antiga.
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0], ("11.0.5", false, false));
    assert_eq!(rows[1], ("11.0.4", true, false));
    assert!(
        rows[2..]
            .iter()
            .all(|(_, installed, older)| !installed && *older)
    );
    assert!(found.versions.iter().all(|v| !v.file_name.is_empty()));
}

#[tokio::test]
async fn modrinth_rebaixar_avisa_grava_a_referencia_e_passa_na_conformidade() {
    let temp = tempfile::tempdir().unwrap();
    let root = root_of(&temp);
    let (_server, client) = modrinth_server().await;
    let sources = AddSources::new().with(Arc::new(ModrinthAdd::new(client)));
    let mut modmenu = modrinth_metafile(
        "Mod Menu (meu nome)",
        "versions-modmenu-fabric-1.21.1",
        MODMENU_NEW,
        Side::Client,
    );
    modmenu.pin = true;
    modmenu.option = Some(ModOption {
        optional: true,
        description: "Menu de mods".into(),
        default: true,
    });
    pack(
        &root,
        &[
            ("mods/modmenu.pw.toml", modmenu),
            (
                "mods/fabric-api.pw.toml",
                modrinth_metafile(
                    "Fabric API",
                    "versions-fabric-api-fabric-1.21.1",
                    "Mys3P7lK",
                    Side::Both,
                ),
            ),
        ],
    )
    .await;
    let cancel = CancellationToken::new();
    let path = "mods/modmenu.pw.toml";

    // A revisão: rebaixamento, e a Placeholder API que a versão pede e o pack não tem.
    let reviewed = plan(&root, &sources, path, MODMENU_OLD, &cancel)
        .await
        .unwrap();
    assert!(reviewed.downgrade && !reviewed.same);
    assert_eq!(reviewed.current.as_deref(), Some("11.0.5"));
    assert_eq!(reviewed.to.number, "11.0.4");
    let missing: Vec<&str> = reviewed
        .new_dependencies
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    assert_eq!(missing, ["Text Placeholder API"]);
    assert!(reviewed.conflicts.is_empty());
    let same = plan(&root, &sources, path, MODMENU_NEW, &cancel)
        .await
        .unwrap();
    assert!(same.same && !same.downgrade);

    let Some(cli) = packwiz(&root) else { return };
    // A versão mostrada no diálogo já não é a do item: nada é gravado.
    let before = fs::read_to_string(root.join(path)).unwrap();
    let stale = apply(
        &root,
        &sources,
        path,
        Some(MODMENU_OLD),
        MODMENU_OLD,
        &cli,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(stale.code, Code::PackChangedExternally);
    // Versão de outro projeto: recusada.
    let other = apply(
        &root,
        &sources,
        path,
        Some(MODMENU_NEW),
        "Mys3P7lK",
        &cli,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(other.code, Code::InvalidInput);
    assert_eq!(fs::read_to_string(root.join(path)).unwrap(), before);

    let changed = apply(
        &root,
        &sources,
        path,
        Some(MODMENU_NEW),
        MODMENU_OLD,
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    assert!(changed.downgrade);
    assert_eq!(changed.from.as_deref(), Some("11.0.5"));
    assert_eq!(changed.to, "11.0.4");
    let text = fs::read_to_string(root.join(path)).unwrap();
    let metafile = Metafile::parse(&text).unwrap().value;
    let expected = modrinth_metafile(
        "x",
        "versions-modmenu-fabric-1.21.1",
        MODMENU_OLD,
        Side::Client,
    );
    // A referência nova, com o nome, o lado, o fixar e o opcional de antes.
    assert_eq!(metafile.filename, expected.filename);
    assert_eq!(metafile.download, expected.download);
    assert_eq!(metafile.update, expected.update);
    assert_eq!(metafile.name, "Mod Menu (meu nome)");
    assert_eq!(metafile.side, Side::Client);
    assert!(metafile.pin);
    assert_eq!(
        metafile.option.as_ref().unwrap().description,
        "Menu de mods"
    );
    assert_conforming(&root).await;

    // Escolher a versão instalada não grava nada.
    let index = fs::read(root.join("index.toml")).unwrap();
    let again = apply(
        &root,
        &sources,
        path,
        Some(MODMENU_OLD),
        MODMENU_OLD,
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    assert!(!again.downgrade);
    assert_eq!(fs::read(root.join("index.toml")).unwrap(), index);
}

#[tokio::test]
async fn arquivo_local_e_caminho_desconhecido_nao_tem_versoes() {
    let temp = tempfile::tempdir().unwrap();
    let root = root_of(&temp);
    let (_server, client) = modrinth_server().await;
    let sources = AddSources::new().with(Arc::new(ModrinthAdd::new(client)));
    pack(&root, &[]).await;
    write(&root, "mods/solto.jar", "jar");
    let cancel = CancellationToken::new();
    let local = options(&root, &sources, "mods/solto.jar", &cancel)
        .await
        .unwrap_err();
    assert_eq!(local.code, Code::InvalidInput);
    let unknown = options(&root, &sources, "mods/nada.pw.toml", &cancel)
        .await
        .unwrap_err();
    assert_eq!(unknown.code, Code::ItemNotFound);
    assert_eq!(unknown.params["path"], "mods/nada.pw.toml");
}

// ---------------------------------------------------------------------------------------------
// CurseForge (servidor simulado).

fn cf_mod(id: u64, name: &str) -> Value {
    json!({
        "id": id, "gameId": 432, "name": name, "slug": format!("mod-{id}"),
        "links": {}, "summary": "", "status": 4, "downloadCount": 1,
        "isFeatured": false, "primaryCategoryId": 0, "categories": [], "classId": 6,
        "authors": [], "logo": null, "screenshots": [], "mainFileId": 0,
        "latestFiles": [], "latestFilesIndexes": [], "isAvailable": true
    })
}

fn cf_file(id: u64, mod_id: u64, name: &str, date: &str, deps: &Value) -> Value {
    json!({
        "id": id, "gameId": 432, "modId": mod_id, "isAvailable": true,
        "displayName": name, "fileName": format!("{name}.jar"), "releaseType": 1,
        "fileStatus": 4,
        "hashes": [{"value": format!("{id:040}"), "algo": 1}],
        "fileDate": date, "fileLength": 10, "downloadCount": 1,
        "downloadUrl": null, "gameVersions": ["1.21.1", "Fabric"],
        "sortableGameVersions": [], "dependencies": deps, "fileFingerprint": 1
    })
}

/// `POST /mods` e `POST /mods/files`: devolve os pedidos pelos IDs do corpo.
struct CfByIds(Vec<Value>, &'static str);

impl Respond for CfByIds {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        let ids: Vec<u64> = body[self.1]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_u64)
            .collect();
        let found: Vec<&Value> = self
            .0
            .iter()
            .filter(|item| ids.contains(&item["id"].as_u64().unwrap()))
            .collect();
        ResponseTemplate::new(200).set_body_json(json!({ "data": found }))
    }
}

#[tokio::test]
async fn curseforge_troca_de_versao_com_dependencia_e_incompatibilidade() {
    let temp = tempfile::tempdir().unwrap();
    let root = root_of(&temp);
    let server = MockServer::start().await;
    let new_deps = json!([
        { "modId": 3003, "relationType": 3 },
        { "modId": 4004, "relationType": 5 }
    ]);
    let files = vec![
        cf_file(
            2002,
            1001,
            "mod-cf-2.0",
            "2026-06-01T10:00:00.12Z",
            &new_deps,
        ),
        cf_file(2001, 1001, "mod-cf-1.0", "2026-01-01T10:00:00Z", &json!([])),
        cf_file(2000, 1001, "mod-cf-0.9", "2025-06-01T10:00:00Z", &json!([])),
    ];
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(CfByIds(
            vec![
                cf_mod(1001, "Mod CF"),
                cf_mod(3003, "Biblioteca CF"),
                cf_mod(4004, "Rival CF"),
            ],
            "modIds",
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(CfByIds(files.clone(), "fileIds"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/v1/mods/1001/files$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": files,
            "pagination": { "index": 0, "pageSize": 50, "resultCount": 3, "totalCount": 3 }
        })))
        .mount(&server)
        .await;
    let client = CurseforgeClient::with_base_url(
        http(),
        &format!("{}/v1", server.uri()),
        Some(SecretString::from("chave-de-teste".to_owned())),
    )
    .unwrap();
    let sources = AddSources::new().with(Arc::new(CurseforgeAdd::new(client)));
    let cf = |project: u32, file: u32, name: &str| {
        Metafile::curseforge(
            &CurseForgeFile {
                name: name.into(),
                project_id: project,
                file_id: file,
                filename: format!("{name}-{file}.jar"),
                hash_format: HashFormat::Sha1,
                hash: format!("{file:040}"),
            },
            Side::Both,
        )
    };
    pack(
        &root,
        &[
            ("mods/mod-cf.pw.toml", cf(1001, 2001, "Mod CF")),
            ("mods/rival.pw.toml", cf(4004, 5005, "Rival CF")),
        ],
    )
    .await;
    let cancel = CancellationToken::new();
    let path = "mods/mod-cf.pw.toml";
    let found = options(&root, &sources, path, &cancel).await.unwrap();
    assert_eq!(found.source, SourceId::Curseforge);
    assert_eq!(found.current.as_deref(), Some("mod-cf-1.0"));
    let rows: Vec<(&str, bool, bool)> = found
        .versions
        .iter()
        .map(|v| (v.id.as_str(), v.installed, v.older))
        .collect();
    assert_eq!(
        rows,
        [
            ("2002", false, false),
            ("2001", true, false),
            ("2000", false, true)
        ]
    );

    let newer = plan(&root, &sources, path, "2002", &cancel).await.unwrap();
    assert!(!newer.downgrade);
    assert_eq!(newer.new_dependencies.len(), 1);
    assert_eq!(newer.new_dependencies[0].name, "Biblioteca CF");
    assert_eq!(newer.conflicts.len(), 1);
    assert_eq!(newer.conflicts[0].with, "Rival CF");
    let older = plan(&root, &sources, path, "2000", &cancel).await.unwrap();
    assert!(older.downgrade);
    assert!(older.new_dependencies.is_empty() && older.conflicts.is_empty());

    let Some(cli) = packwiz(&root) else { return };
    let changed = apply(&root, &sources, path, Some("2001"), "2000", &cli, &cancel)
        .await
        .unwrap();
    assert!(changed.downgrade);
    let metafile = Metafile::parse(&fs::read_to_string(root.join(path)).unwrap())
        .unwrap()
        .value;
    assert_eq!(metafile.filename, "mod-cf-0.9.jar");
    assert_eq!(metafile.name, "Mod CF");
    let text = fs::read_to_string(root.join(path)).unwrap();
    assert!(text.contains("file-id = 2000"), "{text}");
    assert!(text.contains("project-id = 1001"), "{text}");
    assert_conforming(&root).await;
}
