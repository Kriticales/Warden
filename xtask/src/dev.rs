//! `cargo xtask dev`: abre o app em modo de desenvolvimento (`tauri dev`), isolado das pastas
//! e do cofre reais do dono (ADR-0048; QUALITY §13.6).
//!
//! Define, para o processo do app:
//! - `WARDEN_DATA_ROOT`: `%LOCALAPPDATA%\Warden-dev\<worktree>\` no Windows
//!   (`$XDG_DATA_HOME/Warden-dev/<worktree>/` ou `~/.local/share/...` no Linux);
//! - `WARDEN_SECRET_BACKEND=file:<WARDEN_DATA_ROOT>/cofre-de-teste`;
//! - as variáveis do `.env` do repositório principal, sem imprimir valores.
//!
//! Quem lê `WARDEN_DATA_ROOT` e `WARDEN_SECRET_BACKEND` é o app, a partir da F0-05.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

use crate::env_file;
use crate::util::{Cmd, desktop_dir, warn_long_target, workspace_root};

/// Pasta base dos dados de desenvolvimento, conforme a plataforma.
fn dev_base() -> Result<PathBuf> {
    if cfg!(windows) {
        let local = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA não definida")?;
        return Ok(PathBuf::from(local).join("Warden-dev"));
    }
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(data).join("Warden-dev"));
    }
    let home = std::env::var_os("HOME").context("HOME não definida")?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("Warden-dev"))
}

/// Nome do worktree: a pasta da raiz do workspace.
pub fn worktree_name(root: &Path) -> String {
    root.file_name().map_or_else(
        || "warden".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Valor de `WARDEN_SECRET_BACKEND` para uma pasta de dados.
pub fn secret_backend(data_root: &Path) -> OsString {
    let mut value = OsString::from("file:");
    value.push(data_root.join("cofre-de-teste"));
    value
}

/// `cargo xtask dev [-- <argumentos do tauri dev>]`.
pub fn run(extra: &[String]) -> Result<()> {
    warn_long_target();
    let root = workspace_root();
    if !desktop_dir().join("node_modules").exists() {
        bail!("dev: dependências do frontend ausentes. Rode `cargo xtask setup` antes.");
    }

    let data_root = dev_base()?.join(worktree_name(&root));
    let vault = data_root.join("cofre-de-teste");
    std::fs::create_dir_all(&vault)
        .with_context(|| format!("falha ao criar {}", vault.display()))?;
    println!("dev: WARDEN_DATA_ROOT = {}", data_root.display());
    println!("dev: WARDEN_SECRET_BACKEND = file:{}", vault.display());

    let mut cmd = Cmd::cargo()
        .cwd(desktop_dir())
        .args(["tauri", "dev"])
        .args(extra)
        .env("WARDEN_DATA_ROOT", &data_root)
        .env("WARDEN_SECRET_BACKEND", secret_backend(&data_root));

    let env_path = env_file::main_repo_env_path()?;
    match env_file::load(&env_path)? {
        Some(vars) if !vars.is_empty() => {
            println!(
                "dev: variáveis carregadas do .env: {}",
                vars.names().join(", ")
            );
            for (name, value) in vars.iter() {
                cmd = cmd.env(name, value);
            }
        }
        _ => println!(
            "dev: sem .env em {}; seguindo sem chaves.",
            env_path.display()
        ),
    }
    cmd.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_do_worktree_e_a_pasta_da_raiz() {
        let root = PathBuf::from("C:/Users/x/orca/workspaces/Warden/feat-f0-01-esqueleto");
        assert_eq!(worktree_name(&root), "feat-f0-01-esqueleto");
    }

    #[test]
    fn cofre_de_teste_fica_dentro_da_pasta_de_dados() {
        let data_root = PathBuf::from("dados");
        let expected = format!("file:{}", data_root.join("cofre-de-teste").display());
        assert_eq!(secret_backend(&data_root).to_string_lossy(), expected);
    }

    #[test]
    fn pasta_base_nunca_e_a_do_app_instalado() {
        let base = dev_base().unwrap();
        assert!(base.ends_with("Warden-dev"));
        assert!(!base.to_string_lossy().contains("dev.kriticales.warden"));
    }
}
