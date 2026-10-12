//! Aceite do arquivo do computador e dos links em "Adicionar" (P1-11; SPEC T08; CA-T08-04,
//! CA-T08-05 e CA-T08-07).
//!
//! Os jars são sintéticos, montados aqui com os descritores **reais** copiados pelo corpus da
//! `warden-jarmeta` (os jars de terceiros não são versionados). O Modrinth simulado devolve
//! respostas reais gravadas (`warden-modrinth`); para "o jar é o Sodium", o `/version_files`
//! simulado devolve a versão gravada do Sodium para o hash do jar sintético. A CurseForge
//! simulada é montada no formato da API (os termos dela proíbem guardar respostas). Com o
//! sidecar real do packwiz, o pack resultante passa na conformidade (`check_conformance`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use warden_core::CancellationToken;
use warden_curseforge::{CurseforgeClient, SecretString};
use warden_http::{HttpClient, HttpConfig, Url};
use warden_modrinth::{MetadataCache, ModrinthClient};
use warden_packwiz::{Loader, Metafile, PackIndex, PackManifest};
use warden_packwiz_cli::{Packwiz, RunContext, check_conformance};
use warden_project::add::link::{DirectError, download_direct, resolve_modrinth_link};
use warden_project::add::local::{
    FileFacts, HashLookup, LocalAdd, add_local_apply, identify, inspect, read_file,
};
use warden_project::add::modrinth::ModrinthAdd;
use warden_project::add::plan::{AddPlanRequest, NodeRole, add_plan};
use warden_project::add::url::{DirectLink, LinkClass, ModrinthLink, classify_link};
use warden_project::add::{AddApplyRequest, AddSources, ApplyItem, ChannelPolicy, add_apply};
use warden_project::search::{PackTarget, ProjectKind, SourceId};
use warden_project::side::SideChoice;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};
use zip::write::{SimpleFileOptions, ZipWriter};

const MODRINTH_FIXTURES: &str = "../warden-modrinth/tests/fixtures/http";
const CORPUS: &str = "../warden-jarmeta/tests/corpus/jars";
const KEY: &str = "$2a$10$chaveFalsaDoWardenParaTestes0123456789abcdefghijkl";
const SODIUM: &str = "AANobbMI";
const SODIUM_VERSION: &str = "SMxNOGZ6";

// --- Jars sintéticos --------------------------------------------------------------------

fn corpus(path: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(CORPUS)
            .join(path),
    )
    .unwrap()
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, content) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// Um jar Fabric com o `fabric.mod.json` real do Sodium (para Minecraft 1.20.1).
fn sodium_jar() -> Vec<u8> {
    zip_of(&[(
        "fabric.mod.json",
        corpus("fabric-1.20.1-sodium/fabric.mod.json").as_bytes(),
    )])
}

/// Um jar Forge com o `mods.toml` real do JEI (Minecraft 1.20.1).
fn jei_jar() -> Vec<u8> {
    zip_of(&[(
        "META-INF/mods.toml",
        corpus("forge-1.20.1-jei/META-INF/mods.toml").as_bytes(),
    )])
}

// --- Packs e packwiz --------------------------------------------------------------------

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

fn packwiz(dir: &Path) -> Option<Packwiz> {
    Some(Packwiz::new(
        binary()?,
        dir.join("packwiz-cache"),
        dir.join("packwiz-config.toml"),
    ))
}

/// Pack vazio já com o índice do packwiz.
async fn pack(root: &Path, minecraft: &str, loader: Loader, cli: &Packwiz) {
    fs::create_dir_all(root).unwrap();
    let mut manifest = PackManifest::new("Teste", minecraft);
    manifest.set_loader_version(loader, "1.0.0");
    fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    fs::write(
        root.join("index.toml"),
        PackIndex::default().to_toml_string(),
    )
    .unwrap();
    cli.refresh(root, false, RunContext::new(&CancellationToken::new()))
        .await
        .unwrap();
}

async fn assert_conforming(root: &Path, cli: &Packwiz) {
    let staging = tempfile::tempdir().unwrap();
    let report = check_conformance(
        cli,
        root,
        &staging.path().join("copia"),
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    assert!(report.is_conforming(), "{:#?}", report.differences);
}

fn index_files(root: &Path) -> Vec<String> {
    PackIndex::parse(&fs::read_to_string(root.join("index.toml")).unwrap())
        .unwrap()
        .value
        .files
        .into_iter()
        .map(|entry| entry.file)
        .collect()
}

/// O arquivo escrito numa pasta com espaços e acentos, como no Windows.
fn user_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let folder = dir.join("Área de Trabalho").join("Mods baixados");
    fs::create_dir_all(&folder).unwrap();
    let file = folder.join(name);
    fs::write(&file, bytes).unwrap();
    file
}

fn local(file: &Path, kind: ProjectKind, accept: bool) -> LocalAdd {
    let (_, facts) = read_file(file).unwrap();
    LocalAdd {
        path: file.to_path_buf(),
        sha256: facts.sha256,
        kind,
        link: None,
        name: String::new(),
        side: SideChoice::Both,
        accept_incompatible: accept,
    }
}

fn http() -> HttpClient {
    let mut config = HttpConfig::for_version("1");
    config.max_retries = 0;
    HttpClient::new(config).unwrap()
}

fn modrinth_fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(MODRINTH_FIXTURES)
        .join(format!("2026-10-04-{name}.json"));
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// Modrinth simulado: `/version_files` acha o jar sintético do Sodium (sha512) e devolve a
/// versão gravada `SMxNOGZ6`; projetos e versões vêm das respostas gravadas.
async fn modrinth_server(known_sha512: &str) -> MockServer {
    let server = MockServer::start().await;
    let version = modrinth_fixture(&format!("version-{SODIUM_VERSION}"));
    let known = known_sha512.to_owned();
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(move |request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["algorithm"], "sha512");
            let mut found = serde_json::Map::new();
            for hash in body["hashes"].as_array().unwrap() {
                if hash.as_str() == Some(known.as_str()) {
                    found.insert(known.clone(), version.clone());
                }
            }
            ResponseTemplate::new(200).set_body_json(Value::Object(found))
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(modrinth_fixture("projects-3")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/versions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(modrinth_fixture("versions-2")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/project/sodium"))
        .respond_with(ResponseTemplate::new(200).set_body_json(modrinth_fixture("project-sodium")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/version/{SODIUM_VERSION}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(modrinth_fixture(&format!("version-{SODIUM_VERSION}"))),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v2/project/{SODIUM}/version")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(modrinth_fixture("project-sodium-versions-fabric-1.21.1")),
        )
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

/// CurseForge simulada (formato da API): a impressão do jar do JEI é o arquivo 4712345 do
/// projeto 238222.
async fn curseforge_server(jei_fingerprint: u32) -> MockServer {
    let server = MockServer::start().await;
    let file = json!({
        "id": 4_712_345, "gameId": 432, "modId": 238_222, "isAvailable": true,
        "displayName": "jei-1.20.1-forge-15.20.0.106.jar",
        "fileName": "jei-1.20.1-forge-15.20.0.106.jar", "releaseType": 1, "fileStatus": 4,
        "hashes": [{"value": "0123456789abcdef0123456789abcdef01234567", "algo": 1}],
        "fileDate": "2026-09-19T11:47:57.347Z", "fileLength": 1000, "downloadCount": 7,
        "gameVersions": ["1.20.1", "Forge"],
        "sortableGameVersions": [
            {"gameVersionName": "1.20.1", "gameVersion": "1.20.1", "gameVersionTypeId": 75125},
            {"gameVersionName": "Forge", "gameVersion": "", "gameVersionTypeId": 68441}
        ],
        "dependencies": [], "isServerPack": false, "fileFingerprint": jei_fingerprint
    });
    Mock::given(method("POST"))
        .and(path("/v1/fingerprints/432"))
        .respond_with(move |request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let wanted: Vec<u64> = body["fingerprints"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_u64)
                .collect();
            let matches: Vec<Value> = wanted
                .iter()
                .filter(|&&fingerprint| fingerprint == u64::from(jei_fingerprint))
                .map(|_| json!({"id": 238_222, "file": file.clone(), "latestFiles": []}))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({
                "data": {"isCacheBuilt": true, "exactMatches": matches, "exactFingerprints": [],
                         "partialMatches": [], "installedFingerprints": wanted,
                         "unmatchedFingerprints": null}
            }))
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [{
            "id": 238_222, "gameId": 432, "name": "Just Enough Items (JEI)", "slug": "jei",
            "links": {"websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/jei"},
            "summary": "JEI", "status": 4, "downloadCount": 1, "classId": 6,
            "primaryCategoryId": 423, "authors": [{"id": 1, "name": "mezz"}],
            "latestFilesIndexes": [], "dateCreated": "2020-01-01T00:00:00Z",
            "dateModified": "2026-10-01T00:00:00Z", "allowModDistribution": true,
            "isAvailable": true
        }]})))
        .mount(&server)
        .await;
    server
}

fn curseforge_client(server: &MockServer) -> CurseforgeClient {
    CurseforgeClient::with_base_url(
        http(),
        &format!("{}/v1", server.uri()),
        Some(SecretString::from(KEY.to_owned())),
    )
    .unwrap()
}

fn direct(text: &str) -> DirectLink {
    match classify_link(text) {
        LinkClass::Direct(link) => link,
        other => panic!("{other:?}"),
    }
}

// --- Arquivo do computador --------------------------------------------------------------

/// Arquivo não identificado: copiado para `mods/` e indexado pelo `packwiz refresh`.
#[tokio::test]
async fn arquivo_nao_identificado_e_copiado_para_a_pasta_do_tipo() {
    let temp = tempfile::tempdir().unwrap();
    let Some(cli) = packwiz(temp.path()) else {
        return;
    };
    let root = temp.path().join("Meu pack ç");
    pack(&root, "1.20.1", Loader::Forge, &cli).await;
    let jar = jei_jar();
    let file = user_file(temp.path(), "Just Enough Items (1.20.1).jar", &jar);
    let (bytes, facts) = read_file(&file).unwrap();
    let report = inspect(&root, &bytes, &facts).unwrap();
    assert_eq!(report.kind, Some(ProjectKind::Mod));
    assert!(report.incompatible.is_empty(), "{:?}", report.incompatible);
    assert_eq!(report.in_pack, None);

    let result = add_local_apply(
        &root,
        &[local(&file, ProjectKind::Mod, false)],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.added[0].path, "mods/Just Enough Items (1.20.1).jar");
    assert_eq!(
        fs::read(root.join("mods/Just Enough Items (1.20.1).jar")).unwrap(),
        jar
    );
    assert!(index_files(&root).contains(&"mods/Just Enough Items (1.20.1).jar".to_owned()));
    assert_conforming(&root, &cli).await;

    // O mesmo arquivo de novo: já está no pack, nada muda.
    let again = add_local_apply(
        &root,
        &[local(&file, ProjectKind::Mod, false)],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(again.added.is_empty());
    assert_eq!(again.skipped, ["mods/Just Enough Items (1.20.1).jar"]);
    let report = inspect(&root, &bytes, &facts).unwrap();
    assert!(report.in_pack.unwrap().same_file);
}

/// Resource pack e shader vão para as pastas deles; vários arquivos entram numa transação só.
#[tokio::test]
async fn varios_arquivos_numa_transacao() {
    let temp = tempfile::tempdir().unwrap();
    let Some(cli) = packwiz(temp.path()) else {
        return;
    };
    let root = temp.path().join("pack");
    pack(&root, "1.21.1", Loader::Fabric, &cli).await;
    let resource = user_file(
        temp.path(),
        "Faithful 32x.zip",
        &zip_of(&[("pack.mcmeta", b"{\"pack\":{\"pack_format\":34}}")]),
    );
    let shader = user_file(
        temp.path(),
        "Complementary.zip",
        &zip_of(&[("shaders/final.fsh", b"void main(){}")]),
    );
    let result = add_local_apply(
        &root,
        &[
            local(&resource, ProjectKind::ResourcePack, false),
            local(&shader, ProjectKind::Shader, false),
        ],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let paths: Vec<&str> = result.added.iter().map(|item| item.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "resourcepacks/Faithful 32x.zip",
            "shaderpacks/Complementary.zip"
        ]
    );
    assert_conforming(&root, &cli).await;
}

/// Um arquivo que falha no meio (mudou depois de lido) não deixa nenhum outro no pack.
#[tokio::test]
async fn falha_no_meio_nao_deixa_nada_pela_metade() {
    let temp = tempfile::tempdir().unwrap();
    let Some(cli) = packwiz(temp.path()) else {
        return;
    };
    let root = temp.path().join("pack");
    pack(&root, "1.20.1", Loader::Forge, &cli).await;
    let before = fs::read_to_string(root.join("index.toml")).unwrap();
    let good = user_file(temp.path(), "jei.jar", &jei_jar());
    let changed = user_file(temp.path(), "outro.jar", &zip_of(&[("a.txt", b"1")]));
    let mut second = local(&changed, ProjectKind::Mod, false);
    fs::write(&changed, zip_of(&[("a.txt", b"2")])).unwrap();
    second.sha256 = FileFacts::of("outro.jar", &zip_of(&[("a.txt", b"1")])).sha256;
    let error = add_local_apply(
        &root,
        &[local(&good, ProjectKind::Mod, false), second],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.params.get("reason").map(String::as_str),
        Some("changed")
    );
    assert!(!root.join("mods/jei.jar").exists());
    assert_eq!(fs::read_to_string(root.join("index.toml")).unwrap(), before);
}

/// CA-T08-05: um jar Fabric num pack Forge é bloqueado com a explicação; "Adicionar mesmo
/// assim" copia o arquivo, e o diagnóstico passa a mostrar o erro de loader (`E_LOADER`).
#[tokio::test]
async fn ca_t08_05_jar_fabric_num_pack_forge() {
    let temp = tempfile::tempdir().unwrap();
    let Some(cli) = packwiz(temp.path()) else {
        return;
    };
    let root = temp.path().join("pack");
    pack(&root, "1.20.1", Loader::Forge, &cli).await;
    let file = user_file(temp.path(), "sodium-fabric-0.5.13.jar", &sodium_jar());
    let (bytes, facts) = read_file(&file).unwrap();
    let report = inspect(&root, &bytes, &facts).unwrap();
    assert_eq!(
        serde_json::to_value(&report.incompatible).unwrap(),
        json!([{"kind": "loader", "found": ["fabric"], "pack": "forge"}])
    );

    let error = add_local_apply(
        &root,
        &[local(&file, ProjectKind::Mod, false)],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.params.get("reason").map(String::as_str),
        Some("incompatible")
    );
    assert!(!root.join("mods").exists());

    add_local_apply(
        &root,
        &[local(&file, ProjectKind::Mod, true)],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let copied = root.join("mods/sodium-fabric-0.5.13.jar");
    assert!(copied.is_file());
    assert_conforming(&root, &cli).await;

    // O diagnóstico (passagem completa, com os metadados do jar) aponta o loader errado.
    let jar = warden_jarmeta::read_jar_file(&copied, &warden_jarmeta::Limits::default()).unwrap();
    let input = warden_diagnostics::pretest::Input {
        pack: warden_diagnostics::pretest::PackContext {
            minecraft: "1.20.1".into(),
            minecraft_release_target: None,
            loader: warden_jarmeta::Loader::Forge,
            loader_version: "47.3.0".into(),
            language_loader_versions: std::collections::HashMap::new(),
            java_major: 17,
            minimum_java_major: Some(17),
            resource_pack_format: None,
            acceptable_game_versions: Vec::new(),
        },
        items: vec![warden_diagnostics::pretest::PackItem {
            path: "mods/sodium-fabric-0.5.13.jar".into(),
            filename: "sodium-fabric-0.5.13.jar".into(),
            side: warden_packwiz::Side::Both,
            file_hash: Some(facts.sha256.clone()),
            api: None,
            jar: Some(jar),
            accepted_minecraft_versions: Vec::new(),
            resource_pack_format: None,
        }],
        pass: warden_diagnostics::pretest::Pass::Complete,
    };
    let findings = warden_diagnostics::pretest::analyze(&input).unwrap();
    assert!(
        findings
            .iter()
            .any(|finding| finding.rule().as_str() == "E_LOADER"),
        "{findings:#?}"
    );
}

/// Outro arquivo do pack já usa o nome: recusado, nada é sobrescrito.
#[tokio::test]
async fn nome_ja_usado_no_pack_e_recusado() {
    let temp = tempfile::tempdir().unwrap();
    let Some(cli) = packwiz(temp.path()) else {
        return;
    };
    let root = temp.path().join("pack");
    pack(&root, "1.20.1", Loader::Forge, &cli).await;
    fs::create_dir_all(root.join("mods")).unwrap();
    fs::write(root.join("mods/jei.jar"), zip_of(&[("x", b"antigo")])).unwrap();
    cli.refresh(&root, false, RunContext::new(&CancellationToken::new()))
        .await
        .unwrap();
    let file = user_file(temp.path(), "jei.jar", &jei_jar());
    let error = add_local_apply(
        &root,
        &[local(&file, ProjectKind::Mod, false)],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.params.get("reason").map(String::as_str),
        Some("nameTaken")
    );
    assert_eq!(
        fs::read(root.join("mods/jei.jar")).unwrap(),
        zip_of(&[("x", b"antigo")])
    );
}

// --- Identificação pelo hash ------------------------------------------------------------

/// CA-T08-04: o jar do Sodium é achado no Modrinth pelo hash; aceitar a referência grava o
/// `.pw.toml` (`[update.modrinth]`, `sha512`) pelo caminho normal e não copia o jar.
#[tokio::test]
async fn ca_t08_04_jar_identificado_vira_referencia() {
    let temp = tempfile::tempdir().unwrap();
    let Some(cli) = packwiz(temp.path()) else {
        return;
    };
    let root = temp.path().join("pack");
    pack(&root, "1.21.1", Loader::Fabric, &cli).await;
    let jar = sodium_jar();
    let file = user_file(temp.path(), "sodium.jar", &jar);
    let (_, facts) = read_file(&file).unwrap();
    let server = modrinth_server(&facts.sha512).await;
    let modrinth = modrinth_client(&server);
    let target = PackTarget::read(&root).unwrap();
    let identified = identify(
        HashLookup {
            modrinth: Some(&modrinth),
            curseforge: None,
        },
        &[facts, FileFacts::of("desconhecido.jar", b"outro")],
        &target,
        &CancellationToken::new(),
    )
    .await;
    assert!(identified.failed.is_empty());
    let found = &identified.matches[0][0];
    assert_eq!(found.source, SourceId::Modrinth);
    assert_eq!(found.project_id, SODIUM);
    assert_eq!(found.version_id, SODIUM_VERSION);
    assert_eq!(found.title, "Sodium");
    assert_eq!(found.kind, Some(ProjectKind::Mod));
    assert!(found.compatible);
    assert!(
        identified.matches[1].is_empty(),
        "desconhecido não é achado"
    );

    // Aceitar a referência: o caminho normal com a versão achada.
    let sources = AddSources::new().with(Arc::new(ModrinthAdd::new(modrinth)));
    let plan = add_plan(
        &root,
        &sources,
        ChannelPolicy::default(),
        &AddPlanRequest {
            items: vec![warden_project::add::AddChoice {
                source: SourceId::Modrinth,
                project_id: found.project_id.clone(),
                version_id: Some(found.version_id.clone()),
            }],
        },
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let request = AddApplyRequest {
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
    };
    add_apply(&root, &sources, &request, &cli, &CancellationToken::new())
        .await
        .unwrap();
    let metafile = Metafile::parse(&fs::read_to_string(root.join("mods/sodium.pw.toml")).unwrap())
        .unwrap()
        .value;
    assert_eq!(metafile.modrinth_ids(), Some((SODIUM, SODIUM_VERSION)));
    assert_eq!(metafile.download.hash_format, "sha512");
    let jars: Vec<_> = fs::read_dir(root.join("mods"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jar"))
        .collect();
    assert!(jars.is_empty(), "o jar não é copiado: {jars:?}");
    assert_conforming(&root, &cli).await;
}

/// Com a chave, a CurseForge também é consultada pela impressão digital; sem ela, nenhuma
/// requisição vai para a CurseForge. Fonte fora do ar não é erro: fica em `failed`.
#[tokio::test]
async fn curseforge_pela_impressao_digital_so_com_chave() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("pack");
    fs::create_dir_all(&root).unwrap();
    let mut manifest = PackManifest::new("Teste", "1.20.1");
    manifest.set_loader_version(Loader::Forge, "47.3.0");
    let target = PackTarget::from_manifest(&manifest).unwrap();
    let jar = jei_jar();
    let facts = FileFacts::of("jei.jar", &jar);
    let curseforge = curseforge_server(facts.murmur2).await;
    let modrinth = modrinth_server("nenhum").await;
    let modrinth_api = modrinth_client(&modrinth);
    let cf = curseforge_client(&curseforge);

    let identified = identify(
        HashLookup {
            modrinth: Some(&modrinth_api),
            curseforge: Some(&cf),
        },
        std::slice::from_ref(&facts),
        &target,
        &CancellationToken::new(),
    )
    .await;
    let [found] = identified.matches[0].as_slice() else {
        panic!("{identified:?}");
    };
    assert_eq!(found.source, SourceId::Curseforge);
    assert_eq!(found.project_id, "238222");
    assert_eq!(found.version_id, "4712345");
    assert_eq!(found.title, "Just Enough Items (JEI)");
    assert!(found.compatible);

    let before = curseforge.received_requests().await.unwrap().len();
    let identified = identify(
        HashLookup {
            modrinth: Some(&modrinth_api),
            curseforge: None,
        },
        std::slice::from_ref(&facts),
        &target,
        &CancellationToken::new(),
    )
    .await;
    assert!(identified.matches[0].is_empty());
    assert_eq!(curseforge.received_requests().await.unwrap().len(), before);

    // Modrinth fora do ar.
    let down = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&down)
        .await;
    let identified = identify(
        HashLookup {
            modrinth: Some(&modrinth_client(&down)),
            curseforge: None,
        },
        std::slice::from_ref(&facts),
        &target,
        &CancellationToken::new(),
    )
    .await;
    assert_eq!(identified.failed, [SourceId::Modrinth]);
}

// --- Links ------------------------------------------------------------------------------

/// Link do Modrinth: projeto e versão (pelo número ou pelo ID) resolvidos pela API.
#[tokio::test]
async fn link_do_modrinth_projeto_e_versao() {
    let server = modrinth_server("nenhum").await;
    let client = modrinth_client(&server);
    let cancel = CancellationToken::new();
    let LinkClass::Modrinth(project) = classify_link("https://modrinth.com/mod/sodium") else {
        panic!()
    };
    let target = resolve_modrinth_link(&client, &project, &cancel)
        .await
        .unwrap();
    assert_eq!(target.project_id, SODIUM);
    assert_eq!(target.title, "Sodium");
    assert_eq!(target.kind, ProjectKind::Mod);
    assert_eq!(target.version_id, None);

    let by_id = ModrinthLink::Version {
        project: "sodium".into(),
        version: SODIUM_VERSION.into(),
    };
    let target = resolve_modrinth_link(&client, &by_id, &cancel)
        .await
        .unwrap();
    assert_eq!(target.version_id.as_deref(), Some(SODIUM_VERSION));
    assert_eq!(target.choice().version_id.as_deref(), Some(SODIUM_VERSION));

    let number = modrinth_fixture(&format!("version-{SODIUM_VERSION}"))["version_number"]
        .as_str()
        .unwrap()
        .to_owned();
    let by_number = ModrinthLink::Version {
        project: "sodium".into(),
        version: number,
    };
    let target = resolve_modrinth_link(&client, &by_number, &cancel)
        .await
        .unwrap();
    assert_eq!(target.version_id.as_deref(), Some(SODIUM_VERSION));

    let missing = ModrinthLink::Version {
        project: "sodium".into(),
        version: "0.0.0-nao-existe".into(),
    };
    let error = resolve_modrinth_link(&client, &missing, &cancel)
        .await
        .unwrap_err();
    assert_eq!(error.code, warden_project::ProjectErrorCode::ItemNotFound);
}

/// CA-T08-07: `http://` é recusado sem nenhuma requisição (o servidor simulado não recebe
/// nada).
#[tokio::test]
async fn ca_t08_07_http_recusado_sem_download() {
    let server = MockServer::start().await;
    let link = format!("{}/mods/sodium.jar", server.uri());
    assert!(link.starts_with("http://"));
    assert_eq!(classify_link(&link), LinkClass::Insecure);
    assert!(server.received_requests().await.unwrap().is_empty());
}

/// Link direto: baixado uma vez (pelo servidor simulado), lido como o arquivo do computador e
/// gravado como referência (`[download]` com o link `https` e o `sha256`), sem copiar o jar.
#[tokio::test]
async fn link_direto_vira_referencia_com_sha256() {
    let temp = tempfile::tempdir().unwrap();
    let Some(cli) = packwiz(temp.path()) else {
        return;
    };
    let root = temp.path().join("pack");
    pack(&root, "1.20.1", Loader::Forge, &cli).await;
    let jar = jei_jar();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/files.example.com/mods/Just%20Enough%20Items.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(jar.clone()))
        .expect(1)
        .mount(&server)
        .await;
    let link = direct("https://files.example.com/mods/Just%20Enough%20Items.jar");
    let via = Url::parse(&format!("{}/", server.uri())).unwrap();
    let staging = temp.path().join("staging");
    let downloaded = download_direct(
        &http(),
        &link,
        &staging,
        Some(&via),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(downloaded.path, staging.join("Just Enough Items.jar"));
    assert_eq!(downloaded.hashes.sha256, FileFacts::of("x", &jar).sha256);

    let (bytes, facts) = read_file(&downloaded.path).unwrap();
    let report = inspect(&root, &bytes, &facts).unwrap();
    assert_eq!(report.mod_info.unwrap().id, "jei");
    let result = add_local_apply(
        &root,
        &[LocalAdd {
            path: downloaded.path.clone(),
            sha256: facts.sha256.clone(),
            kind: ProjectKind::Mod,
            link: Some(link.url().to_string()),
            name: String::new(),
            side: SideChoice::Client,
            accept_incompatible: false,
        }],
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.added[0].path, "mods/just-enough-items.pw.toml");
    let metafile =
        Metafile::parse(&fs::read_to_string(root.join("mods/just-enough-items.pw.toml")).unwrap())
            .unwrap()
            .value;
    assert_eq!(
        metafile.download.url,
        "https://files.example.com/mods/Just%20Enough%20Items.jar"
    );
    assert_eq!(metafile.download.hash_format, "sha256");
    assert_eq!(metafile.download.hash, facts.sha256);
    assert_eq!(metafile.filename, "Just Enough Items.jar");
    assert_eq!(metafile.side, warden_packwiz::Side::Client);
    assert!(!root.join("mods/Just Enough Items.jar").exists());
    assert_conforming(&root, &cli).await;
}

/// Link direto que falha (404) ou é cancelado: a pasta temporária some e nada fica no disco.
#[tokio::test]
async fn link_direto_cancelado_ou_com_erro_nao_deixa_nada() {
    let temp = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/files.example.com/lento.jar"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(jei_jar())
                .set_delay(Duration::from_secs(30)),
        )
        .mount(&server)
        .await;
    let via = Url::parse(&format!("{}/", server.uri())).unwrap();

    let staging = temp.path().join("nao-existe");
    let error = download_direct(
        &http(),
        &direct("https://files.example.com/sumiu.jar"),
        &staging,
        Some(&via),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            error,
            DirectError::Http(warden_http::Error::NotFound { .. })
        ),
        "{error:?}"
    );
    assert!(!staging.exists());

    let staging = temp.path().join("lento");
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        canceller.cancel();
    });
    let started = std::time::Instant::now();
    let error = download_direct(
        &http(),
        &direct("https://files.example.com/lento.jar"),
        &staging,
        Some(&via),
        &cancel,
    )
    .await
    .unwrap_err();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "o cancelamento demorou"
    );
    assert!(
        matches!(error, DirectError::Http(warden_http::Error::Cancelled)),
        "{error:?}"
    );
    assert!(!staging.exists());
}

// --- Rede (`cargo xtask test-network`) ----------------------------------------------------

/// O jar real do Sodium, baixado do Modrinth, é achado pelo hash no Modrinth (e, com a chave
/// do `.env`, consultada na CurseForge), e serve só para Fabric (CA-T08-04 e CA-T08-05 com
/// dados reais).
#[tokio::test]
#[ignore = "rede"]
async fn rede_jar_real_do_sodium_identificado_nas_duas_fontes() {
    let temp = tempfile::tempdir().unwrap();
    let modrinth = ModrinthClient::new(http(), None).unwrap();
    let version = modrinth
        .version(SODIUM_VERSION, None)
        .await
        .expect("versão do Sodium no Modrinth");
    let file = version.primary_file().expect("arquivo principal");
    // O link do CDN do Modrinth é link de versão (não direto): baixado aqui pela `warden-http`.
    let request = warden_http::DownloadRequest::new(
        Url::parse(&file.url).unwrap(),
        temp.path().join("download").join(&file.filename),
    );
    let downloaded = http()
        .download(&request, &warden_core::NoProgress, None)
        .await
        .unwrap();
    let (bytes, facts) = read_file(&downloaded.path).unwrap();
    assert_eq!(facts.sha512, file.hashes.sha512.to_ascii_lowercase());

    let mut fabric = PackManifest::new("Teste", "1.21.1");
    fabric.set_loader_version(Loader::Fabric, "0.16.0");
    let fabric = PackTarget::from_manifest(&fabric).unwrap();
    let key = std::env::var("CURSEFORGE_API_KEY").ok();
    let curseforge =
        key.map(|key| CurseforgeClient::new(http(), Some(SecretString::from(key))).unwrap());
    let identified = identify(
        HashLookup {
            modrinth: Some(&modrinth),
            curseforge: curseforge.as_ref(),
        },
        std::slice::from_ref(&facts),
        &fabric,
        &CancellationToken::new(),
    )
    .await;
    assert!(identified.failed.is_empty(), "{:?}", identified.failed);
    let modrinth_match = identified.matches[0]
        .iter()
        .find(|found| found.source == SourceId::Modrinth)
        .expect("achado no Modrinth");
    assert_eq!(modrinth_match.project_id, SODIUM);
    assert!(modrinth_match.compatible);
    // Na CurseForge (com a chave), o mesmo jar só pode ser o Sodium de lá; se o autor publicou
    // um arquivo diferente, não há achado (a procura não falhou: `failed` está vazio).
    for found in identified.matches[0]
        .iter()
        .filter(|found| found.source == SourceId::Curseforge)
    {
        assert_eq!(found.project_id, "394468", "{found:?}");
    }

    let root = temp.path().join("pack");
    fs::create_dir_all(&root).unwrap();
    let mut forge = PackManifest::new("Teste", "1.21.1");
    forge.set_loader_version(Loader::NeoForge, "21.1.0");
    fs::write(root.join("pack.toml"), forge.to_toml_string()).unwrap();
    fs::write(
        root.join("index.toml"),
        PackIndex::default().to_toml_string(),
    )
    .unwrap();
    let report = inspect(&root, &bytes, &facts).unwrap();
    assert!(
        matches!(
            report.incompatible.as_slice(),
            [warden_project::add::local::Incompatibility::Loader { .. }]
        ),
        "{:?}",
        report.incompatible
    );
}
