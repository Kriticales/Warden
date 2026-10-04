//! `cargo xtask setup`: prepara a máquina para desenvolver o Warden, só no espaço do usuário
//! (QUALITY §13.1): ferramentas do cargo com versão fixada e dependências do pnpm.
//!
//! Idempotente: ferramenta já instalada na versão certa não é recompilada. Nada aqui usa
//! instalador, administrador ou muda configuração do sistema.

use anyhow::{Context as _, Result};

use crate::util::{Cmd, find_program, warn_long_target};

/// Ferramentas instaladas com `cargo install --locked`: (crate, versão).
pub const TOOLS: &[(&str, &str)] = &[
    ("cargo-nextest", "0.9.146"),
    ("cargo-deny", "0.20.2"),
    ("cargo-llvm-cov", "0.9.1"),
    ("tauri-cli", "2.12.1"),
    ("tauri-driver", "2.1.0"),
];

/// Lê a saída de `cargo install --list` (linhas `nome vX.Y.Z:`).
pub fn installed(list: &str) -> Vec<(String, String)> {
    list.lines()
        .filter(|line| !line.starts_with(char::is_whitespace))
        .filter_map(|line| {
            let mut parts = line.trim_end_matches(':').split_whitespace();
            let name = parts.next()?;
            let version = parts.next()?.strip_prefix('v')?;
            Some((name.to_owned(), version.to_owned()))
        })
        .collect()
}

/// `cargo xtask setup`.
pub fn run() -> Result<()> {
    warn_long_target();
    find_program("node").context("o Node 24 é pré-requisito (README.md)")?;
    find_program("pnpm").context("o pnpm 12 é pré-requisito (README.md)")?;

    let list = Cmd::cargo().args(["install", "--list"]).read()?;
    let present = installed(&list);
    for (name, version) in TOOLS {
        if present.iter().any(|(n, v)| n == name && v == version) {
            println!("setup: {name} {version} já instalado.");
            continue;
        }
        Cmd::cargo()
            .args(["install", name, "--version", version, "--locked"])
            .run()?;
    }

    Cmd::pnpm()?.args(["install", "--frozen-lockfile"]).run()?;
    println!("\nsetup: pronto. Próximos passos: `cargo xtask dev` ou `cargo xtask check`.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_a_lista_do_cargo_install() {
        let list = "cargo-nextest v0.9.146:\n    cargo-nextest.exe\ntauri-cli v2.11.4:\n    cargo-tauri.exe\nxtask v0.1.0 (/repo/xtask):\n    xtask.exe\n";
        assert_eq!(
            installed(list),
            [
                ("cargo-nextest".to_owned(), "0.9.146".to_owned()),
                ("tauri-cli".to_owned(), "2.11.4".to_owned()),
                ("xtask".to_owned(), "0.1.0".to_owned()),
            ]
        );
    }
}
