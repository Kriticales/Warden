//! "Não substituir se o jogador já tiver" e "Copiar para defaultconfigs/" (C-04; SPEC T12) com
//! o packwiz real (o `preserve` sobrevive ao `packwiz refresh`) e o packwiz-installer real (o
//! arquivo do jogador fica nas atualizações), quando presentes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use warden_catalog::Loader;
use warden_core::CancellationToken;
use warden_packwiz::PackIndex;
use warden_packwiz_cli::{Packwiz, RunContext};
use warden_project::ProjectErrorCode as Code;
use warden_project::configs::{ConfigOrigin, read_file, save_to_pack};
use warden_project::create::{CreatePack, create};
use warden_project::preserve::{
    CopyOutcome, PreserveState, copy_to_defaultconfigs, preserve_of, set_preserve,
};
use warden_project::registry::Registry;

fn require_externals() -> bool {
    std::env::var("WARDEN_REQUIRE_EXTERNALS").ok().as_deref() == Some("1")
}

fn binary() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "packwiz-x86_64-pc-windows-msvc.exe"
    } else {
        "packwiz-x86_64-unknown-linux-gnu"
    };
    let path = std::env::var_os("WARDEN_PACKWIZ_BIN").map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/desktop/src-tauri/binaries")
                .join(name)
        },
        PathBuf::from,
    );
    if path.is_file() {
        Some(path)
    } else {
        assert!(!require_externals(), "sidecar packwiz ausente");
        None
    }
}

/// Java, bootstrap e packwiz-installer baixados por `cargo xtask installer`.
fn installer() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let dir = std::env::var_os("WARDEN_INSTALLER_CACHE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(|local| PathBuf::from(local).join("Warden-dev/cache/installer"))
        });
    let found = dir.and_then(|dir| {
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("externals.json")).ok()?).ok()?;
        let get = |key: &str| value[key].as_str().map(PathBuf::from);
        let paths = (get("java")?, get("bootstrap")?, get("installer")?);
        (paths.0.is_file() && paths.1.is_file() && paths.2.is_file()).then_some(paths)
    });
    if found.is_none() {
        assert!(
            !require_externals(),
            "instalador real ausente; rode cargo xtask installer"
        );
    }
    found
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, bytes).unwrap();
}

async fn real_pack(root: &Path, binary: PathBuf) -> (PathBuf, Packwiz) {
    fs::create_dir_all(root.join("dados")).unwrap();
    let registry = Registry::open(root.join("dados/packs.json")).unwrap();
    let cli = Packwiz::new(
        binary,
        root.join("dados/cache"),
        root.join("dados/config.toml"),
    );
    let made = create(
        &CreatePack {
            name: "Pack do Jogador".into(),
            author: "Autor".into(),
            description: String::new(),
            destination: Some(root.join("pack")),
            minecraft: "1.20.1".into(),
            loader: Some(Loader::Forge),
            loader_version: Some("47.3.0".into()),
        },
        root,
        &registry,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    write(&made.path, "options.txt", b"renderDistance:12\nfov:0.5\n");
    write(&made.path, "config/ajuste.toml", b"a = 1\n");
    write(
        &made.path,
        "config/create-server.toml",
        b"#Velocidade\nspeed = 2\n",
    );
    cli.refresh(
        &made.path,
        false,
        RunContext::new(&CancellationToken::new()),
    )
    .await
    .unwrap();
    (made.path, cli)
}

fn index_of(path: &Path) -> PackIndex {
    warden_packwiz::read_pack(path)
        .unwrap()
        .index
        .unwrap()
        .value
}

async fn save(pack: &Path, cli: &Packwiz, path: &str, text: &str) {
    let read = read_file(pack, ConfigOrigin::Pack, path).unwrap();
    save_to_pack(pack, path, text, &read.hash, cli, &CancellationToken::new())
        .await
        .unwrap();
}

fn install(temp: &Path, pack: &Path, into: &Path) {
    let Some((java, bootstrap, main)) = installer() else {
        return;
    };
    let output = Command::new(java)
        .arg("-jar")
        .arg(bootstrap)
        .arg("--bootstrap-no-update")
        .arg("--bootstrap-main-jar")
        .arg(main)
        .args(["-g", "-s", "client", "--pack-folder"])
        .arg(into)
        .arg(pack.join("pack.toml"))
        .current_dir(temp)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn preserve_sobrevive_ao_refresh_e_o_installer_real_mantem_o_arquivo_do_jogador() {
    let Some(binary) = binary() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary).await;
    let cancel = CancellationToken::new();
    assert_eq!(
        preserve_of(&pack, "options.txt").unwrap(),
        PreserveState {
            in_index: true,
            preserve: false
        }
    );

    assert!(
        set_preserve(&pack, "options.txt", true, &cli, &cancel)
            .await
            .unwrap()
    );
    assert!(
        !set_preserve(&pack, "options.txt", true, &cli, &cancel)
            .await
            .unwrap()
    );
    assert!(index_of(&pack).entry("options.txt").unwrap().preserve);

    // Primeira instalação: o jogador recebe o options.txt do pack.
    let player = temp.path().join("jogador");
    install(temp.path(), &pack, &player);
    let installed = installer().is_some();
    if installed {
        assert_eq!(
            fs::read_to_string(player.join("options.txt")).unwrap(),
            "renderDistance:12\nfov:0.5\n"
        );
        // O jogador muda as próprias preferências.
        fs::write(player.join("options.txt"), "renderDistance:6\n").unwrap();
    }

    // O autor muda o options.txt e outra config; o refresh roda nas duas gravações.
    save(&pack, &cli, "options.txt", "renderDistance:16\nfov:0.5\n").await;
    save(&pack, &cli, "config/ajuste.toml", "a = 2\n").await;
    cli.refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap();
    let index = index_of(&pack);
    assert!(
        index.entry("options.txt").unwrap().preserve,
        "o refresh apagou o preserve"
    );
    assert!(!index.entry("config/ajuste.toml").unwrap().preserve);

    // Atualização: o options.txt do jogador fica; a outra config chega.
    install(temp.path(), &pack, &player);
    if installed {
        assert_eq!(
            fs::read_to_string(player.join("options.txt")).unwrap(),
            "renderDistance:6\n"
        );
        assert_eq!(
            fs::read_to_string(player.join("config/ajuste.toml")).unwrap(),
            "a = 2\n"
        );
    }

    // Desligar volta a substituir.
    assert!(
        set_preserve(&pack, "options.txt", false, &cli, &cancel)
            .await
            .unwrap()
    );
    assert!(!index_of(&pack).entry("options.txt").unwrap().preserve);
    install(temp.path(), &pack, &player);
    if installed {
        assert_eq!(
            fs::read_to_string(player.join("options.txt")).unwrap(),
            "renderDistance:16\nfov:0.5\n"
        );
    }
}

#[tokio::test]
async fn arquivo_fora_do_indice_nao_tem_preserve() {
    let Some(binary) = binary() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary).await;
    write(&pack, "config/novo.toml", b"a = 1\n");
    assert!(!preserve_of(&pack, "config/novo.toml").unwrap().in_index);
    let error = set_preserve(
        &pack,
        "config/novo.toml",
        true,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, Code::InvalidInput);
}

#[tokio::test]
async fn copia_para_defaultconfigs_pede_confirmacao_para_substituir() {
    let Some(binary) = binary() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let (pack, cli) = real_pack(temp.path(), binary).await;
    let cancel = CancellationToken::new();
    let source = "config/create-server.toml";
    let target = "defaultconfigs/create-server.toml".to_owned();

    let copied = copy_to_defaultconfigs(&pack, source, false, &cli, &cancel)
        .await
        .unwrap();
    assert_eq!(
        copied,
        CopyOutcome::Copied {
            target: target.clone()
        }
    );
    assert_eq!(
        fs::read(pack.join(&target)).unwrap(),
        fs::read(pack.join(source)).unwrap()
    );
    assert!(index_of(&pack).entry(&target).is_some());
    assert_eq!(
        copy_to_defaultconfigs(&pack, source, false, &cli, &cancel)
            .await
            .unwrap(),
        CopyOutcome::AlreadySame {
            target: target.clone()
        }
    );

    save(&pack, &cli, source, "#Velocidade\nspeed = 3\n").await;
    assert_eq!(
        copy_to_defaultconfigs(&pack, source, false, &cli, &cancel)
            .await
            .unwrap(),
        CopyOutcome::NeedsOverwrite {
            target: target.clone()
        }
    );
    assert_eq!(
        fs::read_to_string(pack.join(&target)).unwrap(),
        "#Velocidade\nspeed = 2\n"
    );
    assert_eq!(
        copy_to_defaultconfigs(&pack, source, true, &cli, &cancel)
            .await
            .unwrap(),
        CopyOutcome::Copied {
            target: target.clone()
        }
    );
    assert_eq!(
        fs::read_to_string(pack.join(&target)).unwrap(),
        "#Velocidade\nspeed = 3\n"
    );

    let error = copy_to_defaultconfigs(&pack, "config/ajuste.toml", false, &cli, &cancel)
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::InvalidInput);
}
