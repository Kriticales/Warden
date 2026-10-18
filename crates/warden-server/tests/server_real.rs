//! CA-T27-01 com servidores de verdade (QUALITY §4.1, "Servidor local real"): para Fabric
//! 1.21.1, NeoForge 1.21.1, Forge 1.20.1 e Forge 1.12.2, o servidor local instala pelos
//! instaladores oficiais, chega a `Done (…)! For help, type "help"` e fecha com `stop` em até
//! 60 s, com o `server.properties` controlado e a porta liberada no fim.
//!
//! Sob demanda (baixam Java, servidor e bibliotecas):
//! `cargo nextest run -p warden-server --run-ignored only -E 'test(/server_/)'`.
//! A pasta de dados é `WARDEN_SERVER_DADOS` (para reaproveitar o cache entre execuções) ou uma
//! temporária.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::missing_panics_doc
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use warden_core::{CancellationToken, NoProgress};
use warden_http::{HttpClient, HttpConfig};
use warden_java::{
    JavaChoiceRequest, JavaRuntimes, JavaRuntimesConfig, LoaderKind, Platform, ProcessProbe,
};
use warden_launcher::outcome::Outcome;
use warden_launcher::{GameSpec, JavaRuntime, LoaderSpec};
use warden_server::process::wait_ready;
use warden_server::properties::{PROPERTIES_FILE, port_is_free};
use warden_server::{
    ControlledProperties, Installer, RunningServer, ServerCommand, free_port, server_memory_mb,
    write_eula, write_properties,
};

fn http() -> HttpClient {
    HttpClient::new(HttpConfig::for_version("teste-servidor")).unwrap()
}

fn data_root() -> (PathBuf, Option<tempfile::TempDir>) {
    if let Some(dir) = std::env::var_os("WARDEN_SERVER_DADOS").filter(|value| !value.is_empty()) {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        return (dir, None);
    }
    let temp = tempfile::tempdir().unwrap();
    (temp.path().to_path_buf(), Some(temp))
}

fn java_service(root: &Path) -> JavaRuntimes {
    let config = JavaRuntimesConfig {
        runtimes_dir: root.join("shared").join("runtimes"),
        downloads_dir: root.join("cache").join("tmp").join("java"),
        platform: Platform::current().unwrap(),
    };
    JavaRuntimes::with_clients(
        config,
        http(),
        warden_java::adoptium::AdoptiumClient::new(http()).unwrap(),
        warden_java::mojang::MojangRuntimeClient::new(http()).unwrap(),
        Arc::new(ProcessProbe::default()),
    )
    .unwrap()
}

async fn run(name: &str, minecraft: &str, loader: LoaderSpec, kind: LoaderKind) {
    let (root, _temp) = data_root();
    let cancel = CancellationToken::new();
    let java_service = java_service(&root);
    let choice = java_service
        .choose(&JavaChoiceRequest::automatic(
            minecraft,
            kind,
            loader.version(),
        ))
        .unwrap();
    let runtime = java_service
        .ensure_choice(&choice, &NoProgress, &cancel)
        .await
        .unwrap();
    let java = JavaRuntime::from_installed(&runtime);
    let cache = warden_catalog::CatalogCache::open(&root.join("cache/metadata.sqlite")).unwrap();
    let catalog = Arc::new(warden_catalog::Catalog::new(http(), cache).unwrap());
    let installer = Installer::new(http(), catalog, root.join("cache").join("server"));
    let spec = GameSpec::new(minecraft, loader).unwrap();
    let dir = root.join("servidores").join(name);
    let started = Instant::now();
    let server = installer
        .install(&spec, &dir, &java.java, &NoProgress, &cancel)
        .await
        .unwrap();
    eprintln!("{name}: instalado em {:?}", started.elapsed());
    // Instalar de novo não baixa nem roda o instalador.
    let again = Instant::now();
    let same = installer
        .install(&spec, &dir, &java.java, &NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(same, server);
    assert!(again.elapsed() < Duration::from_secs(5));

    write_eula(&dir, Some("2026-10-13T12:00:00Z")).unwrap();
    let port = free_port().unwrap();
    write_properties(
        &dir,
        &ControlledProperties {
            port,
            motd: format!("Warden {name}"),
        },
    )
    .unwrap();
    let properties = std::fs::read_to_string(dir.join(PROPERTIES_FILE)).unwrap();
    for line in [
        "server-ip=127.0.0.1",
        "online-mode=false",
        "enable-rcon=false",
        &format!("server-port={port}"),
    ] {
        assert!(properties.lines().any(|l| l == line), "{line}");
    }
    let command = ServerCommand {
        java,
        memory_mb: server_memory_mb(0).max(2048),
        extra_jvm_args: Vec::new(),
    }
    .build(&server);
    let (running, mut events) = RunningServer::spawn(&command, Some(dir.join("warden-output.log")))
        .await
        .unwrap();
    let handle = running.handle();
    let ready = wait_ready(&mut events, Duration::from_mins(10), |_| {}).await;
    if let Err(failure) = &ready {
        handle.kill().await;
        panic!("{name}: o servidor não ficou pronto: {failure:?}");
    }
    eprintln!("{name}: pronto: {:?}", ready.unwrap());
    assert!(!port_is_free(port));
    let stopping = Instant::now();
    let stop = handle.stop().await;
    let exit = running.wait().await.unwrap();
    assert!(stop.sent && !stop.forced, "{name}: {stop:?}");
    assert!(stopping.elapsed() < Duration::from_mins(1));
    assert_eq!(exit.outcome, Outcome::ClosedNormally, "{name}: {exit:?}");
    assert!(port_is_free(port), "{name}: a porta {port} ficou presa");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "servidor"]
async fn server_fabric_1_21_1() {
    run(
        "fabric-1.21.1",
        "1.21.1",
        LoaderSpec::Fabric {
            version: "0.19.5".into(),
        },
        LoaderKind::Fabric,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "servidor"]
async fn server_neoforge_1_21_1() {
    run(
        "neoforge-1.21.1",
        "1.21.1",
        LoaderSpec::NeoForge {
            version: "21.1.252".into(),
        },
        LoaderKind::NeoForge,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "servidor"]
async fn server_forge_1_20_1() {
    run(
        "forge-1.20.1",
        "1.20.1",
        LoaderSpec::Forge {
            version: "47.4.10".into(),
        },
        LoaderKind::Forge,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "servidor"]
async fn server_forge_1_12_2() {
    run(
        "forge-1.12.2",
        "1.12.2",
        LoaderSpec::Forge {
            version: "14.23.5.2860".into(),
        },
        LoaderKind::Forge,
    )
    .await;
}
