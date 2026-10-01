//! Spike S1 do Warden — protótipo com portablemc.
//!
//! Uso: `pmc-proto <caso> [--java mojang|<caminho>] [--timeout <s>] [--settle <s>]
//!                       [--install-only] [--print-command] [--cancel-before-download]
//!                       [--screenshot]`
//!
//! Instala o caso escolhido num diretório de dados fora do repositório
//! (`$WARDEN_SPIKE_DATA` ou `~/.local/share/warden-spike/pmc`), inicia o jogo em modo
//! offline como `WardenTest`, espera pelos marcadores de carregamento completo no log,
//! aguarda `settle` segundos para confirmar estabilidade e encerra o processo.

mod engine;
mod supervisor;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use engine::{CancelFlag, EngineDirs, EngineEvent, InstallRequest, JavaSpec, LauncherEngine, LoaderSpec, PortableMcEngine};
use supervisor::RunningGame;

struct Case {
    id: &'static str,
    mc: &'static str,
    loader: fn() -> LoaderSpec,
    /// Todos precisam aparecer (em qualquer ordem) para considerar "carregou".
    ready: &'static [&'static str],
}

/// Marcadores definidos empiricamente a partir dos logs reais (ver relatório S1).
const CASES: &[Case] = &[
    Case {
        id: "fabric-1.20.1",
        mc: "1.20.1",
        loader: || LoaderSpec::Fabric { loader_version: None },
        ready: &["Loading Minecraft 1.20.1 with Fabric Loader", "Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas", "Sound engine started"],
    },
    Case {
        id: "forge-1.12.2",
        mc: "1.12.2",
        loader: || LoaderSpec::Forge { version: "1.12.2-14.23.5.2860".into() },
        ready: &["Forge Mod Loader has successfully loaded", "textures-atlas", "Sound engine started"],
    },
    Case {
        id: "neoforge-1.21.1",
        mc: "1.21.1",
        loader: || LoaderSpec::NeoForge { version: "21.1.252".into() },
        ready: &["Launching target 'forgeclient'", "Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas", "Sound engine started"],
    },
    Case {
        id: "forge-1.7.10",
        mc: "1.7.10",
        loader: || LoaderSpec::Forge { version: "1.7.10-10.13.4.1614-1.7.10".into() },
        ready: &["Forge Mod Loader has successfully loaded", "textures/blocks-atlas", "Sound engine started"],
    },
    Case {
        id: "forge-1.16.5",
        mc: "1.16.5",
        loader: || LoaderSpec::Forge { version: "1.16.5-36.2.34".into() },
        ready: &["Launching target 'fmlclient'", "Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas", "Sound engine started"],
    },
    Case {
        id: "vanilla-1.20.1",
        mc: "1.20.1",
        loader: || LoaderSpec::Vanilla,
        ready: &["Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas", "Sound engine started"],
    },
    // Casos de erro proposital, para avaliar as mensagens do motor.
    Case {
        id: "erro-forge-inexistente",
        mc: "1.12.2",
        loader: || LoaderSpec::Forge { version: "1.12.2-14.23.5.9999".into() },
        ready: &[],
    },
    Case {
        id: "erro-fabric-mc-inexistente",
        mc: "1.99.9",
        loader: || LoaderSpec::Fabric { loader_version: None },
        ready: &[],
    },
];

/// Linhas que contêm um padrão fatal mas não são erro. O FML 1.7.10 imprime de
/// propósito um "Crash Report" com as specs do PC ("Loading screen debug info ...
/// THIS IS NOT A ERROR") a partir de `cpw.mods.fml.client.SplashProgress`.
const NOT_FATAL: &[&str] = &["SplashProgress"];

fn is_fatal(line: &str) -> Option<&'static str> {
    if NOT_FATAL.iter().any(|n| line.contains(n)) {
        return None;
    }
    FATAL.iter().copied().find(|f| line.contains(f))
}

const FATAL: &[&str] = &[
    "---- Minecraft Crash Report ----",
    "#@!@# Game crashed!",
    "Exception in thread \"main\"",
    "Could not find or load main class",
    "Error: Could not create the Java Virtual Machine",
    "Encountered an unexpected exception",
];

fn data_root() -> PathBuf {
    std::env::var_os("WARDEN_SPIKE_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").expect("HOME")).join(".local/share/warden-spike/pmc"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(case_id) = args.first() else {
        eprintln!("uso: pmc-proto <caso> [opções]; casos: {}", CASES.iter().map(|c| c.id).collect::<Vec<_>>().join(", "));
        return ExitCode::from(2);
    };
    let Some(case) = CASES.iter().find(|c| c.id == case_id) else {
        eprintln!("caso desconhecido: {case_id}");
        return ExitCode::from(2);
    };
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let timeout = Duration::from_secs(opt("--timeout").and_then(|s| s.parse().ok()).unwrap_or(900));
    let settle = Duration::from_secs(opt("--settle").and_then(|s| s.parse().ok()).unwrap_or(10));
    let java = match opt("--java").as_deref() {
        None | Some("mojang") => JavaSpec::Mojang,
        Some(p) => JavaSpec::Path(PathBuf::from(p)),
    };

    let root = data_root();
    let dirs = EngineDirs::under(&root.join("shared"));
    let req = InstallRequest {
        mc_version: case.mc.to_string(),
        loader: (case.loader)(),
        instance_dir: root.join("instances").join(case.id),
        java,
        offline_player: "WardenTest".into(),
    };
    std::fs::create_dir_all(&req.instance_dir).expect("criar instância");
    std::fs::create_dir_all(root.join("runs")).expect("criar runs");

    println!("== caso {} (MC {}, {:?})", case.id, case.mc, req.loader);
    println!("== dados em {}", root.display());

    let cancel = CancelFlag::default();
    let t0 = Instant::now();
    let mut last_progress = Instant::now() - Duration::from_secs(10);
    let mut sink = |ev: EngineEvent| match ev {
        EngineEvent::Progress { count, total_count, size, total_size } => {
            if last_progress.elapsed() >= Duration::from_secs(2) || count == total_count {
                last_progress = Instant::now();
                println!(
                    "[{:>7.1}s] progresso {count}/{total_count} arquivos, {:.1}/{:.1} MB",
                    t0.elapsed().as_secs_f64(),
                    size as f64 / 1e6,
                    total_size as f64 / 1e6
                );
            }
        }
        EngineEvent::Stage(s) => {
            // Simula o usuário clicando em "cancelar" durante a instalação. Precisa
            // acontecer antes do evento DownloadResources, que é o único ponto em que
            // o portablemc consulta o cancelamento; "Java N necessário" vem logo antes.
            if s.starts_with("Java ") && flag("--cancel-before-download") {
                cancel.cancel();
            }
            println!("[{:>7.1}s] etapa: {s}", t0.elapsed().as_secs_f64());
        }
        other => println!("[{:>7.1}s] {other:?}", t0.elapsed().as_secs_f64()),
    };

    let prepared = match PortableMcEngine.install(&dirs, &req, &mut sink, &cancel) {
        Ok(p) => p,
        Err(e) => {
            println!("!! instalação falhou após {:.1}s: {e}", t0.elapsed().as_secs_f64());
            return ExitCode::from(1);
        }
    };
    println!("== instalação concluída em {:.1}s", t0.elapsed().as_secs_f64());
    println!("== java: {}", prepared.java.display());
    println!("== mainClass: {}", prepared.main_class);
    let cmdline_len: usize = prepared.jvm_args.iter().chain(&prepared.game_args).map(|a| a.len() + 1).sum::<usize>()
        + prepared.main_class.len()
        + prepared.java.as_os_str().len();
    println!("== tamanho da linha de comando: {cmdline_len} caracteres");
    if flag("--print-command") {
        println!("== jvm_args:");
        for a in &prepared.jvm_args {
            println!("   {a}");
        }
        println!("== game_args: {}", redact(&prepared.game_args).join(" "));
    }
    if flag("--install-only") {
        return ExitCode::SUCCESS;
    }

    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let capture = root.join("runs").join(format!("{}-{stamp}.log", case.id));
    let mut game = match RunningGame::spawn(prepared.command(), &capture) {
        Ok(g) => g,
        Err(e) => {
            println!("!! falha ao iniciar processo: {e}");
            return ExitCode::from(1);
        }
    };
    println!("== jogo iniciado (pid {}), saída em {}", game.pid(), capture.display());

    let mut seen = vec![None::<Duration>; case.ready.len()];
    let outcome = loop {
        if game.elapsed() > timeout {
            break Outcome::Timeout;
        }
        match game.next_line(Duration::from_millis(500)) {
            Ok(line) => {
                for (i, m) in case.ready.iter().enumerate() {
                    if seen[i].is_none() && line.text.contains(m) {
                        seen[i] = Some(line.at);
                        println!("[{:>7.1}s] marcador: {m}", line.at.as_secs_f64());
                    }
                }
                if let Some(f) = is_fatal(&line.text) {
                    break Outcome::Fatal(format!("{f} :: {}", line.text.chars().take(200).collect::<String>()));
                }
                if seen.iter().all(Option::is_some) {
                    break Outcome::Ready(line.at);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(st) = game.try_exit_status() {
                    break Outcome::Exited(format!("{st}"));
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                break Outcome::Exited(game.try_exit_status().map(|s| s.to_string()).unwrap_or_default());
            }
        }
    };

    let ok = match &outcome {
        Outcome::Ready(at) => {
            println!("== CARREGOU em {:.1}s após o spawn; aguardando {}s de estabilidade", at.as_secs_f64(), settle.as_secs());
            let until = Instant::now() + settle;
            let mut crashed = None;
            while Instant::now() < until {
                if let Ok(line) = game.next_line(Duration::from_millis(200)) {
                    if let Some(f) = is_fatal(&line.text) {
                        crashed = Some(f.to_string());
                        break;
                    }
                }
                if game.try_exit_status().is_some() {
                    crashed = Some("processo saiu sozinho".into());
                    break;
                }
            }
            match crashed {
                None => true,
                Some(c) => {
                    println!("!! instável depois de carregar: {c}");
                    false
                }
            }
        }
        Outcome::Timeout => {
            println!("!! tempo esgotado; marcadores vistos: {:?}", case.ready.iter().zip(&seen).filter(|(_, s)| s.is_some()).map(|(m, _)| m).collect::<Vec<_>>());
            false
        }
        Outcome::Fatal(f) => {
            println!("!! erro fatal no log: {f}");
            false
        }
        Outcome::Exited(s) => {
            println!("!! processo saiu antes de carregar: {s}");
            false
        }
    };

    if ok && flag("--screenshot") {
        let png = root.join("runs").join(format!("{}-{stamp}.png", case.id));
        let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/screenshot.py");
        match std::process::Command::new("python3").arg(script).arg(&png).status() {
            Ok(st) if st.success() => println!("== captura de tela: {}", png.display()),
            other => println!("!! captura de tela falhou: {other:?}"),
        }
    }

    let report = game.stop(Duration::from_secs(20));
    println!(
        "== encerrado: status={:?} forçado={} em {:.1}s",
        report.status.map(|s| s.to_string()),
        report.forced,
        report.took.as_secs_f64()
    );
    if ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

enum Outcome {
    Ready(Duration),
    Timeout,
    Fatal(String),
    Exited(String),
}

/// Esconde o UUID e o token na impressão da linha de comando.
fn redact(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut hide_next = false;
    for a in args {
        if hide_next {
            out.push("<omitido>".into());
            hide_next = false;
            continue;
        }
        hide_next = matches!(a.as_str(), "--uuid" | "--accessToken" | "--session");
        out.push(a.clone());
    }
    out
}
