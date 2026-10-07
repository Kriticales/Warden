//! Simula a integração de duas branches independentes (comando, i18n, xtask e bindings) e falha se o merge conflitar.
//!
//! Cria um worktree descartável a partir do `HEAD`, abre duas branches que acrescentam, cada
//! uma, um comando do IPC, um namespace de i18n e um subcomando do xtask, mais uma mexida no
//! mesmo trecho do `bindings.ts`, e faz `git merge`. Passa só se o merge terminar sem
//! conflito. Com `--compile`, também compila o resultado: regenera o bindings (precisa conter
//! os dois comandos) e confere que o xtask lista os dois subcomandos.
//!
//! Só olha o que está **commitado** no `HEAD`.

use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result, bail, ensure};

use crate::merge_driver;
use crate::util::{Cmd, desktop_dir, target_dir, workspace_root};

/// Argumentos de `cargo xtask check-integration`.
#[derive(clap::Args)]
pub struct Args {
    /// Também compila o merge (lento): gera o bindings e lista os subcomandos do xtask.
    #[arg(long)]
    pub compile: bool,
}

fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    Cmd::new("git")
        .cwd(cwd)
        .args([
            "-c",
            "user.name=check-integration",
            "-c",
            "user.email=check-integration@invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .read()
}

fn write(root: &Path, relative: &str, text: &str) -> Result<()> {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().context("caminho sem pasta")?)?;
    fs::write(&path, text).with_context(|| format!("falha ao gravar {relative}"))
}

/// Acrescenta, na branch atual do worktree, tudo o que uma tarefa independente acrescentaria.
fn add_feature(root: &Path, tag: &str) -> Result<()> {
    let snake = format!("integ_{tag}");
    let kebab = format!("integ-{tag}");
    let camel = format!("integ{}", tag.to_uppercase());
    write(
        root,
        &format!("apps/desktop/src-tauri/src/commands/{snake}.rs"),
        &format!(
            "//! Comando de exemplo da `check-integration` (descartável).\n\n\
             use crate::error::AppError;\n\n\
             // pendente-na-ui: SIM-{tag} exemplo da simulação\n\
             #[tauri::command]\n#[specta::specta]\n\
             #[allow(clippy::unnecessary_wraps)]\n\
             pub(crate) fn {snake}_ping() -> Result<(), AppError> {{\n    Ok(())\n}}\n"
        ),
    )?;
    write(
        root,
        &format!("apps/desktop/src/i18n/pt-BR/{kebab}.ts"),
        &format!(
            "export const {camel} = {{ titulo: 'Exemplo {tag}' }} as const;\n\n\
             declare module '../catalogo' {{\n  interface Catalogo {{\n    {camel}: typeof {camel};\n  }}\n}}\n"
        ),
    )?;
    write(
        root,
        &format!("xtask/src/tasks/{snake}.rs"),
        &format!(
            "//! Subcomando de exemplo da check-integration {tag}.\n\n\
             /// Sem argumentos.\n#[derive(clap::Args)]\npub struct Args {{}}\n\n\
             #[allow(clippy::unnecessary_wraps)]\n\
             pub fn run(_args: &Args) -> anyhow::Result<()> {{\n    Ok(())\n}}\n"
        ),
    )?;
    // Mexida no mesmo trecho do bindings.ts nas duas branches: conflito de texto garantido.
    let bindings = root.join("apps/desktop/src/lib/ipc/bindings.ts");
    let text = fs::read_to_string(&bindings)?;
    let (head, tail) = text
        .split_once("\n};\n")
        .context("bindings.ts sem o fim do objeto commands")?;
    let line = format!("\n/** simulação {tag} */\n{camel}Ping: () => null,");
    fs::write(&bindings, format!("{head}{line}\n}};\n{tail}"))?;
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-q", "-m", &format!("simulação {tag}")])?;
    Ok(())
}

fn simulate(work: &Path, compile: bool) -> Result<()> {
    git(work, &["switch", "-q", "-c", "xtask-sim-a"])?;
    add_feature(work, "a")?;
    git(work, &["switch", "-q", "--detach", "HEAD~1"])?;
    git(work, &["switch", "-q", "-c", "xtask-sim-b"])?;
    add_feature(work, "b")?;

    if let Err(error) = git(work, &["merge", "--no-edit", "xtask-sim-a"]) {
        let conflicts = git(work, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
        bail!(
            "o merge das duas branches de exemplo conflitou:\n{error:#}\narquivos em conflito:\n{conflicts}\n\
             Um registro compartilhado voltou a exigir edição manual (ARCHITECTURE §4.1)."
        );
    }
    for file in [
        "apps/desktop/src-tauri/src/commands/integ_a.rs",
        "apps/desktop/src-tauri/src/commands/integ_b.rs",
        "apps/desktop/src/i18n/pt-BR/integ-a.ts",
        "apps/desktop/src/i18n/pt-BR/integ-b.ts",
        "xtask/src/tasks/integ_a.rs",
        "xtask/src/tasks/integ_b.rs",
    ] {
        ensure!(work.join(file).is_file(), "{file} sumiu no merge");
    }
    println!("check-integration: merge sem conflito (bindings.ts resolvido pelo driver).");
    if compile {
        compile_merge(work)?;
    }
    Ok(())
}

/// Compila o resultado do merge e confere o que foi descoberto automaticamente.
fn compile_merge(work: &Path) -> Result<()> {
    // Os sidecars do packwiz não são versionados: copia os do worktree principal.
    let from = desktop_dir().join("src-tauri").join("binaries");
    let to = work.join("apps/desktop/src-tauri/binaries");
    for entry in fs::read_dir(&from)?.filter_map(Result::ok) {
        if entry.path().is_file() && entry.file_name() != ".gitignore" {
            fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    let target = target_dir()?;
    let cargo = || Cmd::cargo().cwd(work).env("CARGO_TARGET_DIR", &target);
    let out = tempfile::tempdir()?;
    let bindings = out.path().join("bindings.ts");
    cargo()
        .args(["run", "--quiet", "--locked", "-p", "warden-app", "--example", "export_bindings", "--"])
        .args([&bindings])
        .run()?;
    let generated = fs::read_to_string(&bindings)?;
    for name in ["integAPing: (", "integBPing: ("] {
        ensure!(generated.contains(name), "o bindings gerado não tem {name}");
    }
    // O xtask em execução trava o próprio .exe no Windows: o --help roda noutra pasta target.
    let help = Cmd::cargo()
        .cwd(work)
        .env("CARGO_TARGET_DIR", out.path().join("target-xtask"))
        .args(["run", "--quiet", "--locked", "-p", "xtask", "--", "--help"])
        .read()?;
    for name in ["integ-a", "integ-b"] {
        ensure!(help.contains(name), "o xtask não lista o subcomando {name}");
    }
    println!("check-integration: bindings e xtask do merge compilam com os dois acréscimos.");
    Ok(())
}

/// `cargo xtask check-integration`.
pub fn run(args: &Args) -> Result<()> {
    merge_driver::ensure()?;
    let root = workspace_root();
    let dir = tempfile::tempdir().context("falha ao criar pasta temporária")?;
    let work = dir.path().join("wt");
    let work_text = work.to_string_lossy().into_owned();
    git(&root, &["worktree", "add", "-q", "--detach", &work_text, "HEAD"])?;
    let result = simulate(&work, args.compile);
    // Limpeza sempre, mesmo com falha.
    let _ = git(&root, &["worktree", "remove", "--force", &work_text]);
    for branch in ["xtask-sim-a", "xtask-sim-b"] {
        let _ = git(&root, &["branch", "-D", branch]);
    }
    result
}
