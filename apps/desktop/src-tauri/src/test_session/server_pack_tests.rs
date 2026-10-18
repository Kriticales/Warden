//! CA-T19-08: "Testar no servidor deste computador depois de gerar" (E-03), pelo app inteiro,
//! com o servidor simulado da `warden-server` no servidor local da L-09: o pacote correto
//! termina com o tempo até "Done"; um mod de cliente marcado como "Cliente e servidor" faz a
//! validação falhar com o trecho do log do servidor e o mod apontado. Sem a EULA aceita no
//! Warden, nada é gerado.
//!
//! Usam o Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME` e o sidecar do packwiz; sem eles, avisam e
//! passam.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};

use warden_export::{ServerPackOptions, ServerPackVariant};
use warden_packwiz_cli::Packwiz;

use super::tests::{fake_state, register, write_pack};
use super::*;
use crate::commands::server_pack::{ServerPackCheck, ServerPackRunRequest, run};
use crate::error::ErrorCode;
use crate::operations::OperationKind;
use crate::state::AppState;

/// Um servidor por vez nestes testes, quando rodam no mesmo processo.
static ONE_SERVER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn server_class_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../crates/warden-server/tests/fixtures/java")
}

/// O estado com o jogo e o servidor simulados, e o packwiz real (`None` sem Java ou sidecar).
fn server_state(root: &Path) -> Option<(Arc<AppState>, Packwiz)> {
    let binary = crate::commands::inventory::tests::packwiz_binary()?;
    let state = fake_state(root)?;
    let mut state = Arc::try_unwrap(state).ok()?;
    let tests = Arc::try_unwrap(state.tests)
        .ok()?
        .with_fake_server(server_class_dir());
    state.tests = Arc::new(tests);
    let start = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .ok()?
        .local_addr()
        .ok()?
        .port();
    state.servers.set_first_port(start);
    let packwiz = Packwiz::new(
        binary,
        state.paths.packwiz_cache_dir(),
        state.paths.packwiz_config_file(),
    );
    Some((Arc::new(state), packwiz))
}

fn accept_eula(state: &AppState) {
    state
        .settings
        .set_eula_accepted_at(Some("2026-10-13T12:00:00Z".into()))
        .unwrap();
}

fn embedded(validate: bool) -> ServerPackRunRequest {
    ServerPackRunRequest {
        options: ServerPackOptions {
            variant: ServerPackVariant::Embedded,
            memory_mb: 2048,
            confirm_third_party: true,
        },
        validate,
    }
}

async fn generate(
    state: &AppState,
    packwiz: &Packwiz,
    pack: PackId,
    root: &Path,
    destination: &Path,
    request: &ServerPackRunRequest,
) -> Result<crate::commands::server_pack::ServerPackRun, AppError> {
    let operation =
        state
            .operations
            .start(OperationKind::new("export.serverPack"), Some(pack), true);
    let result = run(state, pack, root, destination, request, packwiz, &operation).await;
    operation.finish(result)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t19_08_pacote_correto_termina_testado_com_o_tempo_ate_done() {
    let _one = ONE_SERVER.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let Some((state, packwiz)) = server_state(dir.path()) else {
        return;
    };
    accept_eula(&state);
    let root = dir.path().join("pack");
    write_pack(&root, &[("config/a.toml", b"x = 1")], "");
    let pack = register(&state, &root);
    let destination = dir.path().join("vale-servidor.zip");
    let run = generate(&state, &packwiz, pack, &root, &destination, &embedded(true))
        .await
        .unwrap();
    assert!(destination.is_file());
    match run.check {
        Some(ServerPackCheck::Tested { ready_ms }) => assert!(ready_ms > 0),
        other => panic!("esperava o pacote testado: {other:?}"),
    }
    // O servidor fechou, a pasta temporária saiu e nenhum eula.txt foi para o pacote.
    assert!(state.servers.current().is_none());
    let tmp = state.paths.tmp_dir();
    assert!(
        std::fs::read_dir(&tmp).map_or(true, |entries| entries.filter_map(Result::ok).all(
            |entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with("server-pack-")
        )),
        "a pasta temporária da validação ficou"
    );
    let zip = zip::ZipArchive::new(std::fs::File::open(&destination).unwrap()).unwrap();
    assert!(zip.index_for_name("eula.txt").is_none());
    assert!(zip.index_for_name("config/a.toml").is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t19_08_mod_de_cliente_como_dos_dois_lados_reprova_com_log_e_mod() {
    let _one = ONE_SERVER.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let Some((state, packwiz)) = server_state(dir.path()) else {
        return;
    };
    accept_eula(&state);
    let root = dir.path().join("pack");
    write_pack(
        &root,
        &[(
            "mods/zoomcliente-1.0.jar",
            b"jar de cliente marcado como dos dois lados",
        )],
        "",
    );
    // O servidor simulado derruba como um mod de cliente faria.
    std::fs::write(
        root.join(crate::server_session::FAKE_ARGS_FILE),
        "travar-cliente",
    )
    .unwrap();
    let pack = register(&state, &root);
    let destination = dir.path().join("vale-servidor.zip");
    let run = generate(&state, &packwiz, pack, &root, &destination, &embedded(true))
        .await
        .unwrap();
    // O pacote fica gerado; a validação diz por que o servidor não abriu.
    assert!(destination.is_file());
    let Some(ServerPackCheck::Crashed { crash }) = run.check else {
        panic!("esperava a validação reprovada: {:?}", run.check);
    };
    assert!(!crash.excerpt.is_empty(), "sem o trecho do log");
    assert_eq!(
        crash.client_class.as_deref(),
        Some("net.minecraft.client.gui.screens.Screen")
    );
    assert!(
        crash
            .mods
            .iter()
            .any(|item| item.name.contains("zoomcliente")),
        "{:?}",
        crash.mods
    );
    assert!(state.servers.current().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sem_a_eula_aceita_a_validacao_nao_gera_nada_e_sem_validar_gera() {
    let _one = ONE_SERVER.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let Some((state, packwiz)) = server_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[("config/a.toml", b"x = 1")], "");
    let pack = register(&state, &root);
    let destination = dir.path().join("vale-servidor.zip");
    let error = generate(&state, &packwiz, pack, &root, &destination, &embedded(true))
        .await
        .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::Server(warden_server::ServerErrorCode::EulaNotAccepted)
    );
    assert!(!destination.exists());
    let run = generate(
        &state,
        &packwiz,
        pack,
        &root,
        &destination,
        &embedded(false),
    )
    .await
    .unwrap();
    assert!(run.check.is_none());
    assert!(destination.is_file());
}
