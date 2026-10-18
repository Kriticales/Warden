//! O ciclo de vida do servidor local com o servidor simulado (`fixtures/java/WardenFakeServer`):
//! EULA, `server.properties`, abertura, "pronto", jogador entrando, comandos pelo stdin, `stop`,
//! encerramento forçado e a porta liberada (CA-T27-01 na parte sem rede, CA-T27-02, CA-T27-05).
//!
//! Usam o Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`; sem Java, avisam e passam (com
//! `WARDEN_REQUIRE_EXTERNALS=1`, a falta é falha).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::missing_panics_doc
)]

use std::io::Write as _;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use warden_launcher::outcome::Outcome;
use warden_launcher::{GameSpec, JavaRuntime, LoaderSpec};
use warden_server::process::wait_ready;
use warden_server::properties::{free_port_from, port_is_free};
use warden_server::{
    ControlledProperties, InstalledServer, ReadyFailure, RunningServer, ServerCommand, ServerEvent,
    ServerLaunch, write_eula, write_properties,
};

fn test_java() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let candidate = std::env::var_os("WARDEN_TEST_JAVA")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("JAVA_HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join("bin").join(exe))
        });
    if let Some(java) = candidate.filter(|path| path.is_file()) {
        return Some(java);
    }
    assert!(
        std::env::var_os("WARDEN_REQUIRE_EXTERNALS").is_none_or(|value| value != "1"),
        "WARDEN_REQUIRE_EXTERNALS=1, mas não há Java (defina WARDEN_TEST_JAVA ou JAVA_HOME)"
    );
    eprintln!("AVISO: sem Java (WARDEN_TEST_JAVA ou JAVA_HOME); teste com processo Java pulado.");
    None
}

fn class_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("java")
}

/// Um servidor simulado pronto para abrir na pasta `dir`, com a EULA aceita e a porta livre.
fn prepared(dir: &Path, mode: &[&str]) -> (InstalledServer, u16) {
    write_eula(dir, Some("2026-10-13T12:00:00Z")).unwrap();
    // Os testes rodam ao mesmo tempo: cada um parte de uma porta dada pelo sistema (no app há
    // um servidor por vez e a busca começa na 25565).
    let start = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let port = free_port_from(start).unwrap();
    write_properties(
        dir,
        &ControlledProperties {
            port,
            motd: "Vale de Teste".into(),
        },
    )
    .unwrap();
    let server = InstalledServer {
        dir: dir.to_path_buf(),
        spec: GameSpec::new("1.20.1", LoaderSpec::Vanilla).unwrap(),
        release_time: "2023-06-12T13:25:51+00:00".into(),
        launch: ServerLaunch::Class {
            class_dir: class_dir(),
            main_class: "WardenFakeServer".into(),
            args: mode.iter().map(|&arg| arg.to_owned()).collect(),
        },
    };
    (server, port)
}

fn command(java: &Path) -> ServerCommand {
    ServerCommand {
        java: JavaRuntime {
            java: java.to_path_buf(),
            launcher: java.to_path_buf(),
            major: 21,
        },
        memory_mb: 256,
        extra_jvm_args: Vec::new(),
    }
}

/// Lê as linhas até uma que contenha `text` (ou a saída do servidor).
async fn wait_line(events: &mut warden_server::ServerEvents, text: &str) -> Vec<String> {
    let mut seen = Vec::new();
    let found = tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(event) = events.recv().await {
            match event {
                ServerEvent::Line(line) => {
                    let hit = line.text.contains(text);
                    seen.push(line.text);
                    if hit {
                        return true;
                    }
                }
                ServerEvent::Exited(_) => return false,
                ServerEvent::Ready { .. } | ServerEvent::Joined { .. } => {}
            }
        }
        false
    })
    .await;
    assert!(matches!(found, Ok(true)), "sem a linha {text:?}: {seen:?}");
    seen
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t27_01_abre_fica_pronto_recebe_comando_e_fecha_com_stop() {
    let Some(java) = test_java() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (server, port) = prepared(dir.path(), &[]);
    let log = dir.path().join("sessao").join("output.log");
    let (running, mut events) =
        RunningServer::spawn(&command(&java).build(&server), Some(log.clone()))
            .await
            .unwrap();
    let handle = running.handle();

    let mut lines = Vec::new();
    let ready = wait_ready(&mut events, Duration::from_secs(30), |event| {
        if let ServerEvent::Line(line) = event {
            lines.push(line.text.clone());
        }
    })
    .await
    .unwrap();
    assert!(
        matches!(
            ready,
            ServerEvent::Ready {
                reported_ms: Some(512),
                ..
            }
        ),
        "{ready:?}"
    );
    // O servidor recebeu o server.properties do Warden e escuta só em 127.0.0.1.
    assert!(
        lines.iter().any(|line| line
            .contains("online-mode=false enable-rcon=false enable-query=false motd=Vale de Teste")),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains(&format!("127.0.0.1:{port}")))
    );
    assert!(
        !port_is_free(port),
        "a porta fica ocupada com o servidor aberto"
    );

    // Um jogador entra (o que o jogo faz pelo Quick Play).
    let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
    client.write_all(b"Jogador\n").unwrap();
    let joined = tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(event) = events.recv().await {
            if let ServerEvent::Joined { player, .. } = event {
                return player;
            }
        }
        String::new()
    })
    .await
    .unwrap();
    assert_eq!(joined, "Jogador");

    handle.send_command("say oi").await.unwrap();
    wait_line(&mut events, "Comando recebido: say oi").await;
    assert!(handle.send_command("").await.is_err());
    assert!(handle.send_command("a\nstop").await.is_err());

    let stop = handle.stop().await;
    assert!(stop.sent && !stop.forced, "{stop:?}");
    let exit = running.wait().await.unwrap();
    assert_eq!(exit.outcome, Outcome::ClosedNormally);
    assert!(exit.duration_ms < 60_000);
    drop(client);
    // CA-T27-05: ao fim, a porta está livre.
    assert!(port_is_free(port), "a porta {port} ficou presa");
    let output = std::fs::read_to_string(&log).unwrap();
    assert!(output.contains("Done (0.512s)!") && output.contains("Stopping the server"));
    assert!(handle.send_command("say depois").await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t27_05_servidor_que_ignora_o_stop_e_encerrado_a_forca() {
    let Some(java) = test_java() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (server, port) = prepared(dir.path(), &["ignorar-stop"]);
    let (running, mut events) = RunningServer::spawn_with_stop_timeout(
        &command(&java).build(&server),
        None,
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    let handle = running.handle();
    wait_ready(&mut events, Duration::from_secs(30), |_| {})
        .await
        .unwrap();
    let stop = handle.stop().await;
    assert!(stop.sent && stop.forced, "{stop:?}");
    let exit = running.wait().await.unwrap();
    assert_eq!(exit.outcome, Outcome::StoppedByUser);
    assert!(handle.has_exited());
    assert!(port_is_free(port), "nenhum java órfão segurando a porta");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fechar_o_warden_encerra_o_servidor_na_hora() {
    let Some(java) = test_java() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (server, port) = prepared(dir.path(), &[]);
    let (running, mut events) = RunningServer::spawn(&command(&java).build(&server), None)
        .await
        .unwrap();
    wait_ready(&mut events, Duration::from_secs(30), |_| {})
        .await
        .unwrap();
    running.handle().kill().await;
    let exit = running.wait().await.unwrap();
    assert!(exit.stop_requested);
    assert!(port_is_free(port));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t27_02_sem_a_eula_o_servidor_nao_fica_pronto() {
    let Some(java) = test_java() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (server, _) = prepared(dir.path(), &[]);
    // Revogada: o Warden regrava eula=false (e recusa); o servidor, se abrisse, sairia.
    assert!(write_eula(dir.path(), None).is_err());
    let (_running, mut events) = RunningServer::spawn(&command(&java).build(&server), None)
        .await
        .unwrap();
    let failure = wait_ready(&mut events, Duration::from_secs(30), |_| {})
        .await
        .unwrap_err();
    assert!(matches!(failure, ReadyFailure::Exited(_)), "{failure:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t27_04_mod_de_cliente_derruba_o_servidor_antes_do_pronto() {
    let Some(java) = test_java() else { return };
    let dir = tempfile::tempdir().unwrap();
    let (server, _) = prepared(dir.path(), &["travar-cliente"]);
    let (_running, mut events) = RunningServer::spawn(&command(&java).build(&server), None)
        .await
        .unwrap();
    let mut lines = Vec::new();
    let failure = wait_ready(&mut events, Duration::from_secs(30), |event| {
        if let ServerEvent::Line(line) = event {
            lines.push(line.text.clone());
        }
    })
    .await
    .unwrap_err();
    let ReadyFailure::Exited(exit) = failure else {
        panic!("esperava a saída: {failure:?}");
    };
    assert_eq!(exit.outcome, Outcome::Crashed);
    assert_eq!(exit.exit_code, Some(1));
    assert_eq!(exit.artifacts.crash_reports.len(), 1);
    let class = lines
        .iter()
        .find_map(|line| warden_server::markers::client_class(line));
    assert_eq!(
        class.as_deref(),
        Some("net.minecraft.client.gui.screens.Screen")
    );
    let excerpt = warden_server::markers::crash_excerpt(&lines);
    assert!(
        excerpt
            .iter()
            .any(|line| line.contains("zoomcliente-1.0.jar")),
        "{excerpt:?}"
    );
}
