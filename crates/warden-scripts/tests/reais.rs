//! Golden tests com logs reais do KubeJS 6 e do CraftTweaker 14 (CA-T26-02; `tests/reais/`).

use std::path::{Path, PathBuf};

use warden_scripts::problems::{ProblemLevel, ScriptProblem, read_problems};

fn dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/reais")
        .join(name)
}

fn located(problems: &[ScriptProblem]) -> Vec<(ProblemLevel, &str, u32)> {
    problems
        .iter()
        .filter_map(|p| Some((p.level, p.file.as_deref()?, p.line?)))
        .collect()
}

#[test]
fn kubejs_6_erro_na_linha_16_de_receitas() {
    let game = dir("kubejs-6-fabric-1.20.1");
    let read = read_problems(&game);
    let files: Vec<&str> = read.logs.iter().map(|log| log.file.as_str()).collect();
    assert_eq!(files, ["logs/kubejs/startup.log", "logs/kubejs/server.log"]);
    assert_eq!(
        located(&read.problems),
        [
            (ProblemLevel::Error, "kubejs/startup_scripts/itens.js", 3),
            (ProblemLevel::Error, "kubejs/server_scripts/receitas.js", 16),
            (ProblemLevel::Error, "kubejs/server_scripts/receitas.js", 18),
        ]
    );
    assert_eq!(read.problems.len(), 3, "{:#?}", read.problems);
    let line16 = &read.problems[1];
    assert_eq!(
        line16.message,
        "Failed to create recipe for type 'kubejs:shapeless': ItemStack 'result' can't be empty!"
    );
    assert!(
        line16
            .log_line
            .starts_with("[06:03:25] [ERROR] ! receitas.js#16: ")
    );
    assert_eq!(line16.log_file.as_deref(), Some("logs/kubejs/server.log"));

    // A linha 16 do script de verdade tem o ID errado.
    let script = std::fs::read_to_string(game.join("kubejs/server_scripts/receitas.js")).unwrap();
    assert!(
        script
            .lines()
            .nth(15)
            .unwrap()
            .contains("minecraft:copper_ingott")
    );
    let startup = std::fs::read_to_string(game.join("kubejs/startup_scripts/itens.js")).unwrap();
    assert!(startup.lines().nth(2).unwrap().contains("event.criar"));
}

#[test]
fn crafttweaker_14_erros_com_linha_e_coluna() {
    let game = dir("crafttweaker-14-fabric-1.20.1");
    let read = read_problems(&game);
    assert_eq!(read.logs.len(), 1);
    assert_eq!(read.logs[0].file, "logs/crafttweaker.log");
    let with_position: Vec<(&str, Option<u32>, Option<u32>)> = read
        .problems
        .iter()
        .filter(|p| p.file.is_some())
        .map(|p| (p.file.as_deref().unwrap_or(""), p.line, p.column))
        .collect();
    assert_eq!(
        with_position,
        [
            ("scripts/receitas.zs", Some(2), Some(108)),
            ("scripts/receitas.zs", Some(6), Some(1)),
        ]
    );
    assert!(read.problems.iter().all(|p| p.level == ProblemLevel::Error));
    assert!(
        read.problems
            .iter()
            .any(|p| p.message.contains("<item:minecraft:stickk>"))
    );
    // Nenhuma linha de rastro de pilha vira item.
    assert!(
        read.problems
            .iter()
            .all(|p| !p.log_line.trim_start().starts_with("at "))
    );
}
