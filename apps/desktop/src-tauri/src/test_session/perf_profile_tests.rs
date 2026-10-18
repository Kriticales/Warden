//! "Testar com perfil de desempenho" pelo Testar inteiro, com o jogo simulado (ROADMAP L-12;
//! CA-T13-13): a abertura com o registro detalhado, o `debug.log` real do Forge 1.20.1 e um
//! perfil real do spark salvos na instância durante o teste, e a sessão gravada com o tempo por
//! mod, o resumo do spark, a cópia do arquivo e nada disso em "O que mudou durante o teste".
//!
//! Usam o Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`; sem Java, avisam e passam.

#![allow(clippy::print_stderr)] // aviso de teste pulado
// Os números do resumo já saem arredondados a uma casa.
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;
use std::sync::Mutex as StdMutex;

use warden_launcher::session::SESSION_FILE;
use warden_packwiz::PackManifest;
use warden_perf::mod_timing::{TRACE_PROPERTY, TimingSource, TimingStatus};
use warden_perf::spark::SparkKind;

use super::tests::{class_dir, java, java_major, register, spawn_test, wait_running, write_pack};
use super::*;
use crate::commands::perf::{PerfReport, PerfSide, SESSION_FIELD, SPARK_DIR, report_of};
use crate::state::tests::test_state;

/// Os arquivos reais da `warden-perf` (origem em `crates/warden-perf/tests/FIXTURES.md`).
fn golden(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/warden-perf/tests")
            .join(name),
    )
    .unwrap()
}

/// Os argumentos da JVM da última abertura.
type JvmArgs = Arc<StdMutex<Vec<String>>>;

/// Estado com o jogo simulado que guarda os argumentos da JVM de cada abertura.
fn state_capturing_args(root: &Path) -> Option<(Arc<AppState>, JvmArgs)> {
    let Some(java) = java() else {
        eprintln!("sem Java (WARDEN_TEST_JAVA ou JAVA_HOME): teste do jogo simulado pulado");
        return None;
    };
    let args = Arc::new(StdMutex::new(Vec::new()));
    let seen = Arc::clone(&args);
    let mut state = test_state(root);
    let engine = PortableMcEngine::new(EngineDirs::under(&root.join("shared")), None);
    let major = java_major(&java);
    state.tests = Arc::new(
        TestSessions::with_engine(
            Arc::new(engine),
            Some(FakeGame::new(class_dir(), java, major)),
        )
        .with_launch_tweak(LaunchTweak {
            options: Arc::new(move |options: &mut warden_launcher::LaunchOptions| {
                seen.lock().unwrap().clone_from(&options.extra_jvm_args);
            }),
            env: Vec::new(),
            refuse_command: false,
        }),
    );
    Some((Arc::new(state), args))
}

/// O pack de teste com o loader dado no `pack.toml`.
fn use_loader(root: &Path, loader: &str, version: &str) {
    let text = std::fs::read_to_string(root.join("pack.toml")).unwrap();
    let mut manifest = PackManifest::parse(&text).unwrap().value;
    manifest
        .versions
        .get_or_insert_with(BTreeMap::new)
        .insert(loader.into(), version.into());
    std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
}

/// O que um teste deixou.
struct Ran {
    /// A pasta temporária (some quando sai de escopo).
    dir: tempfile::TempDir,
    record: SessionRecord,
    session: PathBuf,
    args: Vec<String>,
}

/// Abre o teste no modo dado, grava os arquivos na instância com o jogo aberto, para o jogo e
/// devolve o registro da sessão e os argumentos da JVM.
async fn run_with_files(
    mode: TestMode,
    loader: (&str, &str),
    files: &[(&str, Vec<u8>)],
) -> Option<Ran> {
    let dir = tempfile::tempdir().unwrap();
    let (state, args) = state_capturing_args(dir.path())?;
    let root = dir.path().join("pack");
    write_pack(&root, &[], "filhos 1");
    use_loader(&root, loader.0, loader.1);
    let pack = register(&state, &root);
    let (task, seen) = spawn_test(
        &state,
        pack,
        TestRequest {
            mode,
            ..TestRequest::default()
        },
    );
    wait_running(&state, &seen).await;
    let game_dir = InstanceDirs::for_pack(&state.paths, pack).game_dir;
    for (path, bytes) in files {
        let file = game_dir.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, bytes).unwrap();
    }
    state.tests.stop(pack).await.unwrap();
    let summary = task.await.unwrap().unwrap();
    let session = InstanceDirs::for_pack(&state.paths, pack)
        .state_dir
        .join("sessions")
        .join(&summary.id);
    let record: SessionRecord =
        serde_json::from_slice(&std::fs::read(session.join(SESSION_FILE)).unwrap()).unwrap();
    let args = args.lock().unwrap().clone();
    Some(Ran {
        dir,
        record,
        session,
        args,
    })
}

/// CA-T13-13 no Forge 1.20.1: o registro detalhado liga só neste teste, o tempo por mod sai do
/// `debug.log`, o perfil do spark vira o resumo (com a cópia na sessão) e nenhum dos dois
/// aparece em "O que mudou durante o teste" (só a config de verdade).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_13_forge_1_20_1_tempo_por_mod_e_spark() {
    let files = [
        (
            "logs/debug.log",
            golden("logs/forge-1.20.1-servidor-debug.log"),
        ),
        (
            "config/spark/profile-2026-10-18_00.42.48.sparkprofile",
            golden("spark/forge-1.20.1-servidor.sparkprofile"),
        ),
        (
            "config/spark/heapsummary-2026-10-18_00.42.53.sparkheap",
            golden("spark/forge-1.20.1-servidor.sparkheap"),
        ),
        ("config/spark/tmp/x", b"temporario".to_vec()),
        ("config/meu-mod.toml", b"ligado = true\n".to_vec()),
    ];
    let Some(Ran {
        dir: _dir,
        record,
        session,
        args,
    }) = run_with_files(TestMode::Performance, ("forge", "47.4.10"), &files).await
    else {
        return;
    };
    assert!(args.iter().any(|arg| arg == TRACE_PROPERTY), "{args:?}");
    assert_eq!(record.extra["mode"], "performance");
    let report: PerfReport = report_of(&record).unwrap();
    assert_eq!(report.side, PerfSide::Client);
    let load = report.load.as_ref().unwrap();
    assert_eq!(load.status, TimingStatus::Measured);
    assert_eq!(load.source, Some(TimingSource::Trace));
    assert_eq!(load.loader, "forge");
    assert_eq!(load.log.as_deref(), Some("logs/debug.log"));
    let ranking: Vec<(&str, u32)> = load
        .mods
        .iter()
        .map(|row| (row.name.as_str(), row.ms))
        .collect();
    assert_eq!(ranking[0], ("jei", 288));
    assert_eq!(load.game_and_loader_ms, 350);

    assert_eq!(report.spark.len(), 2);
    // Gravados no mesmo instante (no NTFS, com a mesma data): a ordem do empate é a do nome.
    let profile = report
        .spark
        .iter()
        .find(|spark| spark.kind == SparkKind::Profile)
        .unwrap();
    assert!(profile.readable);
    assert_eq!(profile.duration_ms, Some(40_234));
    assert_eq!(profile.ticks.as_ref().unwrap().mspt_median, 1.6);
    assert_eq!(profile.top[0].name, "spark");
    assert!(
        report
            .spark
            .iter()
            .any(|spark| spark.kind == SparkKind::Heap)
    );
    assert!(
        session
            .join(SPARK_DIR)
            .join("profile-2026-10-18_00.42.48.sparkprofile")
            .is_file()
    );
    // "O que mudou durante o teste": só a config; os arquivos do spark nunca.
    assert_eq!(record.extra["capture"]["changes"], 1, "{:?}", record.extra);
}

/// CA-T13-13 no Forge 1.12.2: o `Bar Step` do `debug.log`, sem argumento a mais na JVM.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ca_t13_13_forge_1_12_2_bar_step_sem_registro_detalhado() {
    let files = [(
        "logs/debug.log",
        golden("logs/forge-1.12.2-servidor-debug.log"),
    )];
    // O pack de teste é 1.20.1; o jogo simulado ignora a versão, o leitor usa a do pack.
    let dir = tempfile::tempdir().unwrap();
    let Some((state, args)) = state_capturing_args(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[], "filhos 1");
    let text = std::fs::read_to_string(root.join("pack.toml")).unwrap();
    let mut manifest = PackManifest::parse(&text).unwrap().value;
    manifest.versions = Some(BTreeMap::from([
        ("minecraft".into(), "1.12.2".into()),
        ("forge".into(), "14.23.5.2860".into()),
    ]));
    std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    let pack = register(&state, &root);
    let (task, seen) = spawn_test(
        &state,
        pack,
        TestRequest {
            mode: TestMode::Performance,
            // O Java do jogo simulado não é o 8 que o 1.12.2 pede.
            ignore_pack_errors: true,
            ..TestRequest::default()
        },
    );
    wait_running(&state, &seen).await;
    let game_dir = InstanceDirs::for_pack(&state.paths, pack).game_dir;
    for (path, bytes) in &files {
        let file = game_dir.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, bytes).unwrap();
    }
    state.tests.stop(pack).await.unwrap();
    let summary = task.await.unwrap().unwrap();
    let record = session_dir(&state.paths, pack, &summary.id)
        .unwrap()
        .read_record()
        .unwrap();
    assert!(!args.lock().unwrap().iter().any(|arg| arg == TRACE_PROPERTY));
    let report = report_of(&record).unwrap();
    let load = report.load.unwrap();
    assert_eq!(load.source, Some(TimingSource::BarStep));
    let ranking: Vec<(&str, u32)> = load
        .mods
        .iter()
        .map(|row| (row.name.as_str(), row.ms))
        .collect();
    assert_eq!(
        ranking,
        [
            ("Just Enough Items", 138),
            ("AppleSkin", 51),
            ("Clumps", 31)
        ]
    );
    assert!(report.spark.is_empty());
}

/// Fabric: o teste roda, o resultado diz que o loader não registra o tempo de cada mod, e o
/// teste normal não grava o perfil nem liga o registro detalhado.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fabric_sem_tempo_por_mod_e_teste_normal_sem_perfil() {
    let Some(Ran { record, args, .. }) = run_with_files(
        TestMode::Performance,
        ("fabric", "0.16.10"),
        &[("logs/debug.log", b"[x] nada\n".to_vec())],
    )
    .await
    else {
        return;
    };
    assert!(!args.iter().any(|arg| arg == TRACE_PROPERTY), "{args:?}");
    let load = report_of(&record).unwrap().load.unwrap();
    assert_eq!(load.status, TimingStatus::NotRecorded);
    assert_eq!(load.loader, "fabric");

    let Some(Ran { record, args, .. }) = run_with_files(
        TestMode::Normal,
        ("forge", "47.4.10"),
        &[(
            "config/spark/profile-1.sparkprofile",
            golden("spark/forge-1.20.1-servidor.sparkprofile"),
        )],
    )
    .await
    else {
        return;
    };
    assert!(!args.iter().any(|arg| arg == TRACE_PROPERTY));
    assert!(!record.extra.contains_key(SESSION_FIELD));
}

/// Um jar do spark para Forge (só o descritor; o servidor simulado não carrega mods).
fn spark_jar() -> Vec<u8> {
    use std::io::Write as _;
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        writer
            .start_file(
                "META-INF/mods.toml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer
            .write_all(
                b"modLoader=\"javafml\"\nloaderVersion=\"[47,)\"\nlicense=\"GPL-3.0\"\n[[mods]]\nmodId=\"spark\"\nversion=\"1.10.53\"\n",
            )
            .unwrap();
        writer.finish().unwrap();
    }
    buffer.into_inner()
}

/// O estado com o jogo e o servidor simulados (`None` sem Java), como nos testes da L-09.
fn server_state(root: &Path) -> Option<Arc<AppState>> {
    let state = super::tests::fake_state(root)?;
    let mut state = Arc::try_unwrap(state).ok()?;
    let tests = Arc::try_unwrap(state.tests).ok()?.with_fake_server(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/warden-server/tests/fixtures/java"),
    );
    state.tests = Arc::new(tests);
    let start = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .ok()?
        .local_addr()
        .ok()?
        .port();
    state.servers.set_first_port(start);
    state
        .settings
        .set_eula_accepted_at(Some("2026-10-18T12:00:00Z".into()))
        .unwrap();
    Some(Arc::new(state))
}

/// Espera uma linha no console (o servidor simulado ecoa cada comando recebido).
async fn wait_console(seen: &super::tests::Seen, text: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while !seen.console_texts().iter().any(|line| line.contains(text)) {
        assert!(
            std::time::Instant::now() < deadline,
            "\"{text}\" não apareceu no console"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// "Testar como servidor" com o perfil de desempenho: o Warden liga o spark do servidor pelo
/// console quando o jogo abre; no fim, sem perfil salvo ainda, desliga (o servidor simulado
/// salva o arquivo ao receber o comando, como o spark) e o resumo sai do perfil do servidor.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn servidor_local_liga_e_desliga_o_spark_pelo_console() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = server_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(
        &root,
        &[("mods/spark-1.10.53-forge.jar", spark_jar().as_slice())],
        "conectar",
    );
    use_loader(&root, "forge", "47.4.10");
    let pack = register(&state, &root);
    let (task, seen) = spawn_test(
        &state,
        pack,
        TestRequest {
            mode: TestMode::Server,
            performance: true,
            ignore_pack_errors: true,
            ..TestRequest::default()
        },
    );
    wait_console(
        &seen,
        "Comando recebido: spark profiler start --timeout 60 --save-to-file",
    )
    .await;
    // O "spark" do servidor simulado: salva o perfil quando recebe o comando de desligar.
    let spark_dir = crate::server_session::server_dirs(&state.paths, pack)
        .game_dir
        .join("config/spark");
    let saver = {
        let seen = Arc::clone(&seen);
        tokio::spawn(async move {
            wait_console(
                &seen,
                "Comando recebido: spark profiler stop --save-to-file",
            )
            .await;
            std::fs::create_dir_all(&spark_dir).unwrap();
            std::fs::write(
                spark_dir.join("profile-servidor.sparkprofile"),
                golden("spark/neoforge-1.21.1-servidor.sparkprofile"),
            )
            .unwrap();
        })
    };
    state.tests.stop(pack).await.unwrap();
    let summary = tokio::time::timeout(Duration::from_secs(90), task)
        .await
        .expect("o teste não terminou")
        .unwrap()
        .unwrap();
    saver.await.unwrap();
    let record = session_dir(&state.paths, pack, &summary.id)
        .unwrap()
        .read_record()
        .unwrap();
    let report = report_of(&record).unwrap();
    assert_eq!(report.side, PerfSide::Server);
    assert_eq!(
        report.server_spark,
        Some(crate::commands::perf::ServerSpark::Started)
    );
    assert!(report.load.is_none());
    assert_eq!(report.spark.len(), 1);
    assert!(report.spark[0].readable);
    assert_eq!(report.spark[0].platform.as_ref().unwrap().name, "NeoForge");
}

/// Sem o spark entre os mods do servidor, nada é mandado e o resultado diz isso.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn servidor_local_sem_spark_nao_manda_comando() {
    let dir = tempfile::tempdir().unwrap();
    let Some(state) = server_state(dir.path()) else {
        return;
    };
    let root = dir.path().join("pack");
    write_pack(&root, &[], "conectar");
    let pack = register(&state, &root);
    let (task, seen) = spawn_test(
        &state,
        pack,
        TestRequest {
            mode: TestMode::Server,
            performance: true,
            ..TestRequest::default()
        },
    );
    wait_console(&seen, "jogo conectado").await;
    state.tests.stop(pack).await.unwrap();
    let summary = tokio::time::timeout(Duration::from_secs(90), task)
        .await
        .expect("o teste não terminou")
        .unwrap()
        .unwrap();
    assert!(
        !seen
            .console_texts()
            .iter()
            .any(|line| line.contains("spark profiler"))
    );
    let record = session_dir(&state.paths, pack, &summary.id)
        .unwrap()
        .read_record()
        .unwrap();
    let report = report_of(&record).unwrap();
    assert_eq!(
        report.server_spark,
        Some(crate::commands::perf::ServerSpark::NotInstalled)
    );
    assert!(report.spark.is_empty());
}
