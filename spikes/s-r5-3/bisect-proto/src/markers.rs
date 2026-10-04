//! Catálogo de marcadores observados no log. Cada linha que casa com um padrão é
//! registrada com o tempo desde o início do processo (só a primeira ocorrência de
//! cada marcador). Os padrões vieram do S1 e da R5A §3.4 e foram ajustados com os
//! logs reais deste spike (ver o relatório S-R5-3).

pub struct Marker {
    pub id: &'static str,
    /// A linha casa se contiver **todos** os trechos.
    pub all: &'static [&'static str],
    /// ... e nenhum destes.
    pub none: &'static [&'static str],
}

const fn m(id: &'static str, all: &'static [&'static str]) -> Marker {
    Marker { id, all, none: &[] }
}

pub const CATALOG: &[Marker] = &[
    m("fabric-loader", &["with Fabric Loader"]),
    m("fml-legacy-version", &["Forge Mod Loader version"]),
    m("launch-target", &["Launching target '"]),
    m("neoforge-loading", &["NeoForge mod loading, version"]),
    m("forge-loading", &["Forge mod loading, version"]),
    m("setting-user", &["Setting user: "]),
    m("fml-loaded", &["Forge Mod Loader has successfully loaded"]),
    m("sound-started", &["Sound engine started"]),
    m("atlas-blocks", &["blocks-atlas"]),
    // 1.7.10: o FML cria um atlas vazio 16x16 antes de carregar os mods; o definitivo vem depois.
    Marker { id: "atlas-blocks-final", all: &["Created: ", "textures/blocks-atlas"], none: &["16x16"] },
    m("atlas-textures", &["textures-atlas"]),
    m("atlas-blocks-png", &["blocks.png-atlas"]),
    m("modernfix-took", &["Game took ", " seconds to start"]),
    m("starting-integrated", &["Starting integrated minecraft server"]),
    m("preparing-spawn", &["Preparing spawn area"]),
    m("preparing-start-region", &["Preparing start region"]),
    m("connecting", &["Connecting to "]),
    m("logged-in", &["logged in with entity id"]),
    m("joined-chat", &["[CHAT]", "joined the game"]),
    Marker { id: "joined", all: &["joined the game"], none: &["[CHAT]"] },
    m("server-done", &["Done (", "For help, type"]),
    m("server-stopping", &["Stopping server"]),
    m("startup-query", &["StartupQuery"]),
    m("fml-query-auto", &["fml.queryResult"]),
    m("missing-registry", &["issing registry"]),
    m("missing-mods", &["issing mods"]),
    m("crash-report-saved", &["crash report saved to", "#@!@#"]),
    m("crash-report", &["---- Minecraft Crash Report ----"]),
    m("fabric-incompatible", &["Incompatible mods found!"]),
    m("fabric-gui", &["FabricGuiEntry"]),
    m("mod-resolution", &["Mod resolution failed"]),
    m("loading-errors", &["Loading errors encountered"]),
    m("modloading-issue", &["ModLoadingException"]),
    m("missing-dependency", &["requires", "which is missing"]),
    m("experimental", &["xperimental"]),
    m("stopping", &["Stopping!"]),
];

pub fn matches(marker: &Marker, line: &str) -> bool {
    marker.all.iter().all(|p| line.contains(p)) && !marker.none.iter().any(|p| line.contains(p))
}

/// Faixa da matriz L-05 a que a versão pertence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// 1.7.10 (e 1.8–1.12 Forge legado, LaunchWrapper).
    Legacy,
    /// 1.13–1.19.4.
    Modern,
    /// 1.20+ (Quick Play).
    QuickPlay,
}

pub fn band(mc: &str) -> Band {
    let minor: u32 = mc.split('.').nth(1).and_then(|s| s.parse().ok()).unwrap_or(99);
    let major: u32 = mc.split('.').next().and_then(|s| s.parse().ok()).unwrap_or(1);
    if major > 1 || minor >= 20 {
        Band::QuickPlay
    } else if minor >= 13 {
        Band::Modern
    } else {
        Band::Legacy
    }
}

/// Marcadores que precisam aparecer (todos) para considerar "cliente pronto" (menu).
/// `loader` é "vanilla", "fabric", "forge" ou "neoforge".
pub fn ready_set(mc: &str, loader: &str) -> Vec<&'static str> {
    let mut v = vec!["sound-started"];
    match band(mc) {
        Band::Legacy => {
            v.push(if mc.starts_with("1.7") { "atlas-blocks-final" } else { "atlas-textures" });
            if loader == "forge" {
                v.push("fml-loaded");
            }
        }
        _ => v.push("atlas-blocks-png"),
    }
    v
}

/// Qualquer um destes indica "entrou no mundo".
pub const WORLD_ANY: &[&str] = &["logged-in", "joined-chat", "joined"];

/// Padrões fatais (do S1, com a exceção do `SplashProgress` do FML 1.7.10).
pub fn is_fatal(line: &str) -> Option<&'static str> {
    const NOT_FATAL: &[&str] = &["SplashProgress"];
    const FATAL: &[&str] = &[
        "---- Minecraft Crash Report ----",
        "#@!@# Game crashed!",
        "Exception in thread \"main\"",
        "Could not find or load main class",
        "Error: Could not create the Java Virtual Machine",
        "Encountered an unexpected exception",
    ];
    if NOT_FATAL.iter().any(|n| line.contains(n)) {
        return None;
    }
    FATAL.iter().copied().find(|f| line.contains(f))
}
