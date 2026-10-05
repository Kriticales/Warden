//! Clientes do Adoptium e da Mojang contra as respostas reais gravadas
//! (`tests/fixtures/http/`), servidas por um servidor simulado.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod comum;

use comum::{fixture, http};
use warden_java::adoptium::AdoptiumClient;
use warden_java::archive::ArchiveKind;
use warden_java::mojang::{ManifestEntry, MojangRuntimeClient, parse_manifest};
use warden_java::{Arch, JavaVersion, Os, Platform};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const WINDOWS: Platform = Platform {
    os: Os::Windows,
    arch: Arch::X64,
};
const LINUX: Platform = Platform {
    os: Os::Linux,
    arch: Arch::X64,
};

async fn adoptium_with_fixtures() -> (MockServer, AdoptiumClient) {
    let server = MockServer::start().await;
    for major in [8, 17, 21, 25] {
        for os in ["windows", "linux"] {
            Mock::given(method("GET"))
                .and(path(format!("/v3/assets/latest/{major}/hotspot")))
                .and(query_param("os", os))
                .and(query_param("image_type", "jre"))
                .and(query_param("architecture", "x64"))
                .and(query_param("vendor", "eclipse"))
                .respond_with(ResponseTemplate::new(200).set_body_string(fixture(&format!(
                    "2026-10-05-adoptium-latest-{major}-{os}-x64-jre.json"
                ))))
                .mount(&server)
                .await;
        }
    }
    for os in ["windows", "linux"] {
        Mock::given(method("GET"))
            .and(path("/v3/assets/version/%5B8.0.0%2C8.0.313%29"))
            .and(query_param("os", os))
            .and(query_param("release_type", "ga"))
            .respond_with(ResponseTemplate::new(200).set_body_string(fixture(&format!(
                "2026-10-05-adoptium-version-8-ate-312-{os}-x64-jre.json"
            ))))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v3/info/available_releases"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixture("2026-10-05-adoptium-available_releases.json")),
        )
        .mount(&server)
        .await;
    let client = AdoptiumClient::with_base_url(http(), &format!("{}/v3/", server.uri())).unwrap();
    (server, client)
}

#[tokio::test]
async fn adoptium_mais_novo_de_cada_major() {
    let (_server, client) = adoptium_with_fixtures().await;
    let expected = [
        (8, "jdk8u504-b01", JavaVersion::new(8, 0, 504, 0, 1)),
        (17, "jdk-17.0.20.1+1", JavaVersion::new(17, 0, 20, 1, 1)),
        (21, "jdk-21.0.12.1+1", JavaVersion::new(21, 0, 12, 1, 1)),
        (25, "jdk-25.0.4.1+1", JavaVersion::new(25, 0, 4, 1, 1)),
    ];
    for (major, release, version) in expected {
        let windows = client.latest(major, WINDOWS, None).await.unwrap().unwrap();
        assert_eq!(windows.release_name, release);
        assert_eq!(windows.version, version);
        assert_eq!(windows.kind, ArchiveKind::Zip, "{}", windows.package_name);
        assert_eq!(windows.sha256.len(), 64);
        assert!(windows.size > 30_000_000);
        assert_eq!(windows.link.scheme(), "https");
        let linux = client.latest(major, LINUX, None).await.unwrap().unwrap();
        assert_eq!(linux.kind, ArchiveKind::TarGz, "{}", linux.package_name);
        assert_eq!(linux.version, version);
    }
    let java21 = client.latest(21, WINDOWS, None).await.unwrap().unwrap();
    assert_eq!(
        java21.sha256,
        "d35f31e712f0fcf6ac5a093edc90204fbff22f720ba3950bd09d331d5e621636"
    );
}

#[tokio::test]
async fn adoptium_java_8_ate_312() {
    let (_server, client) = adoptium_with_fixtures().await;
    for platform in [WINDOWS, LINUX] {
        let build = client
            .latest_up_to(8, 312, platform, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(build.release_name, "jdk8u312-b07");
        assert_eq!(build.version, JavaVersion::new(8, 0, 312, 0, 7));
    }
}

#[tokio::test]
async fn adoptium_majors_publicados() {
    let (_server, client) = adoptium_with_fixtures().await;
    let releases = client.available_releases(None).await.unwrap();
    assert_eq!(releases.available_lts_releases, vec![8, 11, 17, 21, 25]);
    // A tabela só usa majors LTS publicados (critério 4; o teste de rede confere ao vivo).
    let table = warden_java::CompatibilityTable::builtin().unwrap();
    for major in &table.majors {
        assert!(
            releases.available_lts_releases.contains(major),
            "Java {major}"
        );
    }
}

#[tokio::test]
async fn adoptium_sem_build_e_fora_do_ar() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/16/hotspot"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/11/hotspot"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/17/hotspot"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/21/hotspot"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{\"x\":"))
        .mount(&server)
        .await;
    let client = AdoptiumClient::with_base_url(http(), &format!("{}/v3/", server.uri())).unwrap();
    assert!(client.latest(16, WINDOWS, None).await.unwrap().is_none());
    assert!(client.latest(11, WINDOWS, None).await.unwrap().is_none());
    let unavailable = client.latest(17, WINDOWS, None).await.unwrap_err();
    assert!(unavailable.is_source_failure(), "{unavailable:?}");
    assert_eq!(
        warden_core::DomainError::code(&unavailable),
        warden_core::DomainCode::Domain(warden_java::JavaErrorCode::SourceUnavailable)
    );
    let garbage = client.latest(21, WINDOWS, None).await.unwrap_err();
    assert!(garbage.is_source_failure(), "{garbage:?}");
}

#[tokio::test]
async fn adoptium_recusa_pacote_sem_sha256() {
    let server = MockServer::start().await;
    let mut body: serde_json::Value = serde_json::from_str(&fixture(
        "2026-10-05-adoptium-latest-21-windows-x64-jre.json",
    ))
    .unwrap();
    body[0]["binary"]["package"]["checksum"] = serde_json::Value::Null;
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/21/hotspot"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    let client = AdoptiumClient::with_base_url(http(), &format!("{}/v3/", server.uri())).unwrap();
    let error = client.latest(21, WINDOWS, None).await.unwrap_err();
    assert!(error.to_string().contains("SHA-256"), "{error}");
}

async fn mojang_with_fixtures() -> (MockServer, MojangRuntimeClient) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/all.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixture("2026-10-05-mojang-java-runtime-all.json")),
        )
        .mount(&server)
        .await;
    let client =
        MojangRuntimeClient::with_index_url(http(), &format!("{}/all.json", server.uri())).unwrap();
    (server, client)
}

#[tokio::test]
async fn mojang_componentes_por_plataforma() {
    let (_server, client) = mojang_with_fixtures().await;
    let windows = client.components(WINDOWS, None).await.unwrap();
    let names: Vec<&str> = windows.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"jre-legacy"));
    assert!(!names.contains(&"minecraft-java-exe"));
    let pick = |major, cap| MojangRuntimeClient::pick(&windows, major, cap).unwrap();
    assert_eq!(pick(8, None).version, JavaVersion::new(8, 0, 51, 0, 0));
    assert_eq!(pick(8, Some(312)).name, "jre-legacy");
    assert_eq!(pick(17, None).version_name, "17.0.15");
    assert_eq!(pick(21, None).name, "java-runtime-delta");
    assert_eq!(pick(25, None).name, "java-runtime-epsilon");
    assert!(MojangRuntimeClient::pick(&windows, 11, None).is_none());

    let linux = client.components(LINUX, None).await.unwrap();
    let legacy = MojangRuntimeClient::pick(&linux, 8, None).unwrap();
    assert_eq!(legacy.version_name, "8u202");
    // Linux ARM: a Mojang não publica.
    let arm = Platform {
        os: Os::Linux,
        arch: Arch::Aarch64,
    };
    assert!(client.components(arm, None).await.unwrap().is_empty());
}

#[test]
fn mojang_manifestos_reais() {
    let linux =
        parse_manifest(fixture("2026-10-05-mojang-jre-legacy-linux-manifest.json").as_bytes())
            .unwrap();
    let count = |kind: fn(&ManifestEntry) -> bool| linux.values().filter(|e| kind(e)).count();
    assert_eq!(count(|e| matches!(e, ManifestEntry::File { .. })), 300);
    assert_eq!(count(|e| matches!(e, ManifestEntry::Directory)), 88);
    assert_eq!(count(|e| matches!(e, ManifestEntry::Link { .. })), 3);
    assert!(matches!(
        linux.get("bin/java"),
        Some(ManifestEntry::File {
            executable: true,
            ..
        })
    ));
    let windows =
        parse_manifest(fixture("2026-10-05-mojang-epsilon-windows-x64-manifest.json").as_bytes())
            .unwrap();
    assert!(matches!(
        windows.get("bin/javaw.exe"),
        Some(ManifestEntry::File { .. })
    ));
    assert!(
        !windows
            .values()
            .any(|e| matches!(e, ManifestEntry::Link { .. }))
    );
}

#[tokio::test]
async fn mojang_manifesto_com_sha1_errado_e_recusado() {
    let (server, client) = mojang_with_fixtures().await;
    let body = fixture("2026-10-05-mojang-epsilon-windows-x64-manifest.json");
    Mock::given(method("GET"))
        .and(path("/manifest.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.replace("bin", "BIN")))
        .mount(&server)
        .await;
    let mut component =
        MojangRuntimeClient::pick(&client.components(WINDOWS, None).await.unwrap(), 25, None)
            .unwrap()
            .clone();
    component.manifest_url =
        warden_http::parse_url(&format!("{}/manifest.json", server.uri())).unwrap();
    let error = client.manifest(&component, None).await.unwrap_err();
    assert_eq!(
        warden_core::DomainError::code(&error),
        warden_core::DomainCode::Domain(warden_java::JavaErrorCode::DownloadCorrupted)
    );
}
