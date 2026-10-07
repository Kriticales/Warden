//! A-05: escrita externa recarrega em até 2 s; escritas do Warden não geram recarga dupla.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use warden_core::PackId;
use warden_project::watch::{Area, PackWatcher, WatchConfig, WriteLedger};

/// Limite do CA-T05-01.
const WITHIN: Duration = Duration::from_secs(2);

fn start(root: &Path, ledger: &WriteLedger) -> (PackWatcher, Receiver<Vec<Area>>, PackId) {
    let pack = PackId::new();
    let (sender, receiver) = mpsc::channel();
    let watcher = PackWatcher::start(
        pack,
        root,
        ledger.clone(),
        WatchConfig::default(),
        move |areas| {
            let _ = sender.send(areas);
        },
    )
    .unwrap();
    (watcher, receiver, pack)
}

fn pack_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("config")).unwrap();
    fs::create_dir_all(dir.path().join("mods")).unwrap();
    fs::write(dir.path().join("pack.toml"), "name = \"a\"\n").unwrap();
    fs::write(dir.path().join("config/a.toml"), "x = 1\n").unwrap();
    dir
}

#[test]
fn a05_config_editado_por_fora_avisa_em_ate_2s() {
    let dir = pack_dir();
    let (_watcher, receiver, _) = start(dir.path(), &WriteLedger::default());

    let started = Instant::now();
    fs::write(dir.path().join("config/a.toml"), "x = 2\n").unwrap();
    let areas = receiver.recv_timeout(WITHIN).expect("sem aviso em 2 s");

    assert!(started.elapsed() < WITHIN);
    assert_eq!(areas, [Area::Configs]);
    assert!(receiver.recv_timeout(Duration::from_millis(900)).is_err());
}

#[test]
fn a05_rajada_externa_vira_um_aviso_com_todas_as_areas() {
    let dir = pack_dir();
    let (_watcher, receiver, _) = start(dir.path(), &WriteLedger::default());

    for round in 0..5 {
        fs::write(dir.path().join("config/a.toml"), format!("x = {round}\n")).unwrap();
        fs::write(dir.path().join("mods/a.pw.toml"), format!("n = {round}\n")).unwrap();
    }
    let areas = receiver.recv_timeout(WITHIN).expect("sem aviso em 2 s");

    assert_eq!(areas, [Area::Inventory, Area::Configs]);
    assert!(receiver.recv_timeout(Duration::from_millis(900)).is_err());
}

#[test]
fn a05_escrita_do_warden_nao_gera_recarga() {
    let dir = pack_dir();
    let ledger = WriteLedger::default();
    let (_watcher, receiver, pack) = start(dir.path(), &ledger);

    {
        let _scope = ledger.begin(pack);
        fs::write(dir.path().join("config/a.toml"), "x = 3\n").unwrap();
        fs::write(dir.path().join("index.toml"), "hash-format = \"sha256\"\n").unwrap();
    }

    assert!(
        receiver.recv_timeout(Duration::from_millis(1800)).is_err(),
        "a escrita do Warden foi tratada como mudança externa"
    );
    // Passada a folga, a escrita externa volta a ser vista.
    fs::write(dir.path().join("config/a.toml"), "x = 4\n").unwrap();
    assert_eq!(receiver.recv_timeout(WITHIN).unwrap(), [Area::Configs]);
}

#[test]
fn a05_ruido_do_git_e_arquivos_temporarios_sao_ignorados() {
    let dir = pack_dir();
    fs::create_dir_all(dir.path().join(".git/objects")).unwrap();
    let (_watcher, receiver, _) = start(dir.path(), &WriteLedger::default());

    fs::write(dir.path().join(".git/objects/blob"), b"x").unwrap();
    fs::write(dir.path().join("pack.toml.1-0.warden-tmp"), b"x").unwrap();

    assert!(receiver.recv_timeout(Duration::from_millis(1500)).is_err());
}

#[test]
fn a05_historico_muda_quando_o_git_move_o_head() {
    let dir = pack_dir();
    fs::create_dir_all(dir.path().join(".git")).unwrap();
    let (_watcher, receiver, _) = start(dir.path(), &WriteLedger::default());

    fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();

    assert_eq!(receiver.recv_timeout(WITHIN).unwrap(), [Area::History]);
}

#[test]
fn a05_pasta_do_pack_apagada_avisa_todas_as_areas() {
    let dir = pack_dir();
    let root = dir.path().join("pack");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("pack.toml"), "x").unwrap();
    let (_watcher, receiver, _) = start(&root, &WriteLedger::default());

    fs::remove_dir_all(&root).unwrap();

    // O sistema pode contar os arquivos apagados antes da pasta: vale o que chegar em 2 s.
    let deadline = Instant::now() + WITHIN;
    let mut seen = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(areas) = receiver.recv_timeout(left) else {
            break;
        };
        seen.extend(areas);
        if seen.contains(&Area::History) && seen.contains(&Area::Configs) {
            break;
        }
    }
    for area in [Area::Inventory, Area::Configs, Area::Meta, Area::History] {
        assert!(seen.contains(&area), "faltou {area:?}: {seen:?}");
    }
}

#[test]
fn a05_soltar_o_vigia_para_os_avisos() {
    let dir = pack_dir();
    let (watcher, receiver, _) = start(dir.path(), &WriteLedger::default());
    drop(watcher);

    fs::write(dir.path().join("config/a.toml"), "x = 9\n").unwrap();

    assert!(receiver.recv_timeout(Duration::from_millis(1200)).is_err());
}
