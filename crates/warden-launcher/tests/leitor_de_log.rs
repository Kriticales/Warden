//! O leitor de saída com golden logs reais do S-R5-3 (`tests/fixtures/logs/`): XML do log4j com
//! exceção (Fabric) e a mistura de XML e texto do NeoForge na mesma saída.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::too_many_lines,
    clippy::missing_panics_doc
)]

mod comum;

use warden_launcher::log::{LogLevel, LogLine, LogParser, LogSource};

fn parse(name: &str) -> Vec<LogLine> {
    let text = std::fs::read_to_string(comum::fixtures_dir().join("logs").join(name)).unwrap();
    let mut parser = LogParser::new(LogSource::Stdout);
    let mut lines: Vec<LogLine> = text
        .lines()
        .enumerate()
        .flat_map(|(index, line)| parser.push(line, index as u64))
        .collect();
    lines.extend(parser.finish(0));
    lines
}

#[test]
fn fabric_mixin_quebrado_com_stack_trace() {
    let lines = parse("fab-mixin.log");
    let failure = lines
        .iter()
        .find(|line| {
            line.text
                .starts_with("Mixin apply for mod wardenfalhas failed")
        })
        .expect("a linha do mixin");
    assert_eq!(failure.level, Some(LogLevel::Error));
    assert!(failure.timestamp_ms.is_some());
    assert!(
        failure
            .text
            .contains("InvalidInjectionException: Critical injection failure")
    );
    assert!(failure.text.contains("\tat org.spongepowered.asm.mixin"));
    // Nenhum pedaço de XML vazou como texto.
    assert!(!lines.iter().any(|line| line.text.contains("<log4j:")));
}

#[test]
fn neoforge_mistura_xml_e_texto() {
    let lines = parse("neo-dep-faltando.log");
    let xml = lines
        .iter()
        .filter(|line| line.timestamp_ms.is_some())
        .count();
    let text = lines
        .iter()
        .filter(|line| line.timestamp_ms.is_none() && line.time.is_some())
        .count();
    assert!(xml >= 6, "{xml}");
    assert!(text >= 11, "{text}");
    let user = lines
        .iter()
        .find(|line| line.text == "Setting user: WardenTest")
        .expect("Setting user");
    assert_eq!(user.thread.as_deref(), Some("Render thread"));
    assert_eq!(user.logger.as_deref(), Some("minecraft/Minecraft"));
    assert_eq!(user.level, Some(LogLevel::Info));
    let missing = lines
        .iter()
        .find(|line| {
            line.text
                .starts_with("Missing or unsupported mandatory dependencies")
        })
        .expect("dependência faltando");
    assert_eq!(missing.level, Some(LogLevel::Error));
    assert!(missing.text.contains("Mod ID: 'moonlight'"));
    assert!(!lines.iter().any(|line| line.text.contains("<log4j:")));
}
