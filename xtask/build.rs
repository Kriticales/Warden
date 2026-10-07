//! Descoberta dos subcomandos em `src/tasks/*.rs` (ARCHITECTURE §3; ROADMAP §1).
//!
//! Cada arquivo `src/tasks/<nome_com_underscore>.rs` vira o subcomando `cargo xtask
//! <nome-com-hifen>`, sem editar `main.rs`. Convenção do arquivo:
//!
//! - a primeira linha `//!` é a descrição do subcomando (`--help`);
//! - `pub struct Args` com `#[derive(clap::Args)]` (pode ser vazia);
//! - `pub fn run(args: &Args) -> anyhow::Result<()>`.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    if let Err(error) = generate() {
        println!("cargo:warning=registro de subcomandos falhou: {error}");
        std::process::exit(1);
    }
}

/// `UpperCamelCase` de um nome em `snake_case`.
fn pascal(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect()
}

fn generate() -> Result<(), String> {
    println!("cargo:rerun-if-changed=src/tasks");
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").ok_or("CARGO_MANIFEST_DIR ausente")?;
    let tasks_dir = PathBuf::from(manifest).join("src").join("tasks");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&tasks_dir)
        .map_err(|error| format!("falha ao ler src/tasks: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "rs")
                && path.file_stem().is_some_and(|stem| stem != "mod")
        })
        .collect();
    files.sort();

    let mut modules = String::new();
    let mut variants = String::new();
    let mut arms = String::new();
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let name = file
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .ok_or("arquivo sem nome")?;
        let source = std::fs::read_to_string(file)
            .map_err(|error| format!("falha ao ler {}: {error}", file.display()))?;
        let about = source
            .lines()
            .find_map(|line| line.strip_prefix("//!"))
            .map_or("", str::trim);
        let variant = pascal(&name);
        let command = name.replace('_', "-");
        let path = file.to_string_lossy().replace('\\', "/");
        let _ = writeln!(modules, "#[path = {path:?}]\npub mod {name};");
        let _ = writeln!(
            variants,
            "    #[command(name = {command:?}, about = {about:?})]\n    {variant}({name}::Args),"
        );
        let _ = writeln!(
            arms,
            "        AutoCommand::{variant}(args) => {name}::run(args),"
        );
    }
    let out = format!(
        "// Gerado pelo build.rs. Não edite.\n{modules}\n\
         #[derive(clap::Subcommand)]\npub enum AutoCommand {{\n{variants}}}\n\n\
         pub fn dispatch(command: &AutoCommand) -> anyhow::Result<()> {{\n    \
         match command {{\n{arms}    }}\n}}\n"
    );
    let out_dir = std::env::var_os("OUT_DIR").ok_or("OUT_DIR ausente")?;
    std::fs::write(PathBuf::from(out_dir).join("tasks_registry.rs"), out)
        .map_err(|error| format!("falha ao gravar o registro: {error}"))
}
