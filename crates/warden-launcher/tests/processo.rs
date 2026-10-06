//! Processo do jogo com um Java de verdade e o jogo simulado
//! (`tests/fixtures/java/WardenFakeGame.java`): critério 1 (UUID offline igual ao do Java),
//! CA-T13-02 (parar encerra o jogo e os filhos em até 5 s), CA-T13-03 (acentos corretos),
//! CA-T13-07 (o Warden morrer não deixa `java` órfão), `@argfile` e classificação da saída.
//!
//! Usam o Java de `WARDEN_TEST_JAVA` ou `JAVA_HOME`; sem Java, avisam e passam (com
//! `WARDEN_REQUIRE_EXTERNALS=1`, a falta é falha). Os Javas 8 e 21 exigidos pelo CA-T13-03
//! ficam em `rede_acentos.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

mod comum;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use warden_launcher::command::{TargetOs, build};
use warden_launcher::log::LogSource;
use warden_launcher::outcome::Outcome;
use warden_launcher::process::{GameEvent, GameEvents, GameExit, GameProcess, SpawnOptions};
use warden_launcher::{
    GameSpec, InstalledGame, JavaRuntime, LaunchCommand, LaunchOptions, LoaderSpec, LoggingConfig,
    OfflineProfile, OfflineUuid, PlayerName,
};

/// Os 10 nomes do critério 1 e o UUID que o `java.util.UUID.nameUUIDFromBytes` deu para cada
/// um (gerados por `WardenFakeGame uuid` com o Temurin 17 em 2026-10-05; o teste roda o Java
/// de novo quando há um).
const JAVA_UUIDS: [(&str, &str); 10] = [
    ("WardenTest", "6c5aa2b1-c084-39d7-99a8-edc5b372e5f9"),
    ("Steve", "5627dd98-e6be-3c21-b8a8-e92344183641"),
    ("Notch", "b50ad385-829d-3141-a216-7e7d7539ba7f"),
    ("Alex", "36532b5e-c442-3dbb-a24c-c7e55d0f979a"),
    ("Dinnerbone", "4d258a81-2358-3084-8166-05b9faccad80"),
    ("abc", "3d5cec06-bd15-31fa-982f-5dac8c06f1c7"),
    ("___", "3aa10d45-6372-352e-b9af-2eba497ed912"),
    ("Jogador_123", "10e76a9a-5540-30cb-934e-b3bbd73a0ce5"),
    ("xX_Pro_Xx", "cf83a8cb-1b1f-3830-9c96-523c6a35b744"),
    ("1234567890123456", "619a03af-1e70-351d-b9fd-d83cd253f4d8"),
];

fn class_dir() -> PathBuf {
    comum::fixtures_dir().join("java")
}

/// O `javaw` ao lado do `java` (Windows), que é o programa do jogo de verdade.
fn launcher_for(java: &Path) -> PathBuf {
    let javaw = java.with_file_name("javaw.exe");
    if cfg!(windows) && javaw.is_file() {
        javaw
    } else {
        java.to_path_buf()
    }
}

/// O major do Java (lendo `release` da pasta do Java).
fn java_major(java: &Path) -> u32 {
    let home = java.parent().and_then(Path::parent).unwrap();
    let release = std::fs::read_to_string(home.join("release")).unwrap_or_default();
    let version = release
        .lines()
        .find_map(|line| line.strip_prefix("JAVA_VERSION="))
        .unwrap_or("\"17\"")
        .trim_matches('"');
    let mut parts = version.split(['.', '_']);
    let first: u32 = parts.next().unwrap().parse().unwrap();
    if first == 1 {
        parts.next().unwrap().parse().unwrap()
    } else {
        first
    }
}

/// Um "jogo" instalado que é o `WardenFakeGame`.
fn fake_game(mode_args: &[&str], release_time: &str) -> InstalledGame {
    InstalledGame {
        spec: GameSpec::new(
            "1.20.1",
            LoaderSpec::Fabric {
                version: "0.19.5".into(),
            },
        )
        .unwrap(),
        version_id: "simulado".into(),
        hierarchy: vec!["simulado".into()],
        release_time: release_time.into(),
        main_class: "WardenFakeGame".into(),
        jvm_args: vec!["-cp".into(), class_dir().display().to_string()],
        game_args: mode_args.iter().map(|arg| (*arg).to_owned()).collect(),
        game_dir_placeholder: "<nenhum>".into(),
        logging: LoggingConfig::None,
    }
}

fn options(java: &Path, game_dir: &Path) -> LaunchOptions {
    LaunchOptions {
        game_dir: game_dir.to_path_buf(),
        state_dir: game_dir.join("state"),
        java: JavaRuntime {
            java: java.to_path_buf(),
            launcher: launcher_for(java),
            major: java_major(java),
        },
        memory_mb: 256,
        extra_jvm_args: Vec::new(),
        system_props: Vec::new(),
        quick_play: None,
        quick_play_path: None,
        player: OfflineProfile::from_name("WardenTest").unwrap(),
    }
}

fn command(java: &Path, game_dir: &Path, mode_args: &[&str]) -> LaunchCommand {
    build(
        &fake_game(mode_args, "2023-06-12T13:25:51+00:00"),
        &options(java, game_dir),
        TargetOs::current(),
    )
    .unwrap()
}

/// Lê os eventos até a saída, com tempo-limite.
async fn collect(events: &mut GameEvents, limit: Duration) -> (Vec<String>, Option<GameExit>) {
    let mut lines = Vec::new();
    let deadline = Instant::now() + limit;
    while let Ok(Some(event)) = tokio::time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        events.recv(),
    )
    .await
    {
        match event {
            GameEvent::Line(line) => lines.push(line.text),
            GameEvent::Exited(exit) => return (lines, Some(exit)),
        }
    }
    (lines, None)
}

/// Espera até ver `count` linhas que começam com `prefix`.
async fn wait_for_lines(events: &mut GameEvents, prefix: &str, count: usize) -> Vec<String> {
    let mut found = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    while found.len() < count {
        let event = tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            events.recv(),
        )
        .await
        .expect("tempo esgotado esperando o jogo simulado")
        .expect("o jogo simulado fechou antes da hora");
        match event {
            GameEvent::Line(line) if line.text.starts_with(prefix) => found.push(line.text),
            GameEvent::Line(_) => {}
            GameEvent::Exited(exit) => panic!("o jogo simulado saiu antes da hora: {exit:?}"),
        }
    }
    found
}

/// Espera `count` linhas `filho <pid>` e devolve os pids. Os filhos também escrevem
/// `filho vivo` a cada 200 ms: com a máquina carregada, o primeiro filho já bate antes de o
/// último escrever o pid, então só contam as linhas cujo resto é um número.
async fn wait_for_child_pids(events: &mut GameEvents, count: usize) -> Vec<u32> {
    let mut pids = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    while pids.len() < count {
        let event = tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            events.recv(),
        )
        .await
        .expect("tempo esgotado esperando os filhos do jogo simulado")
        .expect("o jogo simulado fechou antes da hora");
        match event {
            GameEvent::Line(line) => {
                if let Some(pid) = line
                    .text
                    .strip_prefix("filho ")
                    .and_then(|pid| pid.trim().parse::<u32>().ok())
                {
                    pids.push(pid);
                }
            }
            GameEvent::Exited(exit) => panic!("o jogo simulado saiu antes da hora: {exit:?}"),
        }
    }
    pids
}

/// Se um processo com esse pid ainda existe.
fn process_alive(pid: u32) -> bool {
    if cfg!(windows) {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
    } else {
        Path::new(&format!("/proc/{pid}")).exists()
    }
}

#[tokio::test]
async fn uuid_offline_igual_ao_do_java_para_10_nomes() {
    for (name, expected) in JAVA_UUIDS {
        let uuid = OfflineUuid::for_name(&PlayerName::new(name).unwrap());
        assert_eq!(uuid.hyphenated(), expected, "{name}");
    }
    let Some(java) = comum::test_java() else {
        return;
    };
    let mut args = vec![
        "-cp".to_owned(),
        class_dir().display().to_string(),
        "WardenFakeGame".to_owned(),
        "uuid".to_owned(),
    ];
    args.extend(JAVA_UUIDS.iter().map(|(name, _)| (*name).to_owned()));
    let output = std::process::Command::new(&java)
        .args(&args)
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let from_java: Vec<(String, String)> = text
        .lines()
        .map(|line| {
            let (name, uuid) = line.split_once('=').unwrap();
            (name.to_owned(), uuid.to_owned())
        })
        .collect();
    assert_eq!(from_java.len(), 10);
    for (name, uuid) in from_java {
        let ours = OfflineUuid::for_name(&PlayerName::new(&name).unwrap());
        assert_eq!(ours.hyphenated(), uuid, "{name}");
    }
}

#[tokio::test]
async fn acentos_chegam_certos_com_o_java_local() {
    let Some(java) = comum::test_java() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("sessao").join("output.log");
    let (game, mut events) = GameProcess::spawn(
        &command(&java, dir.path(), &["acentos"]),
        SpawnOptions {
            output_log: Some(log.clone()),
            stdin: false,
        },
    )
    .await
    .unwrap();
    let (lines, exit) = collect(&mut events, Duration::from_secs(30)).await;
    let exit = exit.expect("o jogo simulado não saiu");
    assert_eq!(exit.outcome, Outcome::ClosedNormally);
    assert_eq!(exit.exit_code, Some(0));
    assert_accents(&lines);
    let saved = std::fs::read_to_string(&log).unwrap();
    assert!(saved.contains("Ação, coração, pé, avô, vovó, pão, Über, ñ"));
    assert!(!saved.contains("Ã"));
    assert_eq!(game.wait().await.unwrap().outcome, Outcome::ClosedNormally);
}

/// As linhas do modo `acentos` chegaram sem `Ã`.
pub fn assert_accents(lines: &[String]) {
    let joined = lines.join("\n");
    assert!(
        joined.contains("Ação, coração, pé, avô, vovó, pão, Über, ñ"),
        "{joined}"
    );
    assert!(
        joined.contains("Configuração inválida: ç ã õ é"),
        "{joined}"
    );
    assert!(
        joined.contains("Exceção: não foi possível ler o arquivo 'café.txt'"),
        "{joined}"
    );
    assert!(!joined.contains('Ã'), "{joined}");
    assert!(!joined.contains('\u{FFFD}'), "{joined}");
}

#[tokio::test]
async fn parar_jogo_encerra_o_jogo_e_os_filhos_em_ate_5_segundos() {
    let Some(java) = comum::test_java() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let (game, mut events) = GameProcess::spawn(
        &command(&java, dir.path(), &["filhos", "3"]),
        SpawnOptions::default(),
    )
    .await
    .unwrap();
    let children = wait_for_child_pids(&mut events, 3).await;
    let handle = game.handle();
    if let Some(active) = handle.active_processes() {
        // O jogo, os 3 filhos e o `conhost.exe` de cada `java` de console.
        assert!(active >= 4, "o jogo e os 3 filhos no Job Object: {active}");
    }
    let started = Instant::now();
    handle.stop().await;
    let (_, exit) = collect(&mut events, Duration::from_secs(5)).await;
    let took = started.elapsed();
    let exit = exit.expect("o jogo não saiu");
    assert!(took <= Duration::from_secs(5), "parar levou {took:?}");
    assert_eq!(exit.outcome, Outcome::StoppedByUser);
    assert!(exit.stop_requested);
    // Os pipes só fecham quando todos os processos que os herdaram morreram; mesmo assim,
    // confere cada filho pelo pid.
    if let Some(active) = handle.active_processes() {
        assert_eq!(active, 0);
    }
    for pid in children {
        assert!(!process_alive(pid), "filho {pid} ficou vivo");
    }
    assert!(!process_alive(game.pid()));
}

#[tokio::test]
async fn jogo_travado_e_encerrado() {
    let Some(java) = comum::test_java() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let (game, mut events) = GameProcess::spawn(
        &command(&java, dir.path(), &["travar"]),
        SpawnOptions::default(),
    )
    .await
    .unwrap();
    wait_for_lines(&mut events, "pronto", 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let started = Instant::now();
    game.handle().stop().await;
    let exit = game.wait().await.unwrap();
    assert!(started.elapsed() <= Duration::from_secs(5));
    assert_eq!(exit.outcome, Outcome::StoppedByUser);
}

#[tokio::test]
async fn saida_classificada() {
    let Some(java) = comum::test_java() else {
        return;
    };
    for (args, expected, crash) in [
        (&["sair", "0"][..], Outcome::ClosedNormally, false),
        (&["sair", "3"][..], Outcome::Crashed, false),
        (&["crash"][..], Outcome::Crashed, true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (game, _events) =
            GameProcess::spawn(&command(&java, dir.path(), args), SpawnOptions::default())
                .await
                .unwrap();
        let exit = game.wait().await.unwrap();
        assert_eq!(exit.outcome, expected, "{args:?}");
        assert!(!exit.stop_requested);
        assert_eq!(
            exit.artifacts.crash_reports.len(),
            usize::from(crash),
            "{args:?}"
        );
    }
}

#[tokio::test]
async fn programa_inexistente_e_launch_failed() {
    let dir = tempfile::tempdir().unwrap();
    let command = LaunchCommand {
        program: dir.path().join("nao-existe.exe"),
        args: Vec::new(),
        cwd: dir.path().to_path_buf(),
        env: Vec::new(),
        argfile: None,
    };
    let error = GameProcess::spawn(&command, SpawnOptions::default())
        .await
        .unwrap_err();
    assert!(
        matches!(error, warden_launcher::Error::LaunchFailed { .. }),
        "{error}"
    );
}

#[tokio::test]
async fn argfile_chega_igual_ao_java() {
    let Some(java) = comum::test_java() else {
        return;
    };
    if java_major(&java) < 9 {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let tricky = [
        "com espaço",
        "C:\\caminho\\com\\barras",
        "aspas \"dentro\"",
        "fim com barra\\",
        "",
    ];
    let filler = "x".repeat(31_000);
    let mut mode = vec!["args"];
    mode.extend(tricky);
    mode.push(&filler);
    let mut game = fake_game(&mode, "2023-06-12T13:25:51+00:00");
    game.jvm_args
        .insert(0, "-Dwarden.teste=valor com espaço".into());
    let command = build(&game, &options(&java, dir.path()), TargetOs::current()).unwrap();
    assert!(
        command.argfile.is_some(),
        "a linha deveria ir para o @argfile"
    );
    let (game_process, mut events) = GameProcess::spawn(&command, SpawnOptions::default())
        .await
        .unwrap();
    let (lines, exit) = collect(&mut events, Duration::from_secs(30)).await;
    assert_eq!(exit.unwrap().exit_code, Some(0), "{lines:?}");
    let received: Vec<&str> = lines
        .iter()
        .filter_map(|line| {
            line.strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
        })
        .collect();
    let mut expected: Vec<&str> = tricky.to_vec();
    expected.push(&filler);
    assert_eq!(received, expected);
    assert!(lines.contains(&"propriedade=valor com espaço".to_owned()));
    drop(game_process);
}

/// Modo filho do CA-T13-07: abre o jogo simulado com filhos, repassa as linhas e espera ser
/// morto pelo teste (simula o Warden fechando no meio do jogo).
#[tokio::test]
#[ignore = "processo filho do teste fechar_o_warden_nao_deixa_java_orfao"]
async fn filho_abre_o_jogo_e_espera_morrer() {
    let Some(dir) = std::env::var_os("WARDEN_TESTE_FILHO_DIR") else {
        return;
    };
    let java = comum::test_java().unwrap();
    let (game, mut events) = GameProcess::spawn(
        &command(&java, Path::new(&dir), &["filhos", "2"]),
        SpawnOptions::default(),
    )
    .await
    .unwrap();
    println!("jogo {}", game.pid());
    while let Some(GameEvent::Line(line)) = events.recv().await {
        if line.source == LogSource::Stdout && !line.text.ends_with("vivo") {
            println!("{}", line.text);
        }
    }
}

#[tokio::test]
async fn fechar_o_warden_nao_deixa_java_orfao() {
    if comum::test_java().is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut helper = tokio::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "filho_abre_o_jogo_e_espera_morrer",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("WARDEN_TESTE_FILHO_DIR", dir.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let stdout = helper.stdout.take().unwrap();
    let mut reader = tokio::io::AsyncBufReadExt::lines(tokio::io::BufReader::new(stdout));
    let mut pids = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    while pids.len() < 3 {
        let line = tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            reader.next_line(),
        )
        .await
        .expect("tempo esgotado esperando o processo filho")
        .unwrap()
        .expect("o processo filho fechou antes da hora");
        for prefix in ["jogo ", "filho "] {
            if let Some(pid) = line
                .strip_prefix(prefix)
                .and_then(|pid| pid.trim().parse::<u32>().ok())
            {
                pids.push(pid);
            }
        }
    }
    for pid in &pids {
        assert!(process_alive(*pid), "{pid} deveria estar vivo antes");
    }
    // O "Warden" morre sem chance de limpar nada.
    helper.start_kill().unwrap();
    helper.wait().await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while pids.iter().any(|pid| process_alive(*pid)) && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if cfg!(windows) {
        for pid in &pids {
            assert!(!process_alive(*pid), "java {pid} ficou órfão");
        }
    } else {
        // No Linux não há Job Object: o Warden encerra o grupo ao fechar com confirmação
        // (`GameHandle::stop`); morte abrupta do Warden não é coberta (ADR-0057).
        for pid in &pids {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status();
        }
    }
}

/// CA-T13-03 com os Javas que o critério pede: Temurin 8 (sem argumentos de codificação; a
/// saída sai em Windows-1252) e Temurin 21 (`-Dstdout.encoding=UTF-8`), baixados pela
/// `warden-java`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "rede"]
async fn rede_acentos_corretos_no_java_8_e_no_java_21() {
    let (root, _temp) = comum::data_root();
    let java_service = comum::java_service(&root);
    let cancel = warden_core::CancellationToken::new();
    for major in [8, 21] {
        let runtime = java_service
            .ensure(
                warden_java::JavaRequirement {
                    major,
                    max_update: None,
                },
                &warden_core::NoProgress,
                &cancel,
            )
            .await
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let (_game, mut events) = GameProcess::spawn(
            &command(&runtime.java, dir.path(), &["acentos"]),
            SpawnOptions::default(),
        )
        .await
        .unwrap();
        let (lines, exit) = collect(&mut events, Duration::from_secs(30)).await;
        assert_eq!(exit.unwrap().exit_code, Some(0), "Java {major}: {lines:?}");
        eprintln!("Java {major}: {}", lines.last().unwrap());
        assert_accents(&lines);
    }
}
