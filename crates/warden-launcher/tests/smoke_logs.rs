//! Contrato dos marcadores da matriz L-05 sobre logs reais e redigidos do S-R5-3.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod comum;
#[path = "matrix/markers.rs"]
mod markers;

use std::path::Path;

use markers::{Progress, parse_fixture};
use warden_launcher::{InstalledGame, LoggingConfig};

fn read(root: &Path, name: &str) -> String {
    std::fs::read_to_string(root.join(name)).unwrap()
}

#[test]
fn as_vinte_combinacoes_chegam_ao_menu_e_ao_mundo() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/matrix/matriz");
    assert_eq!(comum::MATRIX.len(), 20);
    for combo in comum::MATRIX {
        let legacy = matches!(
            combo.minecraft,
            "1.7.10" | "1.12.2" | "1.16.5" | "1.18.2" | "1.19.2"
        );
        let mut progress = Progress::default();
        let client = read(&root, &format!("{}-cliente.log", combo.name));
        for line in parse_fixture(&client) {
            progress.observe(combo.minecraft, combo.loader, &line, false);
        }
        if legacy {
            let server = read(&root, &format!("{}-servidor.log", combo.name));
            for line in parse_fixture(&server) {
                progress.observe(combo.minecraft, combo.loader, &line, true);
            }
        }
        assert!(progress.passed(legacy), "{}: {progress:?}", combo.name);
    }
}

#[test]
fn atlas_vazio_e_falso_crash_do_forge_1_7_10_nao_antecipam_pronto() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/matrix/mecanismos");
    let log = read(&root, "forge-1.7.10-falso-crash-report-splash.log");
    let mut progress = Progress::default();
    let lines = parse_fixture(&log);
    for line in &lines {
        progress.observe("1.7.10", "forge", line, false);
    }
    assert!(progress.failure.is_none(), "{progress:?}");
    let empty = lines
        .iter()
        .find(|line| line.text.contains("Created: 16x16 textures/blocks-atlas"))
        .unwrap();
    assert!(
        progress
            .ready_ms
            .is_none_or(|ready| ready > empty.elapsed_ms)
    );
}

#[test]
fn falhas_reais_nao_passam_por_pronto_ou_mundo() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/matrix/falhas");
    for (name, mc, loader) in [
        ("f112-dep-faltando", "1.12.2", "forge"),
        ("f112-duplicado", "1.12.2", "forge"),
        ("f112-java21", "1.12.2", "forge"),
        ("fab-dep-faltando", "1.20.1", "fabric"),
        ("fab-entrar", "1.20.1", "fabric"),
        ("fab-memoria", "1.20.1", "fabric"),
        ("fab-mixin", "1.20.1", "fabric"),
        ("forge-dep-faltando", "1.20.1", "forge"),
        ("forge-entrar", "1.20.1", "forge"),
        ("forge-mixin", "1.20.1", "forge"),
        ("neo-dep-faltando", "1.21.1", "neoforge"),
        ("neo-entrar", "1.21.1", "neoforge"),
        ("neo-memoria", "1.21.1", "neoforge"),
        ("neo-mixin", "1.21.1", "neoforge"),
    ] {
        let log = read(&root, &format!("{name}.log"));
        let mut progress = Progress::default();
        for line in parse_fixture(&log) {
            progress.observe(mc, loader, &line, false);
        }
        assert!(progress.failure.is_some(), "{name}: {progress:?}");
        assert!(!progress.passed(false), "{name}: {progress:?}");
    }
}

#[test]
fn log4shell_tem_configuracao_segura_em_cada_faixa() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for combo in comum::MATRIX {
        let plan: InstalledGame = serde_json::from_slice(
            &std::fs::read(root.join("matriz").join(format!("{}.json", combo.name))).unwrap(),
        )
        .unwrap();
        let golden = read(&root.join("golden"), &format!("{}.txt", combo.name));
        assert_eq!(plan.spec, combo.spec());
        assert!(
            !matches!(plan.logging, LoggingConfig::None),
            "{}",
            combo.name
        );
        if combo.loader == "neoforge" || (combo.loader == "forge" && combo.minecraft != "1.7.10") {
            assert!(
                matches!(plan.logging, LoggingConfig::Loader),
                "{}",
                combo.name
            );
            assert!(
                !golden.contains("-Dlog4j.configurationFile="),
                "{}",
                combo.name
            );
        } else {
            assert!(
                matches!(plan.logging, LoggingConfig::Mojang { .. }),
                "{}",
                combo.name
            );
            assert!(
                golden.contains("-Dlog4j.configurationFile="),
                "{}",
                combo.name
            );
        }
        if matches!(combo.minecraft, "1.7.10" | "1.12.2" | "1.16.5" | "1.18.2") {
            assert!(
                golden.contains("-Dlog4j2.formatMsgNoLookups=true"),
                "{}",
                combo.name
            );
        }
    }
}
