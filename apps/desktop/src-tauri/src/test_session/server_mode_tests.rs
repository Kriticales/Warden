//! "Testar como servidor" pelo app inteiro (L-09), com o jogo simulado da `warden-launcher`
//! (modo `conectar`) e o servidor simulado da `warden-server`: EULA, pasta do servidor só com
//! os itens de servidor, `server.properties`, o jogo entrando pelo Quick Play, comandos, parar
//! os dois e o "Por que travou" do servidor.
//!
//! Usam o Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`; sem Java, avisam e passam.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::time::{Duration, Instant};

use super::super::tests::{Seen, fake_state, register, sink, spawn_test, write_pack};
use super::*;
use crate::error::ErrorCode;
use crate::state::AppState;
use crate::test_session::{TestMode, TestRequest, run_test};

/// Um servidor por vez nestes testes (no app há um só, `ServerSessions`), quando rodam no mesmo
/// processo.
static ONE_SERVER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Pasta do `WardenFakeServer.class`.
fn server_class_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../crates/warden-server/tests/fixtures/java")
}

/// O estado com o jogo e o servidor simulados (`None` sem Java).
fn server_state(root: &Path) -> Option<Arc<AppState>> {
    let state = fake_state(root)?;
    let mut state = Arc::try_unwrap(state).ok()?;
    let tests = Arc::try_unwrap(state.tests).ok()?;
    state.tests = Arc::new(tests.with_fake_server(server_class_dir()));
    // Cada teste (o nextest roda um processo por teste) parte de uma porta dada pelo sistema:
    // dois servidores simulados não disputam a 25565.
    let start = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .ok()?
        .local_addr()
        .ok()?
        .port();
    state.servers.set_first_port(start);
    Some(Arc::new(state))
}

fn accept_eula(state: &AppState) {
    state
        .settings
        .set_eula_accepted_at(Some("2026-10-13T12:00:00Z".into()))
        .unwrap();
}

fn server_request(only: bool) -> TestRequest {
    TestRequest {
        mode: TestMode::Server,
        server_only: only,
        ..TestRequest::default()
    }
}

/// Um metafile "Só cliente" com download que não existe (se o Warden tentasse baixar para o
/// servidor, o teste falharia).
const CLIENT_ONLY: &[u8] = b"name = \"Zoom de Cliente\"\nfilename = \"so-cliente.jar\"\nside = \"client\"\n\n[download]\nurl = \"https://127.0.0.1:9/so-cliente.jar\"\nhash-format = \"sha256\"\nhash = \"0000000000000000000000000000000000000000000000000000000000000000\"\n";

fn port_is_free(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).is_ok()
}

/// Espera uma linha no console.
async fn wait_console(seen: &Seen, text: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !seen.console_texts().iter().any(|line| line.contains(text)) {
        assert!(
            Instant::now() < deadline,
            "sem {text:?} no console: {:?}",
            seen.console_texts()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t27_02_sem_a_eula_nada_abre_e_nenhum_eula_true_e_gravado() {
    let _one = ONE_SERVER.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = server_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[("config/a.toml", b"x = 1")], "conectar");
    let pack = register(&state, &root);
    // Uma pasta de servidor de um aceite antigo, já revogado.
    let server = server_dirs(&state.paths, pack);
    std::fs::create_dir_all(&server.game_dir).unwrap();
    std::fs::write(server.game_dir.join("eula.txt"), "eula=true\n").unwrap();

    let (sink, seen) = sink();
    let error = run_test(&state, pack, server_request(false), sink)
        .await
        .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::Server(warden_server::ServerErrorCode::EulaNotAccepted)
    );
    assert!(!warden_server::eula::eula_file_accepts(&server.game_dir));
    assert!(seen.console_texts().is_empty(), "nada abriu");
    assert!(state.servers.current().is_none());
    assert!(state.tests.current().is_none());
    // Depois do aceite, a pergunta não volta (o teste segue sem ela).
    accept_eula(&state);
    assert!(eula_accepted_at(&state).is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t27_01_servidor_e_jogo_abrem_o_jogo_entra_e_param_juntos() {
    let _one = ONE_SERVER.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = server_state(dir.path()) else {
        return;
    };
    accept_eula(&state);
    let root = dir.path().join("pack");
    write_pack(
        &root,
        &[
            ("config/a.toml", b"x = 1"),
            ("server.properties", b"difficulty=hard\nserver-ip=0.0.0.0\n"),
        ],
        "conectar",
    );
    let pack = register(&state, &root);
    let (task, seen) = spawn_test(&state, pack, server_request(false));
    wait_console(&seen, "joined the game").await;
    wait_console(&seen, "jogo conectado").await;

    let server = state.servers.current().expect("servidor aberto");
    assert_eq!(server.phase, ServerPhase::Ready);
    let port = server.port.unwrap();
    assert!(!port_is_free(port));
    let dirs = server_dirs(&state.paths, pack);
    let properties = std::fs::read_to_string(dirs.game_dir.join("server.properties")).unwrap();
    for line in [
        "difficulty=hard",
        "server-ip=127.0.0.1",
        "online-mode=false",
        "enable-rcon=false",
    ] {
        assert!(
            properties.lines().any(|l| l == line),
            "{line}: {properties}"
        );
    }
    assert!(std::fs::read_to_string(dirs.game_dir.join("config/a.toml")).is_ok());

    // As linhas do servidor vão com a origem própria; as do jogo, com a do jogo.
    let origins: Vec<(ConsoleOrigin, String)> = seen
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            OperationEvent::Console { lines } => Some(lines.clone()),
            _ => None,
        })
        .flatten()
        .map(|line| (line.origin, line.text))
        .collect();
    assert!(
        origins
            .iter()
            .any(|(origin, text)| *origin == ConsoleOrigin::Server && text.contains("Done ("))
    );
    assert!(origins.iter().any(|(origin, text)| *origin == ConsoleOrigin::Game && text.contains("jogo conectado")));

    state.servers.send_command(pack, "say oi").await.unwrap();
    wait_console(&seen, "Comando recebido: say oi").await;

    // "Parar jogo e servidor".
    state.tests.stop(pack).await.unwrap();
    let summary = tokio::time::timeout(Duration::from_secs(70), task)
        .await
        .expect("o teste não terminou")
        .unwrap()
        .unwrap();
    assert_eq!(summary.mode, TestMode::Server);
    assert_eq!(summary.outcome, Outcome::StoppedByUser);
    let side = summary.server.expect("lado do servidor");
    assert_eq!(side.outcome, Outcome::ClosedNormally);
    assert_eq!(side.game, ServerGame::Joined);
    assert!(side.ready_ms.is_some() && !side.stop_forced && !side.only);
    assert_eq!(side.port, port);
    // CA-T27-05: a porta foi liberada e nada ficou registrado.
    assert!(port_is_free(port));
    assert!(state.servers.current().is_none());
    assert!(state.tests.current().is_none());
    let session = crate::test_session::session_dir(&state.paths, pack, &summary.id).unwrap();
    let server_log = std::fs::read_to_string(server_log_path(session.path())).unwrap();
    assert!(server_log.contains("Stopping the server"));
    let texts = seen.console_texts();
    assert!(texts.iter().any(|text| text == "servidor:console.parando"));
    assert!(texts.iter().any(|text| text == "servidor:console.fechou"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t27_04_mod_de_cliente_derruba_o_servidor_e_so_cliente_fica_de_fora() {
    let _one = ONE_SERVER.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = server_state(dir.path()) else {
        return;
    };
    accept_eula(&state);
    let root = dir.path().join("pack");
    write_pack(
        &root,
        &[
            (
                "mods/zoomcliente-1.0.jar",
                b"jar de cliente marcado como dos dois lados",
            ),
            ("mods/so-cliente.pw.toml", CLIENT_ONLY),
        ],
        "filhos 1",
    );
    std::fs::write(
        root.join(crate::server_session::FAKE_ARGS_FILE),
        "travar-cliente",
    )
    .unwrap();
    let pack = register(&state, &root);
    let (sink, _seen) = sink();
    let summary = run_test(&state, pack, server_request(true), sink)
        .await
        .unwrap();
    assert_eq!(summary.outcome, Outcome::Crashed);
    let side = summary.server.expect("lado do servidor");
    assert!(side.only);
    assert_eq!(side.outcome, Outcome::Crashed);
    assert_eq!(side.game, ServerGame::NotOpened);
    assert_eq!(side.ready_ms, None);
    let crash = side.crash.expect("Por que travou");
    assert_eq!(
        crash.client_class.as_deref(),
        Some("net.minecraft.client.gui.screens.Screen")
    );
    assert!(!crash.excerpt.is_empty());
    assert!(
        crash
            .mods
            .iter()
            .any(|item| item.name.contains("zoomcliente")),
        "{:?}",
        crash.mods
    );
    // O "Só cliente" não foi para o servidor; o resto foi.
    let dirs = server_dirs(&state.paths, pack);
    assert!(dirs.game_dir.join("mods/zoomcliente-1.0.jar").is_file());
    assert!(!dirs.game_dir.join("mods/so-cliente.jar").exists());
    assert!(!dirs.game_dir.join("mods/so-cliente.pw.toml").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn so_o_servidor_parar_manda_stop_e_fechar_o_warden_nao_deixa_orfao() {
    let _one = ONE_SERVER.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = server_state(dir.path()) else {
        return;
    };
    accept_eula(&state);
    let root = dir.path().join("pack");
    write_pack(&root, &[("config/a.toml", b"x = 1")], "filhos 1");
    let pack = register(&state, &root);

    // "Só o servidor" e "Parar".
    let (task, seen) = spawn_test(&state, pack, server_request(true));
    wait_console(&seen, "servidor:console.pronto").await;
    let port = state.servers.current().unwrap().port.unwrap();
    assert_eq!(state.tests.current().unwrap().state, GamePhase::Running);
    state.tests.stop(pack).await.unwrap();
    let summary = tokio::time::timeout(Duration::from_secs(70), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(summary.outcome, Outcome::StoppedByUser);
    let side = summary.server.unwrap();
    assert!(side.only && !side.stop_forced);
    assert!(port_is_free(port));
    assert!(
        seen.console_texts()
            .iter()
            .any(|text| text.contains("Stopping the server"))
    );

    // CA-T27-05: "Fechar o Warden" com o servidor aberto.
    let (task, seen) = spawn_test(&state, pack, server_request(true));
    wait_console(&seen, "servidor:console.pronto").await;
    let port = state.servers.current().unwrap().port.unwrap();
    state.servers.kill_all().await;
    assert!(state.tests.stop_all(crate::test_session::QUIT_WAIT).await);
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        port_is_free(port),
        "nenhum java do servidor segurando a porta"
    );
    assert!(state.servers.current().is_none());
}
