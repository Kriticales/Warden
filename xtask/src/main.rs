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
mod e2e;
mod env_file;
mod fixtures_packwiz;
mod installer;
mod network;
mod packwiz;
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
    /// Compila o sidecar do packwiz (commit fixado + patches) para Windows e Linux.
    BuildPackwiz {
        /// Compila mesmo que o commit e os patches não tenham mudado.
        #[arg(long)]
        force: bool,
    },
    /// Gera as fixtures da warden-packwiz com o packwiz real (rede e chave do .env).
    FixturesPackwiz {
        /// Binário do packwiz (padrão: `WARDEN_PACKWIZ_BIN` ou o da F0-03).
        #[arg(long)]
        packwiz: Option<std::path::PathBuf>,
        /// Commit do packwiz (padrão: `third_party/packwiz/COMMIT`).
        #[arg(long)]
        commit: Option<String>,
    },
    /// Baixa o packwiz-installer (e o bootstrap) e um JRE Temurin fixados, para a conformidade da L-03.
    Installer,
    /// Baixa o `msedgedriver` da versão do WebView2 (Windows) ou confere o `WebKitWebDriver` (Linux).
    E2eDriver,
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
        Command::BuildPackwiz { force } => packwiz::run(force),
        Command::FixturesPackwiz { packwiz, commit } => {
            fixtures_packwiz::run(fixtures_packwiz::Options { packwiz, commit })
        }
        Command::E2eDriver => e2e::run(),
        Command::Installer => installer::run(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("\nerro: {error:#}");
            ExitCode::FAILURE
        }
    }
}
