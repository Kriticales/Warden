//! Script de build da `warden-app`: roda o `tauri-build` e grava o commit do código na
//! variável `WARDEN_COMMIT`, lida pelo comando `app_info`.
//!
//! O commit vem de `WARDEN_COMMIT` no ambiente do build (CI e versão de teste podem fixá-lo)
//! ou, sem ela, do `git rev-parse`. Sem git, o app mostra a versão sem commit.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=WARDEN_COMMIT");
    let commit = std::env::var("WARDEN_COMMIT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(commit_from_git)
        .unwrap_or_default();
    println!("cargo:rustc-env=WARDEN_COMMIT={}", commit.trim());

    build_tauri();
}

/// O `tauri-build` embute o manifesto do Windows (Common Controls v6) só nos executáveis
/// (`[[bin]]`). Sem ele, os binários de teste e de exemplo que ligam o Tauri nem abrem
/// (`STATUS_ENTRYPOINT_NOT_FOUND`). Por isso o manifesto vai pelo linker, para todos os alvos.
fn build_tauri() {
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if !msvc {
        tauri_build::build();
        return;
    }
    let manifest = absolute("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    let attributes = tauri_build::Attributes::new().windows_attributes(windows);
    if let Err(error) = tauri_build::try_build(attributes) {
        println!("cargo:warning=tauri-build falhou: {error:#}");
        std::process::exit(1);
    }
}

/// Lê o commit atual com `git` e pede nova execução do build quando o `HEAD` ou a ponta da
/// branch mudarem (vale também para worktrees, em que `.git` é um arquivo).
fn commit_from_git() -> Option<String> {
    for path in ["HEAD", "packed-refs"] {
        if let Some(file) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={}", absolute(&file).display());
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(file) = git(&["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={}", absolute(&file).display());
    }
    git(&["rev-parse", "--short=12", "HEAD"])
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// `git rev-parse --git-path` devolve caminhos relativos à pasta atual do build.
fn absolute(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return path;
    }
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .map_or(path.clone(), |dir| dir.join(&path))
}
