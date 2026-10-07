//! CA-T19-09 e base de CA-T18-09: a instância pronta para o Prism com packwiz real e
//! servidores simulados da CurseForge, do Modrinth e da CDN.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(linker_messages)]

use std::collections::BTreeSet;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use secrecy::SecretString;
use serde_json::{Value, json};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
mod rede;

use warden_core::{CancellationToken, NoProgress};
use warden_curseforge::CurseforgeClient;
use warden_export::{
    ExportErrorCode, ExportSource, PrismOptions, PrismServices, analyze_prism, export_prism,
};
use warden_http::{ExpectedHash, HttpClient, HttpConfig, Url};
use warden_instance::{DownloadCache, FileOrigin};
use warden_modrinth::ModrinthClient;
use warden_packwiz::{CurseForgeFile, Metafile, ModrinthFile};
use warden_packwiz::{HashFormat, PackManifest, Side};
use warden_packwiz_cli::{Packwiz, RunContext};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "$2a$10$chaveFalsaDoWardenParaTestes0123456789abcdefghijkl";
const SODIUM: &[u8] = b"SODIUM-JAR";
const JEI: &[u8] = b"JEI-JAR-DA-CURSEFORGE";
const BLOCKED: &[u8] = b"MOD-BLOQUEADO";
const LINK: &[u8] = b"MOD-POR-LINK";

fn hex_of<D: sha2::digest::Digest>(bytes: &[u8]) -> String {
    hex::encode(D::digest(bytes))
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
            Some("1")
        );
        None
    }
}

struct World {
    _temp: tempfile::TempDir,
    root: PathBuf,
    temp: PathBuf,
    cli: Packwiz,
    cdn: MockServer,
    services: PrismServices,
}

async fn world(with_key: bool, version: &str) -> Option<World> {
    let binary = sidecar()?;
    let temp_dir = tempfile::tempdir().unwrap();
    let temp = temp_dir.path().to_path_buf();
    let root = temp.join("pack");
    fs::create_dir_all(root.join("mods")).unwrap();
    fs::create_dir_all(root.join("config")).unwrap();

    let mut manifest = PackManifest::new("Pack de Teste", "1.21.1");
    manifest.version = version.to_owned();
    manifest.description = "Um pack para o Prism".to_owned();
    manifest
        .versions
        .get_or_insert_default()
        .insert("fabric".to_owned(), "0.16.5".to_owned());
    fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    fs::write(
        root.join("index.toml"),
        "hash-format = \"sha256\"\nfiles = []\n",
    )
    .unwrap();

    let metafiles = [
        (
            "mods/sodium.pw.toml",
            Metafile::modrinth(
                &ModrinthFile {
                    title: "Sodium".into(),
                    project_id: "AANobbMI".into(),
                    version_id: "ver1".into(),
                    filename: "sodium.jar".into(),
                    url: "https://cdn.modrinth.com/data/AANobbMI/versions/ver1/sodium.jar".into(),
                    hash_format: HashFormat::Sha512,
                    hash: hex_of::<Sha512>(SODIUM),
                },
                Side::Both,
            ),
        ),
        (
            "mods/jei.pw.toml",
            curseforge("JEI", 238_222, 4_712_345, "jei.jar", JEI, Side::Both),
        ),
        (
            "mods/bloqueado.pw.toml",
            curseforge(
                "Bloqueado",
                448_233,
                5_000_001,
                "bloqueado.jar",
                BLOCKED,
                Side::Client,
            ),
        ),
        (
            "mods/link.pw.toml",
            Metafile::url(
                "Por link",
                "https://example.com/dl/link.jar",
                &hex_of::<Sha256>(LINK),
            )
            .unwrap(),
        ),
    ];
    for (file, meta) in metafiles {
        fs::write(root.join(file), meta.to_toml_string()).unwrap();
    }
    fs::write(root.join("mods/local.jar"), b"JAR-LOCAL").unwrap();
    fs::write(root.join("config/a.txt"), b"ajuste").unwrap();

    let cli = Packwiz::new(binary, temp.join("cache"), temp.join("config.toml"));
    cli.refresh(&root, false, RunContext::new(&CancellationToken::new()))
        .await
        .unwrap();

    // Servidor da CurseForge (API) e da CDN: o mesmo, em caminhos diferentes.
    let cdn = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
            cf_file(238_222, 4_712_345, "jei.jar", Some("https://edge.forgecdn.net/files/4712/345/jei.jar")),
            cf_file(448_233, 5_000_001, "bloqueado.jar", None),
        ]})))
        .mount(&cdn)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": [
            {"id": 448_233, "links": {"websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/entityculling"},
             "allowModDistribution": false}
        ]})))
        .mount(&cdn)
        .await;
    Mock::given(method("GET"))
        .and(path("/files/4712/345/jei.jar"))
        .and(header("x-api-key", KEY))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(JEI))
        .mount(&cdn)
        .await;
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            hex_of::<Sha1>(BLOCKED): {
                "id": "swapVer", "project_id": "ECULLING", "name": "Entity Culling",
                "version_number": "1.7.0", "version_type": "release",
                "files": [{
                    "hashes": {"sha1": hex_of::<Sha1>(BLOCKED), "sha512": hex_of::<Sha512>(BLOCKED)},
                    "url": "https://cdn.modrinth.com/data/ECULLING/versions/swapVer/bloqueado.jar",
                    "filename": "bloqueado.jar", "primary": true, "size": BLOCKED.len()
                }]
            }
        })))
        .mount(&cdn)
        .await;

    let http = HttpClient::new(HttpConfig::for_version("1")).unwrap();
    let key = with_key.then(|| SecretString::from(KEY.to_owned()));
    let curseforge =
        CurseforgeClient::with_base_url(http.clone(), &format!("{}/v1", cdn.uri()), key).unwrap();
    let modrinth =
        ModrinthClient::with_base_url(http.clone(), &format!("{}/v2", cdn.uri()), None).unwrap();
    let cache = Arc::new(DownloadCache::open(&temp.join("downloads")).unwrap());
    // O que o teste do pack já baixou: Modrinth e link direto (que só existem por https).
    for (name, bytes, format, hash) in [
        (
            "sodium.jar",
            SODIUM,
            HashFormat::Sha512,
            hex_of::<Sha512>(SODIUM),
        ),
        ("link.jar", LINK, HashFormat::Sha256, hex_of::<Sha256>(LINK)),
    ] {
        let file = temp.join(name);
        fs::write(&file, bytes).unwrap();
        cache
            .import_file(
                &file,
                &ExpectedHash::new(format, hash),
                &FileOrigin::url("https://x.invalid/"),
            )
            .unwrap()
            .unwrap();
    }
    let mut services = PrismServices::new(http, Some(curseforge), modrinth, cache);
    services.cdn_download_base = Some(Url::parse(&cdn.uri()).unwrap());
    Some(World {
        _temp: temp_dir,
        root,
        temp,
        cli,
        cdn,
        services,
    })
}

fn curseforge(
    name: &str,
    project: u32,
    file: u32,
    filename: &str,
    bytes: &[u8],
    side: Side,
) -> Metafile {
    Metafile::curseforge(
        &CurseForgeFile {
            name: name.into(),
            project_id: project,
            file_id: file,
            filename: filename.into(),
            hash_format: HashFormat::Sha1,
            hash: hex_of::<Sha1>(bytes),
        },
        side,
    )
}

fn cf_file(project: u64, id: u64, name: &str, url: Option<&str>) -> Value {
    json!({"id": id, "modId": project, "fileName": name, "downloadUrl": url,
           "fileLength": 10, "hashes": []})
}

async fn generate(
    w: &World,
    name: &str,
    options: &PrismOptions,
) -> warden_export::Result<warden_export::PrismResult> {
    export_prism(
        &w.root,
        &ExportSource::Current,
        &w.temp.join(name),
        &w.temp.join("staging"),
        &w.cli,
        &w.services,
        options,
        &NoProgress,
        &CancellationToken::new(),
    )
    .await
}

fn code(error: &warden_export::Error) -> ExportErrorCode {
    use warden_core::{DomainCode, DomainError as _};
    match error.code() {
        DomainCode::Domain(code) => code,
        other => panic!("código inesperado: {other:?}"),
    }
}

fn read_zip(path: &Path) -> (Value, BTreeSet<String>) {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut text = String::new();
    zip.by_name("modrinth.index.json")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    let overrides = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_owned())
        .filter_map(|n| n.strip_prefix("overrides/").map(str::to_owned))
        .collect();
    (serde_json::from_str(&text).unwrap(), overrides)
}

fn accept_all() -> PrismOptions {
    PrismOptions {
        confirm_local_files: true,
        swaps: vec!["mods/bloqueado.pw.toml".into()],
    }
}

#[tokio::test]
async fn analise_lista_bloqueado_com_troca_e_arquivos_de_terceiros() {
    let Some(w) = world(true, "1.2.0").await else {
        return;
    };
    let analysis = analyze_prism(
        &w.root,
        &ExportSource::Current,
        &w.services,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        (analysis.modrinth, analysis.curseforge, analysis.links),
        (1, 2, 1)
    );
    assert_eq!(analysis.minecraft, "1.21.1");
    assert_eq!(analysis.loader, "fabric 0.16.5");
    assert_eq!(analysis.blocked.len(), 1);
    let blocked = &analysis.blocked[0];
    assert_eq!(blocked.path, "mods/bloqueado.pw.toml");
    assert!(
        blocked
            .page_url
            .as_deref()
            .unwrap()
            .ends_with("/files/5000001")
    );
    assert_eq!(blocked.swap.as_ref().unwrap().project_id, "ECULLING");
    assert_eq!(analysis.local_files.len(), 1);
    assert_eq!(analysis.local_files[0].path, "mods/local.jar");
    // Dois da CurseForge, um link e o jar local em overrides/mods/.
    assert_eq!(analysis.untrusted, 4);
}

#[tokio::test]
async fn gera_arquivo_valido_deterministico_e_confere_ca_t19_09() {
    let Some(w) = world(true, "1.2.0").await else {
        return;
    };
    let first = generate(&w, "a.mrpack", &accept_all()).await.unwrap();
    let second = generate(&w, "b.mrpack", &accept_all()).await.unwrap();
    assert_eq!(
        fs::read(&first.path).unwrap(),
        fs::read(&second.path).unwrap()
    );
    assert_eq!(first.sha256, second.sha256);
    assert_eq!(
        first.sha256,
        hex_of::<Sha256>(&fs::read(&first.path).unwrap())
    );

    let (index, overrides) = read_zip(&first.path);
    assert_eq!(index["formatVersion"], 1);
    assert_eq!(index["versionId"], "1.2.0");
    assert_eq!(index["name"], "Pack de Teste");
    assert_eq!(
        index["dependencies"],
        json!({"minecraft": "1.21.1", "fabric-loader": "0.16.5"})
    );
    let files = index["files"].as_array().unwrap();
    let by_path = |p: &str| {
        files
            .iter()
            .find(|f| f["path"] == p)
            .unwrap_or_else(|| panic!("{p}"))
    };
    assert_eq!(files.len(), 4);

    let sodium = by_path("mods/sodium.jar");
    assert_eq!(
        sodium["downloads"][0],
        "https://cdn.modrinth.com/data/AANobbMI/versions/ver1/sodium.jar"
    );
    assert_eq!(sodium["hashes"]["sha1"], hex_of::<Sha1>(SODIUM));
    assert_eq!(sodium["hashes"]["sha512"], hex_of::<Sha512>(SODIUM));
    assert_eq!(sodium["fileSize"], SODIUM.len());
    assert_eq!(
        sodium["env"],
        json!({"client": "required", "server": "required"})
    );

    let jei = by_path("mods/jei.jar");
    assert_eq!(
        jei["downloads"][0],
        "https://edge.forgecdn.net/files/4712/345/jei.jar"
    );
    assert_eq!(jei["hashes"]["sha1"], hex_of::<Sha1>(JEI));
    assert_eq!(jei["hashes"]["sha512"], hex_of::<Sha512>(JEI));
    assert_eq!(jei["fileSize"], JEI.len());

    // A troca vale só para o arquivo gerado: vem do Modrinth, com o lado do metafile (client).
    let swapped = by_path("mods/bloqueado.jar");
    assert_eq!(
        swapped["downloads"][0],
        "https://cdn.modrinth.com/data/ECULLING/versions/swapVer/bloqueado.jar"
    );
    assert_eq!(
        swapped["env"],
        json!({"client": "required", "server": "unsupported"})
    );

    let link = by_path("mods/link.jar");
    assert_eq!(link["downloads"][0], "https://example.com/dl/link.jar");

    // overrides/ só com os arquivos do índice que não são referência; nenhum jar da CurseForge.
    assert_eq!(
        overrides,
        BTreeSet::from(["config/a.txt".to_owned(), "mods/local.jar".to_owned()])
    );
    assert_eq!((first.files, first.overrides, first.untrusted), (4, 2, 3));

    // O download da CurseForge levou o x-api-key, e só uma vez (o segundo uso veio do cache).
    let cdn_requests = w.cdn.received_requests().await.unwrap();
    let downloads: Vec<_> = cdn_requests
        .iter()
        .filter(|r| r.url.path().starts_with("/files/"))
        .collect();
    assert_eq!(downloads.len(), 1);
    assert!(downloads[0].headers.get("range").is_none());
}

#[tokio::test]
async fn mod_bloqueado_sem_troca_impede_gerar_e_nada_e_gravado() {
    let Some(w) = world(true, "1.2.0").await else {
        return;
    };
    let options = PrismOptions {
        confirm_local_files: true,
        swaps: vec![],
    };
    let error = generate(&w, "x.mrpack", &options).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::PrismBlocked);
    assert!(!w.temp.join("x.mrpack").exists());
}

#[tokio::test]
async fn arquivo_de_terceiros_pede_confirmacao() {
    let Some(w) = world(true, "1.2.0").await else {
        return;
    };
    let options = PrismOptions {
        confirm_local_files: false,
        swaps: vec!["mods/bloqueado.pw.toml".into()],
    };
    let error = generate(&w, "x.mrpack", &options).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::PrismLocalNeedsConfirmation);
}

#[tokio::test]
async fn sem_chave_da_curseforge_nao_gera_e_a_analise_avisa() {
    let Some(w) = world(false, "1.2.0").await else {
        return;
    };
    let analysis = analyze_prism(
        &w.root,
        &ExportSource::Current,
        &w.services,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(analysis.curseforge_key_missing);
    let error = generate(&w, "x.mrpack", &accept_all()).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::PrismCurseforgeKeyMissing);
    let cdn_requests = w.cdn.received_requests().await.unwrap();
    assert!(cdn_requests.is_empty(), "nenhuma requisição sem chave");
}

#[tokio::test]
async fn pack_sem_versao_e_destino_ocupado_tem_codigo_proprio() {
    let Some(w) = world(true, "").await else {
        return;
    };
    let error = generate(&w, "x.mrpack", &accept_all()).await.unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::PrismVersionRequired);
    let analysis = analyze_prism(
        &w.root,
        &ExportSource::Current,
        &w.services,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(analysis.version.is_empty());

    let Some(w) = world(true, "1.0.0").await else {
        return;
    };
    fs::write(w.temp.join("existe.mrpack"), b"de outra pessoa").unwrap();
    let error = generate(&w, "existe.mrpack", &accept_all())
        .await
        .unwrap_err();
    assert_eq!(code(&error), ExportErrorCode::DestinationNotEmpty);
    assert_eq!(
        fs::read(w.temp.join("existe.mrpack")).unwrap(),
        b"de outra pessoa"
    );
}
