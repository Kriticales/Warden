//! CA-T19-01 a 03 com packwiz e packwiz-installer reais, quando presentes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(linker_messages)]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use warden_core::CancellationToken;
use warden_export::{ExportAlert, ExportFormat, ExportSource, export, preview};
use warden_packwiz::{PackManifest, read_pack};
use warden_packwiz_cli::{Packwiz, RunContext, check_conformance};
use warden_project::hygiene;
use warden_versioning::{Identity, InitialPoint, Moment, PackRepo};

fn sidecar() -> Option<PathBuf> {
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
        assert_ne!(
            std::env::var("WARDEN_REQUIRE_EXTERNALS").ok().as_deref(),
            Some("1")
        );
        None
    }
}

fn externals() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let dir = std::env::var_os("WARDEN_INSTALLER_CACHE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(|local| PathBuf::from(local).join("Warden-dev/cache/installer"))
        })?;
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("externals.json")).ok()?).ok()?;
    let get = |key: &str| value[key].as_str().map(PathBuf::from);
    let paths = (get("java")?, get("bootstrap")?, get("installer")?);
    (paths.0.is_file() && paths.1.is_file() && paths.2.is_file()).then_some(paths)
}

fn check_installer(temp: &Path, out: &Path) {
    if let Some((java, bootstrap, installer)) = externals() {
        let install = temp.join("instalado");
        let output = Command::new(java)
            .arg("-jar")
            .arg(bootstrap)
            .arg("--bootstrap-no-update")
            .arg("--bootstrap-main-jar")
            .arg(installer)
            .args(["-g", "-s", "client", "--pack-folder"])
            .arg(&install)
            .arg(out.join("pack.toml"))
            .current_dir(temp)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(install.join("config/a.txt")).unwrap(), b"ajuste");
    } else {
        assert_ne!(
            std::env::var("WARDEN_REQUIRE_EXTERNALS").ok().as_deref(),
            Some("1"),
            "instalador real ausente; rode cargo xtask installer"
        );
    }
}

#[test]
fn previa_entrega_alertas_tipados_para_a_interface() {
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path();
    fs::write(
        pack.join("pack.toml"),
        PackManifest::new("Teste", "1.21.1").to_toml_string(),
    )
    .unwrap();
    fs::create_dir_all(pack.join("config")).unwrap();
    fs::create_dir_all(pack.join("crash-reports")).unwrap();
    fs::write(pack.join("options.txt"), b"lang:pt_br").unwrap();
    fs::write(pack.join("extra.txt"), b"extra").unwrap();
    fs::write(
        pack.join("config/grande.bin"),
        vec![0; 20 * 1024 * 1024 + 1],
    )
    .unwrap();
    fs::write(pack.join("crash-reports/log.txt"), b"log").unwrap();
    let mut index = "hash-format = \"sha256\"\n".to_owned();
    for path in [
        "options.txt",
        "extra.txt",
        "config/grande.bin",
        "crash-reports/log.txt",
    ] {
        write!(
            &mut index,
            "\n[[files]]\nfile = \"{path}\"\nhash = \"{}\"\n",
            "0".repeat(64)
        )
        .unwrap();
    }
    fs::write(pack.join("index.toml"), index).unwrap();

    let result = preview(pack, &ExportSource::Current).unwrap();
    let alerts = |path: &str| {
        &result
            .files
            .iter()
            .find(|file| file.path == path)
            .unwrap()
            .alerts
    };
    assert!(alerts("config/grande.bin").contains(&ExportAlert::LargeFile));
    assert!(alerts("options.txt").contains(&ExportAlert::OptionsOverridesPreferences));
    assert!(alerts("extra.txt").contains(&ExportAlert::LooseRootFile));
    let finding = result
        .preflight
        .hygiene
        .iter()
        .find(|item| item.path == "crash-reports")
        .unwrap();
    for cause in &finding.reasons {
        assert!(
            alerts("crash-reports/log.txt").contains(&ExportAlert::Hygiene {
                cause: cause.clone(),
            })
        );
    }
    let wire = serde_json::to_value(&result.files).unwrap();
    assert_eq!(wire[0]["alerts"][0]["kind"], "largeFile");
}

async fn check_zip(pack: &Path, temp: &Path, cli: &Packwiz, cancel: &CancellationToken) {
    let zip_a = temp.join("a.zip");
    let zip_b = temp.join("b.zip");
    for target in [&zip_a, &zip_b] {
        export(
            pack,
            &ExportSource::Current,
            ExportFormat::Zip,
            target,
            &temp.join("staging"),
            cli,
            cancel,
        )
        .await
        .unwrap();
    }
    assert_eq!(fs::read(zip_a).unwrap(), fs::read(zip_b).unwrap());
}

#[tokio::test]
async fn pasta_exportada_e_instalada_sem_lixo() {
    let Some(binary) = sidecar() else {
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let pack = temp.path().join("pack");
    fs::create_dir_all(pack.join("config")).unwrap();
    fs::write(
        pack.join("pack.toml"),
        PackManifest::new("Teste", "1.21.1").to_toml_string(),
    )
    .unwrap();
    fs::write(
        pack.join("index.toml"),
        "hash-format = \"sha256\"\nfiles = []\n",
    )
    .unwrap();
    fs::write(pack.join("config/a.txt"), b"ajuste").unwrap();
    let cli = Packwiz::new(
        binary,
        temp.path().join("cache"),
        temp.path().join("config.toml"),
    );
    let cancel = CancellationToken::new();
    cli.refresh(&pack, false, RunContext::new(&cancel))
        .await
        .unwrap();
    PackRepo::init(
        &pack,
        InitialPoint::Created,
        &Identity::for_pack("Autor", None),
        Moment::now_utc(),
    )
    .unwrap();
    fs::create_dir(pack.join("crash-reports")).unwrap();
    fs::write(pack.join("crash-reports/x.txt"), b"lixo").unwrap();
    let before = preview(&pack, &ExportSource::Current).unwrap();
    assert!(
        before
            .preflight
            .hygiene
            .iter()
            .any(|item| item.path.starts_with("crash-reports"))
    );
    let chosen = before
        .preflight
        .hygiene
        .iter()
        .find(|item| item.path == "crash-reports")
        .or_else(|| {
            before
                .preflight
                .hygiene
                .iter()
                .find(|item| item.path == "crash-reports/x.txt")
        })
        .unwrap()
        .path
        .clone();
    hygiene::fix(&pack, &[chosen], "Autor", &cli, &cancel)
        .await
        .unwrap();
    assert!(!pack.join("crash-reports/x.txt").exists());
    let out = temp.path().join("saida");
    let result = export(
        &pack,
        &ExportSource::Current,
        ExportFormat::Folder,
        &out,
        &temp.path().join("staging"),
        &cli,
        &cancel,
    )
    .await
    .unwrap();
    let index = read_pack(&out).unwrap().index.unwrap().value;
    let mut expected = vec!["pack.toml".to_owned(), "index.toml".to_owned()];
    expected.extend(
        index
            .normalized_entries()
            .into_iter()
            .map(|entry| entry.file),
    );
    expected.sort();
    expected.dedup();
    assert_eq!(result.files, expected);
    assert!(!out.join("crash-reports/x.txt").exists());
    let report = check_conformance(
        &cli,
        &out,
        &temp.path().join("check"),
        RunContext::new(&cancel),
    )
    .await
    .unwrap();
    assert!(report.is_conforming(), "{:?}", report.differences);
    check_zip(&pack, temp.path(), &cli, &cancel).await;
    check_installer(temp.path(), &out);
}
