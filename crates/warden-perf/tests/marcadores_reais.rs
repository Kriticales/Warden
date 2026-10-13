//! Os tempos de carregamento sobre os golden logs reais da matriz L-05 (S-R5-3, capturados no
//! Windows e redigidos), lidos pelo mesmo `LogParser` da supervisão do jogo. O tempo de cada
//! linha aqui é o número da linha: o que se confere é **qual** linha marca cada momento.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use warden_launcher::log::{LogLine, LogParser, LogSource};
use warden_perf::{LoadTimes, LoadTracker, Loader};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../warden-launcher/tests/matrix")
}

fn parse(raw: &str) -> Vec<LogLine> {
    let mut parser = LogParser::new(LogSource::Stdout);
    let mut output = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        output.extend(parser.push(line, index as u64));
    }
    output.extend(parser.finish(raw.lines().count() as u64));
    output
}

fn loader(name: &str) -> Loader {
    match name {
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        "forge" => Loader::Forge,
        "neoforge" => Loader::NeoForge,
        _ => Loader::Vanilla,
    }
}

fn track(path: &Path, minecraft: &str, loader_name: &str) -> (LoadTracker, Vec<LogLine>) {
    let raw =
        std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let lines = parse(&raw);
    let mut tracker = LoadTracker::new(minecraft, loader(loader_name));
    for line in &lines {
        tracker.observe(&line.text, line.elapsed_ms);
    }
    (tracker, lines)
}

/// O texto da linha que marcou `at`.
fn line_at(lines: &[LogLine], at: u32) -> &str {
    &lines
        .iter()
        .find(|line| line.elapsed_ms == u64::from(at))
        .expect("linha do marco")
        .text
}

#[test]
fn as_vinte_combinacoes_carregam_no_marcador_da_faixa() {
    let dir = fixtures().join("matriz");
    let mut clients: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with("-cliente.log"))
        .collect();
    clients.sort();
    assert_eq!(clients.len(), 20, "{clients:?}");
    for name in clients {
        let (loader_name, minecraft) = name
            .trim_end_matches("-cliente.log")
            .split_once('-')
            .unwrap();
        let (tracker, lines) = track(&dir.join(&name), minecraft, loader_name);
        assert!(!tracker.failed(), "{name}");
        let loaded = tracker
            .times()
            .loaded_ms
            .unwrap_or_else(|| panic!("{name}: não carregou"));
        let marker = line_at(&lines, loaded);
        assert!(
            marker.contains("Sound engine started")
                || marker.contains("atlas")
                || marker.contains("successfully loaded"),
            "{name}: {marker}"
        );
        // O som e o atlas da faixa vieram até a linha do marco.
        let until: Vec<&str> = lines
            .iter()
            .take_while(|line| line.elapsed_ms <= u64::from(loaded))
            .map(|line| line.text.as_str())
            .collect();
        assert!(
            until
                .iter()
                .any(|text| text.contains("Sound engine started")),
            "{name}"
        );
        if let Some(world) = tracker.times().world_ms {
            assert!(
                line_at(&lines, world).contains("logged in with entity id")
                    || line_at(&lines, world).contains("joined the game"),
                "{name}"
            );
        }
        // Com Quick Play (1.20+), o cliente entra no mundo e o log mostra.
        let quick_play = !matches!(
            minecraft,
            "1.7.10" | "1.12.2" | "1.16.5" | "1.18.2" | "1.19.2"
        );
        if quick_play {
            assert!(tracker.times().world_ms.is_some(), "{name}: sem mundo");
        }
    }
}

#[test]
fn forge_1_7_10_nao_marca_no_atlas_vazio_nem_no_falso_crash() {
    let (tracker, lines) = track(
        &fixtures().join("mecanismos/forge-1.7.10-falso-crash-report-splash.log"),
        "1.7.10",
        "forge",
    );
    assert!(!tracker.failed());
    let empty = lines
        .iter()
        .find(|line| line.text.contains("Created: 16x16 textures/blocks-atlas"))
        .unwrap();
    assert!(
        tracker
            .times()
            .loaded_ms
            .is_none_or(|loaded| u64::from(loaded) > empty.elapsed_ms)
    );
}

#[test]
fn falhas_ao_carregar_nao_tem_abriu_em() {
    let dir = fixtures().join("falhas");
    for (name, minecraft, loader_name) in [
        ("f112-dep-faltando", "1.12.2", "forge"),
        ("f112-duplicado", "1.12.2", "forge"),
        ("f112-java21", "1.12.2", "forge"),
        ("fab-dep-faltando", "1.20.1", "fabric"),
        ("fab-mixin", "1.20.1", "fabric"),
        ("forge-dep-faltando", "1.20.1", "forge"),
        ("forge-mixin", "1.20.1", "forge"),
        ("neo-dep-faltando", "1.21.1", "neoforge"),
        ("neo-mixin", "1.21.1", "neoforge"),
    ] {
        let (tracker, _) = track(&dir.join(format!("{name}.log")), minecraft, loader_name);
        assert!(tracker.failed(), "{name}");
        assert_eq!(tracker.times(), LoadTimes::default(), "{name}");
    }
}
