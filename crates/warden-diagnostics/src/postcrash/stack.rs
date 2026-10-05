//! Estrutura dos logs: blocos de crash report, pilhas de exceção e mods citados nelas.

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

use super::text::{CleanLine, fixed_regex};

/// Máximo de linhas percorridas numa pilha.
const MAX_STACK_LINES: usize = 400;
/// Máximo de mods tirados de uma pilha.
const MAX_MODS: usize = 5;

static CRASH_HEADER: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"---- Minecraft Crash Report ----"));

/// O crash report de informações do `SplashProgress` do Forge 1.7.10 (S-R5-3 §3.1), que não é
/// travamento.
static NOT_AN_ERROR: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"THIS IS NOT A ERROR|^Description: Loading screen debug info"));

static STACK_LINE: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(concat!(
        r"^\s*(?:at |\.\.\. \d+ more|\.\.\. \(|Caused by|Suppressed:|Wrapped by:|",
        r"\[?\s*Stacktrace:|Mixins in Stacktrace:|\S*(?:Exception|Error|Throwable)\b)",
    ))
});

static CAUSED_BY: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"^\s*Caused by(?: \d+)?: (?P<cause>\S.*?)\s*$"));

static NEOFORGE_MODULE: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"\bat (?:[A-Z][A-Z ]*/)?(?P<id>[a-z][a-z0-9_]{1,63})@[^/\s]+/"));

static FORGE_JAR: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"[~\[]\[?(?P<jar>[^\[\]%!/\\:]+?\.jar)(?:%23\d+)?!/"));

static FABRIC_HANDLER: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(
        r"[.$](?:handler|redirect|modify|wrapOperation|localvar|constant)\$[0-9a-z]+\$(?P<id>[a-z][a-z0-9_\-]*)\$",
    )
});

static SUSPECTED_HEADER: LazyLock<Regex> =
    LazyLock::new(|| fixed_regex(r"^\s*Suspected Mods?(?:\(s\))?:\s*(?P<rest>.*)$"));

static SUSPECTED_ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(r"^\s*(?P<name>[^(\t]+?) \((?P<id>[\w\-.]+)\)(?:, Version: .*)?\s*$")
});

/// Módulos e ids que não são mods do pack (jogo, loader, bibliotecas).
const NOT_MODS: [&str; 34] = [
    "minecraft",
    "neoforge",
    "forge",
    "fml",
    "fmlcore",
    "fmlloader",
    "fmlearlydisplay",
    "javafmllanguage",
    "lowcodelanguage",
    "mclanguage",
    "authlib",
    "brigadier",
    "datafixerupper",
    "modlauncher",
    "bootstraplauncher",
    "securejarhandler",
    "eventbus",
    "coremods",
    "accesstransformers",
    "mixinextras",
    "mixin",
    "sponge",
    "spongepowered",
    "loader",
    "mergetool",
    "unsafe",
    "jopt",
    "guava",
    "gson",
    "netty",
    "lwjgl",
    "blaze3d",
    "logging",
    "fabricloader",
];

/// Prefixos de jars que não são mods do pack.
const NOT_MOD_JARS: [&str; 29] = [
    "loader-",
    "fancymodloader-",
    "earlydisplay-",
    "client-",
    "server-",
    "minecraft-",
    "forge-",
    "fmlcore-",
    "fmlloader-",
    "fmlearlydisplay-",
    "javafmllanguage-",
    "lowcodelanguage-",
    "mclanguage-",
    "neoforge-",
    "authlib-",
    "brigadier-",
    "datafixerupper-",
    "modlauncher-",
    "bootstraplauncher-",
    "securejarhandler-",
    "eventbus-",
    "coremods-",
    "mixin-",
    "sponge-mixin-",
    "mixinextras-",
    "guava-",
    "netty-",
    "lwjgl",
    "jopt-simple-",
];

/// Os blocos de crash report de um arquivo, como faixas de índices das linhas.
///
/// Num arquivo de crash report o arquivo inteiro é o bloco. Num log, o bloco vai do cabeçalho
/// até antes do próximo registro do log. O crash report de informações do `SplashProgress`
/// (Forge 1.7.10) não conta.
pub(crate) fn crash_blocks(lines: &[CleanLine], whole_file: bool) -> Vec<Range<usize>> {
    if whole_file {
        return if lines.iter().any(|line| NOT_AN_ERROR.is_match(&line.text)) {
            Vec::new()
        } else {
            std::iter::once(0..lines.len()).collect()
        };
    }
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if CRASH_HEADER.is_match(&lines[index].text) {
            let start = index;
            let mut end = index + 1;
            while end < lines.len()
                && !lines[end].record_start
                && !CRASH_HEADER.is_match(&lines[end].text)
            {
                end += 1;
            }
            let fake = lines[start..end]
                .iter()
                .any(|line| NOT_AN_ERROR.is_match(&line.text));
            if !fake {
                blocks.push(start..end);
            }
            index = end;
        } else {
            index += 1;
        }
    }
    blocks
}

/// Se a linha parece parte de uma pilha de exceção.
fn is_stack_line(text: &str) -> bool {
    STACK_LINE.is_match(text)
}

/// A faixa da pilha que começa em `start` (a linha da exceção ou a anterior a ela): segue
/// enquanto as linhas parecem pilha, pulando até uma linha em branco seguida de mais pilha.
pub(crate) fn stack_range(lines: &[CleanLine], start: usize) -> Range<usize> {
    let mut end = start + 1;
    let limit = lines.len().min(start + MAX_STACK_LINES);
    while end < limit {
        let text = &lines[end].text;
        if lines[end].record_start {
            break;
        }
        let blank_inside = text.trim().is_empty()
            && lines.get(end + 1).is_some_and(|next| {
                let next = next.text.trim_start();
                next.starts_with("at ") || next.starts_with("Caused by")
            });
        if is_stack_line(text) || blank_inside {
            end += 1;
        } else {
            break;
        }
    }
    start..end
}

/// A causa mais profunda (`Caused by:` mais abaixo) da pilha.
pub(crate) fn deepest_cause(lines: &[CleanLine], range: Range<usize>) -> Option<String> {
    lines[range]
        .iter()
        .rev()
        .find_map(|line| CAUSED_BY.captures(&line.text))
        .and_then(|captures| captures.name("cause"))
        .map(|cause| cause.as_str().to_owned())
}

/// Mods citados numa faixa de linhas, na ordem em que aparecem: "Suspected Mods" do Forge,
/// módulos `id@versão` do NeoForge, jars `~[arquivo.jar%23n!/]` do Forge e handlers de mixin do
/// Fabric (`handler$xxx$modid$método`).
///
/// O nome do handler só traz o id do mod no fork do Mixin do Fabric (`handler$uid$modid$método`);
/// no Forge/NeoForge o mesmo lugar é parte do nome do método, então só vale com
/// `fabric_handlers`.
pub(crate) fn mods_in(
    lines: &[CleanLine],
    range: Range<usize>,
    fabric_handlers: bool,
) -> Vec<ModMention> {
    let mut found: Vec<ModMention> = Vec::new();
    let push = |mention: ModMention, found: &mut Vec<ModMention>| {
        if found.len() < MAX_MODS && !found.contains(&mention) {
            found.push(mention);
        }
    };
    let slice = &lines[range];
    // Suspeitos declarados pelo Forge vêm primeiro.
    for (index, line) in slice.iter().enumerate() {
        if let Some(header) = SUSPECTED_HEADER.captures(&line.text) {
            let rest = header.name("rest").map_or("", |m| m.as_str()).trim();
            if rest.eq_ignore_ascii_case("none") || rest.eq_ignore_ascii_case("unknown") {
                continue;
            }
            if let Some(entry) = SUSPECTED_ENTRY.captures(rest)
                && let Some(id) = entry.name("id")
            {
                push(ModMention::Id(id.as_str().to_owned()), &mut found);
            }
            for next in slice.iter().skip(index + 1).take(8) {
                match SUSPECTED_ENTRY.captures(&next.text) {
                    Some(entry) if next.text.starts_with(['\t', ' ']) => {
                        if let Some(id) = entry.name("id") {
                            push(ModMention::Id(id.as_str().to_owned()), &mut found);
                        }
                    }
                    _ => break,
                }
            }
        }
    }
    for line in slice {
        let text = &line.text;
        if !text.trim_start().starts_with("at ") {
            continue;
        }
        for captures in NEOFORGE_MODULE.captures_iter(text) {
            if let Some(id) = captures.name("id") {
                let id = id.as_str();
                if !NOT_MODS.contains(&id) && !id.starts_with("java") && !id.starts_with("jdk") {
                    push(ModMention::Id(id.to_owned()), &mut found);
                }
            }
        }
        for captures in FORGE_JAR.captures_iter(text) {
            if let Some(jar) = captures.name("jar") {
                let jar = jar.as_str();
                let lower = jar.to_lowercase();
                if !NOT_MOD_JARS.iter().any(|prefix| lower.starts_with(prefix)) {
                    push(ModMention::Jar(jar.to_owned()), &mut found);
                }
            }
        }
        for captures in FABRIC_HANDLER
            .captures_iter(text)
            .filter(|_| fabric_handlers)
        {
            if let Some(id) = captures.name("id") {
                let id = id.as_str();
                if !NOT_MODS.contains(&id) {
                    push(ModMention::Id(id.to_owned()), &mut found);
                }
            }
        }
    }
    found
}

/// Mod citado numa pilha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModMention {
    /// Pelo id.
    Id(String),
    /// Pelo nome do jar.
    Jar(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(texts: &[&str]) -> Vec<CleanLine> {
        texts
            .iter()
            .enumerate()
            .map(|(index, text)| CleanLine {
                number: u32::try_from(index + 1).unwrap(),
                text: (*text).to_owned(),
                record_start: text.starts_with('['),
            })
            .collect()
    }

    #[test]
    fn blocos_de_crash_report_no_log_e_no_arquivo() {
        let log = lines(&[
            "[1] x",
            "---- Minecraft Crash Report ----",
            "Description: Initializing game",
            "",
            "java.lang.IllegalStateException: x",
            "[2] depois",
            "[cpw.mods.fml.client.SplashProgress:start:188]: ---- Minecraft Crash Report ----",
            "This is just a prompt for computer specs to be printed. THIS IS NOT A ERROR",
            "[3] fim",
        ]);
        assert_eq!(crash_blocks(&log, false), vec![1..5]);
        assert_eq!(crash_blocks(&log[..3], true), vec![0..3]);
        assert!(crash_blocks(&log[6..8], true).is_empty());
    }

    #[test]
    fn pilha_causa_e_mods() {
        let log = lines(&[
            "java.lang.RuntimeException: externo",
            "\tat TRANSFORMER/supplementaries@1.21-3.9.9/net.x.Y.m(Y.java:1)",
            "\tat TRANSFORMER/minecraft@1.21.1/net.minecraft.client.Minecraft.m(Minecraft.java:1)",
            "\tat java.base/java.lang.Thread.run(Thread.java:1)",
            "\tat net.a.B.c(B.java:2) ~[ColdSweat-2.2.jar%23197!/:2.2] {re:mixin}",
            "\tat net.a.B.c(B.java:2) ~[forge-47.4.10-universal.jar%2310!/:?]",
            "\tat knot//net.minecraft.class_310.handler$zza000$wardenfalhas$warden$iniciar(class_310.java:2996)",
            "Caused by: java.lang.NoClassDefFoundError: net/x/Z",
            "\t... 3 more",
            "",
            "Caused by: java.lang.ClassNotFoundException: net.x.Z",
            "\tat y",
            "",
            "A detailed walkthrough",
        ]);
        let range = stack_range(&log, 0);
        assert_eq!(range, 0..12);
        assert_eq!(
            deepest_cause(&log, range.clone()).as_deref(),
            Some("java.lang.ClassNotFoundException: net.x.Z")
        );
        assert_eq!(
            mods_in(&log, range, true),
            vec![
                ModMention::Id("supplementaries".into()),
                ModMention::Jar("ColdSweat-2.2.jar".into()),
                ModMention::Id("wardenfalhas".into()),
            ]
        );
    }

    #[test]
    fn suspeitos_do_forge_vem_primeiro() {
        let log = lines(&[
            "Suspected Mods: ",
            "\tSupplementaries (supplementaries), Version: 3.1",
            "\t\tIssue tracker URL: https://x",
            "\tat TRANSFORMER/moonlight@1.0/net.x.Y.m(Y.java:1)",
            "Suspected Mod: Quark (quark), Version: 1",
            "Suspected Mods: NONE",
        ]);
        assert_eq!(
            mods_in(&log, 0..log.len(), true),
            vec![
                ModMention::Id("supplementaries".into()),
                ModMention::Id("quark".into()),
                ModMention::Id("moonlight".into()),
            ]
        );
        assert_eq!(deepest_cause(&log, 0..log.len()), None);
    }

    #[test]
    fn limite_de_cinco_mods() {
        let texts: Vec<String> = (0..8)
            .map(|index| format!("\tat TRANSFORMER/mod{index}@1/net.X.m(X.java:1)"))
            .collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let log = lines(&refs);
        assert_eq!(mods_in(&log, 0..log.len(), true).len(), MAX_MODS);
    }
}
