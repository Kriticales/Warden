//! Jogo real da matriz L-05. Ignorado no check normal: abre janelas no Windows, baixa
//! servidores e usa Xvfb/Mesa no Linux. Execute uma combinação por vez com
//! `WARDEN_SMOKE_ONLY=<id> WARDEN_SMOKE_GAME=1 cargo test -p warden-launcher
//! --test smoke_game -- --ignored --nocapture`.

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
#[path = "matrix/markers.rs"]
mod markers;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use markers::Progress;
use tokio::process::Command;
use warden_core::{CancellationToken, NoProgress};
use warden_instance::{
    DownloadCache, InstanceDirs, MaterializeOptions, OnModified, Outcome, Sources, materialize,
};
use warden_java::JavaChoiceRequest;
use warden_launcher::process::{GameEvent, GameProcess, SpawnOptions};
use warden_launcher::{
    CatalogForgeResolver, EngineDirs, JavaRuntime, LaunchCommand, LaunchOptions, LauncherEngine,
    OfflineProfile, PortableMcEngine, QuickPlay,
};

fn modern(mc: &str) -> bool {
    matches!(mc, "1.20.1" | "1.21.1" | "26.2" | "26.3")
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn copy_world(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if entry.file_name() == "session.lock" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy_world(&path, &target);
        } else {
            std::fs::copy(path, target).unwrap();
        }
    }
}

fn python() -> String {
    std::env::var("WARDEN_SMOKE_PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into())
}

async fn pack_script(args: &[&str]) {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/matrix/pack.py");
    let status = Command::new(python())
        .arg(script)
        .args(args)
        .kill_on_drop(true)
        .status()
        .await
        .unwrap();
    assert!(status.success(), "pack.py {args:?}: {status}");
}

async fn prepare(combo: &comum::Combo, root: &Path, java: &Path, port: u16) -> PathBuf {
    let mode = if modern(combo.minecraft) {
        "world"
    } else {
        "legacy"
    };
    let name = if mode == "world" {
        format!("vanilla-{}", combo.minecraft)
    } else {
        combo.name.to_owned()
    };
    let directory = root.join("servers").join(name);
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/matrix/prepare.py");
    let mut child = Command::new(python())
        .arg(script)
        .args([
            mode,
            combo.minecraft,
            combo.loader,
            combo.version,
            directory.to_str().unwrap(),
            java.to_str().unwrap(),
            &port.to_string(),
        ])
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let status = tokio::time::timeout(Duration::from_mins(15), child.wait())
        .await
        .expect("preparo do servidor excedeu 15 min")
        .unwrap();
    assert!(
        status.success(),
        "falha no preparo de {}: {status}",
        combo.name
    );
    directory
}

fn server_command(directory: &Path, java: &Path) -> LaunchCommand {
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(directory.join("warden-args.json")).unwrap())
            .unwrap();
    LaunchCommand {
        program: java.to_path_buf(),
        args,
        cwd: directory.to_path_buf(),
        env: Vec::new(),
        argfile: None,
    }
}

async fn run_one(combo: &comum::Combo, root: &Path) {
    let shared = root.join("shared");
    let java_service = comum::java_service(root);
    let cache = warden_catalog::CatalogCache::open(&root.join("cache/metadata.sqlite")).unwrap();
    let catalog = Arc::new(warden_catalog::Catalog::new(comum::http(), cache).unwrap());
    let engine = PortableMcEngine::new(
        EngineDirs::under(&shared),
        Some(Arc::new(CatalogForgeResolver::new(catalog))),
    );
    let cancel = CancellationToken::new();
    let choice = java_service
        .choose(&JavaChoiceRequest::automatic(
            combo.minecraft,
            combo.loader_kind(),
            (!combo.version.is_empty()).then_some(combo.version),
        ))
        .unwrap();
    let runtime = java_service
        .ensure_choice(&choice, &NoProgress, &cancel)
        .await
        .unwrap();
    let java = JavaRuntime::from_installed(&runtime);
    let game = engine
        .install(&combo.spec(), &java, &NoProgress, &cancel)
        .await
        .unwrap();
    let instance = root.join("instances").join(combo.name);
    let game_dir = instance.join("minecraft");
    let pack_dir = root.join("packs").join(combo.name);
    pack_script(&[
        "create",
        combo.name,
        combo.minecraft,
        combo.loader,
        combo.version,
        pack_dir.to_str().unwrap(),
    ])
    .await;
    let sources = Sources {
        http: comum::http(),
        curseforge: None,
        cache: Arc::new(DownloadCache::open(&root.join("cache/mods")).unwrap()),
    };
    let dirs = InstanceDirs {
        game_dir: game_dir.clone(),
        state_dir: instance.join("state"),
    };
    let materialized = materialize(
        &sources,
        &pack_dir,
        &dirs,
        &MaterializeOptions {
            on_modified: OnModified::Overwrite,
            ..MaterializeOptions::default()
        },
        &NoProgress,
        &cancel,
    )
    .await
    .unwrap();
    match materialized {
        Outcome::Done(report) => assert!(report.is_complete(), "{}: {report:?}", combo.name),
        Outcome::NeedsReview(modified) => {
            panic!("pack exige revisão: {}: {modified:?}", combo.name)
        }
    }
    pack_script(&["verify", combo.name, game_dir.to_str().unwrap()]).await;
    let port = free_port();
    let server_dir = prepare(combo, root, &java.java, port).await;
    std::fs::create_dir_all(&game_dir).unwrap();
    // A opção surgiu em 1.19.4. Versões anteriores descartam a chave ao salvar.
    if modern(combo.minecraft) {
        std::fs::write(game_dir.join("options.txt"), "onboardAccessibility:false\n").unwrap();
    }
    if combo.loader == "forge" {
        std::fs::create_dir_all(game_dir.join("config")).unwrap();
        std::fs::write(
            game_dir.join("config/forge-client.toml"),
            "[client]\nshowLoadWarnings = false\n",
        )
        .unwrap();
    }
    if modern(combo.minecraft) {
        let world = game_dir.join("saves/wardentest");
        if world.exists() {
            let resolved = world.canonicalize().unwrap();
            let inside = game_dir.canonicalize().unwrap();
            assert!(
                resolved.starts_with(&inside),
                "mundo fora da instância de teste"
            );
            std::fs::remove_dir_all(&world).unwrap();
        }
        copy_world(&server_dir.join("world"), &world);
        let _ = std::fs::remove_file(game_dir.join("quickplay.json"));
    }
    let mut props = Vec::new();
    if matches!(combo.loader, "fabric" | "quilt") {
        props.push(("fabric.noGui".into(), "true".into()));
    }
    if combo.name == "forge-1.12.2" {
        props.push(("fml.queryResult".into(), "confirm".into()));
        if cfg!(target_os = "linux") {
            // LWJGL 2 tenta ler o primeiro monitor XRandR; Xvfb não anuncia nenhum.
            props.push(("LWJGL_DISABLE_XRANDR".into(), "true".into()));
        }
    }
    let options = LaunchOptions {
        game_dir: game_dir.clone(),
        state_dir: instance.join("state"),
        java,
        memory_mb: 3072,
        extra_jvm_args: Vec::new(),
        system_props: props,
        quick_play: Some(if modern(combo.minecraft) {
            QuickPlay::Singleplayer {
                world: "wardentest".into(),
            }
        } else {
            QuickPlay::Multiplayer {
                host: "127.0.0.1".into(),
                port,
            }
        }),
        quick_play_path: modern(combo.minecraft).then(|| "quickplay.json".into()),
        player: OfflineProfile::from_name("WardenTest").unwrap(),
    };
    let mut command = engine.command(&game, &options).unwrap();
    let argv = command.argfile.as_ref().map_or_else(
        || command.args.join("\n"),
        |argfile| argfile.contents.clone(),
    );
    if modern(combo.minecraft) {
        assert!(argv.contains("--quickPlaySingleplayer"), "{}", combo.name);
        assert!(argv.contains("--quickPlayPath"), "{}", combo.name);
    } else {
        assert!(argv.contains("--server"), "{}", combo.name);
        assert!(argv.contains("--port"), "{}", combo.name);
    }
    if matches!(combo.loader, "fabric" | "quilt") {
        assert!(argv.contains("-Dfabric.noGui=true"), "{}", combo.name);
    }
    if combo.name == "forge-1.12.2" {
        assert!(argv.contains("-Dfml.queryResult=confirm"), "{}", combo.name);
        if cfg!(target_os = "linux") {
            assert!(
                argv.contains("-DLWJGL_DISABLE_XRANDR=true"),
                "{}",
                combo.name
            );
        }
    }
    command.env.push(("ALSOFT_DRIVERS".into(), "null".into()));
    if cfg!(target_os = "linux") && combo.minecraft == "26.3" {
        // SDL 3 usa GLX por padrão. O GLX do Xvfb/Mesa não oferece o visual
        // pedido pelo RenderPearl; EGL usa o mesmo Mesa sem esse visual GLX.
        command.env.push(("SDL_VIDEO_FORCE_EGL".into(), "1".into()));
    }
    let mut server = None;
    let mut server_events = None;
    let mut progress = Progress::default();
    if !modern(combo.minecraft) {
        let (running, mut events) = GameProcess::spawn(
            &server_command(&server_dir, &options.java.java),
            SpawnOptions {
                output_log: Some(instance.join("server.log")),
                stdin: true,
            },
        )
        .await
        .unwrap();
        let ready = tokio::time::timeout(Duration::from_mins(4), async {
            while let Some(event) = events.recv().await {
                match event {
                    GameEvent::Line(line) => {
                        progress.observe(combo.minecraft, combo.loader, &line, true);
                    }
                    GameEvent::Exited(exit) => {
                        return Err(format!("servidor saiu antes de ficar pronto: {exit:?}"));
                    }
                }
                if progress.server_ready_ms.is_some() {
                    return Ok(());
                }
            }
            Err("saída do servidor acabou".to_owned())
        })
        .await;
        if !matches!(&ready, Ok(Ok(()))) {
            running.handle().stop().await;
            let _ = running.wait().await;
            panic!("servidor não ficou pronto: {}: {ready:?}", combo.name);
        }
        server = Some(running);
        server_events = Some(events);
    }
    let opened = GameProcess::spawn(
        &command,
        SpawnOptions {
            output_log: Some(instance.join("client.log")),
            stdin: false,
        },
    )
    .await;
    let (client, mut client_events) = match opened {
        Ok(value) => value,
        Err(error) => {
            if let Some(running) = server {
                running.handle().stop().await;
                let _ = running.wait().await;
            }
            panic!("cliente não abriu: {error}");
        }
    };
    let client_clock = Instant::now();
    let result = tokio::time::timeout(Duration::from_mins(6), async {
        loop {
            tokio::select! {
                event = client_events.recv() => match event {
                    Some(GameEvent::Line(line)) => {
                        progress.observe(combo.minecraft, combo.loader, &line, false);
                    }
                    Some(GameEvent::Exited(exit)) => return Err(format!("cliente saiu antes do mundo: {exit:?}")),
                    None => return Err("saída do cliente acabou".to_owned()),
                },
                event = async { server_events.as_mut().unwrap().recv().await }, if server_events.is_some() => {
                    match event {
                        Some(GameEvent::Line(mut line)) => {
                            // O servidor foi aberto antes do cliente; os tempos da matriz
                            // sempre começam no spawn do cliente (S-R5-3 §3).
                            line.elapsed_ms = u64::try_from(client_clock.elapsed().as_millis()).unwrap_or(u64::MAX);
                            progress.observe(combo.minecraft, combo.loader, &line, true);
                        },
                        Some(GameEvent::Exited(exit)) => return Err(format!("servidor saiu antes do mundo: {exit:?}")),
                        None => return Err("saída do servidor acabou".to_owned()),
                    }
                }
            }
            if progress.failure.is_some() || progress.passed(!modern(combo.minecraft)) {
                return Ok(());
            }
        }
    }).await;
    if matches!(&result, Ok(Ok(()))) && progress.passed(!modern(combo.minecraft)) {
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    client.handle().stop().await;
    let client_exit = client.wait().await.unwrap();
    if let Some(running) = server {
        let _ = running.handle().send_line("stop").await;
        let _ = tokio::time::timeout(Duration::from_secs(30), running.handle().stop()).await;
        let _ = running.wait().await;
    }
    assert!(
        matches!(&result, Ok(Ok(()))),
        "{}: {result:?}; {progress:?}",
        combo.name
    );
    assert!(
        progress.passed(!modern(combo.minecraft)),
        "{}: {progress:?}",
        combo.name
    );
    if modern(combo.minecraft) {
        let path = game_dir.join("quickplay.json");
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(data[0]["type"], "singleplayer", "{}", combo.name);
    }
    if combo.loader == "forge" && modern(combo.minecraft) {
        let settings = std::fs::read_to_string(game_dir.join("config/forge-client.toml")).unwrap();
        assert!(
            settings.contains("showLoadWarnings = false"),
            "{}",
            combo.name
        );
    }
    assert!(client_exit.stop_requested, "{}", combo.name);
    let final_options = std::fs::read_to_string(game_dir.join("options.txt")).unwrap();
    if modern(combo.minecraft) {
        assert!(
            final_options
                .lines()
                .any(|line| line == "onboardAccessibility:false"),
            "{}: opção de acessibilidade ausente ao fechar",
            combo.name
        );
    }
    println!(
        "{}: pronto={} ms, mundo={} ms, saída={:?}",
        combo.name,
        progress.ready_ms.unwrap(),
        progress.world_ms.unwrap(),
        client_exit.outcome
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "rede: abre o jogo real e baixa servidores"]
async fn rede_matriz_jogo_real() {
    assert!(
        comum::env_on("WARDEN_SMOKE_GAME"),
        "defina WARDEN_SMOKE_GAME=1"
    );
    let only = std::env::var("WARDEN_SMOKE_ONLY").unwrap_or_default();
    let (root, _temp) = comum::data_root();
    let selected: Vec<_> = comum::MATRIX
        .iter()
        .filter(|combo| only.is_empty() || only.split(',').any(|name| name == combo.name))
        .collect();
    assert!(
        !selected.is_empty(),
        "WARDEN_SMOKE_ONLY não casa com a matriz"
    );
    let mut failures = Vec::new();
    for combo in selected {
        let name = combo.name;
        let combo = *combo;
        let root = root.clone();
        // Isola o panic de cada combinação para sempre executar as demais.
        match tokio::spawn(async move { run_one(&combo, &root).await }).await {
            Ok(()) => println!("RESUMO {name}: OK"),
            Err(error) => {
                println!("RESUMO {name}: FALHOU: {error}");
                failures.push(name);
            }
        }
    }
    assert!(failures.is_empty(), "combinações com falha: {failures:?}");
}
