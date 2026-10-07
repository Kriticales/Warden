//! Testes do Testar com o jogo simulado da `warden-launcher` (`WardenFakeGame`): o fluxo
//! inteiro do app (um jogo por vez, sincronização da instância, processo, console, parar,
//! sessão gravada), sem baixar Java nem Minecraft.
//!
//! Usam o Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`; sem Java, avisam e passam (como os testes
//! de processo da `warden-launcher`).

#![allow(clippy::print_stderr)] // aviso de teste pulado

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
fn java() -> Option<PathBuf> {
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
fn class_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../crates/warden-launcher/tests/fixtures/java")
}

/// O major do Java (arquivo `release` da pasta do Java).
fn java_major(java: &Path) -> u32 {
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
fn fake_state(root: &Path) -> Option<Arc<AppState>> {
    let Some(java) = java() else {
        eprintln!("sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): teste do jogo simulado pulado");
        return None;
    };
    let mut state = test_state(root);
    let engine = PortableMcEngine::new(EngineDirs::under(&root.join("shared")), None);
    let major = java_major(&java);
    state.tests = Arc::new(TestSessions::with_engine(
        Arc::new(engine),
        Some(FakeGame::new(class_dir(), java, major)),
    ));
    Some(Arc::new(state))
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Grava um pack vanilla 1.20.1 com os arquivos dados (todos no índice) e o registra.
fn write_pack(root: &Path, files: &[(&str, &[u8])], fake_args: &str) {
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
            metafile: false,
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

fn register(state: &AppState, root: &Path) -> PackId {
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
struct Seen {
    events: StdMutex<Vec<OperationEvent>>,
    states: StdMutex<Vec<GameState>>,
}

impl Seen {
    fn console_texts(&self) -> Vec<String> {
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

    fn phases(&self) -> Vec<GamePhase> {
        self.states
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.state)
            .collect()
    }
}

fn sink() -> (TestSink, Arc<Seen>) {
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
fn spawn_test(
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

/// Espera o jogo abrir (e a primeira linha chegar).
async fn wait_running(state: &AppState, seen: &Seen) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let running = state
            .tests
            .current()
            .is_some_and(|game| game.state == GamePhase::Running);
        if running && seen.console_texts().len() > 1 {
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

/// Os modos das outras tarefas (L-07, L-09, L-12) ainda são recusados.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn modo_ainda_inexistente_e_recusado() {
    let dir = tempfile::tempdir().unwrap();
    let state = test_state(dir.path());
    let (sink, _) = sink();
    let request = TestRequest {
        mode: TestMode::Performance,
        ..TestRequest::default()
    };
    let error = run_test(&state, PackId::new(), request, sink)
        .await
        .unwrap_err();
    assert_eq!(
        error.code,
        ErrorCode::App(AppErrorCode::TestModeUnavailable)
    );
    assert_eq!(error.params["mode"], "performance");
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
}

/// As combinações do CA-T13-01 nesta tarefa (a matriz completa é da L-05).
const REAL_GAMES: [(&str, RealGame); 2] = [
    (
        "fabric-1.20.1",
        RealGame {
            minecraft: "1.20.1",
            loader: "fabric",
            version: "0.19.5",
        },
    ),
    (
        "forge-1.12.2",
        RealGame {
            minecraft: "1.12.2",
            loader: "forge",
            version: "14.23.5.2860",
        },
    ),
];

/// CA-T13-01 pelo Testar, com o Minecraft de verdade (roteiro no Windows; abre a janela do
/// jogo): o pack mínimo passa por todas as etapas do Testar (Java baixado pela `warden-java`,
/// jogo e loader pelo motor, cópia do pack) e chega ao menu principal (marcadores do S-R5-3:
/// atlas e som; no Forge legado, também o "successfully loaded"), sem sinal de falha; depois o
/// teste para o jogo. Uma combinação por vez:
/// `WARDEN_TEST_REAL_GAME=fabric-1.20.1 cargo test -p warden-app --lib jogo_real -- --ignored
/// --nocapture`. `WARDEN_TEST_REAL_ROOT` guarda os downloads entre rodadas.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "abre o Minecraft de verdade e baixa centenas de MB"]
#[allow(clippy::print_stdout)]
async fn jogo_real_chega_ao_menu_pelo_testar() {
    let Ok(wanted) = std::env::var("WARDEN_TEST_REAL_GAME") else {
        eprintln!("WARDEN_TEST_REAL_GAME ausente: nada a fazer");
        return;
    };
    let (_, game) = REAL_GAMES
        .iter()
        .find(|(name, _)| *name == wanted)
        .unwrap_or_else(|| panic!("combinação desconhecida: {wanted}"));
    let temp = tempfile::tempdir().unwrap();
    let root = std::env::var_os("WARDEN_TEST_REAL_ROOT")
        .map_or_else(|| temp.path().to_path_buf(), PathBuf::from);
    let state = Arc::new(test_state(&root));
    let pack_root = root.join("packs").join(&wanted);
    std::fs::create_dir_all(&pack_root).unwrap();
    let index = PackIndex::default().to_toml_string();
    std::fs::write(pack_root.join("index.toml"), &index).unwrap();
    let mut manifest = PackManifest::new("Pack mínimo", game.minecraft);
    manifest.version = "1.0.0".into();
    manifest.index.hash = sha256(index.as_bytes());
    manifest
        .versions
        .get_or_insert_with(Default::default)
        .insert(game.loader.into(), game.version.into());
    std::fs::write(pack_root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    let pack = state
        .packs
        .records()
        .into_iter()
        .find(|record| record.path == pack_root)
        .map_or_else(|| register(&state, &pack_root), |record| record.id);

    let started = Instant::now();
    let (task, seen) = spawn_test(&state, pack, TestRequest::default());
    let legacy_forge = game.loader == "forge" && game.minecraft == "1.12.2";
    let deadline = Instant::now() + Duration::from_secs(45 * 60);
    let mut loaded_at = None;
    loop {
        let texts = seen.console_texts();
        let has = |needle: &str| texts.iter().any(|text| text.contains(needle));
        let atlas = if game.minecraft == "1.12.2" {
            has("textures-atlas")
        } else {
            has("blocks.png-atlas")
        };
        let ready = atlas
            && has("Sound engine started")
            && (!legacy_forge || has("Forge Mod Loader has successfully loaded"));
        for signal in [
            "Error during pre-loading phase",
            "Missing or unsupported mandatory dependencies",
            "#@!@# Game crashed!",
            "Could not find or load main class",
        ] {
            assert!(!has(signal), "sinal de falha no console: {signal}");
        }
        if ready {
            loaded_at = Some(started.elapsed());
            break;
        }
        if task.is_finished() || Instant::now() > deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let loaded_at = loaded_at.unwrap_or_else(|| {
        let tail: Vec<String> = seen.console_texts().into_iter().rev().take(40).collect();
        panic!("o jogo não chegou ao menu; últimas linhas: {tail:#?}")
    });
    // Alguns segundos no menu, como quem olha.
    tokio::time::sleep(Duration::from_secs(5)).await;
    state.tests.stop(pack).await.unwrap();
    let summary = task.await.unwrap().unwrap();
    assert_eq!(summary.outcome, Outcome::StoppedByUser);
    println!(
        "{wanted}: menu principal em {:.0} s desde o clique (Java {}, {} MB; sessão {}); {} linhas no console",
        loaded_at.as_secs_f64(),
        summary.java_major,
        summary.memory_mb.unwrap_or_default(),
        summary.id,
        seen.console_texts().len()
    );
}
