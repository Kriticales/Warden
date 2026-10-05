//! Instalação, atualização e remoção de Javas com servidores simulados (critério 2 da L-01 e
//! CA-T21-04). Os pacotes são JREs falsos com o formato dos do Temurin; o validador falso lê o
//! arquivo `release` do JRE.

// Nos auxiliares dos testes, `unwrap`, `expect` e `panic!` são a forma idiomática de falhar
// (o `clippy.toml` só os libera dentro das funções `#[test]`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod comum;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use comum::{env, fixture, jre_package, latest_item, platform, sha256};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use warden_core::{CancellationToken, DomainCode, DomainError, NoProgress, PackId};
use warden_java::{
    Error, JavaChoiceReason, JavaChoiceRequest, JavaErrorCode, JavaRequirement, JavaVersion,
    LoaderKind, PackJavaInput, RuntimeSource,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn req(major: u32) -> JavaRequirement {
    JavaRequirement {
        major,
        max_update: None,
    }
}

/// Publica no servidor simulado um JRE como "o mais novo" do major.
async fn publish_latest(server: &MockServer, major: u32, version: &str, release: &str) -> Vec<u8> {
    let parsed = JavaVersion::parse(version).unwrap();
    let (name, bytes) = jre_package(major, version, &format!("{release}-jre"));
    let link = format!("{}/dl/{name}", server.uri());
    Mock::given(method("GET"))
        .and(path(format!("/v3/assets/latest/{major}/hotspot")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([latest_item(
                &parsed, release, &name, &link, &bytes
            )])),
        )
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/dl/{name}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
        .mount(server)
        .await;
    bytes
}

fn base(server: &MockServer) -> String {
    format!("{}/v3/", server.uri())
}

fn no_mojang(server: &MockServer) -> String {
    format!("{}/mojang/all.json", server.uri())
}

#[tokio::test]
async fn instala_o_temurin_valida_e_nao_baixa_de_novo() {
    let server = MockServer::start().await;
    publish_latest(&server, 21, "21.0.12.1+1", "jdk-21.0.12.1+1").await;
    let env = env(&base(&server), &no_mojang(&server));
    let cancel = CancellationToken::new();
    let runtime = env
        .runtimes
        .ensure(req(21), &NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(runtime.source, RuntimeSource::Temurin);
    assert_eq!(runtime.version, JavaVersion::new(21, 0, 12, 1, 1));
    assert_eq!(
        runtime.id.as_str(),
        format!("temurin-21-jdk-21.0.12.1+1-{}", platform().arch_key())
    );
    assert_eq!(runtime.vendor, "Eclipse Adoptium");
    assert!(runtime.java.is_file());
    assert!(runtime.launcher.is_file());
    assert!(runtime.home.join("lib").join("modules").is_file());
    assert!(runtime.archive_sha256.is_some());
    assert_eq!(env.probe.calls.load(Ordering::SeqCst), 1);
    // Só a pasta do Java; o pacote baixado foi apagado.
    assert_eq!(env.runtime_dir_entries(), vec![runtime.id.to_string()]);
    assert_eq!(std::fs::read_dir(env.downloads_dir()).unwrap().count(), 0);

    // Já instalado: nenhum pedido novo.
    let requests_before = server.received_requests().await.unwrap().len();
    let again = env
        .runtimes
        .ensure(req(21), &NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(again, runtime);
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        requests_before
    );
    assert_eq!(env.runtimes.installed().unwrap(), vec![runtime]);
}

/// Servidor que anuncia o pacote inteiro e corta a conexão no meio, em toda tentativa.
async fn truncating_server(bytes: Vec<u8>) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            counter.fetch_add(1, Ordering::SeqCst);
            let bytes = bytes.clone();
            tokio::spawn(async move {
                let mut request = [0_u8; 4096];
                let _ = socket.read(&mut request).await;
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/zip\r\nConnection: close\r\n\r\n",
                    bytes.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&bytes[..bytes.len() / 2]).await;
                let _ = socket.flush().await;
                // Fecha sem mandar o resto.
            });
        }
    });
    (format!("http://{address}"), hits)
}

#[tokio::test]
async fn l01_ca2_download_interrompido_nao_deixa_runtime_parcial() {
    let version = JavaVersion::new(21, 0, 12, 1, 1);
    let (name, bytes) = jre_package(21, "21.0.12.1+1", "jdk-21.0.12.1+1-jre");
    let (cut_base, hits) = truncating_server(bytes.clone()).await;
    let server = MockServer::start().await;
    let link = format!("{cut_base}/dl/{name}");
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/21/hotspot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([latest_item(
                &version,
                "jdk-21.0.12.1+1",
                &name,
                &link,
                &bytes
            )])),
        )
        .mount(&server)
        .await;
    let env = env(&base(&server), &no_mojang(&server));
    let cancel = CancellationToken::new();
    let error = env
        .runtimes
        .ensure(req(21), &NoProgress, &cancel)
        .await
        .unwrap_err();
    assert!(hits.load(Ordering::SeqCst) >= 1);
    assert!(
        matches!(
            error.code(),
            DomainCode::Core(warden_core::CoreErrorCode::NetworkUnavailable)
        ),
        "{error:?}"
    );
    // Nenhum Java instalado, nenhuma pasta em shared/runtimes, nada para o validador.
    assert!(env.runtimes.installed().unwrap().is_empty());
    assert!(
        env.runtime_dir_entries().is_empty(),
        "{:?}",
        env.runtime_dir_entries()
    );
    assert_eq!(env.probe.calls.load(Ordering::SeqCst), 0);
    let overview = env.runtimes.overview(&[]).unwrap();
    assert!(overview.runtimes.is_empty());
    assert_eq!(overview.broken_count, 0);

    // Com a rede de volta, a mesma instalação termina.
    server.reset().await;
    publish_latest(&server, 21, "21.0.12.1+1", "jdk-21.0.12.1+1").await;
    let runtime = env
        .runtimes
        .ensure(req(21), &NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(runtime.version, version);
    assert_eq!(env.runtime_dir_entries(), vec![runtime.id.to_string()]);
}

#[tokio::test]
async fn l01_ca2_pacote_com_hash_errado_nao_e_instalado() {
    let server = MockServer::start().await;
    let version = JavaVersion::new(17, 0, 20, 1, 1);
    let (name, bytes) = jre_package(17, "17.0.20.1+1", "jdk-17.0.20.1+1-jre");
    let mut tampered = bytes.clone();
    let last = tampered.len() - 30;
    tampered[last] ^= 0xFF;
    let link = format!("{}/dl/{name}", server.uri());
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/17/hotspot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([latest_item(
                &version,
                "jdk-17.0.20.1+1",
                &name,
                &link,
                &bytes
            )])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/dl/{name}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tampered))
        .mount(&server)
        .await;
    let env = env(&base(&server), &no_mojang(&server));
    let error = env
        .runtimes
        .ensure(req(17), &NoProgress, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(JavaErrorCode::DownloadCorrupted),
        "{error:?}"
    );
    assert!(env.runtimes.installed().unwrap().is_empty());
    assert!(env.runtime_dir_entries().is_empty());
    // O `.part` com hash errado também foi apagado.
    let leftovers: Vec<_> = std::fs::read_dir(env.downloads_dir())
        .map(|entries| entries.map(|e| e.unwrap().file_name()).collect())
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    assert_eq!(env.probe.calls.load(Ordering::SeqCst), 0);
}

/// Publica um pacote arbitrário (com o hash certo) como o Java 21.
async fn publish_raw(server: &MockServer, name: &str, bytes: Vec<u8>, version: &str) {
    let parsed = JavaVersion::parse(version).unwrap();
    let link = format!("{}/dl/{name}", server.uri());
    Mock::given(method("GET"))
        .and(path(format!("/v3/assets/latest/{}/hotspot", parsed.major)))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([latest_item(
                &parsed, "jdk-x", name, &link, &bytes
            )])),
        )
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/dl/{name}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(server)
        .await;
}

fn package_name() -> &'static str {
    match platform().os {
        warden_java::Os::Windows => "pacote.zip",
        warden_java::Os::Linux => "pacote.tar.gz",
    }
}

#[tokio::test]
async fn l01_ca2_falha_ao_extrair_ou_validar_nao_deixa_runtime() {
    // 1. Pacote com caminho que sai da pasta (zip slip).
    let mut evil = std::io::Cursor::new(Vec::new());
    {
        use std::io::Write as _;
        let mut zip = zip::ZipWriter::new(&mut evil);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("jre/bin/java.exe", options).unwrap();
        zip.write_all(b"x").unwrap();
        zip.start_file("../fora.txt", options).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
    }
    let evil_tar = {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        header.set_mode(0o644);
        // `set_path` recusa `..`; o nome vai cru no cabeçalho, como num pacote malicioso.
        header.as_old_mut().name[..11].copy_from_slice(b"../fora.txt");
        header.set_cksum();
        tar.append(&header, &b"x"[..]).unwrap();
        tar.into_inner().unwrap().finish().unwrap()
    };
    let evil_bytes = match platform().os {
        warden_java::Os::Windows => evil.into_inner(),
        warden_java::Os::Linux => evil_tar,
    };
    // 2. JRE sem o `release` (o validador falha) e 3. JRE que diz ser outro major.
    let (_, no_release) = {
        let (name, bytes) = jre_package(21, "21.0.12.1+1", "jdk-jre");
        (name, strip_release(&bytes))
    };
    let (_, wrong_major) = jre_package(21, "17.0.20.1+1", "jdk-jre");
    let cases: [(&str, Vec<u8>, JavaErrorCode); 3] = [
        ("zip slip", evil_bytes, JavaErrorCode::ArchiveInvalid),
        ("sem release", no_release, JavaErrorCode::ValidationFailed),
        ("outro major", wrong_major, JavaErrorCode::UnexpectedVersion),
    ];
    for (label, bytes, expected) in cases {
        let server = MockServer::start().await;
        publish_raw(&server, package_name(), bytes, "21.0.12.1+1").await;
        let env = env(&base(&server), &no_mojang(&server));
        let error = env
            .runtimes
            .ensure(req(21), &NoProgress, &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            DomainCode::Domain(expected),
            "{label}: {error:?}"
        );
        assert!(env.runtimes.installed().unwrap().is_empty(), "{label}");
        assert!(
            env.runtime_dir_entries().is_empty(),
            "{label}: {:?}",
            env.runtime_dir_entries()
        );
        assert!(!env.dir.path().join("fora.txt").exists(), "{label}");
        assert!(
            !env.dir.path().join("runtimes").join("fora.txt").exists(),
            "{label}"
        );
    }
}

/// O pacote sem o arquivo `release`.
fn strip_release(bytes: &[u8]) -> Vec<u8> {
    match platform().os {
        warden_java::Os::Windows => {
            let mut source = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
            let mut out = std::io::Cursor::new(Vec::new());
            {
                let mut zip = zip::ZipWriter::new(&mut out);
                for index in 0..source.len() {
                    let file = source.by_index_raw(index).unwrap();
                    if !file.name().ends_with("/release") {
                        zip.raw_copy_file(file).unwrap();
                    }
                }
                zip.finish().unwrap();
            }
            out.into_inner()
        }
        warden_java::Os::Linux => {
            let mut source = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
            let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            let mut out = tar::Builder::new(encoder);
            for entry in source.entries().unwrap() {
                let mut entry = entry.unwrap();
                let name = entry.path().unwrap().to_string_lossy().into_owned();
                if name.ends_with("/release") {
                    continue;
                }
                let mut header = entry.header().clone();
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut data).unwrap();
                out.append_data(&mut header, &name, data.as_slice())
                    .unwrap();
            }
            out.into_inner().unwrap().finish().unwrap()
        }
    }
}

#[tokio::test]
async fn l01_ca2_falha_injetada_depois_do_download() {
    let server = MockServer::start().await;
    publish_latest(&server, 25, "25.0.4.1+1", "jdk-25.0.4.1+1").await;
    let env = env(&base(&server), &no_mojang(&server));
    let guard = warden_core::fault::arm("java.install.after_download");
    let error = env
        .runtimes
        .ensure(req(25), &NoProgress, &CancellationToken::new())
        .await
        .unwrap_err();
    drop(guard);
    assert!(matches!(error, Error::Core(_)), "{error:?}");
    assert!(env.runtimes.installed().unwrap().is_empty());
    assert!(env.runtime_dir_entries().is_empty());
    // Sem a falha, instala.
    env.runtimes
        .ensure(req(25), &NoProgress, &CancellationToken::new())
        .await
        .unwrap();
}

#[tokio::test]
async fn l01_ca2_cancelar_no_meio_do_download() {
    let server = MockServer::start().await;
    let version = JavaVersion::new(21, 0, 12, 1, 1);
    let (name, bytes) = jre_package(21, "21.0.12.1+1", "jdk-jre");
    let link = format!("{}/dl/{name}", server.uri());
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/21/hotspot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([latest_item(
                &version,
                "jdk-21.0.12.1+1",
                &name,
                &link,
                &bytes
            )])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/dl/{name}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(bytes)
                .set_delay(Duration::from_secs(30)),
        )
        .mount(&server)
        .await;
    let env = env(&base(&server), &no_mojang(&server));
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        trigger.cancel();
    });
    let error = env
        .runtimes
        .ensure(req(21), &NoProgress, &cancel)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Cancelled), "{error:?}");
    assert!(env.runtimes.installed().unwrap().is_empty());
    assert!(env.runtime_dir_entries().is_empty());
}

#[tokio::test]
async fn sobras_de_queda_sao_apagadas_e_nunca_listadas() {
    let server = MockServer::start().await;
    let env = env(&base(&server), &no_mojang(&server));
    let temp = env.runtimes_dir().join(".tmp-01J00000000000000000000000");
    std::fs::create_dir_all(temp.join("bin")).unwrap();
    std::fs::write(temp.join("bin").join("java.exe"), b"x").unwrap();
    std::fs::write(temp.join("warden-runtime.json"), b"{}").unwrap();
    std::fs::create_dir_all(env.runtimes_dir().join(".trash-01J00000000000000000000000")).unwrap();
    assert!(env.runtimes.installed().unwrap().is_empty());
    assert_eq!(env.runtimes.overview(&[]).unwrap().broken_count, 0);
    assert_eq!(env.runtimes.startup_cleanup().unwrap(), 2);
    assert!(env.runtime_dir_entries().is_empty());
}

/// Índice e manifesto da Mojang com um JRE falso de 3 arquivos.
async fn publish_mojang(server: &MockServer, component: &str, version: &str) {
    let release = format!("JAVA_VERSION=\"{version}\"\n").into_bytes();
    let files = [
        ("release", release),
        ("bin/java.exe", b"MZ".to_vec()),
        ("bin/javaw.exe", b"MZw".to_vec()),
        ("bin/java", b"#!".to_vec()),
    ];
    let mut entries = serde_json::Map::new();
    entries.insert("bin".into(), serde_json::json!({"type": "directory"}));
    for (name, bytes) in &files {
        let sha1 = warden_packwiz::hash::hash_bytes(warden_packwiz::HashFormat::Sha1, bytes);
        let url = format!("{}/objects/{sha1}", server.uri());
        Mock::given(method("GET"))
            .and(path(format!("/objects/{sha1}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
            .mount(server)
            .await;
        entries.insert(
            (*name).into(),
            serde_json::json!({"type": "file", "executable": name.starts_with("bin/"),
                "downloads": {"raw": {"sha1": sha1, "size": bytes.len(), "url": url}}}),
        );
    }
    let manifest = serde_json::to_vec(&serde_json::json!({ "files": entries })).unwrap();
    let manifest_sha1 =
        warden_packwiz::hash::hash_bytes(warden_packwiz::HashFormat::Sha1, &manifest);
    Mock::given(method("GET"))
        .and(path("/mojang/manifest.json"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(manifest.clone()))
        .mount(server)
        .await;
    let key = platform().mojang_key().unwrap();
    let index = serde_json::json!({
        key: {
            component: [{
                "availability": {"group": 1, "progress": 100},
                "manifest": {"sha1": manifest_sha1, "size": manifest.len(), "url": format!("{}/mojang/manifest.json", server.uri())},
                "version": {"name": version, "released": "2025-05-19T08:34:00+00:00"}
            }],
            "minecraft-java-exe": []
        }
    });
    Mock::given(method("GET"))
        .and(path("/mojang/all.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(index))
        .mount(server)
        .await;
}

#[tokio::test]
async fn adoptium_fora_do_ar_usa_o_runtime_da_mojang() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/21/hotspot"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    publish_mojang(&server, "java-runtime-delta", "21.0.7").await;
    let env = env(&base(&server), &no_mojang(&server));
    let runtime = env
        .runtimes
        .ensure(req(21), &NoProgress, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(runtime.source, RuntimeSource::Mojang);
    assert_eq!(runtime.version, JavaVersion::new(21, 0, 7, 0, 0));
    assert_eq!(
        runtime.mojang_component.as_deref(),
        Some("java-runtime-delta")
    );
    assert!(runtime.java.is_file());
    assert_eq!(env.runtime_dir_entries(), vec![runtime.id.to_string()]);
}

#[tokio::test]
async fn sem_nenhuma_fonte_o_erro_e_o_do_adoptium() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/21/hotspot"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mojang/all.json"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let env = env(&base(&server), &no_mojang(&server));
    let error = env
        .runtimes
        .ensure(req(21), &NoProgress, &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(JavaErrorCode::SourceUnavailable)
    );
    assert_eq!(error.params()["source"], "Adoptium");
    assert!(env.runtime_dir_entries().is_empty());
}

#[tokio::test]
async fn forge_1165_antigo_instala_o_java_8_ate_312() {
    let server = MockServer::start().await;
    // A resposta real do endpoint de intervalo (8u312 e 8u302), com o link apontando para cá.
    let mut releases: serde_json::Value = serde_json::from_str(&fixture(&format!(
        "2026-10-05-adoptium-version-8-ate-312-{}-x64-jre.json",
        platform().adoptium_os()
    )))
    .unwrap();
    let (name, bytes) = jre_package(8, "1.8.0_312-b07", "jdk8u312-b07-jre");
    for release in releases.as_array_mut().unwrap() {
        for binary in release["binaries"].as_array_mut().unwrap() {
            binary["package"]["link"] = format!("{}/dl/{name}", server.uri()).into();
            binary["package"]["name"] = name.clone().into();
            binary["package"]["checksum"] = sha256(&bytes).into();
            binary["package"]["size"] = bytes.len().into();
        }
    }
    Mock::given(method("GET"))
        .and(path("/v3/assets/version/%5B8.0.0%2C8.0.313%29"))
        .respond_with(ResponseTemplate::new(200).set_body_json(releases))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/dl/{name}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&server)
        .await;
    let env = env(&base(&server), &no_mojang(&server));
    let request = JavaChoiceRequest::automatic("1.16.5", LoaderKind::Forge, Some("36.2.25"));
    let choice = env.runtimes.choose(&request).unwrap();
    assert_eq!(choice.reason, JavaChoiceReason::Forge1165Old);
    assert!(choice.runtime.is_none());
    let runtime = env
        .runtimes
        .ensure_choice(&choice, &NoProgress, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(runtime.version, JavaVersion::new(8, 0, 312, 0, 7));
    assert_eq!(runtime.update_cap, Some(312));
    // Agora a escolha acha o Java instalado.
    assert_eq!(
        env.runtimes.choose(&request).unwrap().runtime.unwrap().id,
        runtime.id
    );
}

fn pack(name: &str, minecraft: &str, loader: LoaderKind, version: &str) -> PackJavaInput {
    PackJavaInput {
        pack_id: PackId::new(),
        name: name.into(),
        request: JavaChoiceRequest::automatic(minecraft, loader, Some(version)),
    }
}

#[tokio::test]
// Um roteiro só, na ordem do critério: tabela, atualização com jogo aberto, jogo fecha, antiga sai.
#[allow(clippy::too_many_lines)]
async fn ca_t21_04_tabela_de_java_atualizacao_e_remocao_da_antiga() {
    let server = MockServer::start().await;
    publish_latest(&server, 17, "17.0.15", "jdk-17.0.15+6").await;
    publish_latest(&server, 8, "1.8.0_504-b01", "jdk8u504-b01").await;
    let env = env(&base(&server), &no_mojang(&server));
    let cancel = CancellationToken::new();
    let old17 = env
        .runtimes
        .ensure(req(17), &NoProgress, &cancel)
        .await
        .unwrap();
    let java8 = env
        .runtimes
        .ensure(req(8), &NoProgress, &cancel)
        .await
        .unwrap();

    // A tabela: cada Java com os packs que o usam e o motivo.
    let packs = vec![
        pack(
            "Antigão 1.7.10",
            "1.7.10",
            LoaderKind::Forge,
            "10.13.4.1614",
        ),
        pack("Create 1.20.1", "1.20.1", LoaderKind::Forge, "47.4.10"),
        pack("Fabric 1.20.1", "1.20.1", LoaderKind::Fabric, "0.19.5"),
        pack("Novo 26.3", "26.3", LoaderKind::Fabric, "0.19.5"),
    ];
    let overview = env.runtimes.overview(&packs).unwrap();
    assert_eq!(overview.runtimes.len(), 2);
    let row17 = overview
        .runtimes
        .iter()
        .find(|r| r.runtime.id == old17.id)
        .unwrap();
    let names17: Vec<&str> = row17.packs.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names17, vec!["Create 1.20.1", "Fabric 1.20.1"]);
    assert_eq!(
        row17.packs[0].choice.reason,
        JavaChoiceReason::NewestProvenForRange
    );
    let row8 = overview
        .runtimes
        .iter()
        .find(|r| r.runtime.id == java8.id)
        .unwrap();
    assert_eq!(row8.packs.len(), 1);
    let legacy = &row8.packs[0].choice;
    assert_eq!(legacy.reason, JavaChoiceReason::ForgeLegacyJava8);
    // O motivo do Forge 1.7.10 diz que essa versão só abre no Java 8.
    assert!(legacy.automatic.explanation.contains("só abre no Java 8"));
    // O pack 26.3 precisa do Java 25, que ainda será baixado.
    assert_eq!(overview.packs_to_download.len(), 1);
    assert_eq!(overview.packs_to_download[0].choice.major, 25);
    assert_eq!(
        overview.packs_to_download[0].choice.reason,
        JavaChoiceReason::NewestAvailable
    );

    // Uma atualização nova do Java 17 é publicada (servidor simulado).
    server.reset().await;
    publish_latest(&server, 17, "17.0.20.1+1", "jdk-17.0.20.1+1").await;
    publish_latest(&server, 8, "1.8.0_504-b01", "jdk8u504-b01").await;
    // Um jogo está usando o 17 antigo: a nova é instalada, a antiga fica até o jogo fechar.
    let lease = env.runtimes.lease(&old17.id);
    let report = env
        .runtimes
        .check_updates(&NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(report.updated.len(), 1);
    let update = &report.updated[0];
    assert_eq!(update.from, old17.id);
    assert_eq!(update.to.version, JavaVersion::new(17, 0, 20, 1, 1));
    assert!(!update.old_removed);
    assert_eq!(report.up_to_date, vec![java8.id.clone()]);
    let installed = env.runtimes.installed().unwrap();
    assert_eq!(installed.len(), 3);
    let old_now = installed.iter().find(|r| r.id == old17.id).unwrap();
    assert_eq!(old_now.superseded_by.as_ref(), Some(&update.to.id));
    // Os packs do 17 já usam a versão nova.
    let overview = env.runtimes.overview(&packs).unwrap();
    let new_row = overview
        .runtimes
        .iter()
        .find(|r| r.runtime.id == update.to.id)
        .unwrap();
    assert_eq!(new_row.packs.len(), 2);
    let superseded = overview
        .runtimes
        .iter()
        .find(|r| r.runtime.id == old17.id)
        .unwrap();
    assert!(superseded.in_game);
    assert!(superseded.packs.is_empty());
    // Remover à força um Java em uso é recusado.
    let error = env.runtimes.remove(&old17.id).unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(JavaErrorCode::RuntimeInUse)
    );

    // O jogo fechou: a antiga é removida.
    drop(lease);
    assert_eq!(
        env.runtimes.remove_superseded_unused().unwrap(),
        vec![old17.id.clone()]
    );
    let ids: Vec<String> = env
        .runtimes
        .installed()
        .unwrap()
        .iter()
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(!ids.contains(&old17.id.to_string()));
    assert!(!old17.home.exists());

    // Procurar de novo: nada a fazer.
    let report = env
        .runtimes
        .check_updates(&NoProgress, &cancel)
        .await
        .unwrap();
    assert!(report.updated.is_empty());
    assert_eq!(report.up_to_date.len(), 2);
}

#[tokio::test]
async fn ca_t21_04_atualizacao_sem_jogo_aberto_remove_a_antiga_na_hora() {
    let server = MockServer::start().await;
    publish_latest(&server, 17, "17.0.15", "jdk-17.0.15+6").await;
    let env = env(&base(&server), &no_mojang(&server));
    let cancel = CancellationToken::new();
    let old17 = env
        .runtimes
        .ensure(req(17), &NoProgress, &cancel)
        .await
        .unwrap();
    server.reset().await;
    publish_latest(&server, 17, "17.0.20.1+1", "jdk-17.0.20.1+1").await;
    let report = env
        .runtimes
        .check_updates(&NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(report.updated.len(), 1);
    assert!(report.updated[0].old_removed);
    assert_eq!(report.removed, vec![old17.id.clone()]);
    let installed = env.runtimes.installed().unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].version, JavaVersion::new(17, 0, 20, 1, 1));
    assert_eq!(env.runtime_dir_entries(), vec![installed[0].id.to_string()]);
}

#[tokio::test]
async fn atualizacao_com_fonte_fora_do_ar_nao_perde_o_java() {
    let server = MockServer::start().await;
    publish_latest(&server, 21, "21.0.12.1+1", "jdk-21.0.12.1+1").await;
    let env = env(&base(&server), &no_mojang(&server));
    let cancel = CancellationToken::new();
    let java21 = env
        .runtimes
        .ensure(req(21), &NoProgress, &cancel)
        .await
        .unwrap();
    server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let report = env
        .runtimes
        .check_updates(&NoProgress, &cancel)
        .await
        .unwrap();
    assert_eq!(report.failed, vec![java21.id.clone()]);
    assert_eq!(env.runtimes.installed().unwrap(), vec![java21]);
}

#[tokio::test]
async fn remover_javas_sem_uso_e_um_java() {
    let server = MockServer::start().await;
    publish_latest(&server, 17, "17.0.20.1+1", "jdk-17.0.20.1+1").await;
    publish_latest(&server, 21, "21.0.12.1+1", "jdk-21.0.12.1+1").await;
    let env = env(&base(&server), &no_mojang(&server));
    let cancel = CancellationToken::new();
    let java17 = env
        .runtimes
        .ensure(req(17), &NoProgress, &cancel)
        .await
        .unwrap();
    let java21 = env
        .runtimes
        .ensure(req(21), &NoProgress, &cancel)
        .await
        .unwrap();
    std::fs::create_dir_all(env.runtimes_dir().join("pasta-quebrada")).unwrap();
    assert_eq!(env.runtimes.overview(&[]).unwrap().broken_count, 1);
    let packs = vec![pack("Create", "1.20.1", LoaderKind::Forge, "47.4.10")];
    let removed = env.runtimes.remove_unused(&packs).unwrap();
    assert_eq!(removed, vec![java21.id.clone()]);
    assert_eq!(env.runtimes.installed().unwrap(), vec![java17.clone()]);
    assert_eq!(env.runtime_dir_entries(), vec![java17.id.to_string()]);
    env.runtimes.remove(&java17.id).unwrap();
    assert!(env.runtime_dir_entries().is_empty());
    let error = env.runtimes.remove(&java17.id).unwrap_err();
    assert_eq!(
        error.code(),
        DomainCode::Domain(JavaErrorCode::RuntimeNotFound)
    );
}

#[tokio::test]
async fn duas_preparacoes_ao_mesmo_tempo_baixam_uma_vez() {
    let server = MockServer::start().await;
    publish_latest(&server, 21, "21.0.12.1+1", "jdk-21.0.12.1+1").await;
    let env = env(&base(&server), &no_mojang(&server));
    let cancel = CancellationToken::new();
    let (a, b) = tokio::join!(
        env.runtimes.ensure(req(21), &NoProgress, &cancel),
        env.runtimes.ensure(req(21), &NoProgress, &cancel)
    );
    assert_eq!(a.unwrap(), b.unwrap());
    let downloads = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.url.path().starts_with("/dl/"))
        .count();
    assert_eq!(downloads, 1);
    assert_eq!(env.probe.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancelar_a_atualizacao_no_meio_mantem_o_java_antigo_e_nao_deixa_sobra() {
    let server = MockServer::start().await;
    publish_latest(&server, 17, "17.0.15", "jdk-17.0.15+6").await;
    let env = env(&base(&server), &no_mojang(&server));
    let old17 = env
        .runtimes
        .ensure(req(17), &NoProgress, &CancellationToken::new())
        .await
        .unwrap();
    // A atualização nova demora a chegar; o usuário cancela no meio.
    server.reset().await;
    let version = JavaVersion::new(17, 0, 20, 1, 1);
    let (name, bytes) = jre_package(17, "17.0.20.1+1", "jdk-17.0.20.1+1-jre");
    let link = format!("{}/dl/{name}", server.uri());
    Mock::given(method("GET"))
        .and(path("/v3/assets/latest/17/hotspot"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([latest_item(
                &version,
                "jdk-17.0.20.1+1",
                &name,
                &link,
                &bytes
            )])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/dl/{name}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(bytes)
                .set_delay(Duration::from_secs(30)),
        )
        .mount(&server)
        .await;
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        trigger.cancel();
    });
    let error = env
        .runtimes
        .check_updates(&NoProgress, &cancel)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Cancelled), "{error:?}");
    assert_eq!(env.runtimes.installed().unwrap(), vec![old17.clone()]);
    assert_eq!(env.runtime_dir_entries(), vec![old17.id.to_string()]);
}
