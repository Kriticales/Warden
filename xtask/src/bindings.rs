//! `cargo xtask bindings [--check]`: gera o `apps/desktop/src/lib/ipc/bindings.ts` a partir
//! dos comandos e tipos da `warden-app` (`tauri-specta`; ADR-0018).
//!
//! Com `--check`, gera numa pasta temporária e falha se o resultado for diferente do arquivo
//! versionado (ROADMAP F0-01, critério 4).

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use crate::util::{Cmd, desktop_dir};

/// Caminho do arquivo gerado.
pub fn bindings_path() -> PathBuf {
    desktop_dir()
        .join("src")
        .join("lib")
        .join("ipc")
        .join("bindings.ts")
}

fn export_to(path: &Path) -> Result<()> {
    Cmd::cargo()
        .args([
            "run",
            "--quiet",
            "--locked",
            "-p",
            "warden-app",
            "--example",
            "export_bindings",
            "--",
        ])
        .args([path])
        .run()
}

/// `cargo xtask bindings`.
pub fn run(check: bool) -> Result<()> {
    let versioned = bindings_path();
    if !check {
        export_to(&versioned)?;
        println!("bindings: {} atualizado.", versioned.display());
        return Ok(());
    }
    let dir = tempfile::tempdir().context("falha ao criar pasta temporária")?;
    let generated = dir.path().join("bindings.ts");
    export_to(&generated)?;
    let new = std::fs::read(&generated).context("falha ao ler o bindings.ts gerado")?;
    let old = std::fs::read(&versioned).unwrap_or_default();
    if new != old {
        bail!(
            "bindings: {} está desatualizado. Rode `cargo xtask bindings` e inclua o arquivo no \
             mesmo commit da mudança (QUALITY §7.2).",
            versioned.display()
        );
    }
    println!("bindings: {} está atualizado.", versioned.display());
    Ok(())
}
