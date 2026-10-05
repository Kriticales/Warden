//! Golden tests da linha de comando (critério 2 da L-02; ARCHITECTURE §7.1): para cada
//! combinação da matriz L-05, o plano gravado da instalação real
//! (`tests/fixtures/matriz/<combinação>.json`, gerado por `rede_matriz.rs`) passa pelo
//! montador da linha de comando e o resultado é comparado com
//! `tests/fixtures/golden/<combinação>.txt`, revisado à mão.
//!
//! A linha é montada para o Windows em qualquer máquina, com caminhos fixos (`C:\Warden\…`) e
//! a pasta `shared/` trocada por `<SHARED>`. Entrada direta: mundo `mundo-teste` com
//! `--quickPlayPath` nas versões com Quick Play (1.20+); servidor `127.0.0.1:25565` antes.
//! `WARDEN_LAUNCHER_REGRAVAR=1` regrava os arquivos dourados.

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

use std::fmt::Write as _;
use std::path::PathBuf;

use warden_java::{CompatibilityTable, JavaChoiceRequest};
use warden_launcher::command::{TargetOs, build};
use warden_launcher::{InstalledGame, JavaRuntime, LaunchOptions, OfflineProfile, QuickPlay};

/// O major que a política da `warden-java` escolhe para a combinação.
fn java_major(combo: &comum::Combo) -> u32 {
    let request = JavaChoiceRequest::automatic(
        combo.minecraft,
        combo.loader_kind(),
        (!combo.version.is_empty()).then_some(combo.version),
    );
    warden_java::policy::decide(&request, CompatibilityTable::builtin().unwrap())
        .unwrap()
        .requirement
        .major
}

fn options(game: &InstalledGame, major: u32) -> LaunchOptions {
    let quick_play_era = game.release_time.as_str() >= "2023-04-05T12:05:17";
    LaunchOptions {
        game_dir: PathBuf::from(r"C:\Warden\instances\PACK\minecraft"),
        state_dir: PathBuf::from(r"C:\Warden\instances\PACK\state"),
        java: JavaRuntime {
            java: PathBuf::from(format!(
                r"C:\Warden\shared\runtimes\temurin-{major}\bin\java.exe"
            )),
            launcher: PathBuf::from(format!(
                r"C:\Warden\shared\runtimes\temurin-{major}\bin\javaw.exe"
            )),
            major,
        },
        memory_mb: 4096,
        extra_jvm_args: Vec::new(),
        system_props: Vec::new(),
        quick_play: Some(if quick_play_era {
            QuickPlay::Singleplayer {
                world: "mundo-teste".into(),
            }
        } else {
            QuickPlay::Multiplayer {
                host: "127.0.0.1".into(),
                port: 25_565,
            }
        }),
        quick_play_path: Some("warden-quickplay.json".into()),
        player: OfflineProfile::from_name("WardenTest").unwrap(),
    }
}

/// O texto dourado: programa, pasta e um argumento por linha.
fn render(game: &InstalledGame, major: u32) -> String {
    let command = build(game, &options(game, major), TargetOs::Windows).unwrap();
    let mut text = String::new();
    writeln!(text, "# {} (Java {major})", game.spec).unwrap();
    writeln!(text, "programa: {}", command.program.display()).unwrap();
    writeln!(text, "pasta: {}", command.cwd.display()).unwrap();
    for arg in &command.args {
        writeln!(text, "{arg}").unwrap();
    }
    if let Some(argfile) = &command.argfile {
        writeln!(text, "# @argfile {}", argfile.path.display()).unwrap();
        text.push_str(&argfile.contents);
    }
    text
}

#[test]
fn linha_de_comando_de_cada_combinacao_da_matriz() {
    let rewrite = comum::env_on("WARDEN_LAUNCHER_REGRAVAR");
    let golden_dir = comum::fixtures_dir().join("golden");
    std::fs::create_dir_all(&golden_dir).unwrap();
    let mut differences = Vec::new();
    for combo in comum::MATRIX {
        let plan = comum::fixtures_dir()
            .join("matriz")
            .join(format!("{}.json", combo.name));
        let game: InstalledGame = serde_json::from_slice(&std::fs::read(&plan).unwrap()).unwrap();
        assert_eq!(game.spec, combo.spec(), "{}", combo.name);
        let text = render(&game, java_major(&combo));
        let golden = golden_dir.join(format!("{}.txt", combo.name));
        if rewrite {
            std::fs::write(&golden, &text).unwrap();
            continue;
        }
        let stored = std::fs::read_to_string(&golden)
            .unwrap()
            .replace("\r\n", "\n");
        if stored != text {
            differences.push(combo.name);
        }
    }
    assert!(
        differences.is_empty(),
        "linhas diferentes dos arquivos dourados: {differences:?} (revise e regrave com \
         WARDEN_LAUNCHER_REGRAVAR=1)"
    );
}

/// O que cada faixa precisa ter, conferido nos planos reais (além do texto inteiro).
#[test]
fn regras_por_faixa_nos_planos_reais() {
    for combo in comum::MATRIX {
        let plan = comum::fixtures_dir()
            .join("matriz")
            .join(format!("{}.json", combo.name));
        let game: InstalledGame = serde_json::from_slice(&std::fs::read(&plan).unwrap()).unwrap();
        let major = java_major(&combo);
        let command = build(&game, &options(&game, major), TargetOs::Windows).unwrap();
        let args = match &command.argfile {
            Some(argfile) => argfile
                .contents
                .lines()
                .map(|line| line.trim_matches('"').replace("\\\\", "\\"))
                .collect::<Vec<_>>(),
            None => command.args.clone(),
        };
        let has = |needle: &str| args.iter().any(|arg| arg == needle);
        let name = combo.name;
        // Jogador offline e pasta da instância.
        let after = |flag: &str| {
            args.iter()
                .position(|arg| arg == flag)
                .map(|i| args[i + 1].clone())
        };
        assert_eq!(after("--username").as_deref(), Some("WardenTest"), "{name}");
        assert_eq!(
            after("--uuid").as_deref(),
            Some("6c5aa2b1c08439d799a8edc5b372e5f9"),
            "{name}"
        );
        assert_eq!(after("--accessToken").as_deref(), Some("0"), "{name}");
        assert_eq!(
            after("--gameDir").as_deref(),
            Some(r"C:\Warden\instances\PACK\minecraft"),
            "{name}"
        );
        assert!(
            !args.iter().any(|arg| arg.contains("engine-gamedir")),
            "{name}"
        );
        // `--userType legacy` até 1.21.8; `--offlineDeveloperMode` a partir da 1.21.9.
        let modern_offline = game.release_time.as_str() >= "2025-09-30T11:58:43";
        assert_eq!(has("--offlineDeveloperMode"), modern_offline, "{name}");
        if !modern_offline {
            assert_eq!(after("--userType").as_deref(), Some("legacy"), "{name}");
        }
        // Log4j: nunca 1.18 ou antes sem configuração segura; NeoForge sem a da Mojang.
        let mojang_config = args
            .iter()
            .any(|arg| arg.starts_with("-Dlog4j.configurationFile="));
        if combo.loader == "neoforge" {
            assert!(!mojang_config, "{name}");
        }
        if game.release_time.as_str() < "2022-06-07T09:42:18" {
            let forge_verified = combo.loader == "forge" && combo.minecraft != "1.7.10";
            assert!(
                mojang_config || forge_verified,
                "{name}: sem configuração log4j segura"
            );
            assert!(has("-Dlog4j2.formatMsgNoLookups=true"), "{name}");
        }
        // Codificação da saída por Java.
        assert_eq!(has("-Dstdout.encoding=UTF-8"), major >= 19, "{name}");
        assert_eq!(has("-Dfile.encoding=UTF-8"), major == 17, "{name}");
        assert_eq!(args[0], "-Xmx4096M", "{name}");
    }
}
