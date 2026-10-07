//! Descoberta dos subcomandos em `src/tasks/*.rs` (ARCHITECTURE §3; ROADMAP §1).
//!
//! Cada arquivo `src/tasks/<nome_com_underscore>.rs` vira o subcomando `cargo xtask
//! <nome-com-hifen>`, sem editar `main.rs`. Convenção do arquivo:
//!
//! - a primeira linha `//!` é a descrição do subcomando (`--help`);
//! - `pub struct Args` com `#[derive(clap::Args)]` (pode ser vazia);
//! - `pub fn run(args: Args) -> anyhow::Result<()>`.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/tasks");
    let manifest =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let tasks_dir = manifest.join("src").join("tasks");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&tasks_dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension().is_some_and(|ext| ext == "rs")
                        && path.file_stem().is_some_and(|stem| stem != "mod")
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();

    let mut modules = String::new();
    let mut variants = String::new();
    let mut arms = String::new();
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let name = file
            .file_stem()
            .expect("nome do arquivo")
            .to_string_lossy()
            .into_owned();
        let source = std::fs::read_to_string(file).expect("falha ao ler a tarefa");
        let about = source
            .lines()
            .find_map(|line| line.strip_prefix("//!"))
            .map_or("", str::trim);
        let pascal: String = name
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                    .unwrap_or_default()
            })
            .collect();
        let command = name.replace('_', "-");
        let path = file.to_string_lossy().replace('\\', "/");
        let _ = writeln!(modules, "#[path = {path:?}]\npub mod {name};");
        let _ = writeln!(
            variants,
            "    #[command(name = {command:?}, about = {about:?})]\n    {pascal}({name}::Args),"
        );
        let _ = writeln!(
            arms,
            "        AutoCommand::{pascal}(args) => {name}::run(args),"
        );
    }
    let out = format!(
        "// Gerado pelo build.rs. Não edite.\n{modules}\n\
         #[derive(clap::Subcommand)]\npub enum AutoCommand {{\n{variants}}}\n\n\
         pub fn dispatch(command: AutoCommand) -> anyhow::Result<()> {{\n    \
         match command {{\n{arms}    }}\n}}\n"
    );
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    std::fs::write(out_dir.join("tasks_registry.rs"), out).expect("falha ao gravar o registro");
}
