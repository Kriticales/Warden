//! Descoberta dos comandos e eventos do IPC, usada pelo `build.rs` (ARCHITECTURE §4.1).
//!
//! Em vez de uma lista central em `commands/mod.rs` (que dava conflito em toda integração),
//! o build lê `src/commands/*.rs` e `src/events.rs` e gera `commands_registry.rs` na pasta de
//! saída: a declaração dos módulos, `collect_commands![…]` e `collect_events![…]`.
//!
//! Formato exigido (qualquer outro faz o build falhar com a linha e o formato certo, em vez
//! de deixar o comando de fora em silêncio):
//!
//! - comando: a linha exata `#[tauri::command]`, na coluna 0 e sozinha, seguida (depois de
//!   outros atributos, comentários ou linhas vazias) de uma `fn` no nível do arquivo, como
//!   `pub(crate) async fn nome(…)`. Argumentos no atributo (`rename_all`, `async`…), atributo
//!   indentado (dentro de `mod`/`impl`) ou na mesma linha da `fn` são recusados;
//! - evento: uma linha que começa com `#[tauri_specta(event_name`, na coluna 0, seguida de uma
//!   `struct` no nível do arquivo.
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

/// O que procurar: o atributo que marca o item e a palavra-chave do item.
struct Marker {
    /// Começo do atributo, sem espaços (`#[tauri::command`).
    attribute: &'static str,
    /// A linha tem de ser exatamente esta (`None`: basta começar com `attribute`).
    exact: Option<&'static str>,
    keyword: &'static str,
    /// Formato exigido, para a mensagem de erro.
    format: &'static str,
}

const COMMAND: Marker = Marker {
    attribute: "#[tauri::command",
    exact: Some("#[tauri::command]"),
    keyword: "fn",
    format: "a linha exata `#[tauri::command]` na coluna 0, sem argumentos, com a `fn` \
             (no nível do arquivo) nas linhas seguintes",
};

const EVENT: Marker = Marker {
    attribute: "#[tauri_specta(event_name",
    exact: None,
    keyword: "struct",
    format: "`#[tauri_specta(event_name = \"…\")]` na coluna 0, com a `struct` (no nível do \
             arquivo) nas linhas seguintes",
};

/// Remove a visibilidade do começo (`pub`, `pub(crate)`, `pub(in a::b)`…).
fn strip_visibility(line: &str) -> &str {
    let Some(rest) = line.strip_prefix("pub") else {
        return line;
    };
    let rest_trimmed = rest.trim_start();
    if let Some(inner) = rest_trimmed.strip_prefix('(') {
        return inner
            .find(')')
            .map_or(line, |end| inner[end + 1..].trim_start());
    }
    if rest.starts_with(char::is_whitespace) {
        return rest_trimmed;
    }
    line
}

/// Nome do item `keyword` na linha (`pub(crate) async fn nome(…)` → `nome`).
fn item_name(line: &str, keyword: &str) -> Option<String> {
    let mut words = strip_visibility(line).split_whitespace();
    while let Some(word) = words.next() {
        if word == keyword {
            let name = words.next()?;
            let end = name
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(name.len());
            return (end > 0).then(|| name[..end].to_owned());
        }
        if !matches!(word, "async" | "unsafe" | "const") {
            return None;
        }
    }
    None
}

/// Nome do item que vem depois da linha `start` (pula atributos, comentários e vazias). O item
/// tem de estar na coluna 0, como o atributo.
fn next_item(lines: &[&str], start: usize, keyword: &str) -> Option<String> {
    let line = lines[start..].iter().find(|l| {
        let l = l.trim();
        !(l.is_empty() || l.starts_with('#') || l.starts_with("//"))
    })?;
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    item_name(line, keyword)
}

fn scan(source: &str, marker: &Marker) -> Result<Vec<String>, String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut names = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        let compact = compact
            .strip_prefix("#[::")
            .map_or(compact.clone(), |rest| format!("#[{rest}"));
        if !compact.starts_with(marker.attribute) {
            continue;
        }
        let well_formed = match marker.exact {
            Some(exact) => *line == exact,
            None => line.starts_with(marker.attribute),
        };
        let name = if well_formed {
            next_item(&lines, index + 1, marker.keyword)
        } else {
            None
        };
        match name {
            Some(name) => names.push(name),
            None => {
                return Err(format!(
                    "linha {}: `{}` fora do formato. O registro exige {} (ARCHITECTURE §4.1)",
                    index + 1,
                    line.trim(),
                    marker.format
                ));
            }
        }
    }
    Ok(names)
}

/// Funções `#[tauri::command]` do texto, em ordem. Erro se algum atributo fugir do formato.
pub(crate) fn scan_commands(source: &str) -> Result<Vec<String>, String> {
    scan(source, &COMMAND)
}

/// Structs de evento (`#[tauri_specta(event_name = …)]`) do texto, em ordem. Erro se algum
/// atributo fugir do formato.
pub(crate) fn scan_events(source: &str) -> Result<Vec<String>, String> {
    scan(source, &EVENT)
}

/// Lê os módulos de `commands_dir`, em ordem alfabética (`x.rs` ou `x/mod.rs`; o `mod.rs` da
/// própria pasta não conta).
pub(crate) fn discover(commands_dir: &Path) -> Result<Vec<CommandModule>, String> {
    let read_error =
        |error: std::io::Error| format!("falha ao ler {}: {error}", commands_dir.display());
    let mut modules = Vec::new();
    for entry in std::fs::read_dir(commands_dir).map_err(read_error)? {
        let path = entry.map_err(read_error)?.path();
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
        let source = std::fs::read_to_string(&file)
            .map_err(|error| format!("falha ao ler {}: {error}", file.display()))?;
        let in_file = |error: String| format!("{}, {error}", file.display());
        modules.push(CommandModule {
            commands: scan_commands(&source).map_err(in_file)?,
            events: scan_events(&source).map_err(in_file)?,
            name,
            file,
        });
    }
    modules.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(modules)
}

/// Texto de `commands_registry.rs`. `events_source` é o `src/events.rs` (eventos globais).
pub(crate) fn render(modules: &[CommandModule], events_source: &str) -> Result<String, String> {
    let global_events =
        scan_events(events_source).map_err(|error| format!("src/events.rs, {error}"))?;
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
    for event in global_events {
        let _ = writeln!(out, "        crate::events::{event},");
    }
    for module in modules {
        for event in &module.events {
            let _ = writeln!(out, "        {}::{event},", module.name);
        }
    }
    out.push_str("    ]\n}\n");
    Ok(out)
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

pub(in crate::commands) fn dois() {}

//! Um comentário que cita `#[tauri::command]` não conta.
#[tauri::command]
fn tres() {}

#[derive(Event)]
#[tauri_specta(event_name = \"pack-changed\")]
pub struct PackChanged { pub a: u8 }
";

    #[test]
    fn acha_comandos_em_ordem() {
        assert_eq!(scan_commands(SAMPLE).unwrap(), ["app_info", "dois", "tres"]);
    }

    #[test]
    fn acha_eventos() {
        assert_eq!(scan_events(SAMPLE).unwrap(), ["PackChanged"]);
    }

    #[test]
    fn tira_a_visibilidade() {
        for line in [
            "fn a()",
            "pub fn a()",
            "pub(crate) fn a()",
            "pub(super) async fn a()",
            "pub(in crate::x) fn a()",
            "pub ( crate ) unsafe fn a()",
        ] {
            assert_eq!(item_name(line, "fn").as_deref(), Some("a"), "{line}");
        }
        assert_eq!(item_name("pubfn a()", "fn"), None);
        assert_eq!(item_name("struct A;", "fn"), None);
    }

    /// Formas que o registro não aceita: tem de falhar (com a linha), nunca ignorar.
    #[test]
    fn recusa_comando_fora_do_formato() {
        let casos = [
            (
                "#[tauri::command(rename_all = \"snake_case\")]\nfn a() {}\n",
                1,
            ),
            ("#[tauri::command(async)]\nfn a() {}\n", 1),
            ("#[tauri::command ]\nfn a() {}\n", 1),
            ("#[ tauri::command ]\nfn a() {}\n", 1),
            ("#[::tauri::command]\nfn a() {}\n", 1),
            ("#[tauri::command] pub fn a() {}\n", 1),
            ("#[tauri::command] // nota\nfn a() {}\n", 1),
            ("mod m {\n    #[tauri::command]\n    fn a() {}\n}\n", 2),
            ("impl X {\n\t#[tauri::command]\n\tfn a() {}\n}\n", 2),
            ("#[tauri::command]\n    fn a() {}\n", 1),
            ("#[tauri::command]\nstruct A;\n", 1),
            ("#[tauri::command]\n", 1),
            ("fn ok() {}\n\n#[tauri::command]\nmacro_rules! x {}\n", 3),
        ];
        for (source, linha) in casos {
            let erro = scan_commands(source).expect_err(source);
            assert!(
                erro.starts_with(&format!("linha {linha}:")),
                "{source:?} → {erro}"
            );
            assert!(erro.contains("`#[tauri::command]` na coluna 0"), "{erro}");
        }
    }

    #[test]
    fn recusa_evento_fora_do_formato() {
        for source in [
            "mod m {\n    #[tauri_specta(event_name = \"x\")]\n    struct A;\n}\n",
            "#[tauri_specta(event_name = \"x\")]\nfn a() {}\n",
            "#[tauri_specta(event_name = \"x\")] pub struct A;\n",
        ] {
            let erro = scan_events(source).expect_err(source);
            assert!(erro.contains("event_name"), "{erro}");
        }
    }

    #[test]
    fn erro_do_descobridor_cita_o_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("ruim.rs"),
            "#[tauri::command(async)]\nfn a() {}\n",
        )
        .unwrap();
        let erro = discover(dir.path()).unwrap_err();
        assert!(erro.contains("ruim.rs"), "{erro}");
        assert!(erro.contains("linha 1"), "{erro}");
    }

    #[test]
    fn descobre_os_modulos_reais() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands");
        let modules = discover(&dir).unwrap();
        let app = modules.iter().find(|m| m.name == "app").unwrap();
        assert!(app.commands.contains(&"app_info".to_owned()));
        assert!(modules.iter().all(|m| m.name != "mod"));
        let names: Vec<_> = modules.iter().map(|m| m.name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn gera_modulos_comandos_e_eventos() {
        let modules = vec![CommandModule {
            name: "app".into(),
            file: PathBuf::from("C:\\x\\app.rs"),
            commands: vec!["app_info".into()],
            events: vec![],
        }];
        let text = render(&modules, SAMPLE).unwrap();
        assert!(text.contains("#[path = \"C:/x/app.rs\"]\npub(crate) mod app;"));
        assert!(text.contains("app::app_info,"));
        assert!(text.contains("crate::events::PackChanged,"));
    }
}
