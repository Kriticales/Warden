//! Testes contra o Adoptium e a Mojang de verdade (`cargo xtask test-network`; QUALITY §4.1).
//! Marcados `#[ignore = "rede"]`, com nome `rede_*`. Baixam Javas reais (50 a 110 MB cada) para
//! uma pasta temporária dentro de `WARDEN_DATA_ROOT` (ou do sistema, se ela não estiver
//! definida), apagada no fim; nada vai para as pastas do Warden instalado.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use warden_core::{CancellationToken, NoProgress};
use warden_http::{HttpClient, HttpConfig};
use warden_java::adoptium::AdoptiumClient;
use warden_java::mojang::MojangRuntimeClient;
use warden_java::{
    Arch, CompatibilityTable, JavaChoiceRequest, JavaProbe, JavaRequirement, JavaRuntimes,
    JavaRuntimesConfig, LoaderKind, Os, Platform, ProcessProbe, RuntimeSource,
};

fn http() -> HttpClient {
    HttpClient::new(HttpConfig::for_version("teste-rede")).unwrap()
}

/// Pasta temporária dentro de `WARDEN_DATA_ROOT`, quando definida (ADR-0053).
fn temp_dir() -> tempfile::TempDir {
    match std::env::var_os("WARDEN_DATA_ROOT").filter(|value| !value.is_empty()) {
        Some(root) => {
            std::fs::create_dir_all(&root).unwrap();
            tempfile::Builder::new()
                .prefix("java-rede-")
                .tempdir_in(root)
                .unwrap()
        }
        None => tempfile::tempdir().unwrap(),
    }
}

fn service(dir: &std::path::Path, adoptium: Option<&str>) -> JavaRuntimes {
    let http = http();
    let config = JavaRuntimesConfig {
        runtimes_dir: dir.join("shared").join("runtimes"),
        downloads_dir: dir.join("cache").join("tmp").join("java"),
        platform: Platform::current().unwrap(),
    };
    let adoptium = match adoptium {
        Some(base) => AdoptiumClient::with_base_url(http.clone(), base).unwrap(),
        None => AdoptiumClient::new(http.clone()).unwrap(),
    };
    let mojang = MojangRuntimeClient::new(http.clone()).unwrap();
    JavaRuntimes::with_clients(
        config,
        http,
        adoptium,
        mojang,
        Arc::new(ProcessProbe::default()),
    )
    .unwrap()
}

/// Critério 3 da L-01: baixa e valida o Temurin 21 (Windows e Linux na CI noturna).
#[tokio::test]
#[ignore = "rede"]
async fn rede_l01_ca3_baixa_e_valida_temurin_21() {
    let dir = temp_dir();
    let java = service(dir.path(), None);
    let cancel = CancellationToken::new();
    let choice = java
        .choose(&JavaChoiceRequest::automatic(
            "1.21.1",
            LoaderKind::NeoForge,
            Some("21.1.252"),
        ))
        .unwrap();
    assert_eq!(choice.major, 21);
    let runtime = java
        .ensure_choice(&choice, &NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(runtime.source, RuntimeSource::Temurin);
    assert_eq!(runtime.version.major, 21);
    assert!(runtime.launcher.is_file());
    // Validação de verdade, executando o Java instalado.
    let info = ProcessProbe::default().probe(&runtime.java).await.unwrap();
    assert_eq!(info.version, runtime.version);
    assert!(info.is_64bit);
    assert!(
        info.vendor.contains("Adoptium") || info.vendor.contains("Temurin"),
        "{}",
        info.vendor
    );
    // Recém-instalado: nenhuma atualização.
    let report = java.check_updates(&NoProgress, &cancel).await.unwrap();
    assert_eq!(report.up_to_date, vec![runtime.id.clone()]);
    assert!(report.updated.is_empty());
    // Escolha agora acha o Java instalado.
    let again = java
        .choose(&JavaChoiceRequest::automatic(
            "1.21.1",
            LoaderKind::NeoForge,
            Some("21.1.252"),
        ))
        .unwrap();
    assert_eq!(again.runtime.unwrap().id, runtime.id);
    java.remove(&runtime.id).unwrap();
    assert!(java.installed().unwrap().is_empty());
}

/// Critério 4 da L-01: os majors da tabela existem no Adoptium, para Windows e Linux.
#[tokio::test]
#[ignore = "rede"]
async fn rede_l01_ca4_majors_da_tabela_existem_no_adoptium() {
    let client = AdoptiumClient::new(http()).unwrap();
    let releases = client.available_releases(None).await.unwrap();
    let table = CompatibilityTable::builtin().unwrap();
    for major in &table.majors {
        assert!(
            releases.available_lts_releases.contains(major),
            "Java {major} não é LTS publicado: {releases:?}"
        );
        for os in [Os::Windows, Os::Linux] {
            let platform = Platform {
                os,
                arch: Arch::X64,
            };
            let build = client.latest(*major, platform, None).await.unwrap();
            let build = build.unwrap_or_else(|| panic!("sem Java {major} para {platform:?}"));
            assert_eq!(build.version.major, *major);
        }
    }
    // A regra do Forge 1.16.5 antigo precisa do Java 8 até a 312.
    for os in [Os::Windows, Os::Linux] {
        let platform = Platform {
            os,
            arch: Arch::X64,
        };
        let capped = client
            .latest_up_to(8, 312, platform, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(capped.version.security, 312, "{platform:?}");
    }
}

/// A alternativa: com o Adoptium inalcançável, instala o runtime da Mojang (Java 25) arquivo a
/// arquivo e o valida executando.
#[tokio::test]
#[ignore = "rede"]
async fn rede_runtime_da_mojang_quando_o_adoptium_falha() {
    let dir = temp_dir();
    // Porta 9 (discard) recusa conexão: o Adoptium "está fora do ar".
    let java = service(dir.path(), Some("http://127.0.0.1:9/v3/"));
    let runtime = java
        .ensure(
            JavaRequirement {
                major: 25,
                max_update: None,
            },
            &NoProgress,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(runtime.source, RuntimeSource::Mojang);
    assert_eq!(runtime.version.major, 25);
    assert_eq!(
        runtime.mojang_component.as_deref(),
        Some("java-runtime-epsilon")
    );
    let info = ProcessProbe::default().probe(&runtime.java).await.unwrap();
    assert_eq!(info.version.major, 25);
    assert!(info.is_64bit);
}

/// O índice da Mojang publica os quatro majors para Windows e Linux.
#[tokio::test]
#[ignore = "rede"]
async fn rede_mojang_publica_os_majors() {
    let client = MojangRuntimeClient::new(http()).unwrap();
    for os in [Os::Windows, Os::Linux] {
        let platform = Platform {
            os,
            arch: Arch::X64,
        };
        let components = client.components(platform, None).await.unwrap();
        for major in [8, 17, 21, 25] {
            assert!(
                MojangRuntimeClient::pick(&components, major, None).is_some(),
                "Mojang sem Java {major} para {platform:?}"
            );
        }
        // O jre-legacy (8u51 no Windows, 8u202 no Linux) serve ao Forge 1.16.5 antigo.
        assert!(MojangRuntimeClient::pick(&components, 8, Some(312)).is_some());
    }
}
