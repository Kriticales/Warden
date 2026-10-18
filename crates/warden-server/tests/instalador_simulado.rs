//! A instalação do servidor com as fontes simuladas (wiremock): vanilla pelo `downloads.server`
//! do JSON da versão (conferido pelo `sha1`), Fabric pelo lançador do meta e Forge pelo
//! instalador oficial (aqui o `fixtures/java/forge-simulado-installer.jar`, que imita o
//! `--installServer`), com o cache dos downloads e a instalação idempotente. Os testes com os
//! instaladores de verdade ficam em `server_real.rs`.
//!
//! O Forge precisa de Java (`WARDEN_TEST_JAVA` ou `JAVA_HOME`); sem Java, essa parte avisa e
//! passa.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha1::{Digest as _, Sha1};
use warden_catalog::{Catalog, CatalogCache, Endpoints};
use warden_core::{CancellationToken, NoProgress};
use warden_http::{HttpClient, HttpConfig};
use warden_launcher::{GameSpec, LoaderSpec};
use warden_server::install::{FABRIC_JAR, MARKER_FILE, VANILLA_JAR};
use warden_server::{Error, Installer, ServerLaunch};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SERVER_JAR: &[u8] = b"server.jar simulado";
const FABRIC_LAUNCHER: &[u8] = b"fabric-server-launch.jar simulado";

fn sha1_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha1::digest(bytes)
        .iter()
        .fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

fn catalog_fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../warden-catalog/tests/fixtures/http")
            .join(name),
    )
    .unwrap()
}

fn java() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let java = std::env::var_os("WARDEN_TEST_JAVA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("JAVA_HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join("bin").join(exe))
        })
        .filter(|path| path.is_file());
    if java.is_none() {
        assert!(
            std::env::var_os("WARDEN_REQUIRE_EXTERNALS").is_none_or(|value| value != "1"),
            "WARDEN_REQUIRE_EXTERNALS=1, mas não há Java"
        );
        eprintln!("AVISO: sem Java; a parte do instalador do Forge foi pulada.");
    }
    java
}

/// O servidor simulado com o manifesto da Mojang (só a 1.20.1), o JSON da versão e o
/// `server.jar`.
async fn mojang(server: &MockServer) {
    let version = serde_json::json!({
        "id": "1.20.1",
        "releaseTime": "2023-06-12T13:25:51+00:00",
        "downloads": { "server": {
            "url": format!("{}/objetos/server.jar", server.uri()),
            "sha1": sha1_hex(SERVER_JAR),
            "size": SERVER_JAR.len(),
        }},
    })
    .to_string();
    let version_sha = sha1_hex(version.as_bytes());
    let version_path = format!("/v1/packages/{version_sha}/1.20.1.json");
    let manifest = serde_json::json!({
        "latest": { "release": "1.20.1", "snapshot": "1.20.1" },
        "versions": [{
            "id": "1.20.1",
            "type": "release",
            "url": format!("https://piston-meta.mojang.com{version_path}"),
            "time": "2023-06-12T13:25:51+00:00",
            "releaseTime": "2023-06-12T13:25:51+00:00",
            "sha1": version_sha,
            "complianceLevel": 1,
        }],
    })
    .to_string();
    for (route, body) in [
        (
            "/mc/game/version_manifest_v2.json".to_owned(),
            manifest.into_bytes(),
        ),
        (version_path, version.into_bytes()),
        ("/objetos/server.jar".to_owned(), SERVER_JAR.to_vec()),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(server)
            .await;
    }
}

fn installer(server: &MockServer, cache: &Path) -> Installer {
    let http = HttpClient::new(HttpConfig::for_version("teste-servidor")).unwrap();
    let catalog = Catalog::with_endpoints(
        http.clone(),
        Endpoints::single(&server.uri()).unwrap(),
        CatalogCache::in_memory(None).unwrap(),
    );
    Installer::new(http, Arc::new(catalog), cache.to_path_buf())
}

#[tokio::test]
async fn vanilla_e_fabric_baixam_conferem_e_instalam_uma_vez() {
    let server = MockServer::start().await;
    mojang(&server).await;
    Mock::given(path("/v2/versions/installer"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"[{"version":"1.1.0","stable":false},{"version":"1.0.1","stable":true}]"#,
        ))
        .mount(&server)
        .await;
    Mock::given(path("/v2/versions/loader/1.20.1/0.19.5/1.0.1/server/jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(FABRIC_LAUNCHER))
        .expect(1)
        .mount(&server)
        .await;
    let root = tempfile::tempdir().unwrap();
    let setup = installer(&server, &root.path().join("cache"));
    let cancel = CancellationToken::new();

    let vanilla = GameSpec::new("1.20.1", LoaderSpec::Vanilla).unwrap();
    let dir = root.path().join("vanilla");
    let installed = setup
        .install(&vanilla, &dir, Path::new("java"), &NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(
        installed.launch,
        ServerLaunch::Jar {
            jar: VANILLA_JAR.into()
        }
    );
    assert_eq!(installed.release_time, "2023-06-12T13:25:51+00:00");
    assert_eq!(std::fs::read(dir.join(VANILLA_JAR)).unwrap(), SERVER_JAR);
    assert!(dir.join(MARKER_FILE).is_file());
    assert!(dir.join(warden_server::log4j::LOG4J_CONFIG_FILE).is_file());

    let fabric = GameSpec::new(
        "1.20.1",
        LoaderSpec::Fabric {
            version: "0.19.5".into(),
        },
    )
    .unwrap();
    let dir = root.path().join("fabric");
    let installed = setup
        .install(&fabric, &dir, Path::new("java"), &NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(
        installed.launch,
        ServerLaunch::Jar {
            jar: FABRIC_JAR.into()
        }
    );
    assert_eq!(
        std::fs::read(dir.join(FABRIC_JAR)).unwrap(),
        FABRIC_LAUNCHER
    );
    assert_eq!(std::fs::read(dir.join(VANILLA_JAR)).unwrap(), SERVER_JAR);
    assert_eq!(
        std::fs::read_to_string(dir.join("fabric-server-launcher.properties")).unwrap(),
        "serverJar=server.jar\n"
    );
    // De novo: nada é baixado (o `expect(1)` confere ao soltar o servidor).
    let again = setup
        .install(&fabric, &dir, Path::new("java"), &NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(again, installed);
    // Outra pasta usa o cache.
    let other = root.path().join("fabric-2");
    setup
        .install(&fabric, &other, Path::new("java"), &NoProgress, &cancel)
        .await
        .unwrap();
    assert!(other.join(FABRIC_JAR).is_file());
}

#[tokio::test]
async fn quilt_nao_tem_servidor_e_fonte_fora_do_ar_e_erro_de_download() {
    let server = MockServer::start().await;
    let root = tempfile::tempdir().unwrap();
    let setup = installer(&server, &root.path().join("cache"));
    let cancel = CancellationToken::new();
    let quilt = GameSpec::new(
        "1.20.1",
        LoaderSpec::Quilt {
            version: "0.24.0".into(),
        },
    )
    .unwrap();
    // Sem manifesto: a fonte falha antes de qualquer coisa.
    let error = setup
        .install(&quilt, root.path(), Path::new("java"), &NoProgress, &cancel)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Download { .. }), "{error}");

    mojang(&server).await;
    let error = setup
        .install(
            &quilt,
            &root.path().join("q"),
            Path::new("java"),
            &NoProgress,
            &cancel,
        )
        .await
        .unwrap_err();
    assert!(matches!(&error, Error::LoaderUnsupported { loader } if loader == "Quilt"));
}

#[tokio::test]
async fn forge_pelo_instalador_oficial_com_o_sha1_do_maven() {
    let Some(java) = java() else { return };
    let server = MockServer::start().await;
    mojang(&server).await;
    Mock::given(path("/net/minecraftforge/forge/maven-metadata.xml"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(catalog_fixture("2026-10-05-forge-maven-metadata.xml")),
        )
        .mount(&server)
        .await;
    Mock::given(path("/net/minecraftforge/forge/promotions_slim.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(catalog_fixture("2026-10-05-forge-promotions_slim.json")),
        )
        .mount(&server)
        .await;
    let jar = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/java/forge-simulado-installer.jar"),
    )
    .unwrap();
    let base = "/net/minecraftforge/forge/1.20.1-47.4.10/forge-1.20.1-47.4.10-installer.jar";
    Mock::given(path(base))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(jar.clone()))
        .mount(&server)
        .await;
    Mock::given(path(format!("{base}.sha1")))
        .respond_with(ResponseTemplate::new(200).set_body_string(sha1_hex(&jar)))
        .mount(&server)
        .await;
    let root = tempfile::tempdir().unwrap();
    let setup = installer(&server, &root.path().join("cache"));
    let cancel = CancellationToken::new();
    let forge = |version: &str| {
        GameSpec::new(
            "1.20.1",
            LoaderSpec::Forge {
                version: version.into(),
            },
        )
        .unwrap()
    };

    let dir = root.path().join("forge");
    let installed = setup
        .install(&forge("47.4.10"), &dir, &java, &NoProgress, &cancel)
        .await
        .unwrap();
    let name = if cfg!(windows) {
        "win_args.txt"
    } else {
        "unix_args.txt"
    };
    assert_eq!(
        installed.launch,
        ServerLaunch::ArgsFile {
            file: format!("libraries/net/minecraftforge/forge/1.20.1-47.3.0/{name}")
        }
    );
    // O instalador sai da pasta do servidor; o cache fica.
    assert!(!dir.join("forge-1.20.1-47.4.10-installer.jar").exists());
    assert!(dir.join("installer-output.log").is_file());

    // Versão que não existe no Maven.
    let error = setup
        .install(
            &forge("1.0.0"),
            &root.path().join("x"),
            &java,
            &NoProgress,
            &cancel,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, Error::LoaderVersionNotFound { .. }),
        "{error}"
    );

    // O instalador falhou, ou terminou sem instalar.
    for (marker, expected) in [("falhar", "falhou"), ("nada", "sem servidor")] {
        let dir = root.path().join(marker);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(marker), "").unwrap();
        let error = setup
            .install(&forge("47.4.10"), &dir, &java, &NoProgress, &cancel)
            .await
            .unwrap_err();
        match (expected, &error) {
            ("falhou", Error::InstallerFailed { code, output, .. }) => {
                assert_eq!(*code, Some(3));
                assert!(output.contains("error during installation"));
            }
            ("sem servidor", Error::InstallerOutputMissing { output, .. }) => {
                assert!(output.contains("Failed to validate certificates"));
            }
            _ => panic!("{marker}: {error:?}"),
        }
    }

    // Cancelado antes: nada roda.
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let error = setup
        .install(
            &forge("47.4.10"),
            &root.path().join("c"),
            &java,
            &NoProgress,
            &cancelled,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Cancelled), "{error:?}");
}
