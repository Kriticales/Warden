//! Erros e avisos dos scripts lidos dos logs da instância de teste (ARCHITECTURE §10.2;
//! SPEC T26 "Erros dos scripts").
//!
//! Formatos (conferidos no código do KubeJS 6 e 7, `ConsoleJS.writeToFile`, e em logs reais
//! gerados por um servidor Fabric 1.20.1 em `tests/reais/`):
//!
//! - KubeJS, `logs/kubejs/{startup,server,client}.log`:
//!   `[05:43:00] [ERROR] ! receitas.js#16: Failed to create recipe …` e
//!   `[05:43:02] [WARN] receitas.js#10: …`. O arquivo é relativo à pasta do tipo
//!   (`server_scripts/`); as linhas do rastro de pilha repetem o prefixo e são puladas.
//! - CraftTweaker moderno, `logs/crafttweaker.log`:
//!   `[05:43:03.347][ERROR][CraftTweaker-ZenCode]: Could not get item …` e, nas linhas da
//!   exceção, `org.openzen…ParseException: receitas.zs:2:107: Invalid parameters …`.
//! - CraftTweaker do 1.12.2, `crafttweaker.log` na raiz: `[INITIALIZATION][SERVER][ERROR] …`,
//!   com `receitas.zs:5` quando há posição. A leitura é tolerante: qualquer grupo `[ERROR]` ou
//!   `[WARN]` no começo da linha vale.
//!
//! Só o fim de cada log é lido ([`MAX_LOG_BYTES`]) e no máximo [`MAX_PROBLEMS`] itens saem.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use regex::Regex;
use serde::Serialize;

use crate::detect::KubejsScriptType;

/// Quanto do fim de cada log é lido.
pub const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;
/// Quantos itens a lista guarda.
pub const MAX_PROBLEMS: usize = 500;

/// Gravidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProblemLevel {
    /// Erro.
    Error,
    /// Aviso.
    Warning,
}

/// Um erro ou aviso de script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScriptProblem {
    /// Gravidade.
    pub level: ProblemLevel,
    /// O script, relativo à pasta do pack (`kubejs/server_scripts/receitas.js`), quando o log
    /// diz.
    pub file: Option<String>,
    /// Linha (começa em 1).
    pub line: Option<u32>,
    /// Coluna (começa em 1), quando o log diz.
    pub column: Option<u32>,
    /// A mensagem, sem o prefixo do log.
    pub message: String,
    /// A linha original do log.
    pub log_line: String,
    /// O log de onde veio, relativo à pasta do jogo (`logs/kubejs/server.log`), ou `null` quando
    /// veio ao vivo do servidor web do KubeJS.
    pub log_file: Option<String>,
}

/// Um log lido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LogRead {
    /// Caminho relativo à pasta do jogo.
    pub file: String,
    /// Última modificação (milissegundos desde 1970).
    #[specta(type = Option<specta_typescript::Number>)]
    pub modified_ms: Option<u64>,
}

/// O que o painel "Erros dos scripts" mostra.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScriptProblems {
    /// Os itens, na ordem dos logs.
    pub problems: Vec<ScriptProblem>,
    /// Os logs que existiam e foram lidos.
    pub logs: Vec<LogRead>,
    /// Se a lista veio ao vivo do servidor web do KubeJS 7 (jogo aberto).
    pub live: bool,
}

fn kubejs_line() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\[\d{1,2}:\d{2}:\d{2}(?:\.\d+)?\] \[(ERROR|WARN)\] (?:! )?(.*)$")
            .unwrap_or_else(|error| unreachable!("expressão fixa: {error}"))
    })
}

fn kubejs_source() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^([^\s#][^#]*?)#(\d+): (.*)$")
            .unwrap_or_else(|error| unreachable!("expressão fixa: {error}"))
    })
}

fn bracket_prefix() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^((?:\[[^\]\n]*\]\s*)+):?\s*(.*)$")
            .unwrap_or_else(|error| unreachable!("expressão fixa: {error}"))
    })
}

fn zs_location() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"([A-Za-z0-9_./ -]*?[A-Za-z0-9_.-]+\.zs):(\d+)(?::(\d+))?(?::|\s*>)?\s*(.*)$")
            .unwrap_or_else(|error| unreachable!("expressão fixa: {error}"))
    })
}

/// Linha de rastro de pilha ou de cabeçalho de exceção (não vira item sozinha).
fn is_stack_line(message: &str) -> bool {
    let trimmed = message.trim_start();
    message.starts_with(char::is_whitespace)
        || trimmed.starts_with("at ")
        || trimmed.starts_with("Caused by:")
        || (trimmed.starts_with("...") && trimmed.ends_with("more"))
}

fn level_of(text: &str) -> Option<ProblemLevel> {
    match text {
        "ERROR" | "SEVERE" | "FATAL" => Some(ProblemLevel::Error),
        "WARN" | "WARNING" => Some(ProblemLevel::Warning),
        _ => None,
    }
}

fn to_u32(text: &str) -> Option<u32> {
    text.parse().ok().filter(|&n| n > 0)
}

/// Lê um log do KubeJS do tipo dado.
#[must_use]
pub fn parse_kubejs_log(text: &str, script_type: KubejsScriptType) -> Vec<ScriptProblem> {
    let log_file = format!("logs/kubejs/{}.log", script_type.name());
    let mut problems: Vec<ScriptProblem> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        let Some(captures) = kubejs_line().captures(line) else {
            continue;
        };
        let level = level_of(&captures[1]).unwrap_or(ProblemLevel::Error);
        let message = &captures[2];
        if message.trim().is_empty() || is_stack_line(message) {
            continue;
        }
        let (file, number, message) = match kubejs_source().captures(message) {
            Some(source) => {
                let file = kubejs_file_for(&source[1], script_type);
                (file, to_u32(&source[2]), source[3].to_owned())
            }
            None => (None, None, message.to_owned()),
        };
        // Cabeçalho da exceção logo depois de um erro com posição (`dev.….SomeException: …`).
        if file.is_none()
            && level == ProblemLevel::Error
            && looks_like_exception(&message)
            && problems.last().is_some_and(|last| last.file.is_some())
        {
            continue;
        }
        problems.push(ScriptProblem {
            level,
            file,
            line: number,
            column: None,
            message,
            log_line: line.to_owned(),
            log_file: Some(log_file.clone()),
        });
    }
    problems
}

/// `java.lang.RuntimeException: …` e afins.
fn looks_like_exception(message: &str) -> bool {
    let head = message.split(':').next().unwrap_or(message);
    !head.contains(' ')
        && head.contains('.')
        && (head.ends_with("Exception") || head.ends_with("Error"))
}

/// O script do pack a partir do que o KubeJS escreveu (`receitas.js`, `sub/receitas.js`, ou
/// com a pasta de outro tipo, `startup_scripts:itens.js`).
pub(crate) fn kubejs_file_for(source: &str, script_type: KubejsScriptType) -> Option<String> {
    let source = source.trim();
    if !source.to_ascii_lowercase().ends_with(".js") || source.starts_with('<') {
        return None;
    }
    let (folder, file) = match source.split_once(':') {
        Some((folder, file)) if folder.ends_with("_scripts") => (folder.to_owned(), file),
        Some(_) => return None,
        None => (format!("{}_scripts", script_type.name()), source),
    };
    let file = file.trim_start_matches('/');
    if file.is_empty() || file.split('/').any(|part| part == ".." || part.is_empty()) {
        return None;
    }
    Some(format!("kubejs/{folder}/{file}"))
}

/// Lê um log do CraftTweaker (moderno ou do 1.12.2). `log_file` é o caminho relativo à pasta
/// do jogo.
#[must_use]
pub fn parse_crafttweaker_log(text: &str, log_file: &str) -> Vec<ScriptProblem> {
    let mut problems = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        let (level, message) = if let Some(captures) = bracket_prefix().captures(line) {
            let level = captures[1]
                .split(['[', ']'])
                .map(str::trim)
                .find_map(level_of);
            let Some(level) = level else {
                continue;
            };
            (level, captures[2].to_owned())
        } else {
            // Linha sem prefixo: só vale a exceção com a posição no script.
            if is_stack_line(line) || !line.contains(".zs:") {
                continue;
            }
            (ProblemLevel::Error, line.to_owned())
        };
        if message.trim().is_empty() || is_stack_line(&message) {
            continue;
        }
        let located = zs_location().captures(&message).and_then(|captures| {
            let file = crafttweaker_file(&captures[1])?;
            Some((
                file,
                to_u32(&captures[2]),
                captures.get(3).and_then(|c| c.as_str().parse::<u32>().ok()),
                captures[4].trim().to_owned(),
            ))
        });
        let problem = if let Some((file, number, column, rest)) = located {
            ScriptProblem {
                level,
                file: Some(file),
                line: number,
                // O ZenCode conta a coluna a partir de 0.
                column: column.map(|c| c.saturating_add(1)),
                message: if rest.is_empty() {
                    message.clone()
                } else {
                    rest
                },
                log_line: line.to_owned(),
                log_file: Some(log_file.to_owned()),
            }
        } else {
            // Cabeçalhos como "Error while running scripts:" não dizem nada sozinhos.
            if message.trim_end().ends_with(':') {
                continue;
            }
            ScriptProblem {
                level,
                file: None,
                line: None,
                column: None,
                message,
                log_line: line.to_owned(),
                log_file: Some(log_file.to_owned()),
            }
        };
        problems.push(problem);
    }
    problems
}

fn crafttweaker_file(name: &str) -> Option<String> {
    let name = name.trim().trim_start_matches("./");
    let name = name.strip_prefix("scripts/").unwrap_or(name);
    if name.is_empty() || name.split('/').any(|part| part == ".." || part.is_empty()) {
        return None;
    }
    Some(format!("scripts/{name}"))
}

/// Os logs que o painel lê, relativos à pasta do jogo, com o leitor de cada um.
const KUBEJS_LOGS: [KubejsScriptType; 3] = KubejsScriptType::ALL;
const CRAFTTWEAKER_LOGS: [&str; 2] = ["logs/crafttweaker.log", "crafttweaker.log"];

/// Lê os erros dos logs da pasta do jogo da instância de teste (`<instância>/minecraft`).
/// Logs que não existem são pulados; nada aqui falha.
#[must_use]
pub fn read_problems(game_dir: &Path) -> ScriptProblems {
    let mut result = ScriptProblems::default();
    for script_type in KUBEJS_LOGS {
        let relative = format!("logs/kubejs/{}.log", script_type.name());
        if let Some((text, read)) = read_log(game_dir, &relative) {
            result.problems.extend(parse_kubejs_log(&text, script_type));
            result.logs.push(read);
        }
    }
    for relative in CRAFTTWEAKER_LOGS {
        if let Some((text, read)) = read_log(game_dir, relative) {
            result
                .problems
                .extend(parse_crafttweaker_log(&text, relative));
            result.logs.push(read);
        }
    }
    result.problems.truncate(MAX_PROBLEMS);
    result
}

/// O fim de um log (até [`MAX_LOG_BYTES`]), como texto (bytes inválidos trocados).
fn read_log(game_dir: &Path, relative: &str) -> Option<(String, LogRead)> {
    let path = warden_core::resolve_inside(game_dir, relative).ok()?;
    let mut file = std::fs::File::open(&path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    let start = metadata.len().saturating_sub(MAX_LOG_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_LOG_BYTES).read_to_end(&mut bytes).ok()?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0 {
        // Começou no meio de uma linha: descarta o pedaço.
        if let Some(newline) = text.find('\n') {
            text.drain(..=newline);
        }
    }
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok());
    Some((
        text,
        LogRead {
            file: relative.to_owned(),
            modified_ms,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubejs_com_e_sem_posicao() {
        let log = "\
[05:43:00] [INIT] KubeJS 2001.6.5-build.26; MC 2001 fabric
[05:43:00] [INFO] Loaded script server_scripts:receitas.js in 0.003 s
[05:43:02] [WARN] Error parsing recipe minecraft:kjs/x: Item array cannot be empty
[06:01:36] [ERROR] ! receitas.js#16: Failed to create recipe for type 'kubejs:shapeless': ItemStack 'result' can't be empty!
[06:01:36] [ERROR] ! java.lang.IllegalStateException: ItemStack 'result' can't be empty!
[06:01:36] [ERROR] !   at dev.latvian.mods.kubejs.recipe.RecipeJS.x(RecipeJS.java:10)
[06:01:36] [ERROR] ! sub/tags.js#3: oops
[06:01:36] [WARN] startup_scripts:itens.js#2: aviso
[06:01:36] [ERROR] ! <unknown source>#4: sem arquivo
[06:01:36] [ERROR] ! ../fora.js#4: fora
";
        let problems = parse_kubejs_log(log, KubejsScriptType::Server);
        let summary: Vec<(ProblemLevel, Option<&str>, Option<u32>)> = problems
            .iter()
            .map(|p| (p.level, p.file.as_deref(), p.line))
            .collect();
        assert_eq!(
            summary,
            [
                (ProblemLevel::Warning, None, None),
                (
                    ProblemLevel::Error,
                    Some("kubejs/server_scripts/receitas.js"),
                    Some(16)
                ),
                (
                    ProblemLevel::Error,
                    Some("kubejs/server_scripts/sub/tags.js"),
                    Some(3)
                ),
                (
                    ProblemLevel::Warning,
                    Some("kubejs/startup_scripts/itens.js"),
                    Some(2)
                ),
                (ProblemLevel::Error, None, Some(4)),
                (ProblemLevel::Error, None, Some(4)),
            ]
        );
        assert_eq!(
            problems[1].message,
            "Failed to create recipe for type 'kubejs:shapeless': ItemStack 'result' can't be empty!"
        );
        assert!(
            problems[1]
                .log_line
                .starts_with("[06:01:36] [ERROR] ! receitas.js#16")
        );
        assert_eq!(
            problems[1].log_file.as_deref(),
            Some("logs/kubejs/server.log")
        );
    }

    #[test]
    fn crafttweaker_moderno_e_1_12() {
        let modern = "\
[05:43:03.347][INFO][CraftTweaker-ZenCode]: Loading file 'receitas.zs' with priority 0
[05:43:03.347][ERROR][CraftTweaker-ZenCode]: Could not get item with name: <item:minecraft:stickk>! Item does not appear to exist!
[05:43:03.349][ERROR][CraftTweaker-ZenCode]: Error while running scripts:
org.openzen.zenscript.lexer.ParseException: receitas.zs:2:107: Invalid parameters to BEP item
\tat knot//org.openzen.zenscript.parser.ParsedFile.parse(ParsedFile.java:215)
[05:43:03.351][ERROR][CraftTweaker]: Scripts are invalid!
";
        let problems = parse_crafttweaker_log(modern, "logs/crafttweaker.log");
        assert_eq!(problems.len(), 3);
        assert_eq!(problems[0].file, None);
        assert_eq!(problems[1].file.as_deref(), Some("scripts/receitas.zs"));
        assert_eq!(problems[1].line, Some(2));
        assert_eq!(problems[1].column, Some(108));
        assert_eq!(problems[1].message, "Invalid parameters to BEP item");
        assert_eq!(problems[2].message, "Scripts are invalid!");

        let classic = "\
[INITIALIZATION][CLIENT][INFO] Loading scripts
[INITIALIZATION][CLIENT][ERROR] receitas.zs:5 > No such item: <minecraft:stickk>
[INITIALIZATION][CLIENT][WARNING] aviso geral
";
        let problems = parse_crafttweaker_log(classic, "crafttweaker.log");
        assert_eq!(problems.len(), 2);
        assert_eq!(problems[0].line, Some(5));
        assert_eq!(problems[0].column, None);
        assert_eq!(problems[0].message, "No such item: <minecraft:stickk>");
        assert_eq!(problems[1].level, ProblemLevel::Warning);
    }

    #[test]
    fn le_os_logs_da_instancia() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs/kubejs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(
            logs.join("server.log"),
            "[06:01:36] [ERROR] ! a.js#2: x\r\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("crafttweaker.log"),
            "[INITIALIZATION][SERVER][ERROR] b.zs:3 > y\n",
        )
        .unwrap();
        let read = read_problems(dir.path());
        assert_eq!(read.problems.len(), 2);
        assert_eq!(read.logs.len(), 2);
        assert_eq!(read.logs[0].file, "logs/kubejs/server.log");
        assert!(read.logs[0].modified_ms.is_some());
        assert!(!read.live);
        assert_eq!(
            read_problems(&dir.path().join("nada")),
            ScriptProblems::default()
        );
    }

    #[test]
    fn log_grande_le_so_o_fim() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs/kubejs");
        std::fs::create_dir_all(&logs).unwrap();
        let mut text = String::new();
        let filler = "[00:00:00] [INFO] linha de enchimento para passar do limite de leitura\n";
        while u64::try_from(text.len()).unwrap() < MAX_LOG_BYTES + 1024 {
            text.push_str(filler);
        }
        text.push_str("[06:01:36] [ERROR] ! fim.js#9: no fim\n");
        std::fs::write(logs.join("startup.log"), text).unwrap();
        let read = read_problems(dir.path());
        assert_eq!(read.problems.len(), 1);
        assert_eq!(
            read.problems[0].file.as_deref(),
            Some("kubejs/startup_scripts/fim.js")
        );
    }
}
