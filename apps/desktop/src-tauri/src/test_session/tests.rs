//! Testes do Testar com o jogo simulado da `warden-launcher` (`WardenFakeGame`): o fluxo
//! inteiro do app (um jogo por vez, sincronização da instância, processo, console, parar,
//! sessão gravada), sem baixar Java nem Minecraft.
//!
//! Usam o Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`; sem Java, avisam e passam (como os testes
//! de processo da `warden-launcher`).

#![allow(clippy::print_stderr)] // aviso de teste pulado
// Os auxiliares são `pub(super)`: os testes do "Testar como servidor" (`server_mode`) usam os
// mesmos (L-09).

use std::collections::BTreeMap;
use std::sync::Mutex as StdMutex;
use std::time::Instant;

use sha2::{Digest as _, Sha256};
use warden_launcher::session::SESSION_FILE;
use warden_packwiz::{IndexEntry, PackIndex, PackManifest};
use warden_project::registry::PackRecord;

use super::*;
use crate::error::ErrorCode;
use crate::state::tests::test_state;

/// O `java` dos testes.
pub(super) fn java() -> Option<PathBuf> {
    if let Some(java) = std::env::var_os("WARDEN_TEST_JAVA").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(java));
    }
    let home = std::env::var_os("JAVA_HOME").filter(|v| !v.is_empty())?;
    let java = Path::new(&home)
        .join("bin")
        .join(if cfg!(windows) { "java.exe" } else { "java" });
    java.is_file().then_some(java)
}

/// Pasta do `WardenFakeGame.class`.
pub(super) fn class_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../crates/warden-launcher/tests/fixtures/java")
}

/// O major do Java (arquivo `release` da pasta do Java).
pub(super) fn java_major(java: &Path) -> u32 {
    let home = java.parent().and_then(Path::parent).unwrap();
    let release = std::fs::read_to_string(home.join("release")).unwrap_or_default();
    release
        .lines()
        .find_map(|line| line.strip_prefix("JAVA_VERSION="))
        .map(|version| version.trim_matches('"'))
        .and_then(|version| {
            let mut parts = version.split('.');
            let first: u32 = parts.next()?.parse().ok()?;
            if first == 1 {
                parts.next()?.parse().ok()
            } else {
                Some(first)
            }
        })
        .unwrap_or(17)
}

/// Estado de teste com o jogo simulado no lugar do motor.
pub(super) fn fake_state(root: &Path) -> Option<Arc<AppState>> {
    fake_state_with(root, None)
}

/// O mesmo, com a CurseForge simulada em `curseforge_base` (L-06).
pub(super) fn fake_state_with(
    root: &Path,
    curseforge_base: Option<String>,
) -> Option<Arc<AppState>> {
    let Some(java) = java() else {
        eprintln!("sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): teste do jogo simulado pulado");
        return None;
    };
    let mut state = test_state(root);
    let engine = PortableMcEngine::new(EngineDirs::under(&root.join("shared")), None);
    let major = java_major(&java);
    let mut sessions = TestSessions::with_engine(
        Arc::new(engine),
        Some(FakeGame::new(class_dir(), java, major)),
    );
    if let Some(base) = curseforge_base {
        sessions = sessions.with_curseforge_base(base);
    }
    state.tests = Arc::new(sessions);
    Some(Arc::new(state))
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Grava um pack vanilla 1.20.1 com os arquivos dados (todos no índice; os `.pw.toml` como
/// metafiles).
pub(super) fn write_pack(root: &Path, files: &[(&str, &[u8])], fake_args: &str) {
    std::fs::create_dir_all(root).unwrap();
    let mut index = PackIndex::default();
    for (path, bytes) in files {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, bytes).unwrap();
        index.files.push(IndexEntry {
            file: (*path).to_owned(),
            hash: sha256(bytes),
            hash_format: String::new(),
            alias: String::new(),
            metafile: path.ends_with(".pw.toml"),
            preserve: false,
        });
    }
    let index_text = index.to_toml_string();
    std::fs::write(root.join("index.toml"), &index_text).unwrap();
    let mut manifest = PackManifest::new("Vale de Teste", "1.20.1");
    manifest.version = "1.0.0".into();
    manifest.index.hash = sha256(index_text.as_bytes());
    std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    std::fs::write(root.join(fake::ARGS_FILE), fake_args).unwrap();
}

pub(super) fn register(state: &AppState, root: &Path) -> PackId {
    let record = PackRecord {
        id: PackId::new(),
        name: "Vale de Teste".into(),
        path: root.to_path_buf(),
        last_test: None,
        extra: BTreeMap::new(),
    };
    let id = record.id;
    state.packs.insert(record).unwrap();
    id
}

/// O que o teste mandou pelo canal e pelo `game-state`.
#[derive(Default)]
pub(super) struct Seen {
    pub(super) events: StdMutex<Vec<OperationEvent>>,
    states: StdMutex<Vec<GameState>>,
}

impl Seen {
    pub(super) fn console_texts(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                OperationEvent::Console { lines } => Some(lines.clone()),
                _ => None,
            })
            .flatten()
            .map(|line| line.text)
            .collect()
    }

    pub(super) fn phases(&self) -> Vec<GamePhase> {
        self.states
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.state)
            .collect()
    }
}

pub(super) fn sink() -> (TestSink, Arc<Seen>) {
    let seen = Arc::new(Seen::default());
    let events = Arc::clone(&seen);
    let states = Arc::clone(&seen);
    (
        TestSink {
            events: Arc::new(move |event| events.events.lock().unwrap().push(event)),
            game_state: Arc::new(move |state: &GameState| {
                states.states.lock().unwrap().push(state.clone());
            }),
        },
        seen,
    )
}

/// Começa um teste em segundo plano.
pub(super) fn spawn_test(
    state: &Arc<AppState>,
    pack: PackId,
    request: TestRequest,
) -> (
    tokio::task::JoinHandle<Result<TestSessionSummary, AppError>>,
    Arc<Seen>,
) {
    let (sink, seen) = sink();
    let state = Arc::clone(state);
    let task = tokio::spawn(async move { run_test(&state, pack, request, sink).await });
    (task, seen)
}

/// Espera o jogo simulado (modo `filhos`) abrir e avisar no console que os filhos já estão
/// abertos ("pai <pid> abriu N filhos").
///
/// Espera essa linha, e não "uma linha qualquer depois de `console.abrindo`": a JVM pode
/// escrever antes do programa (com `JAVA_TOOL_OPTIONS` definida, sai no stderr a linha
/// `Picked up JAVA_TOOL_OPTIONS: …`), e aí o teste pararia o jogo antes de os filhos
/// existirem e de a linha deles chegar.
pub(super) async fn wait_running(state: &AppState, seen: &Seen) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let running = state
            .tests
            .current()
            .is_some_and(|game| game.state == GamePhase::Running);
        let children_open = seen
            .console_texts()
            .iter()
            .any(|text| text.starts_with("pai ") && text.contains(" filhos"));
        if running && children_open {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "o jogo simulado não abriu em 30 s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// CA-T13-02 pelo app inteiro: "Parar jogo" encerra o jogo (com um filho) em até 5 s; a sessão
/// sai como "Encerrado por você", com os campos do Testar no `session.json`, e o `game-state`
/// vai de preparando a aberto e a fechado.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_02_parar_jogo_encerra_e_grava_a_sessao() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[("config/a.toml", b"x = 1")], "filhos 2");
    let pack = register(&state, &root);
    let (task, seen) = spawn_test(&state, pack, TestRequest::default());
    wait_running(&state, &seen).await;

    let asked = Instant::now();
    state.tests.stop(pack).await.unwrap();
    let summary = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("o teste não terminou")
        .unwrap()
        .unwrap();
    let elapsed = asked.elapsed();
    assert!(elapsed < Duration::from_secs(5), "parou em {elapsed:?}");
    assert_eq!(summary.outcome, Outcome::StoppedByUser);
    assert_eq!(summary.minecraft, "1.20.1");
    assert_eq!(summary.pack_version.as_deref(), Some("1.0.0"));
    assert!(state.tests.current().is_none());
    assert_eq!(
        seen.phases(),
        vec![
            GamePhase::Preparing,
            GamePhase::Preparing,
            GamePhase::Running,
            GamePhase::Exited
        ]
    );
    let texts = seen.console_texts();
    assert_eq!(texts.first().map(String::as_str), Some("console.abrindo"));
    assert!(texts.iter().any(|text| text.contains("filhos")));
    assert_eq!(texts.last().map(String::as_str), Some("console.encerrado"));

    // A sessão gravada com os campos do Testar.
    let sessions = sessions_of(&state.paths, pack).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0], summary);
    let dir = session_dir(&state.paths, pack, &summary.id).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join(SESSION_FILE)).unwrap()).unwrap();
    assert_eq!(json["outcome"], "stoppedByUser");
    assert_eq!(json["mode"], "normal");
    assert_eq!(json["profile"], "default");
    assert!(
        json["packTree"]["hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(json["memory"]["auto"], true);
    assert_eq!(json["profileSignature"]["jvmArgs"][0], "-XX:+UseG1GC");
    assert_eq!(json["profileSignature"]["onOpen"], "menu");
    assert!(json["machine"]["cpus"].as_u64().unwrap() >= 1);
    assert_eq!(json["firstLaunch"], true);
    assert_eq!(
        state.packs.get(pack).unwrap().last_test.as_deref(),
        Some("ok")
    );
    // O console gravado é o mesmo que passou ao vivo.
    let (lines, _) = read_output_log(&dir.output_log()).unwrap();
    assert!(lines.iter().any(|line| line.text.contains("filhos")));
    // A instância recebeu o pack.
    let game_dir = InstanceDirs::for_pack(&state.paths, pack).game_dir;
    assert_eq!(
        std::fs::read(game_dir.join("config/a.toml")).unwrap(),
        b"x = 1"
    );
}

/// Um jogo por vez no Warden inteiro: o segundo teste é recusado com o pack do primeiro.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn um_jogo_por_vez() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let first_root = dir.path().join("primeiro");
    write_pack(&first_root, &[], "filhos 1");
    let first = register(&state, &first_root);
    let second_root = dir.path().join("segundo");
    write_pack(&second_root, &[], "filhos 1");
    let second = register(&state, &second_root);

    let (task, seen) = spawn_test(&state, first, TestRequest::default());
    wait_running(&state, &seen).await;
    let (sink, _) = sink();
    let error = run_test(&state, second, TestRequest::default(), sink)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::App(AppErrorCode::GameAlreadyRunning));
    assert_eq!(error.params["packId"], first.to_string());
    assert_eq!(error.params["packName"], "Vale de Teste");
    // Parar o pack errado não mexe no jogo aberto.
    let error = state.tests.stop(second).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::App(AppErrorCode::NoGameRunning));
    assert!(state.tests.current().is_some());

    // "Fechar o Warden" para o jogo e espera a sessão.
    assert!(state.tests.stop_all(QUIT_WAIT).await);
    assert!(state.tests.current().is_none());
    let summary = task.await.unwrap().unwrap();
    assert_eq!(summary.outcome, Outcome::StoppedByUser);
}

/// CA-T13-06 (parte do Testar): um travamento termina com "Travou", o crash report na sessão e
/// "crashed" em Meus packs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_06_travamento_vira_travou_com_o_crash_report() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[], "crash");
    let pack = register(&state, &root);
    let (sink, seen) = sink();
    let summary = run_test(&state, pack, TestRequest::default(), sink)
        .await
        .unwrap();
    assert_eq!(summary.outcome, Outcome::Crashed);
    assert_eq!(summary.crash_reports.len(), 1, "{summary:?}");
    assert!(summary.crash_reports[0].starts_with("crash-reports/"));
    assert_eq!(
        seen.console_texts().last().map(String::as_str),
        Some("console.travou")
    );
    assert_eq!(
        state.packs.get(pack).unwrap().last_test.as_deref(),
        Some("crashed")
    );
}

/// CA-T13-03 (parte do Testar): as linhas com acentos chegam certas ao console e ao log
/// gravado.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_03_acentos_no_console() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[], "acentos");
    let pack = register(&state, &root);
    let (sink, seen) = sink();
    let summary = run_test(&state, pack, TestRequest::default(), sink)
        .await
        .unwrap();
    assert_eq!(summary.outcome, Outcome::ClosedNormally);
    let texts = seen.console_texts().join("\n");
    assert!(
        texts.contains("Ação, coração, pé, avô, vovó, pão"),
        "{texts}"
    );
    assert!(texts.contains("Configuração inválida: ç ã õ é"), "{texts}");
    assert!(!texts.contains("Ã"), "{texts}");
    let dir = session_dir(&state.paths, pack, &summary.id).unwrap();
    let (lines, _) = read_output_log(&dir.output_log()).unwrap();
    assert!(lines.iter().any(|line| line.text.contains("coração")));
}

/// CA-T13-04: tirar um mod do pack e testar de novo tira o jar da instância; o mundo do teste
/// anterior continua lá.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_04_mod_removido_sai_da_instancia_e_o_mundo_fica() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(
        &root,
        &[("mods/a.jar", b"jar a"), ("mods/b.jar", b"jar b")],
        "acentos",
    );
    let pack = register(&state, &root);
    let (sink, _) = sink();
    run_test(&state, pack, TestRequest::default(), sink.clone())
        .await
        .unwrap();
    let game_dir = InstanceDirs::for_pack(&state.paths, pack).game_dir;
    assert!(game_dir.join("mods/b.jar").is_file());
    // O mundo que o jogo criou no primeiro teste.
    let world = game_dir.join("saves/Mundo de teste");
    std::fs::create_dir_all(world.join("region")).unwrap();
    std::fs::write(world.join("level.dat"), b"mundo").unwrap();

    write_pack(&root, &[("mods/a.jar", b"jar a")], "acentos");
    run_test(&state, pack, TestRequest::default(), sink)
        .await
        .unwrap();
    assert!(game_dir.join("mods/a.jar").is_file());
    assert!(!game_dir.join("mods/b.jar").exists(), "o jar removido saiu");
    assert_eq!(std::fs::read(world.join("level.dat")).unwrap(), b"mundo");
    assert_eq!(sessions_of(&state.paths, pack).unwrap().len(), 2);
}

/// Servidor HTTP mínimo que entrega sempre o mesmo arquivo. Diferente do `wiremock`, que
/// devolve o servidor a um conjunto reaproveitado, [`FileServer::stop`] fecha a porta de verdade:
/// depois dele, conectar é recusado, como sem internet.
struct FileServer {
    address: std::net::SocketAddr,
    requests: Arc<std::sync::atomic::AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl FileServer {
    async fn start(body: Vec<u8>) -> Self {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => break,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/java-archive\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(&body).await;
                let _ = stream.shutdown().await;
            }
        });
        Self {
            address,
            requests,
            task,
        }
    }

    fn requests(&self) -> usize {
        self.requests.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Fecha a porta.
    async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

/// Metafile de um mod baixado do link dado.
fn metafile(name: &str, filename: &str, url: &str, jar: &[u8]) -> String {
    format!(
        "name = \"{name}\"\nfilename = \"{filename}\"\nside = \"both\"\n\n[download]\n\
         url = \"{url}\"\nhash-format = \"sha256\"\nhash = \"{}\"\n",
        sha256(jar)
    )
}

/// CA-T13-05 com o jogo simulado: o primeiro teste baixa o mod do servidor simulado; com o
/// servidor desligado (sem internet: a conexão é recusada) o segundo teste abre o jogo sem
/// pedir nada à rede; com o jar apagado da instância e dos downloads guardados, ainda sem
/// rede, o teste não abre o jogo e a mensagem diz exatamente o que falta (o nome do mod) e
/// que a causa é a internet. O Java e o Minecraft sem rede ficam com o roteiro do jogo real
/// (`ca_t13_01_jogo_real_entra_no_mundo_pelo_testar`, com `WARDEN_TEST_REAL_OFFLINE=1`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_05_sem_internet_o_teste_abre_e_diz_o_que_falta() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let jar = b"jar do Vale Mod".to_vec();
    let server = FileServer::start(jar.clone()).await;
    let address = server.address;
    let url = format!("http://{address}/files/vale-mod-1.0.jar");
    let root = dir.path().join("pack");
    let meta = metafile("Vale Mod", "vale-mod-1.0.jar", &url, &jar);
    write_pack(
        &root,
        &[
            ("mods/vale-mod.pw.toml", meta.as_bytes()),
            ("config/a.toml", b"x = 1"),
        ],
        "acentos",
    );
    let pack = register(&state, &root);
    let game_dir = InstanceDirs::for_pack(&state.paths, pack).game_dir;
    let installed = game_dir.join("mods/vale-mod-1.0.jar");

    // 1. Com internet: o mod vem do servidor.
    let (first_sink, _) = sink();
    let first = run_test(&state, pack, TestRequest::default(), first_sink)
        .await
        .unwrap();
    assert_eq!(first.outcome, Outcome::ClosedNormally);
    assert_eq!(std::fs::read(&installed).unwrap(), jar);
    assert_eq!(server.requests(), 1);

    // 2. Sem internet: o servidor sai do ar e a porta passa a recusar conexões.
    server.stop().await;
    assert!(
        std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2)).is_err(),
        "o servidor simulado ainda responde"
    );
    let (second_sink, seen) = sink();
    let second = run_test(&state, pack, TestRequest::default(), second_sink)
        .await
        .unwrap();
    assert_eq!(second.outcome, Outcome::ClosedNormally);
    assert!(
        seen.console_texts()
            .iter()
            .any(|text| text.contains("coração")),
        "o jogo abriu e escreveu no console"
    );
    assert_eq!(std::fs::read(&installed).unwrap(), jar);

    // 3. Sem internet e sem o jar (na instância e nos downloads guardados): o teste para antes
    // de abrir o jogo e diz o que falta.
    std::fs::remove_file(&installed).unwrap();
    std::fs::remove_dir_all(state.paths.downloads_cache_dir()).unwrap();
    let (third_sink, seen) = sink();
    let error = run_test(&state, pack, TestRequest::default(), third_sink)
        .await
        .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::Instance(warden_instance::InstanceErrorCode::SyncIncomplete),
        "{error:?}"
    );
    assert_eq!(error.params["count"], "1");
    assert_eq!(error.params["names"], "Vale Mod");
    assert_eq!(error.params["more"], "0");
    let detail = error.detail.clone().unwrap_or_default();
    assert!(detail.contains("mods/vale-mod.pw.toml"), "{detail}");
    assert!(error.retryable, "sem internet, Tentar de novo resolve");
    assert!(seen.console_texts().is_empty(), "o jogo não abriu");
    assert!(state.tests.current().is_none());
    assert_eq!(sessions_of(&state.paths, pack).unwrap().len(), 2);
    // O resto do pack continua na instância.
    assert_eq!(
        std::fs::read(game_dir.join("config/a.toml")).unwrap(),
        b"x = 1"
    );
}

/// CA-T13-05 (parte do Minecraft): sem internet e com um arquivo do jogo apagado, a mensagem
/// diz quais arquivos faltam; sem arquivo faltando, o erro do motor segue como está. O caso
/// completo, com o motor de verdade, está no roteiro do jogo real (`roteiro-jogo-real.md`).
#[test]
fn ca_t13_05_arquivos_do_jogo_que_faltam_aparecem_na_mensagem() {
    let missing = warden_launcher::Error::NetworkUnavailable {
        engine: "download: 1 errors over 1 entries\ncausa: tcp connect error".into(),
        missing: vec![PathBuf::from(
            "shared/libraries/net/sf/jopt-simple/jopt-simple/5.0.4/jopt-simple-5.0.4.jar",
        )],
    };
    let error = install_error(&missing);
    assert_eq!(
        error.code,
        ErrorCode::App(AppErrorCode::TestGameFilesMissing)
    );
    assert_eq!(error.params["count"], "1");
    assert_eq!(error.params["names"], "jopt-simple-5.0.4.jar");
    assert!(error.retryable);
    assert!(
        error
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("falta: shared/libraries/net/sf/jopt-simple")
    );
    let offline = warden_launcher::Error::NetworkUnavailable {
        engine: "dns error".into(),
        missing: Vec::new(),
    };
    assert_eq!(
        install_error(&offline).code,
        ErrorCode::Core(warden_core::CoreErrorCode::NetworkUnavailable)
    );
}

/// Um arquivo do pack mudado na instância pede confirmação antes de ser substituído.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn arquivo_mudado_na_instancia_pede_confirmacao() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[("config/a.toml", b"x = 1")], "acentos");
    let pack = register(&state, &root);
    let (sink, _) = sink();
    run_test(&state, pack, TestRequest::default(), sink.clone())
        .await
        .unwrap();
    let config = InstanceDirs::for_pack(&state.paths, pack)
        .game_dir
        .join("config/a.toml");
    std::fs::write(&config, b"x = 2").unwrap();
    // O pack também mudou: o arquivo teria de ser sobrescrito.
    write_pack(&root, &[("config/a.toml", b"x = 3")], "acentos");
    let error = run_test(&state, pack, TestRequest::default(), sink.clone())
        .await
        .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::App(AppErrorCode::TestInstanceChanged)
    );
    assert_eq!(error.params["count"], "1");
    assert_eq!(error.params["files"], "config/a.toml");
    assert!(state.tests.current().is_none(), "o teste terminou");
    let request = TestRequest {
        replace_instance_changes: true,
        ..TestRequest::default()
    };
    run_test(&state, pack, request, sink).await.unwrap();
    assert_eq!(std::fs::read(&config).unwrap(), b"x = 3");
}

/// Um jar Fabric de "Meu Mod", que exige o Sodium (que não está no pack).
fn meu_mod_jar() -> Vec<u8> {
    use std::io::Write as _;
    let json = serde_json::json!({
        "schemaVersion": 1,
        "id": "meumod",
        "version": "1.0.0",
        "depends": { "sodium": "*" }
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut bytes);
    zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(json.to_string().as_bytes()).unwrap();
    zip.finish().unwrap();
    bytes.into_inner()
}

/// O pack de `write_pack`, com o Fabric 0.16.10 no `pack.toml`.
fn use_fabric(root: &Path) {
    let text = std::fs::read_to_string(root.join("pack.toml")).unwrap();
    let mut manifest = PackManifest::parse(&text).unwrap().value;
    manifest
        .versions
        .get_or_insert_with(BTreeMap::new)
        .insert("fabric".into(), "0.16.10".into());
    std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
}

/// D-03 pelo Testar inteiro, com o jogo simulado: "Verificar o pack" (rápida) passa, a
/// "Verificação final" (completa, com o jar copiado para a instância) acha a dependência que
/// falta e para o teste com `app.TEST_PACK_HAS_ERRORS` antes de abrir o jogo; "Testar mesmo
/// assim" abre o jogo, que trava, e o resultado tem "Por que travou" (CA-T13-06): a análise
/// gravada no `session.json` aponta o crash report com o arquivo e a linha.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d03_erros_do_pre_teste_param_o_teste_e_testar_mesmo_assim_mostra_por_que_travou() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    let jar = meu_mod_jar();
    write_pack(&root, &[("mods/meu-mod.jar", &jar)], "crash");
    use_fabric(&root);
    let pack = register(&state, &root);

    let (first, seen) = sink();
    let error = run_test(&state, pack, TestRequest::default(), first)
        .await
        .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::App(AppErrorCode::TestPackHasErrors),
        "{error:?}"
    );
    assert_eq!(error.params["count"], "1");
    assert_eq!(error.params["step"], "final");
    let stages: Vec<String> = seen
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            OperationEvent::Stage { stage, .. } => Some(stage.clone()),
            _ => None,
        })
        .collect();
    let check = stages.iter().position(|stage| stage == "test.check");
    let last = stages.iter().position(|stage| stage == "test.finalCheck");
    assert!(
        check.is_some() && last.is_some() && check < last,
        "{stages:?}"
    );
    assert!(
        !stages.iter().any(|stage| stage == STAGE_LAUNCH),
        "o jogo não abriu: {stages:?}"
    );
    assert!(sessions_of(&state.paths, pack).unwrap().is_empty());

    let request = TestRequest {
        ignore_pack_errors: true,
        ..TestRequest::default()
    };
    let (second, _) = sink();
    let summary = run_test(&state, pack, request, second).await.unwrap();
    assert_eq!(summary.outcome, Outcome::Crashed);
    let session = session_dir(&state.paths, pack, &summary.id).unwrap();
    let record = session.read_record().unwrap();
    assert!(record.extra.contains_key("diagnosis"), "{:?}", record.extra);
    let diagnosis =
        crate::commands::diagnostics::crash::crash_of_session(&state, pack, &summary.id)
            .await
            .unwrap();
    assert!(diagnosis.found_cause, "{diagnosis:#?}");
    let primary = &diagnosis.findings[0];
    assert_eq!(primary.finding.rule().as_str(), "E_CRASH_REPORT");
    let warden_diagnostics::Evidence::Log {
        file,
        line,
        excerpt,
    } = primary.finding.primary_evidence()
    else {
        panic!("esperava a linha do crash report: {primary:?}");
    };
    assert_eq!(file, "crash-reports/crash-teste-client.txt");
    assert_eq!(*line, 2);
    assert!(excerpt.contains("Initializing game"), "{excerpt}");
    assert!(
        diagnosis
            .sources
            .iter()
            .any(|source| source.name == "output.log")
    );
}

#[test]
fn sessao_com_nome_estranho_nao_existe() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_dev_root(dir.path()).unwrap();
    for bad in [
        "",
        "../x",
        "a/b",
        "2026-10-07_10-00-00",
        "x".repeat(65).as_str(),
    ] {
        let error = session_dir(&paths, PackId::new(), bad).unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::App(AppErrorCode::TestSessionNotFound),
            "{bad}"
        );
    }
}

#[test]
fn loader_do_pack_vira_o_do_launcher() {
    let requirement = |loader: Option<(&str, &str)>| GameRequirement {
        minecraft: "1.20.1".into(),
        loader: loader.map(|(loader, version)| warden_instance::LoaderRequirement {
            loader: loader.into(),
            version: version.into(),
        }),
    };
    assert_eq!(
        game_spec(&requirement(None)).unwrap().loader,
        LoaderSpec::Vanilla
    );
    assert_eq!(
        game_spec(&requirement(Some(("neoforge", "47.1.106"))))
            .unwrap()
            .loader,
        LoaderSpec::NeoForge {
            version: "47.1.106".into()
        }
    );
    let error = game_spec(&requirement(Some(("liteloader", "1.0")))).unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::App(AppErrorCode::TestLoaderUnsupported)
    );
    assert_eq!(
        loader_kind(&LoaderSpec::Forge {
            version: "x".into()
        }),
        LoaderKind::Forge
    );
}

/// Uma combinação do roteiro com o jogo real.
struct RealGame {
    minecraft: &'static str,
    loader: &'static str,
    version: &'static str,
    /// Entra pelo Quick Play num mundo gerado pelo servidor vanilla (1.20+); antes da 1.20,
    /// pelo servidor local do loader com `--server/--port` (L-05, S-R5-3 §10).
    quick_play: bool,
}

/// As combinações do CA-T13-01 nesta tarefa (a matriz completa é da L-05). Os nomes e os mods
/// são os da matriz (`crates/warden-launcher/tests/matrix/mods.json`).
const REAL_GAMES: [(&str, RealGame); 2] = [
    (
        "fabric-1.20.1",
        RealGame {
            minecraft: "1.20.1",
            loader: "fabric",
            version: "0.19.5",
            quick_play: true,
        },
    ),
    (
        "forge-1.12.2",
        RealGame {
            minecraft: "1.12.2",
            loader: "forge",
            version: "14.23.5.2860",
            quick_play: false,
        },
    ),
];

/// O mundo do roteiro (o mesmo da matriz).
const REAL_WORLD: &str = "wardentest";

/// A pasta da matriz da L-05 (`pack.py`, `prepare.py`, marcadores).
fn matrix_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../crates/warden-launcher/tests/matrix")
}

/// Os marcadores de pronto, de mundo e de falha da matriz (S-R5-3 §3 e §5).
#[path = "../../../../../crates/warden-launcher/tests/matrix/markers.rs"]
#[allow(dead_code, clippy::all, clippy::pedantic)]
mod markers;

/// Roda um script da matriz (`pack.py`, `prepare.py`).
async fn matrix_script(script: &str, args: Vec<String>) {
    let python = std::env::var("WARDEN_SMOKE_PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
    let path = matrix_dir().join(script);
    let shown = format!("{script} {args:?}");
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new(python)
            .arg(path)
            .args(args)
            .status()
    })
    .await
    .unwrap()
    .unwrap_or_else(|error| panic!("{shown}: {error}"));
    assert!(status.success(), "{shown}: {status}");
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Copia o mundo do servidor para a instância (sem o `session.lock`).
fn copy_world(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "session.lock" {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_world(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// O servidor descartável da matriz: com internet, o `prepare.py` baixa e gera tudo; sem
/// internet, o já preparado é reaproveitado, só com a porta nova.
async fn prepare_server(
    name: &str,
    game: &RealGame,
    root: &Path,
    java: &Path,
    port: u16,
    offline: bool,
) -> PathBuf {
    let (mode, directory) = if game.quick_play {
        (
            "world",
            root.join("servers")
                .join(format!("vanilla-{}", game.minecraft)),
        )
    } else {
        ("legacy", root.join("servers").join(name))
    };
    if offline {
        let properties = directory.join("server.properties");
        let text = std::fs::read_to_string(&properties).unwrap_or_else(|error| {
            panic!(
                "sem internet, o servidor precisa ter sido preparado numa rodada com internet: {}: {error}",
                properties.display()
            )
        });
        let text: String = text
            .lines()
            .map(|line| {
                if line.starts_with("server-port=") {
                    format!("server-port={port}\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect();
        std::fs::write(&properties, text).unwrap();
    } else {
        matrix_script(
            "prepare.py",
            vec![
                mode.into(),
                game.minecraft.into(),
                game.loader.into(),
                game.version.into(),
                directory.display().to_string(),
                java.display().to_string(),
                port.to_string(),
            ],
        )
        .await;
    }
    directory
}

/// Abre o servidor local (antes da 1.20) e espera o "Done".
async fn start_server(directory: &Path, java: &Path) -> (GameProcess, GameEvents) {
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(directory.join("warden-args.json")).unwrap())
            .unwrap();
    let command = warden_launcher::LaunchCommand {
        program: java.to_path_buf(),
        args,
        cwd: directory.to_path_buf(),
        env: Vec::new(),
        argfile: None,
    };
    let (server, mut events) = GameProcess::spawn(
        &command,
        SpawnOptions {
            output_log: Some(directory.join("server-testar.log")),
            stdin: true,
        },
    )
    .await
    .unwrap();
    let ready = tokio::time::timeout(Duration::from_mins(4), async {
        while let Some(event) = events.recv().await {
            match event {
                GameEvent::Line(line) => {
                    if line.text.contains("Done (") && line.text.contains("For help, type") {
                        return Ok(());
                    }
                }
                GameEvent::Exited(exit) => return Err(format!("o servidor saiu: {exit:?}")),
            }
        }
        Err("a saída do servidor acabou".to_owned())
    })
    .await;
    if !matches!(ready, Ok(Ok(()))) {
        server.handle().stop().await;
        let _ = server.wait().await;
        panic!("o servidor local não ficou pronto: {ready:?}");
    }
    (server, events)
}

/// Sem internet de verdade: um pedido ao meta da Mojang falha.
async fn assert_network_blocked() {
    let http = warden_http::HttpClient::new(warden_http::HttpConfig::for_version("0.0.0")).unwrap();
    let url =
        warden_http::Url::parse("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
            .unwrap();
    let result = tokio::time::timeout(Duration::from_mins(2), http.get(url).send()).await;
    assert!(
        !matches!(result, Ok(Ok(_))),
        "WARDEN_TEST_REAL_OFFLINE=1, mas a internet responde: bloqueie a rede antes"
    );
}

/// A linha do console como o leitor de log da matriz a vê (tempo desde a abertura do jogo).
fn as_log_line(line: &ConsoleLine, game_started_ms: u64) -> warden_launcher::log::LogLine {
    warden_launcher::log::LogLine {
        elapsed_ms: line
            .at_ms
            .unwrap_or(game_started_ms)
            .saturating_sub(game_started_ms),
        timestamp_ms: None,
        time: line.time.clone(),
        level: None,
        thread: line.thread.clone(),
        logger: line.logger.clone(),
        text: line.text.clone(),
        source: warden_launcher::log::LogSource::Stdout,
    }
}

/// CA-T13-05 com o motor de verdade, ainda sem internet: sem o jar do Minecraft, o Testar
/// para antes de abrir o jogo e a mensagem diz o arquivo que falta. O jar volta no fim.
#[allow(clippy::print_stdout)]
async fn missing_game_file_is_named(state: &Arc<AppState>, pack: PackId, minecraft: &str) {
    // O motor grava o jar do jogo na pasta da versão final (`versions/<id>/<id>.jar`).
    let versions = EngineDirs::for_app(&state.paths).versions;
    let jar = std::fs::read_dir(&versions)
        .unwrap()
        .find_map(|entry| {
            let id = entry.ok()?.file_name().to_string_lossy().into_owned();
            let jar = versions.join(&id).join(format!("{id}.jar"));
            (id.contains(minecraft) && jar.is_file()).then_some(jar)
        })
        .expect("o jar do jogo instalado");
    let name = jar.file_name().unwrap().to_string_lossy().into_owned();
    let aside = jar.with_extension("jar.roteiro");
    std::fs::rename(&jar, &aside).unwrap();
    let (sink, seen) = sink();
    let result = run_test(state, pack, TestRequest::default(), sink).await;
    std::fs::rename(&aside, &jar).unwrap();
    let error = result.expect_err("sem o jar do Minecraft e sem internet, o teste abriu");
    assert_eq!(
        error.code,
        ErrorCode::App(AppErrorCode::TestGameFilesMissing),
        "{error:?}"
    );
    assert_eq!(error.params["names"], name, "{error:?}");
    assert!(seen.console_texts().is_empty(), "o jogo abriu");
    println!(
        "sem o {name} e sem internet: app.TEST_GAME_FILES_MISSING com {:?}",
        error.params
    );
}

/// CA-T13-01 pelo Testar, com o Minecraft de verdade (Linux com Xvfb e Mesa; no Windows abre
/// a janela do jogo e ninguém precisa clicar): o pack mínimo da matriz (mod simples, com Mixin
/// no Fabric) passa por todas as etapas do Testar (Java baixado pela `warden-java`, jogo e
/// loader pelo motor, cópia do pack com o mod baixado do Modrinth), chega ao menu principal
/// ("Pronto": atlas e som; no Forge legado, também o "successfully loaded") e entra num mundo
/// ("Mundo": `logged in with entity id`) sem sinal de falha; depois o teste para o jogo e a
/// sessão sai "Encerrado por você". Como na matriz da L-05, o mundo é gerado por um servidor
/// descartável (`prepare.py`): em 1.20+ é copiado para `saves/` e o jogo entra por Quick Play;
/// antes da 1.20 o jogo entra no servidor local com `--server/--port`. Entrar no mundo usa um
/// ajuste da abertura só deste teste ([`LaunchTweak`]); no app, isso é do perfil do teste
/// (L-08).
///
/// Com `WARDEN_TEST_REAL_OFFLINE=1` (CA-T13-05), roda sem internet sobre a pasta de uma rodada
/// anterior com internet: confere que a rede está bloqueada e repete tudo, sem preparar o
/// servidor de novo; depois esconde o jar do Minecraft e confere que a mensagem diz que ele
/// falta.
///
/// Uma combinação por vez (`fabric-1.20.1` ou `forge-1.12.2`), com `WARDEN_TEST_REAL_ROOT`
/// guardando os downloads entre as rodadas; passo a passo em `roteiro-jogo-real.md`, nesta pasta.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "abre o Minecraft de verdade e baixa centenas de MB"]
#[allow(clippy::print_stdout, clippy::too_many_lines)]
async fn ca_t13_01_jogo_real_entra_no_mundo_pelo_testar() {
    let Ok(wanted) = std::env::var("WARDEN_TEST_REAL_GAME") else {
        eprintln!("WARDEN_TEST_REAL_GAME ausente: nada a fazer");
        return;
    };
    let (name, game) = REAL_GAMES
        .iter()
        .find(|(name, _)| *name == wanted)
        .unwrap_or_else(|| panic!("combinação desconhecida: {wanted}"));
    let offline = std::env::var("WARDEN_TEST_REAL_OFFLINE").is_ok_and(|value| value == "1");
    if offline {
        assert_network_blocked().await;
    }
    let temp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("WARDEN_TEST_REAL_ROOT")
        .map_or_else(|| temp.path().to_path_buf(), PathBuf::from);
    assert!(
        !offline || std::env::var_os("WARDEN_TEST_REAL_ROOT").is_some(),
        "sem internet, WARDEN_TEST_REAL_ROOT precisa apontar a pasta da rodada com internet"
    );

    // O pack mínimo da matriz, registrado uma vez só.
    let pack_root = root.join("packs").join(name);
    if !offline {
        matrix_script(
            "pack.py",
            vec![
                "create".into(),
                (*name).into(),
                game.minecraft.into(),
                game.loader.into(),
                game.version.into(),
                pack_root.display().to_string(),
            ],
        )
        .await;
    }
    let mut state = test_state(&root);
    let pack = state
        .packs
        .records()
        .into_iter()
        .find(|record| record.path == pack_root)
        .map_or_else(|| register(&state, &pack_root), |record| record.id);

    // O servidor (que gera o mundo ou recebe o jogo), com o Java que o Testar vai usar.
    let loader = match game.loader {
        "forge" => LoaderKind::Forge,
        _ => LoaderKind::Fabric,
    };
    let choice = state
        .java
        .choose(&JavaChoiceRequest::automatic(
            game.minecraft,
            loader,
            Some(game.version),
        ))
        .unwrap();
    let java = state
        .java
        .ensure_choice(&choice, &warden_core::NoProgress, &CancellationToken::new())
        .await
        .unwrap()
        .java;
    let port = free_port();
    let server_dir = prepare_server(name, game, &root, &java, port, offline).await;

    // A instância do teste: sem a tela de acessibilidade e, em 1.20+, com o mundo.
    let game_dir = InstanceDirs::for_pack(&state.paths, pack).game_dir;
    std::fs::create_dir_all(&game_dir).unwrap();
    std::fs::write(game_dir.join("options.txt"), "onboardAccessibility:false\n").unwrap();
    if game.quick_play {
        let world = game_dir.join("saves").join(REAL_WORLD);
        if world.exists() {
            std::fs::remove_dir_all(&world).unwrap();
        }
        copy_world(&server_dir.join("world"), &world);
        let _ = std::fs::remove_file(game_dir.join("quickplay.json"));
    }
    let quick_play = game.quick_play;
    let legacy_forge = *name == "forge-1.12.2";
    let fabric = game.loader == "fabric";
    let tweak = LaunchTweak {
        options: Arc::new(move |options: &mut warden_launcher::LaunchOptions| {
            if fabric {
                options
                    .system_props
                    .push(("fabric.noGui".into(), "true".into()));
            }
            if legacy_forge {
                options
                    .system_props
                    .push(("fml.queryResult".into(), "confirm".into()));
            }
            if quick_play {
                options.quick_play = Some(warden_launcher::QuickPlay::Singleplayer {
                    world: REAL_WORLD.into(),
                });
                options.quick_play_path = Some("quickplay.json".into());
            } else {
                options.quick_play = Some(warden_launcher::QuickPlay::Multiplayer {
                    host: "127.0.0.1".into(),
                    port,
                });
            }
        }),
        env: vec![("ALSOFT_DRIVERS".into(), "null".into())],
        refuse_command: false,
    };
    state.tests =
        Arc::new(TestSessions::new(&state.paths, &state.catalog).with_launch_tweak(tweak));
    let state = Arc::new(state);
    let server = if game.quick_play {
        None
    } else {
        Some(start_server(&server_dir, &java).await)
    };

    // O clique em Testar.
    let clicked = Instant::now();
    let (task, seen) = spawn_test(&state, pack, TestRequest::default());
    let (server, mut server_events) = server.unzip();
    let mut progress = markers::Progress::default();
    let mut read = 0;
    let mut game_started_ms = None;
    let mut ready_since_click = None;
    let mut world_since_click = None;
    let deadline = Instant::now() + Duration::from_mins(45);
    let mut ticker = tokio::time::interval(Duration::from_millis(250));
    let result: Result<(), String> = loop {
        tokio::select! {
            _ = ticker.tick() => {
                let lines: Vec<ConsoleLine> = seen
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .filter_map(|event| match event {
                        OperationEvent::Console { lines } => Some(lines.clone()),
                        _ => None,
                    })
                    .flatten()
                    .collect();
                for line in &lines[read..] {
                    if line.text == "console.abrindo" {
                        game_started_ms = line.at_ms;
                    }
                    if let (Some(started), ConsoleOrigin::Game) = (game_started_ms, line.origin) {
                        progress.observe(game.minecraft, game.loader, &as_log_line(line, started), false);
                    }
                }
                read = lines.len();
            }
            event = async { server_events.as_mut().unwrap().recv().await }, if server_events.is_some() => {
                match event {
                    Some(GameEvent::Line(mut line)) => {
                        // Os tempos começam na abertura do jogo, como na matriz.
                        line.elapsed_ms = game_started_ms.map_or(0, |started| now_ms().saturating_sub(started));
                        progress.observe(game.minecraft, game.loader, &line, true);
                    }
                    Some(GameEvent::Exited(exit)) => break Err(format!("o servidor saiu antes do mundo: {exit:?}")),
                    None => break Err("a saída do servidor acabou".to_owned()),
                }
            }
        }
        if progress.ready_ms.is_some() {
            ready_since_click.get_or_insert_with(|| clicked.elapsed());
        }
        if progress.world_ms.is_some() {
            world_since_click.get_or_insert_with(|| clicked.elapsed());
        }
        if let Some(reason) = progress.failure {
            break Err(format!("sinal de falha no console: {reason}"));
        }
        if progress.ready_ms.is_some() && progress.world_ms.is_some() {
            break Ok(());
        }
        if task.is_finished() {
            break Err("o teste terminou antes de o jogo entrar no mundo".to_owned());
        }
        if Instant::now() > deadline {
            break Err("o jogo não entrou no mundo em 45 min".to_owned());
        }
    };
    if result.is_ok() {
        // Alguns segundos no mundo, como quem olha.
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    let _ = state.tests.stop(pack).await;
    let summary = tokio::time::timeout(Duration::from_secs(30), task).await;
    if let Some(server) = server {
        let _ = server.handle().send_line("stop").await;
        let _ = tokio::time::timeout(Duration::from_secs(30), server.handle().stop()).await;
        let _ = server.wait().await;
    }
    if let Err(reason) = result {
        let tail: Vec<String> = seen.console_texts().into_iter().rev().take(40).collect();
        let ended = summary.ok().and_then(Result::ok);
        panic!(
            "{wanted}: {reason}; resultado do Testar: {ended:?}; {progress:?}; últimas linhas: {tail:#?}"
        );
    }
    let summary = summary
        .expect("o teste não terminou depois de Parar jogo")
        .unwrap()
        .unwrap();
    assert_eq!(summary.outcome, Outcome::StoppedByUser);
    // O mod veio pela cópia do pack do Testar.
    matrix_script(
        "pack.py",
        vec![
            "verify".into(),
            (*name).into(),
            game_dir.display().to_string(),
        ],
    )
    .await;
    if game.quick_play {
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(game_dir.join("quickplay.json")).unwrap())
                .unwrap();
        assert_eq!(data[0]["type"], "singleplayer");
    }
    let options = std::fs::read_to_string(game_dir.join("options.txt")).unwrap();
    assert!(
        options
            .lines()
            .any(|line| line == "onboardAccessibility:false"),
        "o jogo pediu a tela de acessibilidade"
    );
    if offline {
        missing_game_file_is_named(&state, pack, game.minecraft).await;
    }
    let since_click = |elapsed: Option<Duration>| elapsed.map_or(0.0, |e| e.as_secs_f64());
    println!(
        "{wanted}{}: Pronto em {:.1} s e Mundo em {:.1} s desde a abertura do jogo \
         ({:.0} s e {:.0} s desde o clique em Testar); Java {}, {} MB; sessão {}; {} linhas no console",
        if offline { " (sem internet)" } else { "" },
        Duration::from_millis(progress.ready_ms.unwrap_or_default()).as_secs_f64(),
        Duration::from_millis(progress.world_ms.unwrap_or_default()).as_secs_f64(),
        since_click(ready_since_click),
        since_click(world_since_click),
        summary.java_major,
        summary.memory_mb.unwrap_or_default(),
        summary.id,
        seen.console_texts().len()
    );
}

/// Metafile de um mod da CurseForge (`metadata:curseforge`, hash sha1).
fn curseforge_metafile(name: &str, filename: &str, project: u64, file: u64, jar: &[u8]) -> String {
    format!(
        "name = \"{name}\"\nfilename = \"{filename}\"\nside = \"both\"\n\n[download]\n\
         hash-format = \"sha1\"\nhash = \"{}\"\nmode = \"metadata:curseforge\"\n\n\
         [update]\n[update.curseforge]\nfile-id = {file}\nproject-id = {project}\n",
        sha1_hex(jar)
    )
}

fn sha1_hex(bytes: &[u8]) -> String {
    warden_packwiz::hash::hash_bytes(warden_packwiz::HashFormat::Sha1, bytes)
}

/// CurseForge simulada: o arquivo existe, mas o autor bloqueou o download por apps de
/// terceiros (sem `downloadUrl`), e o projeto tem a página no site.
async fn blocked_curseforge(
    project: u64,
    file: u64,
    filename: &str,
    jar: &[u8],
) -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    let data = |value: serde_json::Value| {
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json; charset=utf-8")
            .set_body_json(serde_json::json!({ "data": value }))
    };
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(data(serde_json::json!([{
            "id": file,
            "gameId": 432,
            "modId": project,
            "isAvailable": true,
            "displayName": filename,
            "fileName": filename,
            "releaseType": 1,
            "fileStatus": 4,
            "hashes": [{ "value": sha1_hex(jar), "algo": 1 }],
            "fileDate": "2026-09-19T11:47:57.347Z",
            "fileLength": jar.len(),
            "downloadCount": 7,
            "downloadUrl": null,
            "gameVersions": ["1.20.1"],
            "dependencies": [],
            "fileFingerprint": warden_packwiz::hash::curseforge_fingerprint(jar),
        }])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/mods"))
        .respond_with(data(serde_json::json!([{
            "id": project,
            "gameId": 432,
            "name": "Entity Culling",
            "slug": "entityculling",
            "links": { "websiteUrl": "https://www.curseforge.com/minecraft/mc-mods/entityculling" },
            "summary": "",
            "status": 4,
            "downloadCount": 1,
            "classId": 6,
            "authors": [],
            "allowModDistribution": false,
            "isAvailable": true,
        }])))
        .mount(&server)
        .await;
    server
}

/// Estado com a CurseForge simulada e a chave, e um pack com o Entity Culling bloqueado.
fn blocked_pack(
    dir: &Path,
    server: &wiremock::MockServer,
    jar: &[u8],
) -> Option<(Arc<AppState>, PackId, PathBuf)> {
    let state = fake_state_with(dir, Some(format!("{}/v1/", server.uri())))?;
    state
        .secrets
        .set(
            warden_secrets::SecretKind::Curseforge,
            &secrecy::SecretString::from("chave-de-teste".to_owned()),
        )
        .unwrap();
    let root = dir.join("pack");
    let meta = curseforge_metafile(
        "Entity Culling",
        "entityculling-1.0.jar",
        448_233,
        6_000_001,
        jar,
    );
    write_pack(
        &root,
        &[
            ("mods/entityculling.pw.toml", meta.as_bytes()),
            ("config/a.toml", b"x = 1"),
        ],
        "acentos",
    );
    let pack = register(&state, &root);
    Some((state, pack, root))
}

/// CA-T20-01 e CA-T20-02 pelo app inteiro, com o jogo simulado: um mod da CurseForge bloqueado
/// para apps de terceiros não derruba o teste. A etapa "Copiar o pack para o teste" publica a
/// lista e espera; o arquivo errado (numa pasta com espaço e acento) é recusado sem cópia; com
/// o certo, o teste continua sozinho, o mod chega à instância e o jogo abre. A pasta do pack
/// não recebe jar nenhum, e o teste seguinte acha o arquivo no cache, sem esperar.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t20_01_e_02_mod_bloqueado_espera_o_arquivo_certo_e_o_teste_continua() {
    use warden_instance::{FoundVia, ManualDownloadStatus};
    let dir = tempfile::tempdir().unwrap();
    let jar = b"jar do Entity Culling".to_vec();
    let server = blocked_curseforge(448_233, 6_000_001, "entityculling-1.0.jar", &jar).await;
    let Some((state, pack, root)) = blocked_pack(dir.path(), &server, &jar) else {
        return;
    };
    let (task, seen) = spawn_test(&state, pack, TestRequest::default());

    // O teste para na lista e espera, sem abrir o jogo.
    let deadline = Instant::now() + Duration::from_secs(30);
    while state.manual_downloads.list(pack).is_empty() {
        assert!(Instant::now() < deadline, "a lista não apareceu em 30 s");
        assert!(!task.is_finished(), "o teste terminou em vez de esperar");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let items = state.manual_downloads.list(pack);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Entity Culling");
    assert_eq!(items[0].file_name, "entityculling-1.0.jar");
    assert!(items[0].has_page);
    assert_eq!(
        state.tests.current().map(|game| game.state),
        Some(GamePhase::Preparing)
    );
    assert!(seen.events.lock().unwrap().iter().any(|event| matches!(
        event,
        OperationEvent::Stage { stage, .. } if stage == crate::commands::cf_blocked::STAGE_MANUAL
    )));

    // Arquivo errado: recusado, nada copiado, o teste continua esperando.
    let downloads = dir.path().join("Meus Downloads").join("Mods é ação");
    std::fs::create_dir_all(&downloads).unwrap();
    let wrong = downloads.join("entityculling-1.0 (1).jar");
    std::fs::write(&wrong, b"outro jar qualquer").unwrap();
    let item = state
        .manual_downloads
        .provide(pack, &items[0].metafile, wrong)
        .await
        .unwrap();
    assert_eq!(
        item.status,
        ManualDownloadStatus::WrongFile {
            chosen_file: "entityculling-1.0 (1).jar".to_owned()
        }
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished());

    // Arquivo certo: o teste continua sozinho e o jogo abre.
    let right = downloads.join("entityculling-1.0.jar");
    std::fs::write(&right, &jar).unwrap();
    let item = state
        .manual_downloads
        .provide(pack, &items[0].metafile, right.clone())
        .await
        .unwrap();
    assert_eq!(
        item.status,
        ManualDownloadStatus::Found {
            via: FoundVia::Selected
        }
    );
    let summary = tokio::time::timeout(Duration::from_secs(60), task)
        .await
        .expect("o teste continuou e terminou")
        .unwrap()
        .unwrap();
    assert_eq!(summary.outcome, Outcome::ClosedNormally);
    assert!(seen.phases().contains(&GamePhase::Running), "o jogo abriu");
    assert!(state.manual_downloads.list(pack).is_empty());
    let game_dir = InstanceDirs::for_pack(&state.paths, pack).game_dir;
    assert_eq!(
        std::fs::read(game_dir.join("mods/entityculling-1.0.jar")).unwrap(),
        jar
    );
    assert!(right.exists(), "o arquivo escolhido fica onde estava");

    // CA-T20-02: a pasta do pack não contém o jar.
    let jars: Vec<PathBuf> = walk(&root)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "jar"))
        .collect();
    assert!(jars.is_empty(), "{jars:?}");

    // O teste seguinte acha o arquivo no cache: nada a esperar.
    std::fs::remove_file(game_dir.join("mods/entityculling-1.0.jar")).unwrap();
    let (sink, _) = sink();
    let again = tokio::time::timeout(
        Duration::from_secs(60),
        run_test(&state, pack, TestRequest::default(), sink),
    )
    .await
    .expect("o segundo teste não esperou downloads manuais")
    .unwrap();
    assert_eq!(again.outcome, Outcome::ClosedNormally);
    assert!(game_dir.join("mods/entityculling-1.0.jar").is_file());
}

/// Todos os arquivos de uma pasta, recursivamente.
fn walk(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found
}

/// As amostras de desempenho que passaram pelo canal.
fn perf_samples(seen: &Seen) -> Vec<OperationEvent> {
    seen.events
        .lock()
        .unwrap()
        .iter()
        .filter(|event| matches!(event, OperationEvent::PerfSample { .. }))
        .cloned()
        .collect()
}

/// Espera uma amostra com os dois tempos e a RAM medida depois de o jogo simulado (modo
/// `marcadores`) ocupar os 64 MB, o que ele faz logo depois de entrar no mundo.
async fn wait_loaded_and_measured(seen: &Seen) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let ready = perf_samples(seen).iter().any(|event| {
            matches!(
                event,
                OperationEvent::PerfSample {
                    rss_mb: Some(rss),
                    loaded_ms: Some(_),
                    world_ms: Some(_),
                    ..
                } if *rss >= 64
            )
        });
        if ready {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "sem amostra com RAM e tempos em 30 s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Um teste com o jogo simulado no modo `marcadores`, parado depois de carregar e de uma
/// amostra completa.
async fn test_with_markers(state: &Arc<AppState>, pack: PackId) -> (TestSessionSummary, Arc<Seen>) {
    let (task, seen) = spawn_test(state, pack, TestRequest::default());
    wait_loaded_and_measured(&seen).await;
    state.tests.stop(pack).await.unwrap();
    let summary = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("o teste não terminou")
        .unwrap()
        .unwrap();
    (summary, seen)
}

/// A memória do jogo de um teste com o jogo simulado (L-11): ele é uma JVM de verdade, que
/// publica o `hsperfdata`; o máximo é a memória do teste. Devolve o resumo gravado na sessão.
fn assert_heap_read(
    seen: &Seen,
    first: &TestSessionSummary,
    peak: u32,
) -> warden_perf::HeapSummary {
    let perf = first.perf.clone().expect("desempenho na sessão");
    let Some(OperationEvent::PerfSample {
        heap_used_mb,
        heap_max_mb,
        heap,
        heap_unavailable,
        peak_rss_mb,
        ..
    }) = perf_samples(seen).last().cloned()
    else {
        unreachable!()
    };
    assert!(heap_used_mb.is_some_and(|mb| mb >= 64), "{heap_used_mb:?}");
    assert_eq!(heap_max_mb, first.memory_mb);
    assert!(heap.is_some_and(|heap| !heap.series_mb.is_empty() && heap.low_heap_s.is_none()));
    assert_eq!(heap_unavailable, None);
    assert!(peak_rss_mb.is_some_and(|mb| mb <= peak));
    let heap = perf.heap.clone().expect("memória do jogo na sessão");
    assert!(heap.peak_used_mb >= 64, "{heap:?}");
    assert_eq!(Some(heap.max_mb), first.memory_mb);
    assert!(heap.readings >= 1);
    assert_eq!(perf.heap_unavailable, None);
    // 64 MB de uma heap de alguns GB, por poucos segundos: nenhum achado de memória.
    assert!(perf.findings.is_empty(), "{:?}", perf.findings);

    heap
}

/// CA-T13-09: toda sessão grava o pico de RAM do processo e os tempos até carregar e até
/// entrar no mundo; com o jogo aberto, as amostras passam pelo canal a cada 1 s e param quando
/// o jogo fecha; a sessão de outra versão do pack vira a comparação; cada teste acrescenta uma
/// linha no histórico `perf/<pack-id>.jsonl` (gancho 1.2).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_09_desempenho_grava_pico_e_tempos_e_compara_versoes() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[], "marcadores 300");
    let pack = register(&state, &root);

    let (first, seen) = test_with_markers(&state, pack).await;
    let perf = first.perf.clone().expect("desempenho na sessão");
    // 64 MB ocupados pelo jogo simulado, mais a própria JVM.
    let peak = perf.peak_rss_mb.expect("pico de RAM");
    assert!(peak >= 64, "pico de {peak} MB");
    let loaded = perf.times.loaded_ms.expect("abriu em");
    let world = perf.times.world_ms.expect("entrou no mundo");
    assert!(loaded >= 300, "carregou em {loaded} ms");
    assert!(
        world >= loaded + 150,
        "mundo em {world} ms, carregou em {loaded} ms"
    );
    assert!(perf.samples >= 1);
    assert_eq!(perf.previous, None, "primeira sessão: nada para comparar");
    if cfg!(windows) {
        assert!(perf.peak_commit_mb.is_some_and(|mb| mb >= 64), "{perf:?}");
    }
    // As amostras param quando o jogo fecha.
    let after_exit = perf_samples(&seen).len();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(perf_samples(&seen).len(), after_exit);
    let heap = assert_heap_read(&seen, &first, peak);

    // O session.json tem o campo; a lista de sessões lê o mesmo.
    let session = session_dir(&state.paths, pack, &first.id).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(session.path().join(SESSION_FILE)).unwrap()).unwrap();
    assert_eq!(json["perf"]["loadedMs"], loaded);
    assert_eq!(json["perf"]["peakRssMb"], peak);
    assert_eq!(sessions_of(&state.paths, pack).unwrap()[0].perf, first.perf);

    // Outra versão do pack, mais lenta para carregar.
    let manifest = std::fs::read_to_string(root.join("pack.toml")).unwrap();
    assert!(manifest.contains("version = \"1.0.0\""), "{manifest}");
    std::fs::write(
        root.join("pack.toml"),
        manifest.replace("version = \"1.0.0\"", "version = \"1.1.0\""),
    )
    .unwrap();
    // Bem mais lenta: a partida da JVM varia alguns segundos entre máquinas e entre a primeira
    // abertura e as seguintes.
    std::fs::write(root.join(fake::ARGS_FILE), "marcadores 6000").unwrap();
    let (second, seen) = test_with_markers(&state, pack).await;
    let perf = second.perf.clone().unwrap();
    let previous = perf.previous.clone().expect("comparação com a 1.0.0");
    assert_eq!(previous.pack_version, "1.0.0");
    assert_eq!(previous.session_id, first.id);
    assert_eq!(previous.loaded_ms, loaded);
    let delta = perf.loaded_delta_ms().unwrap();
    assert!(delta >= 3000, "deveria ser bem mais lento: {delta} ms");
    // A comparação já vai ao vivo, na faixa.
    assert!(perf_samples(&seen).iter().any(|event| matches!(
        event,
        OperationEvent::PerfSample { previous: Some(p), .. } if p.pack_version == "1.0.0"
    )));

    // Histórico: uma linha por teste, com os campos do contrato da 1.2.
    let history = std::fs::read_to_string(state.paths.perf_file(pack)).unwrap();
    let lines: Vec<serde_json::Value> = history
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["sessao"], first.id.as_str());
    assert_eq!(lines[0]["modo"], "normal");
    assert_eq!(lines[0]["carregou_ms"], loaded);
    assert_eq!(lines[0]["ram_pico_mb"], peak);
    // O pico depois das coletas, ou `null` (o jogo simulado quase não coleta), nunca o bruto.
    assert_eq!(
        lines[0].get("heap_pico_mb"),
        Some(&serde_json::json!(heap.peak_after_gc_mb))
    );
    assert_eq!(lines[0]["primeira_abertura"], true);
    assert_eq!(lines[1]["primeira_abertura"], false);
    assert_eq!(lines[1]["resultado"], "stoppedByUser");
    assert_eq!(lines[0]["perfil"]["id"], "default");
    for line in &lines {
        for key in ["data", "computador", "arvore"] {
            assert!(
                line[key].as_str().is_some_and(|text| !text.is_empty()),
                "{key}: {line}"
            );
        }
        assert!(
            line["perfil"]["assinatura"]
                .as_str()
                .is_some_and(|hash| hash.starts_with("sha256:"))
        );
    }
    // A poda das sessões não toca no histórico.
    assert!(
        !state
            .paths
            .perf_file(pack)
            .starts_with(InstanceDirs::for_pack(&state.paths, pack).state_dir)
    );
}

/// CA-T13-10 (parte da faixa): com `-XX:+PerfDisableSharedMem` nos argumentos do teste, a JVM
/// não publica a memória; a amostra traz o motivo (o argumento) desde a primeira, sem número
/// inventado, e a sessão grava o mesmo motivo, sem achados de memória.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_10_argumento_que_desliga_a_leitura_vira_o_motivo() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[], "marcadores 300");
    let pack = register(&state, &root);
    let fields = crate::test_profiles::ProfileFields {
        jvm_args: "-Dx=1 -XX:+PerfDisableSharedMem".into(),
        ..Default::default()
    };
    let profile =
        crate::test_profiles::create_profile(&state, pack, "Sem leitura", fields).unwrap();
    crate::test_profiles::select_profile(&state, pack, &profile.id).unwrap();

    let (summary, seen) = test_with_markers(&state, pack).await;
    let argument = warden_perf::HeapUnavailable::Argument {
        argument: "-XX:+PerfDisableSharedMem".into(),
    };
    let samples = perf_samples(&seen);
    assert!(!samples.is_empty());
    for event in &samples {
        let OperationEvent::PerfSample {
            heap_used_mb,
            heap,
            heap_unavailable,
            ..
        } = event
        else {
            unreachable!()
        };
        assert_eq!(*heap_used_mb, None);
        assert_eq!(*heap, None);
        assert_eq!(heap_unavailable.as_ref(), Some(&argument));
    }
    let perf = summary.perf.unwrap();
    assert_eq!(perf.heap, None);
    assert_eq!(perf.heap_unavailable, Some(argument));
    assert!(perf.findings.is_empty());
}

/// Um jogo que fecha sozinho antes de carregar grava a sessão sem "Abriu em" e sem travar a
/// medição.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn jogo_que_fecha_logo_grava_sem_tempos() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = fake_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[], "sair 0");
    let pack = register(&state, &root);
    let (sink, _) = sink();
    let summary = run_test(&state, pack, TestRequest::default(), sink)
        .await
        .unwrap();
    assert_eq!(summary.outcome, Outcome::ClosedNormally);
    let perf = summary.perf.expect("o campo existe mesmo sem tempos");
    assert_eq!(perf.times, warden_perf::LoadTimes::default());
    assert_eq!(perf.previous, None);
}

/// O jogo simulado com as opções da abertura guardadas em `seen_options` (L-08).
fn fake_state_watching(
    root: &Path,
    seen_options: Arc<StdMutex<Option<warden_launcher::LaunchOptions>>>,
) -> Option<Arc<AppState>> {
    let Some(java) = java() else {
        eprintln!("sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): teste do jogo simulado pulado");
        return None;
    };
    let mut state = test_state(root);
    let engine = PortableMcEngine::new(EngineDirs::under(&root.join("shared")), None);
    let major = java_major(&java);
    let sessions = TestSessions::with_engine(
        Arc::new(engine),
        Some(FakeGame::new(class_dir(), java, major)),
    )
    .with_launch_tweak(LaunchTweak {
        options: Arc::new(move |options| {
            *seen_options.lock().unwrap() = Some(options.clone());
        }),
        env: Vec::new(),
        refuse_command: false,
    });
    state.tests = Arc::new(sessions);
    Some(Arc::new(state))
}

/// Hash de todos os arquivos de uma pasta (caminho relativo + conteúdo).
fn folder_hash(root: &Path) -> String {
    let mut files = walk(root);
    files.sort();
    let mut hasher = Sha256::new();
    for file in files {
        hasher.update(
            file.strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .as_bytes(),
        );
        hasher.update(std::fs::read(&file).unwrap());
    }
    hex::encode(hasher.finalize())
}

fn warnings(seen: &Seen) -> Vec<ErrorCode> {
    seen.events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            OperationEvent::Warning { error } => Some(error.code),
            _ => None,
        })
        .collect()
}

/// CA-T11-04 pelo app inteiro: criar o perfil "PC fraco (4 GB)", escolhê-lo (menu ▾) e testar
/// abre o jogo com 4 GB (`-Xmx4096M`, a linha de comando está no golden de `test_profiles`),
/// com a janela do perfil; o `game-state` e a sessão levam o perfil; nenhum arquivo do pack
/// muda (hash da pasta antes e depois).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t11_04_perfil_escolhido_vale_no_teste_e_o_pack_nao_muda() {
    let dir = tempfile::tempdir().unwrap();
    let seen_options = Arc::new(StdMutex::new(None));
    let Some(state) = fake_state_watching(dir.path(), Arc::clone(&seen_options)) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[], "args");
    let pack = register(&state, &root);
    let before = folder_hash(&root);

    let fields = crate::test_profiles::ProfileFields {
        memory: crate::settings::TestMemory::Fixed { mb: 4096 },
        window: crate::test_profiles::TestWindow::Size {
            width: 1280,
            height: 720,
        },
        jvm_args: "-XX:+PerfDisableSharedMem".into(),
        ..Default::default()
    };
    let profile =
        crate::test_profiles::create_profile(&state, pack, "PC fraco (4 GB)", fields).unwrap();
    crate::test_profiles::select_profile(&state, pack, &profile.id).unwrap();

    let (sink, seen) = sink();
    let summary = run_test(&state, pack, TestRequest::default(), sink)
        .await
        .unwrap();
    assert_eq!(summary.memory_mb, Some(4096));
    let options = seen_options
        .lock()
        .unwrap()
        .clone()
        .expect("opções da abertura");
    assert_eq!(options.memory_mb, 4096);
    assert_eq!(options.quick_play, None);
    let texts = seen.console_texts().join("\n");
    assert!(
        texts.contains("[--width]\n[1280]\n[--height]\n[720]"),
        "{texts}"
    );
    assert!(
        seen.states
            .lock()
            .unwrap()
            .iter()
            .all(|game| game.profile.as_deref() == Some(profile.id.as_str()))
    );
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            session_dir(&state.paths, pack, &summary.id)
                .unwrap()
                .path()
                .join(SESSION_FILE),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(record["profile"], profile.id.as_str());
    assert_eq!(record["profileSignature"]["window"], "1280x720");
    // 4 GB é também a memória automática de um pack pequeno: o que prova o perfil é o valor
    // fixo (não automático) na assinatura.
    assert_eq!(record["profileSignature"]["memoryAuto"], false);
    assert_eq!(record["profileSignature"]["memoryMb"], 4096);
    // `W_PERF_FLAGS`: o argumento que impede ler a memória vira aviso do teste.
    assert!(warnings(&seen).contains(&ErrorCode::App(AppErrorCode::TestPerfFlags)));
    assert_eq!(folder_hash(&root), before, "o pack não pode mudar");
}

/// CA-T11-05 pelo app inteiro (pack 1.20.1): com "Entrar direto no mundo de teste", o jogo
/// recebe `--quickPlaySingleplayer` com a pasta do mundo jogado por último; sem mundo ainda, o
/// teste abre no menu e avisa.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t11_05_entrar_direto_no_mundo_de_teste() {
    let dir = tempfile::tempdir().unwrap();
    let seen_options = Arc::new(StdMutex::new(None));
    let Some(state) = fake_state_watching(dir.path(), Arc::clone(&seen_options)) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[], "args");
    let pack = register(&state, &root);
    let mut profiles = crate::test_profiles::profiles_of(&state, pack)
        .unwrap()
        .profiles;
    profiles[0].fields.on_launch = crate::test_profiles::OnLaunch::World { world: None };
    crate::test_profiles::save_profiles(&state, pack, profiles).unwrap();

    // Sem mundo de teste: menu principal, com aviso.
    let (events, seen) = sink();
    run_test(&state, pack, TestRequest::default(), events)
        .await
        .unwrap();
    assert!(warnings(&seen).contains(&ErrorCode::App(AppErrorCode::TestQuickPlayNoWorld)));
    assert!(
        !seen
            .console_texts()
            .iter()
            .any(|text| text.contains("quickPlay"))
    );

    // Com mundo: entra direto nele.
    let world = InstanceDirs::for_pack(&state.paths, pack)
        .game_dir
        .join("saves")
        .join("Mundo de teste");
    std::fs::create_dir_all(&world).unwrap();
    std::fs::write(world.join("level.dat"), b"nbt").unwrap();
    let (events, seen) = sink();
    run_test(&state, pack, TestRequest::default(), events)
        .await
        .unwrap();
    let texts = seen.console_texts().join("\n");
    assert!(
        texts.contains("[--quickPlaySingleplayer]\n[Mundo de teste]"),
        "{texts}"
    );
    assert!(warnings(&seen).is_empty(), "{:?}", warnings(&seen));
    let options = seen_options.lock().unwrap().clone().unwrap();
    assert_eq!(
        options.quick_play,
        Some(warden_launcher::QuickPlay::Singleplayer {
            world: "Mundo de teste".into()
        })
    );
    // 1.20.1 não tem a tela de boas-vindas que segura o Quick Play: o options.txt fica como está.
    assert!(
        !InstanceDirs::for_pack(&state.paths, pack)
            .game_dir
            .join("options.txt")
            .exists()
    );
}
