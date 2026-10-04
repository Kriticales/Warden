//! `cargo xtask <tarefa>`: automação do desenvolvimento do Warden (ARCHITECTURE §2 e §3).
//!
//! Tudo em Rust, para funcionar igual no PowerShell do Windows e no Linux da CI, sem bash
//! (ADR-0048). O registro de subcomandos (`Command` e o `match` em `main`) é acréscimo-apenas
//! (ROADMAP §1): cada tarefa acrescenta a sua variante e a sua linha.

mod bindings;
mod check;
mod coverage;
mod deps;
mod dev;
mod docs;
mod env_file;
mod network;
mod setup;
mod util;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cargo xtask", about = "Automação do desenvolvimento do Warden")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Instala as ferramentas do cargo (versões fixadas) e as dependências do pnpm.
    Setup,
    /// Abre o app em desenvolvimento, com pastas e cofre de teste próprios deste worktree.
    Dev {
        /// Argumentos repassados ao `tauri dev` (depois de `--`).
        #[arg(last = true)]
        extra: Vec<String>,
    },
    /// Portão de qualidade (QUALITY §12).
    Check {
        /// Só formatação, clippy, testes das crates alteradas, lint e typecheck.
        #[arg(long)]
        fast: bool,
    },
    /// Falha se alguma crate fora da warden-app depender do Tauri.
    CheckDeps,
    /// Confere os links internos dos arquivos Markdown.
    CheckDocs,
    /// Gera o bindings.ts a partir dos comandos da warden-app.
    Bindings {
        /// Só confere se o arquivo versionado está atualizado.
        #[arg(long)]
        check: bool,
    },
    /// Cobertura de testes com os mínimos da QUALITY §4.2.
    Coverage,
    /// Testes contra APIs reais (`#[ignore = "rede"]`, nome `rede_*`), com o .env.
    TestNetwork,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Setup => setup::run(),
        Command::Dev { extra } => dev::run(&extra),
        Command::Check { fast } => check::run(fast),
        Command::CheckDeps => deps::run(),
        Command::CheckDocs => docs::run(),
        Command::Bindings { check } => bindings::run(check),
        Command::Coverage => coverage::run(),
        Command::TestNetwork => network::run(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("\nerro: {error:#}");
            ExitCode::FAILURE
        }
    }
}
