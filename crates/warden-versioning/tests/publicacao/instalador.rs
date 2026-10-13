//! CA-T18-03: o packwiz-installer-bootstrap **real**, apontado para o `pack.toml` publicado
//! (servido pelo servidor local que imita o `raw.githubusercontent.com` a partir do
//! repositório remoto de teste), instala o pack numa pasta vazia; depois de publicar a versão
//! seguinte, rodar de novo atualiza a pasta.
//!
//! Usa o Java e os jars fixados pelo `cargo xtask installer` (`externals.json` em
//! `WARDEN_INSTALLER_CACHE`, como os testes de conformidade da E-01). Sem eles, pula;
//! com `WARDEN_REQUIRE_EXTERNALS=1` (CI), falha.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Ctx, save};

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

fn install(externals: &(PathBuf, PathBuf, PathBuf), folder: &Path, link: &str) {
    let (java, bootstrap, installer) = externals;
    let output = Command::new(java)
        .arg("-jar")
        .arg(bootstrap)
        .arg("--bootstrap-no-update")
        .arg("--bootstrap-main-jar")
        .arg(installer)
        .args(["-g", "-s", "client", "--pack-folder"])
        .arg(folder)
        .arg(link)
        .current_dir(folder.parent().unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "bootstrap falhou:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn ca_t18_03_bootstrap_real_instala_e_atualiza_pelo_link_publicado() {
    let Some(externals) = externals() else {
        assert_ne!(
            std::env::var("WARDEN_REQUIRE_EXTERNALS").ok().as_deref(),
            Some("1"),
            "instalador real ausente; rode cargo xtask installer"
        );
        return;
    };
    let ctx = Ctx::new().await;
    let pack = &ctx.pack;
    pack.write("config/ajuste.toml", b"valor = 1\r\n");
    pack.write("config/sai.toml", b"x = 1\n");
    save(&ctx.repo(), pack, "1.0.0", 10, true);
    let first = ctx.publish("1.0.0", "").await.unwrap();

    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("instancia");
    fs::create_dir(&folder).unwrap();
    // O processo Java é bloqueante; o servidor local roda em outras threads.
    let link = first.pack_link.clone();
    let (ext, dir) = (externals.clone(), folder.clone());
    tokio::task::spawn_blocking(move || install(&ext, &dir, &link))
        .await
        .unwrap();
    assert_eq!(
        fs::read(folder.join("config/ajuste.toml")).unwrap(),
        b"valor = 1\r\n"
    );
    assert!(folder.join("config/sai.toml").is_file());

    pack.write("config/ajuste.toml", b"valor = 2\r\n");
    pack.remove("config/sai.toml");
    pack.write("config/nova.toml", b"nova = true\n");
    save(&ctx.repo(), pack, "1.1.0", 20, true);
    let second = ctx.publish("1.1.0", "").await.unwrap();
    assert_eq!(second.pack_link, first.pack_link);
    let (ext, dir, link) = (externals.clone(), folder.clone(), second.pack_link);
    tokio::task::spawn_blocking(move || install(&ext, &dir, &link))
        .await
        .unwrap();
    assert_eq!(
        fs::read(folder.join("config/ajuste.toml")).unwrap(),
        b"valor = 2\r\n"
    );
    assert_eq!(
        fs::read(folder.join("config/nova.toml")).unwrap(),
        b"nova = true\n"
    );
    assert!(
        !folder.join("config/sai.toml").exists(),
        "o arquivo removido sai da pasta"
    );
}
