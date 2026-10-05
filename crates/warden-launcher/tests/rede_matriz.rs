//! Instalação real das 20 combinações da matriz L-05 com o adaptador do portablemc e o Java
//! da `warden-java` (Temurin), sem abrir o jogo.
//!
//! Pesado (3 a 4 GB de downloads na primeira vez): só roda com `WARDEN_LAUNCHER_MATRIZ=1`.
//! Com `WARDEN_LAUNCHER_REGRAVAR=1`, grava os planos normalizados em
//! `tests/fixtures/matriz/<combinação>.json` (as entradas dos golden tests,
//! `tests/golden.rs`); sem ela, compara com os gravados. `WARDEN_LAUNCHER_DADOS` reaproveita a
//! pasta de dados entre execuções. Rode com `cargo test` (o nextest encerra testes de mais de
//! 3 min): `cargo test -p warden-launcher --test rede_matriz -- --ignored --nocapture`.

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

use std::sync::Arc;
use std::time::Instant;

use warden_core::{CancellationToken, NoProgress};
use warden_java::JavaChoiceRequest;
use warden_launcher::{
    CatalogForgeResolver, EngineDirs, JavaRuntime, LauncherEngine, PortableMcEngine,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "rede"]
async fn rede_matriz_instala_as_20_combinacoes() {
    if !comum::env_on("WARDEN_LAUNCHER_MATRIZ") {
        eprintln!("AVISO: defina WARDEN_LAUNCHER_MATRIZ=1 para instalar a matriz (3 a 4 GB).");
        return;
    }
    let only = std::env::var("WARDEN_LAUNCHER_SO").unwrap_or_default();
    let rewrite = comum::env_on("WARDEN_LAUNCHER_REGRAVAR");
    let (root, _temp) = comum::data_root();
    let shared = root.join("shared");
    let java = comum::java_service(&root);
    let cache =
        warden_catalog::CatalogCache::open(&root.join("cache").join("metadata.sqlite")).unwrap();
    let catalog = Arc::new(warden_catalog::Catalog::new(comum::http(), cache).unwrap());
    let engine = PortableMcEngine::new(
        EngineDirs::under(&shared),
        Some(Arc::new(CatalogForgeResolver::new(catalog))),
    );
    let cancel = CancellationToken::new();
    let out_dir = comum::fixtures_dir().join("matriz");
    std::fs::create_dir_all(&out_dir).unwrap();

    let mut failures = Vec::new();
    for combo in comum::MATRIX {
        if !only.is_empty() && !only.split(',').any(|name| name == combo.name) {
            continue;
        }
        let choice = java
            .choose(&JavaChoiceRequest::automatic(
                combo.minecraft,
                combo.loader_kind(),
                (!combo.version.is_empty()).then_some(combo.version),
            ))
            .unwrap();
        let runtime = java
            .ensure_choice(&choice, &NoProgress, &cancel)
            .await
            .unwrap();
        let started = Instant::now();
        let result = engine
            .install(
                &combo.spec(),
                &JavaRuntime::from_installed(&runtime),
                &NoProgress,
                &cancel,
            )
            .await;
        let game = match result {
            Ok(game) => game,
            Err(error) => {
                eprintln!(
                    "FALHOU {}: {}",
                    combo.name,
                    warden_core::error_chain(&error)
                );
                failures.push(combo.name);
                continue;
            }
        };
        eprintln!(
            "ok {} em {:.1} s (Java {}, log {:?})",
            combo.name,
            started.elapsed().as_secs_f64(),
            runtime.version.major,
            game.logging
        );
        let normalized = comum::normalize(&game, &shared);
        let path = out_dir.join(format!("{}.json", combo.name));
        if rewrite {
            let mut json = serde_json::to_string_pretty(&normalized).unwrap();
            json.push('\n');
            std::fs::write(&path, json).unwrap();
        } else {
            let stored: warden_launcher::InstalledGame =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            if stored != normalized {
                eprintln!(
                    "DIFERENTE {}: rode com WARDEN_LAUNCHER_REGRAVAR=1 e revise",
                    combo.name
                );
                failures.push(combo.name);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "combinações com problema: {failures:?}"
    );
}

/// Fumaça de ponta a ponta com o jogo de verdade (Fabric 1.20.1): instala, monta a linha,
/// abre, espera `Sound engine started`, para pelo Job Object e grava a sessão. Abre a janela
/// do jogo na tela: só com `WARDEN_LAUNCHER_JOGO=1`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "rede"]
async fn rede_abre_e_fecha_o_jogo_de_verdade() {
    use std::time::Duration;
    use warden_launcher::process::{GameEvent, GameProcess, SpawnOptions};
    use warden_launcher::session::{SessionDir, SessionRecord};

    if !comum::env_on("WARDEN_LAUNCHER_JOGO") {
        eprintln!("AVISO: defina WARDEN_LAUNCHER_JOGO=1 para abrir o jogo de verdade.");
        return;
    }
    let (root, _temp) = comum::data_root();
    let java = comum::java_service(&root);
    let engine = PortableMcEngine::new(EngineDirs::under(&root.join("shared")), None);
    let cancel = CancellationToken::new();
    let combo = comum::MATRIX
        .iter()
        .find(|combo| combo.name == "fabric-1.20.1")
        .unwrap();
    let choice = java
        .choose(&JavaChoiceRequest::automatic(
            combo.minecraft,
            combo.loader_kind(),
            Some(combo.version),
        ))
        .unwrap();
    let runtime = java
        .ensure_choice(&choice, &NoProgress, &cancel)
        .await
        .unwrap();
    let java_runtime = JavaRuntime::from_installed(&runtime);
    let game = engine
        .install(&combo.spec(), &java_runtime, &NoProgress, &cancel)
        .await
        .unwrap();
    let instance = root.join("instances").join("fumaca");
    let game_dir = instance.join("minecraft");
    std::fs::create_dir_all(&game_dir).unwrap();
    let options = warden_launcher::LaunchOptions {
        game_dir: game_dir.clone(),
        state_dir: instance.join("state"),
        java: java_runtime,
        memory_mb: 2048,
        extra_jvm_args: Vec::new(),
        system_props: vec![("fabric.noGui".into(), "true".into())],
        quick_play: None,
        quick_play_path: None,
        player: warden_launcher::OfflineProfile::from_name("WardenTest").unwrap(),
    };
    let mut command = engine.command(&game, &options).unwrap();
    command.env.push(("ALSOFT_DRIVERS".into(), "null".into()));
    let session = SessionDir::create(
        &instance.join("state").join("sessions"),
        std::time::SystemTime::now(),
    )
    .unwrap();
    let (process, mut events) = GameProcess::spawn(
        &command,
        SpawnOptions {
            output_log: Some(session.output_log()),
            stdin: false,
        },
    )
    .await
    .unwrap();
    let mut seen = Vec::new();
    let ready = tokio::time::timeout(Duration::from_secs(180), async {
        while let Some(event) = events.recv().await {
            match event {
                GameEvent::Line(line) => {
                    let done = line.text.contains("Sound engine started");
                    seen.push(line);
                    if done {
                        return true;
                    }
                }
                GameEvent::Exited(exit) => panic!("o jogo saiu sozinho: {exit:?}"),
            }
        }
        false
    })
    .await;
    let started = Instant::now();
    process.handle().stop().await;
    let stop_took = started.elapsed();
    let started_at = process.started_at();
    let exit = process.wait().await.unwrap();
    assert!(matches!(ready, Ok(true)), "o jogo não chegou ao som");
    assert!(
        stop_took <= Duration::from_secs(5),
        "parar levou {stop_took:?}"
    );
    assert_eq!(
        exit.outcome,
        warden_launcher::outcome::Outcome::StoppedByUser
    );
    let record = SessionRecord::from_exit(
        started_at,
        &exit,
        &combo.spec(),
        runtime.version.major,
        None,
        &game_dir,
    );
    session.write_record(&record).unwrap();
    let setting_user = seen
        .iter()
        .find(|line| line.text == "Setting user: WardenTest")
        .unwrap();
    assert_eq!(setting_user.thread.as_deref(), Some("Render thread"));
    assert!(seen.iter().any(|line| {
        line.text
            .starts_with("Loading Minecraft 1.20.1 with Fabric Loader 0.19.5")
    }));
    eprintln!(
        "pronto em {} ms; parou em {:?}; {} linhas; sessão {}",
        seen.last().unwrap().elapsed_ms,
        stop_took,
        seen.len(),
        session.path().display()
    );
}
