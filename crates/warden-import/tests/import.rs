//! Aceite da importação (P1-19; SPEC T24): CA-T24-01 a CA-T24-04 e a ida e volta com a
//! exportação da E-02, com o packwiz real (sidecar) e o Modrinth e a CurseForge simulados
//! (wiremock). Nenhum teste sai para a internet.
//!
//! O corpus está em `tests/corpus/` (origem de cada arquivo no `README.md` de lá); os jars e os
//! zips são montados aqui, numa pasta temporária, com bytes gerados.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(linker_messages, clippy::print_stderr, clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use sha1::Sha1;
use sha2::{Digest as _, Sha256, Sha512};
use warden_core::{CancellationToken, NoProgress};
use warden_curseforge::{CurseforgeClient, SecretString};
use warden_http::{HttpClient, HttpConfig, ManualTimer};
use warden_import::{
    CurseforgeTools, ImportErrorCode, ImportExcludedReason, ImportFormat, ImportItemSource,
    ImportLocalReason, ImportPlan, ImportRequest, ImportReviewReason, ImportServices, Stores,
    analyze, apply,
};
use warden_modrinth::ModrinthClient;
use warden_packwiz::{Metafile, PackIndex, PackManifest, Side, read_pack};
use warden_packwiz_cli::{Packwiz, RunContext, check_conformance};
use warden_project::registry::Registry;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const CORPUS: &str = "tests/corpus";

fn corpus(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(relative)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn sha1(bytes: &[u8]) -> String {
    hex(&Sha1::digest(bytes))
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn sha512(bytes: &[u8]) -> String {
    hex(&Sha512::digest(bytes))
}

fn sidecar() -> Option<PathBuf> {
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
            std::env::var("WARDEN_REQUIRE_EXTERNALS").ok().as_deref(),
            Some("1"),
            "WARDEN_REQUIRE_EXTERNALS=1, mas o packwiz não está em {}",
            path.display()
        );
        eprintln!("AVISO: packwiz não encontrado; parte do teste da importação pulada.");
        None
    }
}

fn http() -> HttpClient {
    HttpClient::with_timer(HttpConfig::for_version("1"), Arc::new(ManualTimer::new())).unwrap()
}

fn json_ok(value: Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_json(value)
}

fn write(root: &Path, relative: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Zip com as entradas dadas, na ordem.
fn zip_file(path: &Path, entries: &[(String, Vec<u8>)]) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in entries {
        writer
            .start_file(name.as_str(), zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}

/// Projeto do Modrinth no formato da API (o mínimo que o cliente lê).
fn project(id: &str, slug: &str, title: &str) -> Value {
    json!({
        "id": id, "slug": slug, "title": title, "project_type": "mod", "status": "approved",
        "client_side": "required", "server_side": "required"
    })
}

/// Versão do Modrinth com um arquivo.
fn version(
    id: &str,
    project: &str,
    filename: &str,
    bytes_sha1: &str,
    bytes_sha512: &str,
    environment: Option<&str>,
) -> Value {
    let mut value = json!({
        "id": id, "project_id": project, "version_number": "1.0.0", "version_type": "release",
        "files": [{
            "hashes": {"sha1": bytes_sha1, "sha512": bytes_sha512},
            "url": format!("https://cdn.modrinth.com/data/{project}/versions/{id}/{filename}"),
            "filename": filename, "primary": true, "size": 10
        }]
    });
    if let Some(environment) = environment {
        value["environment"] = json!(environment);
    }
    value
}

/// O Modrinth simulado: `projects` para `GET /v2/projects` e, para `POST /v2/version_files`,
/// o mapa hash → versão de `versions` (só os hashes pedidos).
async fn modrinth_mock(projects: Vec<Value>, versions: BTreeMap<String, Value>) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v2/projects"))
        .respond_with(json_ok(Value::Array(projects)))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(move |request: &Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let mut found = serde_json::Map::new();
            for hash in body["hashes"].as_array().into_iter().flatten() {
                let hash = hash.as_str().unwrap().to_owned();
                if let Some(version) = versions.get(&hash) {
                    found.insert(hash, version.clone());
                }
            }
            json_ok(Value::Object(found))
        })
        .mount(&server)
        .await;
    server
}

fn modrinth_client(server: &MockServer) -> ModrinthClient {
    ModrinthClient::with_base_url(http(), &format!("{}/v2", server.uri()), None).unwrap()
}

fn packwiz(binary: &Path, temp: &Path) -> Packwiz {
    Packwiz::new(
        binary.to_path_buf(),
        temp.join("dados/cache/packwiz"),
        temp.join("dados/packwiz/packwiz.toml"),
    )
}

/// Serviços sem CurseForge (sem chave).
fn services<'a>(modrinth: &'a ModrinthClient, tools: CurseforgeTools<'a>) -> ImportServices<'a> {
    ImportServices {
        stores: Stores {
            modrinth: Some(modrinth),
            curseforge: None,
        },
        curseforge: tools,
    }
}

fn request(temp: &Path, name: &str) -> ImportRequest {
    ImportRequest {
        name: name.to_owned(),
        destination: temp.join("packs").join("importado"),
        include: Vec::new(),
        take_preferences: false,
        scratch: temp.join("dados/cache/staging"),
    }
}

/// Todos os arquivos de uma pasta (caminho relativo → bytes), sem `.git/`.
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if relative == ".git" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                map.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    map
}

// ---------- CA-T24-01: o Create+ real ----------

/// O `.mrpack` do Create+ montado com o índice real recortado, o `xmcl.json` real e os
/// `overrides/` da lista do corpus (bytes gerados).
fn create_plus(dir: &Path) -> PathBuf {
    let mut entries = vec![
        (
            "modrinth.index.json".to_owned(),
            fs::read(corpus("create-plus/modrinth.index.json")).unwrap(),
        ),
        (
            "xmcl.json".to_owned(),
            fs::read(corpus("create-plus/xmcl.json")).unwrap(),
        ),
    ];
    for line in fs::read_to_string(corpus("create-plus/overrides.txt"))
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
    {
        entries.push((
            line.to_owned(),
            format!("conteúdo de {line}\n").into_bytes(),
        ));
    }
    let path = dir.join("Create+ 6.0.0 Alpha D.mrpack");
    zip_file(&path, &entries);
    path
}

fn create_plus_index() -> Value {
    serde_json::from_slice(&fs::read(corpus("create-plus/modrinth.index.json")).unwrap()).unwrap()
}

/// Projetos do Modrinth do índice recortado (nome e slug montados do caminho do arquivo).
fn create_plus_projects(index: &Value) -> Vec<Value> {
    index["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| {
            let url = file["downloads"][0].as_str().unwrap();
            let id = url.split('/').nth(4).unwrap();
            let name = file["path"]
                .as_str()
                .unwrap()
                .rsplit('/')
                .next()
                .unwrap()
                .split(['-', '_'])
                .next()
                .unwrap()
                .to_owned();
            project(id, &name.to_lowercase(), &format!("Projeto {name}"))
        })
        .collect()
}

#[tokio::test]
async fn ca_t24_01_mrpack_real_do_create_plus() {
    let temp = tempfile::tempdir().unwrap();
    let origin = create_plus(temp.path());
    let before = fs::read(&origin).unwrap();
    let index = create_plus_index();
    let server = modrinth_mock(create_plus_projects(&index), BTreeMap::new()).await;
    let modrinth = modrinth_client(&server);
    let binary = sidecar();
    let cli = binary.as_deref().map(|binary| packwiz(binary, temp.path()));
    let fake = Packwiz::new(
        PathBuf::from("nao-usado"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let tools = CurseforgeTools {
        packwiz: cli.as_ref().unwrap_or(&fake),
        key: None,
        staging_parent: &temp.path().join("dados/cache/staging"),
    };
    let plan = analyze(
        &origin,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let summary = &plan.summary;
    assert_eq!(summary.kind, ImportFormat::Mrpack);
    assert_eq!(summary.name, "Create+");
    assert_eq!(summary.version, "6.0.0 Alpha D");
    assert_eq!(summary.minecraft, "1.21.1");
    assert_eq!(summary.loader.as_deref(), Some("neoforge"));
    assert_eq!(summary.loader_version.as_deref(), Some("21.1.228"));

    // Cada arquivo do CDN do Modrinth vira referência, menos o jar de mods/.connector/temp/.
    let files = index["files"].as_array().unwrap();
    assert_eq!(files.len(), 9);
    assert_eq!(summary.references.len(), 8, "{:#?}", summary.references);
    assert!(
        summary
            .references
            .iter()
            .all(|item| item.source == ImportItemSource::Modrinth && !item.by_hash)
    );

    // O lixo não entra.
    let excluded: BTreeMap<&str, ImportExcludedReason> = summary
        .excluded
        .iter()
        .map(|item| (item.path.as_str(), item.reason))
        .collect();
    assert_eq!(excluded[".mixin.out"], ImportExcludedReason::Runtime);
    assert_eq!(excluded["mods/.connector"], ImportExcludedReason::Runtime);
    assert_eq!(excluded["xmcl.json"], ImportExcludedReason::OtherLauncher);
    assert_eq!(
        excluded["config/create-client-1.toml.bak"],
        ImportExcludedReason::Junk
    );
    assert_eq!(excluded["cache"], ImportExcludedReason::UnknownRoot);
    let connector = summary
        .excluded
        .iter()
        .find(|item| item.path == "mods/.connector")
        .unwrap();
    // Dois jars de overrides/ e o de files[].
    assert_eq!(connector.files, 3);

    // env desconhecido: Cliente e servidor, para revisar; opcional: marcado, para revisar.
    let review: BTreeMap<&str, ImportReviewReason> = summary
        .review
        .iter()
        .map(|item| (item.path.as_str(), item.reason))
        .collect();
    let unknown = summary
        .references
        .iter()
        .find(|item| item.name == "Projeto statuseffectbars")
        .unwrap();
    assert_eq!(
        review[unknown.path.as_str()],
        ImportReviewReason::UnknownSide
    );
    let optional = summary
        .references
        .iter()
        .find(|item| item.name == "Projeto appleskin")
        .unwrap();
    assert_eq!(review[optional.path.as_str()], ImportReviewReason::Optional);
    assert_eq!(review.len(), 2);

    // O jar solto não está em loja nenhuma (sem chave, só o Modrinth foi consultado).
    assert_eq!(summary.local_files.len(), 1);
    assert_eq!(
        summary.local_files[0].path,
        "mods/sablephysicscompat-1.3.0.jar"
    );
    assert_eq!(
        summary.local_files[0].reason,
        ImportLocalReason::NoCurseforgeKey
    );

    let Some(cli) = cli else { return };
    let registry = Registry::open(temp.path().join("dados/packs.json")).unwrap();
    // Item desconhecido na raiz: fica de fora, mas a pessoa pode marcar para entrar.
    let defaults = summary
        .excluded
        .iter()
        .find(|item| item.path == "configureddefaults")
        .unwrap();
    assert!(defaults.can_include);
    let mut wanted = request(temp.path(), "Create+ importado");
    wanted.include = vec!["configureddefaults".to_owned()];
    let created = apply(
        &plan,
        &wanted,
        &registry,
        &cli,
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    plan.discard();
    let root = created.path.clone();
    assert_eq!(registry.get(created.id).unwrap().name, "Create+ importado");

    // Metafiles com [update.modrinth], lado pelo env.
    let pack = read_pack(&root).unwrap();
    let index_files: Vec<String> = pack
        .index
        .as_ref()
        .unwrap()
        .value
        .normalized_entries()
        .into_iter()
        .map(|entry| entry.file)
        .collect();
    for item in &summary.references {
        let text = fs::read_to_string(root.join(&item.path)).unwrap();
        let metafile = Metafile::parse(&text).unwrap().value;
        assert!(metafile.modrinth_ids().is_some(), "{}", item.path);
        assert!(index_files.contains(&item.path));
    }
    let unknown = Metafile::parse(&fs::read_to_string(root.join(&unknown.path)).unwrap())
        .unwrap()
        .value;
    assert_eq!(unknown.side, Side::Both);
    let async_particles = summary
        .references
        .iter()
        .find(|item| item.name == "Projeto AsyncParticles")
        .unwrap();
    let client = Metafile::parse(&fs::read_to_string(root.join(&async_particles.path)).unwrap())
        .unwrap()
        .value;
    assert_eq!(client.side, Side::Client);
    assert_eq!(client.filename, "AsyncParticles-21.1.0a-alpha.2+1.21.1.jar");
    let optional = Metafile::parse(&fs::read_to_string(root.join(&optional.path)).unwrap())
        .unwrap()
        .value;
    let option = optional.option.unwrap();
    assert!(option.optional && option.default);

    // Nada de lixo na pasta nem no índice.
    let on_disk = snapshot(&root);
    for junk in [
        ".mixin.out/",
        "mods/.connector/",
        "xmcl.json",
        "cache/",
        ".bak",
    ] {
        assert!(
            !on_disk.keys().any(|path| path.contains(junk)),
            "{junk} entrou no pack: {:?}",
            on_disk.keys().collect::<Vec<_>>()
        );
    }
    assert!(on_disk.contains_key("config/create-common.toml"));
    assert!(on_disk.contains_key("configureddefaults/options.txt"));
    assert!(on_disk.contains_key("mods/sablephysicscompat-1.3.0.jar"));
    // Gancho 1.1: o jar local leva a origem da importação.
    let project = fs::read_to_string(root.join(".warden/project.toml")).unwrap();
    assert!(project.contains("[import]"), "{project}");
    assert!(project.contains("source = \"mrpack\""));
    assert!(project.contains("mods/sablephysicscompat-1.3.0.jar"));
    assert!(project.contains(&sha256(
        "conteúdo de overrides/mods/sablephysicscompat-1.3.0.jar\n".as_bytes()
    )));

    // O refresh seguinte não muda nada.
    let report = check_conformance(
        &cli,
        &root,
        &temp.path().join("conferencia"),
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    assert!(report.is_conforming(), "{report:?}");
    let after = snapshot(&root);
    cli.refresh(&root, false, RunContext::new(&CancellationToken::new()))
        .await
        .unwrap();
    assert_eq!(after, snapshot(&root));

    // A origem nunca muda; não sobra pasta temporária.
    assert_eq!(fs::read(&origin).unwrap(), before);
    let leftovers: Vec<_> = fs::read_dir(root.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers, vec!["importado".to_owned()]);
}

// ---------- CA-T24-02: caminhos perigosos ----------

#[tokio::test]
async fn ca_t24_02_caminhos_perigosos_sao_recusados_sem_criar_nada() {
    let server = modrinth_mock(Vec::new(), BTreeMap::new()).await;
    let modrinth = modrinth_client(&server);
    let temp = tempfile::tempdir().unwrap();
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let staging = temp.path().join("staging");
    let index = |path: &str| {
        json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Mau",
            "dependencies": {"minecraft": "1.21.1"},
            "files": [{
                "path": path, "hashes": {"sha1": "a".repeat(40), "sha512": "b".repeat(128)},
                "downloads": ["https://cdn.modrinth.com/data/A/versions/B/x.jar"], "fileSize": 1
            }]
        })
    };
    let cases = [
        ("../../fora.jar", "parent"),
        ("mods/../../../fora.jar", "parent"),
        ("/etc/cron.d/x", "absolute"),
        ("C:/Windows/System32/x.dll", "drive"),
        ("c:x.jar", "drive"),
        ("mods\\..\\x.jar", "backslash"),
    ];
    for (position, (bad, reason)) in cases.iter().enumerate() {
        let origin = temp.path().join(format!("mau-{position}.mrpack"));
        zip_file(
            &origin,
            &[(
                "modrinth.index.json".to_owned(),
                serde_json::to_vec(&index(bad)).unwrap(),
            )],
        );
        let tools = CurseforgeTools {
            packwiz: &fake,
            key: None,
            staging_parent: &staging,
        };
        let error = analyze(
            &origin,
            &services(&modrinth, tools),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), ImportErrorCode::UnsafePath, "{bad}");
        assert_eq!(error.params()["path"], *bad);
        assert_eq!(error.params()["reason"], *reason);
    }
    // Entrada do próprio zip com `..` (zip slip).
    let origin = temp.path().join("slip.mrpack");
    zip_file(
        &origin,
        &[
            (
                "modrinth.index.json".to_owned(),
                serde_json::to_vec(&index("mods/a.jar")).unwrap(),
            ),
            ("overrides/../../fora.txt".to_owned(), b"x".to_vec()),
        ],
    );
    let tools = CurseforgeTools {
        packwiz: &fake,
        key: None,
        staging_parent: &staging,
    };
    let error = analyze(
        &origin,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ImportErrorCode::UnsafePath);
    // Nenhuma requisição e nada criado além dos próprios arquivos de teste.
    assert!(server.received_requests().await.unwrap().is_empty());
    assert!(!staging.exists());
    assert!(!temp.path().join("fora.txt").exists());
    let created: Vec<_> = fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| !name.ends_with(".mrpack"))
        .collect();
    assert!(created.is_empty(), "{created:?}");
}

// ---------- CA-T24-04: instância do Prism ----------

const SOLTO: &[u8] = b"jar solto achado no Modrinth pelo hash";
const CASEIRO: &[u8] = b"jar feito em casa, sem loja";

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A instância do corpus com os jars, mundos, logs e caches montados aqui.
fn prism_instance(dir: &Path) -> PathBuf {
    let root = dir.join("Vale Sereno");
    copy_dir(&corpus("prism-instancia"), &root);
    let game = root.join(".minecraft");
    write(
        &game,
        "mods/sodium-fabric-0.8.13+mc1.21.1.jar",
        b"o jar do sodium",
    );
    write(&game, "mods/solto-1.0.jar", SOLTO);
    write(&game, "mods/caseiro.jar", CASEIRO);
    write(&game, "servers.dat", [10u8, 0, 0, 0]);
    write(&game, "saves/Mundo/level.dat", [1u8, 2, 3]);
    write(&game, "saves/Mundo/region/r.0.0.mca", [0u8; 16]);
    write(&game, "logs/latest.log", "[main] log do jogo\n");
    write(&game, "crash-reports/crash-1.txt", "travou\n");
    write(&game, ".cache/x", "cache\n");
    write(&game, "screenshots/a.png", [0u8; 4]);
    write(&root, "icon.png", [0u8; 4]);
    root
}

#[tokio::test]
async fn ca_t24_04_instancia_do_prism() {
    let temp = tempfile::tempdir().unwrap();
    let origin = prism_instance(temp.path());
    let versions = BTreeMap::from([(
        sha512(SOLTO),
        version(
            "SOLTOVER",
            "SOLTOPRJ",
            "solto-1.0.jar",
            &sha1(SOLTO),
            &sha512(SOLTO),
            Some("client_and_server"),
        ),
    )]);
    let server = modrinth_mock(vec![project("SOLTOPRJ", "solto", "Mod Solto")], versions).await;
    let modrinth = modrinth_client(&server);
    let binary = sidecar();
    let cli = binary.as_deref().map(|binary| packwiz(binary, temp.path()));
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let tools = CurseforgeTools {
        packwiz: cli.as_ref().unwrap_or(&fake),
        key: None,
        staging_parent: &temp.path().join("staging"),
    };
    let plan: ImportPlan = analyze(
        &origin,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let summary = &plan.summary;
    assert_eq!(summary.kind, ImportFormat::Prism);
    assert_eq!(summary.name, "Vale Sereno no Prism");
    assert_eq!(summary.minecraft, "1.21.1");
    assert_eq!(summary.loader.as_deref(), Some("fabric"));
    assert_eq!(summary.loader_version.as_deref(), Some("0.16.5"));
    let profile = summary.profile.clone().unwrap();
    assert_eq!(profile.memory_mb, Some(6144));
    assert_eq!(profile.jvm_args, "-XX:+UseG1GC -XX:MaxGCPauseMillis=50");
    assert_eq!(
        summary.preferences,
        vec!["options.txt".to_owned(), "servers.dat".to_owned()]
    );

    let refs: BTreeMap<&str, (&str, bool)> = summary
        .references
        .iter()
        .map(|item| (item.path.as_str(), (item.name.as_str(), item.by_hash)))
        .collect();
    assert_eq!(refs["mods/sodium.pw.toml"], ("Sodium", false));
    assert_eq!(refs["mods/solto.pw.toml"], ("Mod Solto", true));
    assert_eq!(summary.local_files.len(), 1);
    assert_eq!(summary.local_files[0].path, "mods/caseiro.jar");
    assert_eq!(
        summary.local_files[0].reason,
        ImportLocalReason::NoCurseforgeKey
    );
    let excluded: Vec<&str> = summary
        .excluded
        .iter()
        .map(|item| item.path.as_str())
        .collect();
    for path in [
        "saves",
        "logs",
        "crash-reports",
        ".cache",
        "screenshots",
        "instance.cfg",
        "mmc-pack.json",
        "icon.png",
    ] {
        assert!(
            excluded.contains(&path),
            "{path} deveria ficar de fora: {excluded:?}"
        );
    }

    let Some(cli) = cli else { return };
    let registry = Registry::open(temp.path().join("dados/packs.json")).unwrap();
    let created = apply(
        &plan,
        &request(temp.path(), "Vale Sereno"),
        &registry,
        &cli,
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let root = created.path;
    // Metadados do Prism reaproveitados, sem os campos x-prismlauncher-*.
    let sodium = fs::read_to_string(root.join("mods/sodium.pw.toml")).unwrap();
    assert!(!sodium.contains("x-prismlauncher"), "{sodium}");
    let parsed = Metafile::parse(&sodium).unwrap();
    assert!(parsed.unknown_keys.is_empty());
    assert_eq!(parsed.value.side, Side::Client);
    assert_eq!(parsed.value.modrinth_ids(), Some(("AANobbMI", "SMxNOGZ6")));
    let solto = Metafile::parse(&fs::read_to_string(root.join("mods/solto.pw.toml")).unwrap())
        .unwrap()
        .value;
    assert_eq!(solto.modrinth_ids(), Some(("SOLTOPRJ", "SOLTOVER")));
    let files = snapshot(&root);
    assert!(files.contains_key("mods/caseiro.jar"));
    assert!(files.contains_key("config/sodium-options.json"));
    // Os jars com metadados não são copiados; mundos, logs, caches e preferências não entram.
    for path in files.keys() {
        assert!(
            ![
                "saves/",
                "logs/",
                "crash-reports/",
                ".cache/",
                "screenshots/",
                "sodium-fabric",
                "solto-1.0.jar",
                "instance.cfg",
                "options.txt",
                "servers.dat"
            ]
            .iter()
            .any(|junk| path.contains(junk)),
            "{path} entrou no pack"
        );
    }
    let manifest = PackManifest::parse(&fs::read_to_string(root.join("pack.toml")).unwrap())
        .unwrap()
        .value;
    assert_eq!(manifest.name, "Vale Sereno");
    assert_eq!(manifest.loaders().first().map(|(_, v)| *v), Some("0.16.5"));
    let report = check_conformance(
        &cli,
        &root,
        &temp.path().join("conf"),
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    assert!(report.is_conforming(), "{report:?}");
}

#[tokio::test]
async fn preferencias_entram_so_quando_a_pessoa_quer() {
    let Some(binary) = sidecar() else { return };
    let temp = tempfile::tempdir().unwrap();
    let origin = prism_instance(temp.path());
    let server = modrinth_mock(Vec::new(), BTreeMap::new()).await;
    let modrinth = modrinth_client(&server);
    let cli = packwiz(&binary, temp.path());
    let tools = CurseforgeTools {
        packwiz: &cli,
        key: None,
        staging_parent: &temp.path().join("staging"),
    };
    let plan = analyze(
        &origin,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let registry = Registry::open(temp.path().join("dados/packs.json")).unwrap();
    let mut wanted = request(temp.path(), "Com preferências");
    wanted.take_preferences = true;
    let created = apply(
        &plan,
        &wanted,
        &registry,
        &cli,
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        fs::read_to_string(created.path.join("options.txt")).unwrap(),
        "version:3955\nautoJump:false\nlang:pt_br\n"
    );
    assert!(created.path.join("servers.dat").is_file());
}

// ---------- Cancelar e falhar no meio ----------

#[tokio::test]
async fn cancelar_no_meio_nao_deixa_pack_pela_metade() {
    let Some(binary) = sidecar() else { return };
    let temp = tempfile::tempdir().unwrap();
    let origin = create_plus(temp.path());
    let index = create_plus_index();
    let server = modrinth_mock(create_plus_projects(&index), BTreeMap::new()).await;
    let modrinth = modrinth_client(&server);
    let cli = packwiz(&binary, temp.path());
    let tools = CurseforgeTools {
        packwiz: &cli,
        key: None,
        staging_parent: &temp.path().join("staging"),
    };
    let plan = analyze(
        &origin,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let registry = Registry::open(temp.path().join("dados/packs.json")).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let error = apply(
        &plan,
        &request(temp.path(), "X"),
        &registry,
        &cli,
        &NoProgress,
        &cancel,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ImportErrorCode::Internal);
    let packs = temp.path().join("packs");
    let left: Vec<_> = fs::read_dir(&packs)
        .map(|dir| dir.map(|e| e.unwrap().file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
    assert!(registry.records().is_empty());

    // A origem mudou depois da revisão: recusado, nada gravado.
    let mut bytes = fs::read(&origin).unwrap();
    bytes.extend_from_slice(b"mais");
    fs::write(&origin, bytes).unwrap();
    let error = apply(
        &plan,
        &request(temp.path(), "X"),
        &registry,
        &cli,
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ImportErrorCode::SourceChanged);
    assert!(registry.records().is_empty());
}

// ---------- Ida e volta com a exportação da E-02 ----------

const CDN_JAR: &[u8] = b"jar do mod hospedado no cdn do modrinth";
const LOCAL_JAR: &[u8] = b"jar local do usuario";

#[tokio::test]
async fn importar_o_que_o_warden_exporta_devolve_o_mesmo_pack() {
    let Some(binary) = sidecar() else { return };
    let temp = tempfile::tempdir().unwrap();
    let cli = packwiz(&binary, temp.path());
    let cancel = CancellationToken::new();

    // Servidor local dos jars, para encher o cache do packwiz sem sair da máquina (a mesma
    // técnica dos testes da E-02).
    let files = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/files/cdn.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(CDN_JAR.to_vec()))
        .mount(&files)
        .await;
    let mut manifest = PackManifest::new("Pack de ida e volta", "1.21.1");
    manifest.set_loader_version(warden_packwiz::Loader::Fabric, "0.16.5");
    manifest.version = "1.2.0".to_owned();
    manifest.description = "Um pack para testar".to_owned();
    let seed = temp.path().join("semente");
    write(&seed, "pack.toml", manifest.to_toml_string());
    write(&seed, "index.toml", PackIndex::default().to_toml_string());
    let mut meta = Metafile::url(
        "cdn",
        &format!("{}/files/cdn.jar", files.uri()),
        &sha256(CDN_JAR),
    )
    .unwrap();
    meta.side = Side::Both;
    write(&seed, "mods/cdn.pw.toml", meta.to_toml_string());
    cli.refresh(&seed, false, RunContext::new(&cancel))
        .await
        .unwrap();
    cli.modrinth_export(
        &seed,
        &temp.path().join("semente.mrpack"),
        None,
        RunContext::new(&cancel),
    )
    .await
    .unwrap();

    // O pack original, como o Warden grava.
    let original = temp.path().join("original");
    write(&original, "pack.toml", manifest.to_toml_string());
    write(
        &original,
        "index.toml",
        PackIndex::default().to_toml_string(),
    );
    let cdn = Metafile::modrinth(
        &warden_packwiz::ModrinthFile {
            title: "Mod do CDN".into(),
            project_id: "CDNPRJ".into(),
            version_id: "CDNVER".into(),
            filename: "cdn.jar".into(),
            url: "https://cdn.modrinth.com/data/CDNPRJ/versions/CDNVER/cdn.jar".into(),
            hash_format: warden_packwiz::HashFormat::Sha512,
            hash: sha512(CDN_JAR),
        },
        Side::Client,
    );
    write(&original, "mods/cdn.pw.toml", cdn.to_toml_string());
    write(&original, "mods/local.jar", LOCAL_JAR);
    write(&original, "config/ajuste.toml", "valor = 1\n");
    write(
        &original,
        ".packwizignore",
        warden_packwiz::hygiene::packwizignore_template(),
    );
    cli.refresh(&original, false, RunContext::new(&cancel))
        .await
        .unwrap();

    // Exportação da E-02 (.mrpack).
    let server = modrinth_mock(
        vec![project("CDNPRJ", "cdn", "Mod do CDN")],
        BTreeMap::new(),
    )
    .await;
    let modrinth = modrinth_client(&server);
    let cf_server = MockServer::start().await;
    let curseforge = CurseforgeClient::with_base_url(
        http(),
        &format!("{}/v1", cf_server.uri()),
        Some(SecretString::from("chave-de-teste")),
    )
    .unwrap();
    let exported = temp.path().join("exportado.mrpack");
    warden_export::export_format(
        &warden_export::FormatRequest {
            root: &original,
            source: &warden_export::ExportSource::Current,
            format: warden_export::LauncherFormat::Mrpack,
            choices: &warden_export::FormatChoices {
                swap: Vec::new(),
                confirm_embed: true,
                version: None,
            },
            destination: &exported,
            staging_parent: &temp.path().join("staging-exportacao"),
            packwiz: &cli,
            key: None,
            lookups: warden_export::Lookups {
                modrinth: &modrinth,
                curseforge: Some(&curseforge),
            },
        },
        &cancel,
    )
    .await
    .unwrap();

    // Importar de volta.
    let tools = CurseforgeTools {
        packwiz: &cli,
        key: None,
        staging_parent: &temp.path().join("staging"),
    };
    let plan = analyze(&exported, &services(&modrinth, tools), &cancel)
        .await
        .unwrap();
    assert!(plan.summary.review.is_empty(), "{:#?}", plan.summary.review);
    let registry = Registry::open(temp.path().join("dados/packs.json")).unwrap();
    let mut wanted = request(temp.path(), "Pack de ida e volta");
    wanted.destination = temp.path().join("de-volta");
    let created = apply(&plan, &wanted, &registry, &cli, &NoProgress, &cancel)
        .await
        .unwrap();

    // O mesmo conteúdo: metafiles, arquivos e o pack.toml (sem o hash do
    // índice, que depende dos arquivos de controle de cada um).
    let content = |root: &Path| {
        let mut files = snapshot(root);
        files.retain(|path, _| {
            !path.starts_with(".warden/")
                && !matches!(
                    path.as_str(),
                    "pack.toml" | "index.toml" | ".gitignore" | ".gitattributes" | "CHANGELOG.md"
                )
        });
        files
    };
    let back = content(&created.path);
    let mut expected = content(&original);
    // O original não tinha os arquivos de controle que o Warden cria; o importado tem.
    expected.insert(
        ".packwizignore".to_owned(),
        warden_packwiz::hygiene::packwizignore_template().into_bytes(),
    );
    assert_eq!(
        back.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>()
    );
    assert_eq!(back, expected);
    let index_of = |root: &Path| {
        let mut entries: Vec<(String, String)> = read_pack(root)
            .unwrap()
            .index
            .unwrap()
            .value
            .normalized_entries()
            .into_iter()
            .map(|entry| (entry.file, entry.hash))
            .collect();
        entries.sort();
        entries
    };
    assert_eq!(index_of(&created.path), index_of(&original));
    let back_manifest = read_pack(&created.path).unwrap().pack.value;
    assert_eq!(back_manifest.name, manifest.name);
    assert_eq!(back_manifest.version, manifest.version);
    assert_eq!(back_manifest.description, manifest.description);
    assert_eq!(back_manifest.versions, manifest.versions);
}

// ---------- CA-T24-03: zip da CurseForge ----------

const KEY: &str = "$2a$10$chaveFalsaDoWardenParaTestes0123456789abcdefghijkl";
const CF_JEI: u64 = 238_222;
const JEI_FILE: u64 = 4_712_345;
const CF_OPTIONAL: u64 = 900_001;
const OPTIONAL_FILE: u64 = 9_000_010;
const JEI_SHA1: &str = "abcdef0123456789abcdef0123456789abcdef01";
const OPTIONAL_SHA1: &str = "1111111111111111111111111111111111111111";

fn cf_project(id: u64, slug: &str, name: &str) -> Value {
    json!({
        "id": id, "gameId": 432, "name": name, "slug": slug,
        "links": {"websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}")},
        "summary": "", "status": 4, "downloadCount": 1, "classId": 6, "primaryCategoryId": 423,
        "authors": [{"id": 1, "name": "Autora"}], "allowModDistribution": true, "isAvailable": true,
        "dateCreated": "2020-01-01T00:00:00Z", "dateModified": "2026-10-01T00:00:00Z"
    })
}

fn cf_file(mod_id: u64, id: u64, name: &str, sha1: &str) -> Value {
    json!({
        "id": id, "gameId": 432, "modId": mod_id, "isAvailable": true, "displayName": name,
        "fileName": name, "releaseType": 1, "fileStatus": 4,
        "hashes": [{"value": sha1, "algo": 1}], "fileDate": "2026-09-19T11:47:57.347Z",
        "fileLength": 1000, "downloadCount": 7,
        "downloadUrl": format!("https://edge.forgecdn.net/files/{}/{}/{name}", id / 1000, id % 1000),
        "gameVersions": ["1.20.1", "Forge"], "sortableGameVersions": [], "dependencies": [],
        "isServerPack": false, "fileFingerprint": id
    })
}

async fn curseforge_mock() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(json_ok(json!({"data": [
            cf_project(CF_JEI, "jei", "Just Enough Items (JEI)"),
            cf_project(CF_OPTIONAL, "mod-opcional", "Mod Opcional"),
        ]})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(json_ok(json!({"data": [
            cf_file(CF_JEI, JEI_FILE, "jei-1.20.1-forge-15.20.0.106.jar", JEI_SHA1),
            cf_file(CF_OPTIONAL, OPTIONAL_FILE, "mod-opcional-1.0.jar", OPTIONAL_SHA1),
        ]})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/fingerprints/432"))
        .respond_with(json_ok(json!({"data": {
            "isCacheBuilt": true, "exactMatches": [], "exactFingerprints": [],
            "partialMatches": [], "partialMatchFingerprints": {}, "installedFingerprints": [],
            "unmatchedFingerprints": null
        }})))
        .mount(&server)
        .await;
    server
}

fn curseforge_zip(dir: &Path) -> PathBuf {
    let path = dir.join("Cozy Create 2.3.0.zip");
    zip_file(
        &path,
        &[
            (
                "manifest.json".to_owned(),
                fs::read(corpus("curseforge-zip/manifest.json")).unwrap(),
            ),
            ("modlist.html".to_owned(), b"<ul></ul>".to_vec()),
            (
                "overrides/config/jei/jei-client.ini".to_owned(),
                b"[a]\nb=1\n".to_vec(),
            ),
            (
                "overrides/mods/extra.jar".to_owned(),
                b"jar extra do modpack".to_vec(),
            ),
            ("overrides/logs/latest.log".to_owned(), b"log\n".to_vec()),
        ],
    );
    path
}

#[tokio::test]
async fn ca_t24_03_zip_da_curseforge_sem_chave_nao_faz_requisicao() {
    let temp = tempfile::tempdir().unwrap();
    let origin = curseforge_zip(temp.path());
    let modrinth_server = modrinth_mock(Vec::new(), BTreeMap::new()).await;
    let modrinth = modrinth_client(&modrinth_server);
    let cf_server = curseforge_mock().await;
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let staging = temp.path().join("staging");
    let tools = CurseforgeTools {
        packwiz: &fake,
        key: None,
        staging_parent: &staging,
    };
    let error = analyze(
        &origin,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ImportErrorCode::CurseforgeKeyMissing);
    assert!(cf_server.received_requests().await.unwrap().is_empty());
    assert!(
        modrinth_server
            .received_requests()
            .await
            .unwrap()
            .is_empty()
    );
    assert!(!staging.exists());
}

#[tokio::test]
async fn ca_t24_03_zip_da_curseforge_pelo_packwiz_com_o_lado_do_modrinth() {
    let Some(binary) = sidecar() else { return };
    let temp = tempfile::tempdir().unwrap();
    let origin = curseforge_zip(temp.path());
    let before = fs::read(&origin).unwrap();
    // O Modrinth conhece o JEI pelo SHA-1, só de cliente (para o lado mudar).
    let versions = BTreeMap::from([(
        JEI_SHA1.to_owned(),
        version(
            "JEIVER",
            "JEIPRJ",
            "jei.jar",
            JEI_SHA1,
            &"c".repeat(128),
            Some("client_only"),
        ),
    )]);
    let modrinth_server = modrinth_mock(vec![project("JEIPRJ", "jei", "JEI")], versions).await;
    let modrinth = modrinth_client(&modrinth_server);
    let cf_server = curseforge_mock().await;
    let curseforge = CurseforgeClient::with_base_url(
        http(),
        &format!("{}/v1", cf_server.uri()),
        Some(SecretString::from(KEY.to_owned())),
    )
    .unwrap();
    let cli = packwiz(&binary, temp.path()).with_curseforge_api(format!("{}/v1", cf_server.uri()));
    let key = SecretString::from(KEY.to_owned());
    let staging = temp.path().join("dados/cache/staging");
    let services = ImportServices {
        stores: Stores {
            modrinth: Some(&modrinth),
            curseforge: Some(&curseforge),
        },
        curseforge: CurseforgeTools {
            packwiz: &cli,
            key: Some(&key),
            staging_parent: &staging,
        },
    };
    let plan = analyze(&origin, &services, &CancellationToken::new())
        .await
        .unwrap();
    let summary = &plan.summary;
    assert_eq!(summary.kind, ImportFormat::CurseforgeZip);
    assert_eq!(summary.name, "Cozy Create");
    assert_eq!(summary.loader.as_deref(), Some("forge"));
    assert_eq!(summary.loader_version.as_deref(), Some("47.3.0"));
    assert_eq!(summary.profile.as_ref().unwrap().memory_mb, Some(6144));
    let refs: BTreeMap<&str, ImportItemSource> = summary
        .references
        .iter()
        .map(|item| (item.path.as_str(), item.source))
        .collect();
    assert_eq!(
        refs.get("mods/jei.pw.toml"),
        Some(&ImportItemSource::Curseforge)
    );
    assert_eq!(
        refs.get("mods/mod-opcional.pw.toml"),
        Some(&ImportItemSource::Curseforge)
    );
    assert_eq!(summary.review.len(), 1);
    assert_eq!(summary.review[0].path, "mods/mod-opcional.pw.toml");
    assert_eq!(
        summary.review[0].reason,
        ImportReviewReason::OptionalDisabled
    );
    assert_eq!(summary.local_files.len(), 1);
    assert_eq!(summary.local_files[0].reason, ImportLocalReason::NotFound);
    let excluded: Vec<&str> = summary
        .excluded
        .iter()
        .map(|item| item.path.as_str())
        .collect();
    assert!(excluded.contains(&"logs"), "{excluded:?}");
    assert!(excluded.contains(&"modlist.html"), "{excluded:?}");
    // O packwiz rodou numa cópia de staging, nunca no destino.
    let requests = cf_server.received_requests().await.unwrap();
    assert!(requests.iter().any(|r| r.url.path() == "/v1/mods/files"));
    assert_eq!(staging.read_dir().unwrap().count(), 1);

    let registry = Registry::open(temp.path().join("dados/packs.json")).unwrap();
    let created = apply(
        &plan,
        &request(temp.path(), "Cozy Create"),
        &registry,
        &cli,
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    plan.discard();
    assert_eq!(staging.read_dir().unwrap().count(), 0);
    let jei = Metafile::parse(&fs::read_to_string(created.path.join("mods/jei.pw.toml")).unwrap())
        .unwrap()
        .value;
    assert_eq!(jei.side, Side::Client, "lado corrigido pelo Modrinth");
    assert_eq!(jei.curseforge_ids(), Some((238_222, 4_712_345)));
    let optional = Metafile::parse(
        &fs::read_to_string(created.path.join("mods/mod-opcional.pw.toml")).unwrap(),
    )
    .unwrap()
    .value;
    assert_eq!(optional.side, Side::Both);
    let option = optional.option.unwrap();
    assert!(option.optional && !option.default);
    assert!(created.path.join("config/jei/jei-client.ini").is_file());
    assert!(!created.path.join("logs").exists());
    assert_eq!(fs::read(&origin).unwrap(), before);
}

// ---------- Instância do app da CurseForge ----------

#[tokio::test]
async fn instancia_do_app_da_curseforge_sem_consultar_a_curseforge() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Cozy Create");
    write(
        &root,
        "minecraftinstance.json",
        serde_json::to_vec(&json!({
            "name": "Cozy Create", "gameVersion": "1.20.1",
            "baseModLoader": {"name": "forge-47.3.0", "minecraftVersion": "1.20.1"},
            "allocatedMemory": 8192, "javaArgsOverride": "-XX:+UseZGC",
            "installedAddons": [{
                "addonID": CF_JEI, "name": "Just Enough Items (JEI)",
                "installedFile": {
                    "id": JEI_FILE, "fileName": "jei-1.20.1-forge-15.20.0.106.jar",
                    "hashes": [{"type": 1, "value": JEI_SHA1}, {"type": 2, "value": "md5"}]
                }
            }]
        }))
        .unwrap(),
    );
    write(
        &root,
        "mods/jei-1.20.1-forge-15.20.0.106.jar",
        b"jar do jei",
    );
    write(&root, "config/jei/jei-client.ini", "[a]\n");
    write(&root, "options.txt", "lang:pt_br\n");
    write(&root, "saves/Mundo/level.dat", [1u8]);
    write(&root, ".curseclient", "x");
    let modrinth_server = modrinth_mock(Vec::new(), BTreeMap::new()).await;
    let modrinth = modrinth_client(&modrinth_server);
    let cf_server = curseforge_mock().await;
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let tools = CurseforgeTools {
        packwiz: &fake,
        key: None,
        staging_parent: &temp.path().join("staging"),
    };
    let plan = analyze(
        &root,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let summary = &plan.summary;
    assert_eq!(summary.kind, ImportFormat::CurseforgeApp);
    assert_eq!(summary.loader_version.as_deref(), Some("47.3.0"));
    assert_eq!(summary.references.len(), 1);
    assert_eq!(summary.references[0].path, "mods/just-enough-items.pw.toml");
    assert_eq!(summary.references[0].source, ImportItemSource::Curseforge);
    assert!(summary.local_files.is_empty());
    assert_eq!(summary.preferences, vec!["options.txt".to_owned()]);
    let profile = summary.profile.clone().unwrap();
    assert_eq!(profile.memory_mb, Some(8192));
    assert_eq!(profile.jvm_args, "-XX:+UseZGC");
    let excluded: Vec<&str> = summary
        .excluded
        .iter()
        .map(|item| item.path.as_str())
        .collect();
    for path in ["saves", "minecraftinstance.json", ".curseclient"] {
        assert!(excluded.contains(&path), "{path}: {excluded:?}");
    }
    assert!(cf_server.received_requests().await.unwrap().is_empty());
}

// ---------- Outros hosts, CDN fora do padrão, CDN da CurseForge, licença e falhas ----------

const CF_LOOSE: &[u8] = b"jar solto que so a CurseForge conhece";

#[tokio::test]
async fn mrpack_com_outros_hosts_curseforge_por_impressao_e_licenca() {
    let temp = tempfile::tempdir().unwrap();
    let odd_sha1 = sha1(b"cdn fora do padrao");
    let lost_sha1 = sha1(b"cdn sem versao");
    let jei_sha1 = JEI_SHA1.to_owned();
    let index = json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "3.1", "name": "Hosts",
        "dependencies": {"minecraft": "1.20.1", "forge": "47.3.0"},
        "files": [
            {"path": "mods/outro.jar", "hashes": {"sha512": "d".repeat(128)},
             "downloads": ["https://files.example.com/outro.jar"], "env": {"client": "required", "server": "required"}},
            {"path": "mods/estranho.jar", "hashes": {"sha1": odd_sha1, "sha512": "e".repeat(128)},
             "downloads": ["https://cdn.modrinth.com/outro/caminho/estranho.jar"]},
            {"path": "mods/perdido.jar", "hashes": {"sha1": lost_sha1},
             "downloads": ["https://cdn.modrinth.com/coisa/perdido.jar"]},
            {"path": "mods/jei.jar", "hashes": {"sha1": jei_sha1, "sha512": "f".repeat(128)},
             "downloads": ["https://edge.forgecdn.net/files/4712/345/jei.jar"],
             "env": {"client": "optional", "server": "unsupported"}},
        ]
    });
    let origin = temp.path().join("hosts.mrpack");
    zip_file(
        &origin,
        &[
            (
                "modrinth.index.json".to_owned(),
                serde_json::to_vec(&index).unwrap(),
            ),
            ("overrides/mods/cf-solto.jar".to_owned(), CF_LOOSE.to_vec()),
        ],
    );
    let pack_sha512 = sha512(&fs::read(&origin).unwrap());
    let mut license_project = project("PACKPRJ", "hosts", "Hosts");
    license_project["license"] = json!({"id": "LicenseRef-All-Rights-Reserved", "name": ""});
    let versions = BTreeMap::from([
        (
            odd_sha1.clone(),
            version(
                "ODDVER",
                "ODDPRJ",
                "estranho.jar",
                &odd_sha1,
                &"e".repeat(128),
                None,
            ),
        ),
        (
            pack_sha512.clone(),
            version(
                "PACKVER",
                "PACKPRJ",
                "hosts.mrpack",
                &sha1(b"x"),
                &pack_sha512,
                None,
            ),
        ),
    ]);
    let modrinth_server = modrinth_mock(
        vec![
            project("ODDPRJ", "estranho", "Mod Estranho"),
            license_project,
        ],
        versions,
    )
    .await;
    let modrinth = modrinth_client(&modrinth_server);
    let cf_server = curseforge_mock().await;
    // A impressão do jar solto: só a CurseForge o conhece.
    let print = warden_packwiz::hash::curseforge_fingerprint(CF_LOOSE);
    let mut loose_file = cf_file(CF_OPTIONAL, 9_000_099, "cf-solto-1.0.jar", &sha1(CF_LOOSE));
    loose_file["fileFingerprint"] = json!(print);
    cf_server.reset().await;
    Mock::given(method("POST"))
        .and(path("/v1/fingerprints/432"))
        .respond_with(json_ok(json!({"data": {
            "isCacheBuilt": true,
            "exactMatches": [{"id": CF_OPTIONAL, "file": loose_file, "latestFiles": []}],
            "exactFingerprints": [print], "partialMatches": [], "partialMatchFingerprints": {},
            "installedFingerprints": [], "unmatchedFingerprints": null
        }})))
        .mount(&cf_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(json_ok(json!({"data": [
            cf_file(CF_JEI, JEI_FILE, "jei.jar", JEI_SHA1),
        ]})))
        .mount(&cf_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(json_ok(json!({"data": [
            cf_project(CF_JEI, "jei", "Just Enough Items"),
            cf_project(CF_OPTIONAL, "cf-solto", "Mod Solto da CurseForge"),
        ]})))
        .mount(&cf_server)
        .await;
    let curseforge = CurseforgeClient::with_base_url(
        http(),
        &format!("{}/v1", cf_server.uri()),
        Some(SecretString::from(KEY.to_owned())),
    )
    .unwrap();
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let services = ImportServices {
        stores: Stores {
            modrinth: Some(&modrinth),
            curseforge: Some(&curseforge),
        },
        curseforge: CurseforgeTools {
            packwiz: &fake,
            key: None,
            staging_parent: &temp.path().join("staging"),
        },
    };
    let plan = analyze(&origin, &services, &CancellationToken::new())
        .await
        .unwrap();
    let summary = &plan.summary;
    let by_name: BTreeMap<&str, (ImportItemSource, bool)> = summary
        .references
        .iter()
        .map(|item| (item.name.as_str(), (item.source, item.by_hash)))
        .collect();
    assert_eq!(by_name["outro"], (ImportItemSource::Url, false));
    assert_eq!(by_name["Mod Estranho"], (ImportItemSource::Modrinth, false));
    assert_eq!(by_name["perdido"], (ImportItemSource::Url, false));
    assert_eq!(
        by_name["Just Enough Items"],
        (ImportItemSource::Curseforge, false)
    );
    assert_eq!(
        by_name["Mod Solto da CurseForge"],
        (ImportItemSource::Curseforge, true)
    );
    assert!(summary.local_files.is_empty());
    let optional = summary
        .review
        .iter()
        .find(|item| item.name == "Just Enough Items")
        .unwrap();
    assert_eq!(optional.reason, ImportReviewReason::Optional);
    let license = summary.license.clone().unwrap();
    assert!(!license.free);
    assert_eq!(license.name, "All Rights Reserved");
    assert!(summary.unavailable_sources.is_empty());
}

#[tokio::test]
async fn loja_fora_do_ar_deixa_o_jar_local_com_o_motivo() {
    let temp = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let modrinth = modrinth_client(&server);
    let index = json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Sem rede",
        "dependencies": {"minecraft": "1.21.1"},
        "files": [{"path": "mods/a.jar", "hashes": {"sha1": "a".repeat(40)},
                   "downloads": ["https://cdn.modrinth.com/data/A/versions/B/a.jar"]}]
    });
    let origin = temp.path().join("sem-rede.mrpack");
    zip_file(
        &origin,
        &[
            (
                "modrinth.index.json".to_owned(),
                serde_json::to_vec(&index).unwrap(),
            ),
            ("overrides/mods/solto.jar".to_owned(), b"solto".to_vec()),
        ],
    );
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let tools = CurseforgeTools {
        packwiz: &fake,
        key: None,
        staging_parent: &temp.path().join("staging"),
    };
    let plan = analyze(
        &origin,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        plan.summary.unavailable_sources,
        vec!["Modrinth".to_owned()]
    );
    assert_eq!(
        plan.summary.local_files[0].reason,
        ImportLocalReason::LookupFailed
    );
    // Sem o nome do projeto, o item ainda vira referência ao Modrinth pelo caminho da URL.
    assert_eq!(
        plan.summary.references[0].source,
        ImportItemSource::Modrinth
    );
    assert_eq!(plan.summary.references[0].name, "a");
}

#[tokio::test]
async fn indices_invalidos_e_pasta_packwiz_sao_recusados() {
    let server = modrinth_mock(Vec::new(), BTreeMap::new()).await;
    let modrinth = modrinth_client(&server);
    let temp = tempfile::tempdir().unwrap();
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let file = |extra: Value| {
        let mut base = json!({"path": "mods/a.jar", "hashes": {"sha1": "a".repeat(40)},
            "downloads": ["https://cdn.modrinth.com/data/A/versions/B/a.jar"]});
        for (key, value) in extra.as_object().unwrap() {
            base[key] = value.clone();
        }
        base
    };
    let cases = [
        (
            json!({"game": "minecraft", "dependencies": {"minecraft": "1.21.1"}, "files": [file(json!({"downloads": []}))]}),
            ImportErrorCode::Unreadable,
        ),
        (
            json!({"game": "minecraft", "dependencies": {"minecraft": "1.21.1"}, "files": [file(json!({"hashes": {"sha1": "curto"}}))]}),
            ImportErrorCode::Unreadable,
        ),
        (
            json!({"game": "terraria", "dependencies": {"minecraft": "1.21.1"}, "files": []}),
            ImportErrorCode::UnknownFormat,
        ),
        (
            json!({"game": "minecraft", "dependencies": {}, "files": []}),
            ImportErrorCode::MinecraftMissing,
        ),
        (
            json!({"game": "minecraft", "dependencies": {"minecraft": "1.21.1", "quilt-loader": "0.26"}, "files": []}),
            ImportErrorCode::UnsupportedLoader,
        ),
        (
            json!({"game": "minecraft", "dependencies": {"minecraft": "1.21.1"}, "files": [file(json!({"path": ""}))]}),
            ImportErrorCode::UnsafePath,
        ),
    ];
    for (position, (index, code)) in cases.into_iter().enumerate() {
        let origin = temp.path().join(format!("caso-{position}.mrpack"));
        zip_file(
            &origin,
            &[(
                "modrinth.index.json".to_owned(),
                serde_json::to_vec(&index).unwrap(),
            )],
        );
        let tools = CurseforgeTools {
            packwiz: &fake,
            key: None,
            staging_parent: &temp.path().join("staging"),
        };
        let error = analyze(
            &origin,
            &services(&modrinth, tools),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), code, "caso {position}");
    }
    // Pasta packwiz: vai para Abrir pack.
    let pack = temp.path().join("pack");
    write(&pack, "pack.toml", "name = \"x\"\n");
    let tools = CurseforgeTools {
        packwiz: &fake,
        key: None,
        staging_parent: &temp.path().join("staging"),
    };
    let error = analyze(
        &pack,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ImportErrorCode::UnknownFormat);
    assert_eq!(error.params()["kind"], "packwiz");
    assert_eq!(warden_import::detect(&pack).unwrap(), ImportFormat::Packwiz);
}

// ---------- Zips maliciosos (zip bomb) ----------

/// Zip com entradas deflate.
fn zip_deflated(path: &Path, entries: &[(String, Vec<u8>)]) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        writer.start_file(name.as_str(), options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}

/// Troca o tamanho descompactado declarado (diretório central e cabeçalho local) das entradas
/// que começam com `prefix`: a *zip bomb* que mente o tamanho.
fn lie_sizes(path: &Path, prefix: &str, declared: u32) {
    let mut bytes = fs::read(path).unwrap();
    let u16_at = |b: &[u8], at: usize| usize::from(u16::from_le_bytes([b[at], b[at + 1]]));
    let u32_at = |b: &[u8], at: usize| {
        usize::try_from(u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])).unwrap()
    };
    let end = bytes.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
    let count = u16_at(&bytes, end + 10);
    let mut at = u32_at(&bytes, end + 16);
    let mut changed = 0;
    for _ in 0..count {
        assert_eq!(&bytes[at..at + 4], b"PK\x01\x02");
        let name_len = u16_at(&bytes, at + 28);
        let extra_len = u16_at(&bytes, at + 30);
        let comment_len = u16_at(&bytes, at + 32);
        let local = u32_at(&bytes, at + 42);
        let name = String::from_utf8(bytes[at + 46..at + 46 + name_len].to_vec()).unwrap();
        if name.starts_with(prefix) {
            bytes[at + 24..at + 28].copy_from_slice(&declared.to_le_bytes());
            bytes[local + 22..local + 26].copy_from_slice(&declared.to_le_bytes());
            changed += 1;
        }
        at += 46 + name_len + extra_len + comment_len;
    }
    assert!(changed > 0);
    fs::write(path, bytes).unwrap();
}

fn small_index() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "1.0.0", "name": "Bomba",
        "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.5"},
        "files": []
    }))
    .unwrap()
}

/// Nada ficou para trás: sem pack, sem pasta temporária, sem staging, sem registro.
fn nothing_left(temp: &Path, registry: Option<&Registry>) {
    let packs = temp.join("packs");
    let left: Vec<_> = fs::read_dir(&packs)
        .map(|dir| dir.map(|e| e.unwrap().file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
    assert!(!temp.join("staging").exists());
    if let Some(registry) = registry {
        assert!(registry.records().is_empty());
    }
}

#[tokio::test]
async fn zip_bomb_com_tamanho_declarado_falso_e_recusada_sem_deixar_pasta() {
    let server = modrinth_mock(Vec::new(), BTreeMap::new()).await;
    let modrinth = modrinth_client(&server);
    let temp = tempfile::tempdir().unwrap();
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let staging = temp.path().join("staging");
    // Muitas entradas pequenas no zip (61 KB no total) que dizem ter 1 byte e rendem 256 KiB
    // cada: mais de 75 MiB no disco sem a conferência.
    let origin = temp.path().join("bomba.mrpack");
    let mut entries = vec![("modrinth.index.json".to_owned(), small_index())];
    entries.extend((0..300).map(|n| {
        (
            format!("overrides/config/b{n:03}.txt"),
            vec![b'7'; 256 * 1024],
        )
    }));
    zip_deflated(&origin, &entries);
    lie_sizes(&origin, "overrides/", 1);
    assert!(fs::metadata(&origin).unwrap().len() < 1024 * 1024);

    let tools = CurseforgeTools {
        packwiz: &fake,
        key: None,
        staging_parent: &staging,
    };
    let plan = analyze(
        &origin,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    let registry = Registry::open(temp.path().join("dados/packs.json")).unwrap();
    let started = std::time::Instant::now();
    let error = apply(
        &plan,
        &request(temp.path(), "Bomba"),
        &registry,
        &fake,
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ImportErrorCode::TooLarge, "{error}");
    assert_eq!(error.params()["name"], "bomba.mrpack");
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    nothing_left(temp.path(), Some(&registry));
}

#[tokio::test]
async fn zip_bomb_pela_razao_de_compressao_e_recusada_na_leitura() {
    let server = modrinth_mock(Vec::new(), BTreeMap::new()).await;
    let modrinth = modrinth_client(&server);
    let temp = tempfile::tempdir().unwrap();
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let staging = temp.path().join("staging");
    // Tamanho honesto, mas 64 MiB de zeros cabem em ~64 KiB.
    let origin = temp.path().join("zeros.mrpack");
    zip_deflated(
        &origin,
        &[
            ("modrinth.index.json".to_owned(), small_index()),
            (
                "overrides/config/zeros.bin".to_owned(),
                vec![0; 64 * 1024 * 1024],
            ),
        ],
    );
    let tools = CurseforgeTools {
        packwiz: &fake,
        key: None,
        staging_parent: &staging,
    };
    let started = std::time::Instant::now();
    let error = analyze(
        &origin,
        &services(&modrinth, tools),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ImportErrorCode::TooLarge, "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert!(server.received_requests().await.unwrap().is_empty());
    nothing_left(temp.path(), None);
}

#[tokio::test]
async fn zip_da_curseforge_com_tamanho_falso_e_recusado_antes_do_packwiz() {
    let temp = tempfile::tempdir().unwrap();
    let origin = temp.path().join("bomba-cf.zip");
    zip_deflated(
        &origin,
        &[
            (
                "manifest.json".to_owned(),
                fs::read(corpus("curseforge-zip/manifest.json")).unwrap(),
            ),
            (
                "overrides/config/bomba.txt".to_owned(),
                vec![b'7'; 32 * 1024 * 1024],
            ),
        ],
    );
    lie_sizes(&origin, "overrides/", 1);
    let modrinth_server = modrinth_mock(Vec::new(), BTreeMap::new()).await;
    let modrinth = modrinth_client(&modrinth_server);
    let cf_server = curseforge_mock().await;
    let curseforge = CurseforgeClient::with_base_url(
        http(),
        &format!("{}/v1", cf_server.uri()),
        Some(SecretString::from(KEY.to_owned())),
    )
    .unwrap();
    // Um packwiz que não existe: se o Warden chegasse a chamá-lo, o erro seria outro.
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let key = SecretString::from(KEY.to_owned());
    let staging = temp.path().join("staging");
    let services = ImportServices {
        stores: Stores {
            modrinth: Some(&modrinth),
            curseforge: Some(&curseforge),
        },
        curseforge: CurseforgeTools {
            packwiz: &fake,
            key: Some(&key),
            staging_parent: &staging,
        },
    };
    let started = std::time::Instant::now();
    let error = analyze(&origin, &services, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.code(), ImportErrorCode::TooLarge, "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert!(cf_server.received_requests().await.unwrap().is_empty());
    assert!(
        modrinth_server
            .received_requests()
            .await
            .unwrap()
            .is_empty()
    );
    nothing_left(temp.path(), None);
}

#[tokio::test]
async fn zip_dentro_de_zip_nao_e_aberto() {
    let server = modrinth_mock(Vec::new(), BTreeMap::new()).await;
    let modrinth = modrinth_client(&server);
    let temp = tempfile::tempdir().unwrap();
    let fake = Packwiz::new(
        PathBuf::from("x"),
        temp.path().join("c"),
        temp.path().join("c.toml"),
    );
    let staging = temp.path().join("staging");
    let tools = || CurseforgeTools {
        packwiz: &fake,
        key: None,
        staging_parent: &staging,
    };
    // Uma bomba de verdade (64 MiB de zeros), que só existe dentro de outro zip.
    let inner = temp.path().join("bomba-interna.zip");
    zip_deflated(
        &inner,
        &[("zeros.bin".to_owned(), vec![0; 64 * 1024 * 1024])],
    );
    let inner_bytes = fs::read(&inner).unwrap();
    fs::remove_file(&inner).unwrap();

    // 1. Um zip que só tem um modpack dentro não é reconhecido (o Warden não abre o de
    //    dentro): recusado sem criar nada.
    let mut nested = vec![("modrinth.index.json".to_owned(), small_index())];
    nested.push(("overrides/config/zeros.zip".to_owned(), inner_bytes.clone()));
    let pack = temp.path().join("dentro.mrpack");
    zip_deflated(&pack, &nested);
    let outer = temp.path().join("embrulho.zip");
    zip_deflated(
        &outer,
        &[("dentro.mrpack".to_owned(), fs::read(&pack).unwrap())],
    );
    let error = analyze(
        &outer,
        &services(&modrinth, tools()),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ImportErrorCode::UnknownFormat);
    nothing_left(temp.path(), None);

    // 2. Um zip dentro do modpack (como um resource pack) é só um arquivo: entra como está,
    //    sem ser descompactado.
    let started = std::time::Instant::now();
    let plan = analyze(
        &pack,
        &services(&modrinth, tools()),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert!(
        plan.summary
            .excluded
            .iter()
            .all(|item| !item.path.contains("zeros"))
    );
    let Some(binary) = sidecar() else { return };
    let cli = packwiz(&binary, temp.path());
    let registry = Registry::open(temp.path().join("dados/packs.json")).unwrap();
    let created = apply(
        &plan,
        &request(temp.path(), "Dentro"),
        &registry,
        &cli,
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        fs::read(created.path.join("config/zeros.zip")).unwrap(),
        inner_bytes
    );
}
