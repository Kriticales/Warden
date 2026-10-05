//! Loader e versão do Minecraft lidos do próprio log, para quando a análise recebe um texto
//! solto (log do computador, de um jogador) sem o pack ao lado.

use std::sync::LazyLock;

use regex::Regex;

use super::LogLoader;
use super::text::{CleanLine, fixed_regex};

/// Linhas que identificam o loader e a versão (S-R5-3 §3, coluna "linha que identifica o
/// loader"; cabeçalhos de crash report).
static SIGNATURES: LazyLock<Vec<(LogLoader, Regex)>> = LazyLock::new(|| {
    vec![
        (
            LogLoader::Quilt,
            fixed_regex(r"Loading Minecraft (?P<mc>\S+) with Quilt Loader (?P<loader>\S+)"),
        ),
        (
            LogLoader::Fabric,
            fixed_regex(r"Loading Minecraft (?P<mc>\S+) with Fabric Loader (?P<loader>\S+)"),
        ),
        (
            LogLoader::NeoForge,
            fixed_regex(
                r"NeoForge mod loading, version (?P<loader>[^,\s]+)(?:, for MC (?P<mc>[^,\s]+))?",
            ),
        ),
        (
            LogLoader::NeoForge,
            fixed_regex(r"--fml\.neoForgeVersion,? (?P<loader>[^,\s\]]+)"),
        ),
        (
            LogLoader::Forge,
            fixed_regex(r"Forge mod loading, version (?P<loader>[^,\s]+), for MC (?P<mc>[^,\s]+)"),
        ),
        (
            LogLoader::Forge,
            fixed_regex(
                r"Forge Mod Loader version (?P<loader>\S+) for Minecraft (?P<mc>\S+) loading",
            ),
        ),
        (
            LogLoader::Forge,
            fixed_regex(r"MinecraftForge v(?P<loader>[\d.]+) Initialized"),
        ),
        (
            LogLoader::Forge,
            fixed_regex(r"--fml\.forgeVersion,? (?P<loader>[^,\s\]]+)"),
        ),
        (
            LogLoader::NeoForge,
            fixed_regex(r"--version,? neoforge-(?:(?P<mc>1\.[\d.]+)-)?(?P<loader>[\d.]+)"),
        ),
        (
            LogLoader::Forge,
            fixed_regex(r"--version,? forge-(?P<mc>\d+\.[\d.]+)-(?P<loader>[\d.]+)"),
        ),
        (
            LogLoader::Forge,
            fixed_regex(r"--version,? (?P<mc>\d+\.[\d.]+)-forge-?(?P<loader>[\d.]+)"),
        ),
        (
            LogLoader::NeoForge,
            fixed_regex(r"^\s*NeoForge: (?:net\.neoforged:)?(?P<loader>[\d.]+\S*)"),
        ),
        (
            LogLoader::Forge,
            fixed_regex(r"^\s*Forge: (?:net\.minecraftforge:)?(?P<loader>[\d.]+\S*)"),
        ),
        (LogLoader::Fabric, fixed_regex(r"^\s*Fabric Mods:\s*$")),
    ]
});

static MINECRAFT_VERSION: LazyLock<Regex> = LazyLock::new(|| {
    fixed_regex(concat!(
        r"(?:^\s*Minecraft Version: (?P<a>\S+)",
        r"|--fml\.mcVersion,? (?P<b>[^,\s\]]+)",
        r"|Starting (?:integrated )?minecraft server version (?P<c>\S+)",
        r"|Minecraft Version ID: (?P<d>\S+))",
    ))
});

/// O que o log diz sobre si mesmo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Detected {
    pub loader: Option<LogLoader>,
    pub loader_version: Option<String>,
    pub minecraft: Option<String>,
}

/// Procura o loader e a versão nas linhas; a primeira linha que identifica vence.
pub(crate) fn detect<'a>(lines: impl IntoIterator<Item = &'a CleanLine>) -> Detected {
    let mut detected = Detected::default();
    for line in lines {
        let text = line.text.as_str();
        for (loader, regex) in SIGNATURES.iter() {
            if detected.loader.is_some_and(|known| known != *loader) {
                continue;
            }
            if let Some(captures) = regex.captures(text) {
                if detected.loader.is_none() {
                    detected.loader = Some(*loader);
                    detected.loader_version =
                        captures.name("loader").map(|m| m.as_str().to_owned());
                }
                if detected.minecraft.is_none() {
                    detected.minecraft = captures.name("mc").map(|m| m.as_str().to_owned());
                }
                break;
            }
        }
        if detected.minecraft.is_none()
            && let Some(captures) = MINECRAFT_VERSION.captures(text)
        {
            detected.minecraft = ["a", "b", "c", "d"]
                .iter()
                .find_map(|name| captures.name(name))
                .map(|m| m.as_str().to_owned());
        }
        if detected.loader.is_some() && detected.minecraft.is_some() {
            break;
        }
    }
    detected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect_text(texts: &[&str]) -> Detected {
        let lines: Vec<CleanLine> = texts
            .iter()
            .map(|text| CleanLine {
                number: 1,
                text: (*text).to_owned(),
                record_start: false,
            })
            .collect();
        detect(&lines)
    }

    #[test]
    fn reconhece_cada_loader() {
        let cases = [
            (
                "Loading Minecraft 1.20.1 with Fabric Loader 0.19.5",
                LogLoader::Fabric,
                Some("1.20.1"),
            ),
            (
                "Loading Minecraft 1.20.1 with Quilt Loader 0.24.0",
                LogLoader::Quilt,
                Some("1.20.1"),
            ),
            (
                "[x]: NeoForge mod loading, version 21.1.252, for MC 1.21.1",
                LogLoader::NeoForge,
                Some("1.21.1"),
            ),
            (
                "Forge mod loading, version 47.4.10, for MC 1.20.1",
                LogLoader::Forge,
                Some("1.20.1"),
            ),
            (
                "Forge Mod Loader version 14.23.5.2860 for Minecraft 1.12.2 loading",
                LogLoader::Forge,
                Some("1.12.2"),
            ),
            (
                "MinecraftForge v10.13.4.1614 Initialized",
                LogLoader::Forge,
                None,
            ),
            (
                "Launching target 'forgeclient' with arguments [--fml.neoForgeVersion, 21.1.252, --fml.mcVersion, 1.21.1]",
                LogLoader::NeoForge,
                Some("1.21.1"),
            ),
            (
                "--fml.forgeVersion, 47.4.10, --fml.mcVersion, 1.20.1",
                LogLoader::Forge,
                Some("1.20.1"),
            ),
            (
                "\tNeoForge: net.neoforged:21.1.252",
                LogLoader::NeoForge,
                None,
            ),
            (
                "\tForge: net.minecraftforge:47.4.10",
                LogLoader::Forge,
                None,
            ),
            ("Fabric Mods: ", LogLoader::Fabric, None),
            (
                "Launching target 'forgeclient' with arguments [--version, neoforge-21.1.252, --x",
                LogLoader::NeoForge,
                None,
            ),
            (
                "[--version, forge-1.20.1-47.4.10, --gameDir",
                LogLoader::Forge,
                Some("1.20.1"),
            ),
            (
                "[--version, 1.20.1-forge-47.2.0, --gameDir",
                LogLoader::Forge,
                Some("1.20.1"),
            ),
        ];
        for (text, loader, minecraft) in cases {
            let detected = detect_text(&[text]);
            assert_eq!(detected.loader, Some(loader), "{text}");
            assert_eq!(detected.minecraft.as_deref(), minecraft, "{text}");
        }
    }

    #[test]
    fn versao_do_minecraft_vem_de_outras_linhas() {
        let detected = detect_text(&[
            "\tMinecraft Version: 1.12.2",
            "MinecraftForge v14.23.5.2860 Initialized",
        ]);
        assert_eq!(detected.loader, Some(LogLoader::Forge));
        assert_eq!(detected.loader_version.as_deref(), Some("14.23.5.2860"));
        assert_eq!(detected.minecraft.as_deref(), Some("1.12.2"));
        let vanilla = detect_text(&["Starting minecraft server version 1.21.4"]);
        assert_eq!(vanilla.loader, None);
        assert_eq!(vanilla.minecraft.as_deref(), Some("1.21.4"));
        assert_eq!(detect_text(&["nada"]), Detected::default());
        // O loader veio de uma linha sem versão do jogo; a versão vem de outra linha do mesmo
        // loader, e a linha de outro loader não troca o que já foi achado.
        let neo = detect_text(&[
            "[--version, neoforge-21.1.252, --gameDir",
            "Loading Minecraft 1.20.1 with Fabric Loader 0.19.5",
            "NeoForge mod loading, version 21.1.252, for MC 1.21.1",
        ]);
        assert_eq!(neo.loader, Some(LogLoader::NeoForge));
        assert_eq!(neo.minecraft.as_deref(), Some("1.21.1"));
    }
}
