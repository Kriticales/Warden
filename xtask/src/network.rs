//! `cargo xtask test-network`: roda os testes contra APIs reais (QUALITY §4.1).
//!
//! Convenção dos testes de rede: marcados `#[ignore = "rede"]` (ficam fora do `check`) e com
//! nome começando por `rede_`, que é o filtro usado aqui (o nextest não filtra pelo motivo do
//! `ignore`). As chaves vêm do `.env` do repositório principal, sem imprimir valores.

use anyhow::{Result, bail};

use crate::env_file;
use crate::util::Cmd;

/// Filtro do nextest para os testes de rede.
pub const FILTER: &str = "test(/(^|::)rede_/)";

/// `cargo xtask test-network`.
pub fn run() -> Result<()> {
    let path = env_file::main_repo_env_path()?;
    let Some(vars) = env_file::load(&path)? else {
        bail!(
            "test-network: {} não existe. Crie o .env com as chaves (CURSEFORGE_API_KEY, \
             GEMINI_API_KEY) entre aspas simples.",
            path.display()
        );
    };
    println!(
        "test-network: variáveis carregadas do .env: {}",
        vars.names().join(", ")
    );
    let mut cmd = Cmd::cargo().args([
        "nextest",
        "run",
        "--workspace",
        "--locked",
        "--run-ignored",
        "only",
        "--no-tests=warn",
        "-E",
        FILTER,
    ]);
    for (name, value) in vars.iter() {
        cmd = cmd.env(name, value);
    }
    cmd.run()
}
