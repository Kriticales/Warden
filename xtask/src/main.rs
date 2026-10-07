//! `cargo xtask <tarefa>`: automação do desenvolvimento do Warden (ARCHITECTURE §2 e §3).
//!
//! Tudo em Rust, para funcionar igual no PowerShell do Windows e no Linux da CI, sem bash
//! (ADR-0048). O registro de subcomandos (`Command` e o `match` em `main`) é acréscimo-apenas
//! (ROADMAP §1): cada tarefa acrescenta a sua variante e a sua linha.

mod bindings;
mod check;
mod check_kits;
mod coverage;
mod deps;
mod dev;
mod docs;
mod e2e;
mod env_file;
mod fixtures_packwiz;
mod installer;
mod network;
mod notices;
mod packwiz;
mod preview;
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
    /// Confere os dados dos mods iniciais e dos kits contra a API do Modrinth (rede).
    CheckKits {
        /// `initial-mods.toml` a conferir (padrão: o do repositório).
        #[arg(long)]
        initial_mods: Option<std::path::PathBuf>,
        /// `kits.toml` a conferir (padrão: o do repositório).
        #[arg(long)]
        kits: Option<std::path::PathBuf>,
    },
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
    /// Gera os avisos de terceiros (Rust, npm, packwiz, fontes) exibidos em "Sobre o Warden".
    Notices,
    /// Versão de teste para o dono no Windows: gera (ou baixa da CI) o instalador, instala e abre.
    Preview {
        /// Baixa o instalador da última execução da CI com sucesso, em vez de compilar.
        #[arg(long)]
        from_ci: bool,
        /// Branch da CI (padrão: `main`).
        #[arg(long, requires = "from_ci")]
        branch: Option<String>,
        /// Pasta de instalação (padrão: `%LOCALAPPDATA%\Warden` ou a da instalação anterior).
        #[arg(long)]
        install_dir: Option<std::path::PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Setup => setup::run(),
        Command::Dev { extra } => dev::run(&extra),
        Command::Check { fast } => check::run(fast),
        Command::CheckDeps => deps::run(),
        Command::CheckDocs => docs::run(),
        Command::CheckKits { initial_mods, kits } => {
            check_kits::run(&check_kits::Options { initial_mods, kits })
        }
        Command::Bindings { check } => bindings::run(check),
        Command::Coverage => coverage::run(),
        Command::TestNetwork => network::run(),
        Command::BuildPackwiz { force } => packwiz::run(force),
        Command::FixturesPackwiz { packwiz, commit } => {
            fixtures_packwiz::run(fixtures_packwiz::Options { packwiz, commit })
        }
        Command::E2eDriver => e2e::run(),
        Command::Installer => installer::run(),
        Command::Notices => notices::run(),
        Command::Preview {
            from_ci,
            branch,
            install_dir,
        } => preview::run(&preview::Options {
            from_ci,
            branch,
            install_dir,
        }),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("\nerro: {error:#}");
            ExitCode::FAILURE
        }
    }
}
