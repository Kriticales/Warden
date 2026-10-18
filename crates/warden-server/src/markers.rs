//! O que o Warden lê na saída do servidor (ARCHITECTURE §7.4 e §7.6):
//!
//! - **pronto:** `Done (12.345s)! For help, type "help"` (de 1.7.10 à mais nova; as antigas
//!   acrescentam ` or "?"`);
//! - **um jogador entrou:** `Nome[/127.0.0.1:54321] logged in with entity id …` e
//!   `Nome joined the game`;
//! - **mod de cliente no servidor** (CA-T27-04): a classe do cliente que um mod tentou carregar
//!   no servidor dedicado (`net/minecraft/client/…` no Forge e no NeoForge, `environment type
//!   SERVER` no Fabric, `NoClassDefFoundError` nos antigos);
//! - o **trecho do log** que o resultado mostra quando o servidor trava.

use std::sync::LazyLock;

use regex::Regex;

/// Pronto. O tempo entre parênteses usa ponto ou vírgula conforme a língua do sistema.
static DONE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"Done \((?P<seconds>[\d.,]+)s\)! For help, type "help"(?: or "\?")?"#).ok()
});

/// Um jogador entrou.
static JOINED: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?:^|\s|:\s)(?P<name>[A-Za-z0-9_]{1,16})(?:\[[^\]]*\] logged in with entity id| joined the game)",
    )
    .ok()
});

/// Classe do cliente carregada no servidor.
static CLIENT_CLASS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        Attempted\ to\ load\ class\ (?P<a>[\w/$.]+)\ for\ invalid\ dist\ DEDICATED_SERVER
        | Cannot\ load\ class\ (?P<b>[\w/$.]+)\ in\ environment\ type\ SERVER
        | (?:NoClassDefFoundError|ClassNotFoundException):\ (?P<c>net[/.]minecraft[/.]client[\w/$.]*)
        ",
    )
    .ok()
});

/// Se a linha diz que o servidor ficou pronto; devolve o tempo que ele próprio mediu, em ms.
#[must_use]
pub fn ready_line(text: &str) -> Option<Option<u64>> {
    let captures = DONE.as_ref()?.captures(text)?;
    let seconds = captures
        .name("seconds")
        .and_then(|value| value.as_str().replace(',', ".").parse::<f64>().ok())
        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // filtrado acima
    Some(seconds.map(|seconds| (seconds * 1000.0).round() as u64))
}

/// Se a linha diz que um jogador entrou; devolve o nome.
#[must_use]
pub fn joined_line(text: &str) -> Option<String> {
    JOINED
        .as_ref()?
        .captures(text)?
        .name("name")
        .map(|name| name.as_str().to_owned())
}

/// A classe do cliente que um mod tentou carregar no servidor (com pontos: `net.minecraft…`).
#[must_use]
pub fn client_class(text: &str) -> Option<String> {
    let captures = CLIENT_CLASS.as_ref()?.captures(text)?;
    ["a", "b", "c"]
        .iter()
        .find_map(|name| captures.name(name))
        .map(|class| class.as_str().replace('/', "."))
}

/// Quantas linhas o trecho do log mostra.
pub const EXCERPT_LINES: usize = 12;

/// O trecho do log do servidor que explica o travamento: a partir do primeiro erro (`ERROR`,
/// `FATAL`, exceção, classe do cliente) entre as últimas 200 linhas, até [`EXCERPT_LINES`]
/// linhas; sem erro, as últimas linhas.
#[must_use]
pub fn crash_excerpt(lines: &[String]) -> Vec<String> {
    let tail_start = lines.len().saturating_sub(200);
    let tail = &lines[tail_start..];
    let is_error = |line: &String| {
        line.contains("/ERROR]")
            || line.contains("/FATAL]")
            || line.contains("[ERROR]")
            || line.contains("[FATAL]")
            || line.contains("Exception")
            || line.contains("Error:")
    };
    let start = tail
        .iter()
        .position(|line| is_error(line) || client_class(line).is_some());
    match start {
        Some(start) => tail[start..].iter().take(EXCERPT_LINES).cloned().collect(),
        None => tail[tail.len().saturating_sub(EXCERPT_LINES)..].to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pronto_em_todas_as_faixas() {
        assert_eq!(
            ready_line(r#"Done (6.912s)! For help, type "help""#),
            Some(Some(6912))
        );
        assert_eq!(
            ready_line(
                r#"[12:00:01] [Server thread/INFO]: Done (12,5s)! For help, type "help" or "?""#
            ),
            Some(Some(12_500))
        );
        assert_eq!(ready_line("Preparing level \"world\""), None);
        assert_eq!(ready_line("Done (abc)! For help"), None);
    }

    #[test]
    fn jogador_entrou() {
        assert_eq!(
            joined_line("Jogador[/127.0.0.1:54321] logged in with entity id 1 at (0.5, 64.0, 0.5)")
                .as_deref(),
            Some("Jogador")
        );
        assert_eq!(
            joined_line("[Server thread/INFO]: Steve joined the game").as_deref(),
            Some("Steve")
        );
        assert_eq!(joined_line("Starting minecraft server"), None);
    }

    #[test]
    fn ca_t27_04_classe_do_cliente_no_servidor() {
        assert_eq!(
            client_class(
                "java.lang.RuntimeException: Attempted to load class net/minecraft/client/gui/screens/Screen for invalid dist DEDICATED_SERVER"
            )
            .as_deref(),
            Some("net.minecraft.client.gui.screens.Screen")
        );
        assert_eq!(
            client_class(
                "java.lang.RuntimeException: Cannot load class net.minecraft.client.MinecraftClient in environment type SERVER"
            )
            .as_deref(),
            Some("net.minecraft.client.MinecraftClient")
        );
        assert_eq!(
            client_class("java.lang.NoClassDefFoundError: net/minecraft/client/Minecraft")
                .as_deref(),
            Some("net.minecraft.client.Minecraft")
        );
        assert_eq!(
            client_class("java.lang.NoClassDefFoundError: com/example/Outra"),
            None
        );
    }

    #[test]
    fn trecho_comeca_no_erro() {
        let mut lines: Vec<String> = (0..30)
            .map(|n| format!("[Server thread/INFO]: linha {n}"))
            .collect();
        lines.push("[Server thread/ERROR]: Encountered an unexpected exception".into());
        lines.push("java.lang.RuntimeException: Attempted to load class net/minecraft/client/Main for invalid dist DEDICATED_SERVER".into());
        lines.push("\tat com.example.Mod.<init>(Mod.java:1)".into());
        let excerpt = crash_excerpt(&lines);
        assert!(excerpt[0].contains("unexpected exception"), "{excerpt:?}");
        assert!(excerpt[1].contains("Attempted to load class"));
        assert_eq!(excerpt.len(), 3);
        let quiet: Vec<String> = (0..30).map(|n| format!("linha {n}")).collect();
        let excerpt = crash_excerpt(&quiet);
        assert_eq!(excerpt.len(), EXCERPT_LINES);
        assert_eq!(excerpt.last().map(String::as_str), Some("linha 29"));
        assert!(crash_excerpt(&[]).is_empty());
    }
}
