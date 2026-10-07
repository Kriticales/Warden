//! Descoberta dos comandos e eventos do IPC, usada pelo `build.rs` (ARCHITECTURE §4.1).
//!
//! Em vez de uma lista central em `commands/mod.rs` (que dava conflito em toda integração),
//! o build lê `src/commands/*.rs` e `src/events.rs` e gera `commands_registry.rs` na pasta de
//! saída: a declaração dos módulos, `collect_commands![…]` e `collect_events![…]`. Um comando
//! é uma `fn` logo abaixo de `#[tauri::command]` escrito na coluna 0; um evento é uma
//! `struct` logo abaixo de `#[tauri_specta(event_name = …)]`. Atributos indentados (módulos de
//! teste) são ignorados.
//!
//! Este arquivo também é compilado pelos testes da `warden-app` (`#[path]` em `lib.rs`).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Módulo de comandos: nome, arquivo, comandos e eventos em ordem de leitura.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CommandModule {
    pub(crate) name: String,
    pub(crate) file: PathBuf,
    pub(crate) commands: Vec<String>,
    pub(crate) events: Vec<String>,
}

/// Nome da `fn`/`struct` que vem depois da linha `start` (pula atributos, comentários e vazias).
fn next_item(lines: &[&str], start: usize, keyword: &str) -> Option<String> {
    let line = lines[start..]
        .iter()
        .map(|l| l.trim())
        .find(|l| !(l.is_empty() || l.starts_with('#') || l.starts_with("//")))?;
    let mut words = line.split_whitespace();
    while let Some(word) = words.next() {
        if word == keyword {
            let name = words.next()?;
            let end = name
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(name.len());
            return Some(name[..end].to_owned());
        }
        if !matches!(word, "pub" | "async" | "unsafe" | "const") && !word.starts_with("pub(") {
            return None;
        }
    }
    None
}

fn scan(source: &str, marker: &str, keyword: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with(marker))
        .filter_map(|(index, _)| next_item(&lines, index + 1, keyword))
        .collect()
}

/// Funções `#[tauri::command]` do texto, em ordem.
pub(crate) fn scan_commands(source: &str) -> Vec<String> {
    scan(source, "#[tauri::command]", "fn")
}

/// Structs de evento (`#[tauri_specta(event_name = …)]`) do texto, em ordem.
pub(crate) fn scan_events(source: &str) -> Vec<String> {
    scan(source, "#[tauri_specta(event_name", "struct")
}

/// Lê os módulos de `commands_dir`, em ordem alfabética (`x.rs` ou `x/mod.rs`; o `mod.rs` da
/// própria pasta não conta).
pub(crate) fn discover(commands_dir: &Path) -> std::io::Result<Vec<CommandModule>> {
    let mut modules = Vec::new();
    for entry in std::fs::read_dir(commands_dir)? {
        let path = entry?.path();
        let (name, file) = if path.is_dir() {
            let file = path.join("mod.rs");
            if !file.is_file() {
                continue;
            }
            (
                path.file_name().map(|n| n.to_string_lossy().into_owned()),
                file,
            )
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            (
                path.file_stem().map(|n| n.to_string_lossy().into_owned()),
                path.clone(),
            )
        } else {
            continue;
        };
        let Some(name) = name.filter(|name| name != "mod") else {
            continue;
        };
        let source = std::fs::read_to_string(&file)?;
        modules.push(CommandModule {
            commands: scan_commands(&source),
            events: scan_events(&source),
            name,
            file,
        });
    }
    modules.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(modules)
}

/// Texto de `commands_registry.rs`. `events_source` é o `src/events.rs` (eventos globais).
pub(crate) fn render(modules: &[CommandModule], events_source: &str) -> String {
    let mut out = String::from("// Gerado pelo build.rs (build_registry.rs). Não edite.\n\n");
    for module in modules {
        let path = module.file.to_string_lossy().replace('\\', "/");
        let _ = writeln!(out, "#[path = {path:?}]\npub(crate) mod {};", module.name);
    }
    out.push_str(
        "\npub(crate) fn commands() -> tauri_specta::Commands<tauri::Wry> {\n    \
         tauri_specta::collect_commands![\n",
    );
    for module in modules {
        for command in &module.commands {
            let _ = writeln!(out, "        {}::{command},", module.name);
        }
    }
    out.push_str(
        "    ]\n}\n\npub(crate) fn events() -> tauri_specta::Events {\n    \
         tauri_specta::collect_events![\n",
    );
    for event in scan_events(events_source) {
        let _ = writeln!(out, "        crate::events::{event},");
    }
    for module in modules {
        for event in &module.events {
            let _ = writeln!(out, "        {}::{event},", module.name);
        }
    }
    out.push_str("    ]\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
/// Docs.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)]
pub(crate) async fn app_info(state: State<'_, S>) -> Result<(), E> {}

// pendente-na-ui: X-01 motivo
#[tauri::command]
// comentário entre atributos
#[specta::specta]
pub(crate) fn dois() {}

mod tests {
    #[tauri::command]
    fn de_teste() {}
}

#[derive(Event)]
#[tauri_specta(event_name = \"pack-changed\")]
pub struct PackChanged { pub a: u8 }
";

    #[test]
    fn acha_comandos_na_coluna_zero_e_ignora_testes() {
        assert_eq!(scan_commands(SAMPLE), ["app_info", "dois"]);
    }

    #[test]
    fn acha_eventos() {
        assert_eq!(scan_events(SAMPLE), ["PackChanged"]);
    }

    #[test]
    fn gera_modulos_comandos_e_eventos() {
        let modules = vec![CommandModule {
            name: "app".into(),
            file: PathBuf::from("C:\\x\\app.rs"),
            commands: vec!["app_info".into()],
            events: vec![],
        }];
        let text = render(&modules, SAMPLE);
        assert!(text.contains("#[path = \"C:/x/app.rs\"]\npub(crate) mod app;"));
        assert!(text.contains("app::app_info,"));
        assert!(text.contains("crate::events::PackChanged,"));
    }
}
