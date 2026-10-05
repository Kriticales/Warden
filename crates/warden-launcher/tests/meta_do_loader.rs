//! Critério 5 da L-02: com a versão do loader fixada e a instalação no cache, abrir de novo
//! não faz nenhum pedido ao meta do loader (S-R5-3 §13).
//!
//! O portablemc usa um cliente HTTP próprio, criado uma vez por processo, que respeita
//! `HTTPS_PROXY`. O teste sobe um proxy local que **conta** cada `CONNECT` por servidor e roda
//! a instalação num processo filho (este mesmo binário de teste) apontado para ele. O proxy
//! só deixa passar o manifesto da Mojang (`piston-meta.mojang.com`, que o motor confere a cada
//! instalação e que não é meta de loader); o resto é recusado e contado.
//!
//! A instalação "no cache" é montada à mão: o perfil do Fabric `fabric-1.20.1-0.19.5` herda de
//! uma versão vanilla mínima e local (sem bibliotecas nem assets), então nenhum arquivo do
//! jogo precisa ser baixado. O controle mostra que o contador funciona: sem o perfil do
//! Fabric, o motor pede `meta.fabricmc.net` e falha.
//!
//! Precisa de rede só para o manifesto da Mojang: `rede_*`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout
)]

mod comum;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use warden_core::{CancellationToken, NoProgress};
use warden_launcher::{
    EngineDirs, GameSpec, JavaRuntime, LauncherEngine, LoaderSpec, PortableMcEngine,
};

/// Único servidor que o proxy deixa passar.
const ALLOWED: &str = "piston-meta.mojang.com";

/// Pedidos `CONNECT` por servidor.
type Counts = Arc<Mutex<BTreeMap<String, usize>>>;

/// Lê o cabeçalho do pedido ao proxy.
async fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).await.ok()? == 0 || head.len() > 8_192 {
            return None;
        }
        head.push(byte[0]);
    }
    String::from_utf8(head).ok()
}

/// Atende uma conexão do proxy.
async fn serve(mut client: TcpStream, counts: Counts) {
    let Some(head) = read_head(&mut client).await else {
        return;
    };
    let target = head
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("CONNECT "))
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or("?")
        .to_owned();
    let host = target.split(':').next().unwrap_or("?").to_owned();
    *counts.lock().unwrap().entry(host.clone()).or_default() += 1;
    if host != ALLOWED {
        let _ = client
            .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
            .await;
        return;
    }
    let Ok(mut upstream) = TcpStream::connect(&target).await else {
        let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await;
        return;
    };
    if client
        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
        .await
        .is_err()
    {
        return;
    }
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
}

/// Sobe o proxy contador.
async fn start_proxy() -> (u16, Counts) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let counts: Counts = Arc::default();
    let accept_counts = Arc::clone(&counts);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(serve(stream, Arc::clone(&accept_counts)));
        }
    });
    (port, counts)
}

/// Grava o JSON de uma versão em `versions/<id>/<id>.json`.
fn write_version(versions: &Path, id: &str, json: &serde_json::Value) {
    let dir = versions.join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{id}.json")),
        serde_json::to_vec_pretty(json).unwrap(),
    )
    .unwrap();
}

/// A instalação local: vanilla mínima e, se pedido, o perfil do Fabric.
fn prepare(shared: &Path, with_fabric_profile: bool) {
    let versions = shared.join("versions");
    write_version(
        &versions,
        "warden-teste-vanilla",
        &serde_json::json!({
            "id": "warden-teste-vanilla",
            "type": "release",
            "releaseTime": "2023-06-12T13:25:51+00:00",
            "time": "2023-06-12T13:25:51+00:00",
            "mainClass": "net.minecraft.client.main.Main",
            "arguments": { "game": ["--gameDir", "${game_directory}"], "jvm": ["-cp", "${classpath}"] },
            "libraries": []
        }),
    );
    std::fs::write(
        versions
            .join("warden-teste-vanilla")
            .join("warden-teste-vanilla.jar"),
        b"",
    )
    .unwrap();
    if with_fabric_profile {
        write_version(
            &versions,
            "fabric-1.20.1-0.19.5",
            &serde_json::json!({
                "id": "fabric-1.20.1-0.19.5",
                "inheritsFrom": "warden-teste-vanilla",
                "type": "release",
                "releaseTime": "2023-06-12T13:25:51+00:00",
                "time": "2023-06-12T13:25:51+00:00",
                "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "arguments": { "game": [], "jvm": [] },
                "libraries": []
            }),
        );
        // O motor procura o `client.jar` na pasta da versão raiz.
        std::fs::write(
            versions
                .join("fabric-1.20.1-0.19.5")
                .join("fabric-1.20.1-0.19.5.jar"),
            b"",
        )
        .unwrap();
    }
}

/// Processo filho: instala o Fabric 1.20.1 com a versão fixa e diz o resultado.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "processo filho de rede_abrir_de_novo_nao_consulta_o_meta_do_loader"]
async fn filho_instala_pelo_proxy() {
    let Some(shared) = std::env::var_os("WARDEN_TESTE_SHARED") else {
        return;
    };
    let java = std::path::PathBuf::from(std::env::var_os("WARDEN_TESTE_JAVA").unwrap());
    let engine = PortableMcEngine::new(EngineDirs::under(Path::new(&shared)), None);
    let spec = GameSpec::new(
        "1.20.1",
        LoaderSpec::Fabric {
            version: "0.19.5".into(),
        },
    )
    .unwrap();
    let runtime = JavaRuntime {
        java: java.clone(),
        launcher: java,
        major: 17,
    };
    match engine
        .install(&spec, &runtime, &NoProgress, &CancellationToken::new())
        .await
    {
        Ok(game) => println!("RESULTADO instalou {}", game.version_id),
        Err(error) => println!(
            "RESULTADO erro {}",
            warden_core::error_chain(&error).replace('\n', " | ")
        ),
    }
}

/// Roda o filho com o proxy e devolve a linha de resultado.
async fn run_child(shared: &Path, java: &Path, port: u16) -> String {
    let proxy = format!("http://127.0.0.1:{port}");
    let output = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "filho_instala_pelo_proxy",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("WARDEN_TESTE_SHARED", shared)
        .env("WARDEN_TESTE_JAVA", java)
        .env("HTTPS_PROXY", &proxy)
        .env("HTTP_PROXY", &proxy)
        .env("ALL_PROXY", &proxy)
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .output()
        .await
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("RESULTADO ").map(ToOwned::to_owned))
        .unwrap_or_else(|| panic!("o filho não respondeu: {stdout}"))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "rede"]
async fn rede_abrir_de_novo_nao_consulta_o_meta_do_loader() {
    let Some(java) = comum::test_java() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();

    // Instalação no cache: nenhum pedido a meta.fabricmc.net.
    let cached = dir.path().join("com-cache");
    prepare(&cached, true);
    let (port, counts) = start_proxy().await;
    let result = run_child(&cached, &java, port).await;
    assert_eq!(result, "instalou fabric-1.20.1-0.19.5", "{counts:?}");
    let seen = counts.lock().unwrap().clone();
    println!("pedidos com cache: {seen:?}");
    assert!(
        seen.keys().all(|host| host == ALLOWED),
        "pedidos fora do manifesto da Mojang: {seen:?}"
    );
    assert!(!seen.contains_key("meta.fabricmc.net"));

    // Controle: sem o perfil do Fabric no cache, o motor pede o meta (e o proxy recusa).
    let empty = dir.path().join("sem-cache");
    prepare(&empty, false);
    let (port, counts) = start_proxy().await;
    let result = run_child(&empty, &java, port).await;
    assert!(result.starts_with("erro"), "{result}");
    let seen = counts.lock().unwrap().clone();
    println!("pedidos sem cache: {seen:?}");
    assert!(
        seen.get("meta.fabricmc.net").copied().unwrap_or(0) >= 1,
        "{seen:?}"
    );
}
