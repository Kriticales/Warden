//! De quem é cada linha (ARCHITECTURE §9.8, "Atribuição"), nesta ordem: jar no frame do stack
//! trace (Forge 1.17+), módulo no frame (NeoForge 1.20.5+), nome do *handler* do mixin (Fabric),
//! "Mixins in Stacktrace", config de mixin citada na mensagem, nome do logger (quando é um mod
//! conhecido) e índice pacote → mod da D-05. Sem sinal: "Desconhecido"; jogo, loader e Java:
//! "Jogo e loader".

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use warden_jarmeta::Loader;
use warden_jarmeta::index::PackIndex;

use crate::postcrash::stack::{FABRIC_HANDLER, FORGE_JAR, NEOFORGE_MODULE, NOT_MOD_JARS, NOT_MODS};
use crate::postcrash::text_fixed_regex;

/// Dono de uma linha.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Owner {
    /// Um item do pack (o metafile, `mods/sodium.pw.toml`), com o mod que deu o sinal.
    Item {
        /// Caminho do metafile.
        item: String,
        /// Id do mod, quando o sinal trouxe (`sodium`).
        mod_id: Option<String>,
    },
    /// Um mod que o índice não conhece (fora do pack ou sem jar indexado).
    Mod(String),
    /// O jogo, o loader ou o Java.
    Game,
    /// Sem sinal.
    Unknown,
}

impl Owner {
    /// É de um mod (do pack ou não)?
    #[must_use]
    pub const fn is_mod(&self) -> bool {
        matches!(self, Self::Item { .. } | Self::Mod(_))
    }
}

/// Pacotes do jogo, do loader, do Java e das bibliotecas comuns.
const GAME_PACKAGES: [&str; 30] = [
    "java.",
    "javax.",
    "jdk.",
    "sun.",
    "com.sun.",
    "net.minecraft.",
    "com.mojang.",
    "net.minecraftforge.",
    "net.neoforged.",
    "cpw.mods.",
    "net.fabricmc.",
    "org.quiltmc.",
    "org.spongepowered.",
    "com.llamalad7.mixinextras.",
    "io.netty.",
    "org.lwjgl.",
    "com.google.",
    "org.apache.",
    "org.slf4j.",
    "it.unimi.",
    "oshi.",
    "com.electronwill.",
    "joptsimple.",
    "org.objectweb.",
    "net.jodah.",
    "com.ibm.icu.",
    "paulscode.",
    "org.jline.",
    "LZMA.",
    "kotlin.",
];

/// Os mesmos pacotes abreviados como o Forge e o NeoForge escrevem no logger (`ne.mi.`).
const GAME_ABBREVIATIONS: [&str; 8] = [
    "ne.mi.", "ne.ne.", "cp.mo.", "co.mo.", "ne.fa.", "or.sp.", "ja.", "jd.",
];

/// Nomes de logger do jogo e do loader (comparados em minúsculas).
const GAME_LOGGERS: [&str; 16] = [
    "minecraft",
    "fml",
    "forge",
    "neoforge",
    "mixin",
    "launchwrapper",
    "modlauncher",
    "fabricloader",
    "fabric-loader",
    "quilt loader",
    "stdout",
    "stderr",
    "mojang",
    "authlib",
    "oshi",
    "mixinextras|service",
];

/// `Mixins in Stacktrace:` e as configs listadas embaixo.
static MIXIN_CONFIG: LazyLock<Regex> =
    LazyLock::new(|| text_fixed_regex(r"(?P<config>[A-Za-z0-9_.\-]+\.json)\b"));

/// `Mixin apply for mod wardenfalhas failed …`, `… from mod sodium -> …` (o próprio Mixin diz o
/// mod da config).
static MIXIN_MOD: LazyLock<Regex> =
    LazyLock::new(|| text_fixed_regex(r"\b(?:for|from) mod (?P<id>[a-z][a-z0-9_\-]{1,63})\b"));

/// Os mods do pack, para atribuir as linhas.
#[derive(Debug, Default)]
pub struct ModDirectory {
    index: PackIndex,
    jars: HashMap<String, String>,
    mods: HashMap<String, (String, String)>,
    abbreviations: HashMap<String, Option<(String, Option<String>)>>,
    fabric: bool,
}

impl ModDirectory {
    /// Sem mods: só "Jogo e loader" e "Desconhecido" (e mods citados pelo id).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// O índice do pack (D-05) e os arquivos dos itens (`(metafile, nome do jar)`).
    #[must_use]
    pub fn new(index: PackIndex, files: impl IntoIterator<Item = (String, String)>) -> Self {
        let loader = index.loader();
        let mut jars = HashMap::new();
        for (item, filename) in files {
            jars.entry(filename.to_lowercase()).or_insert(item);
        }
        let mut mods: HashMap<String, (String, String)> = HashMap::new();
        let mut abbreviations: HashMap<String, Option<(String, Option<String>)>> = HashMap::new();
        for (item, jar_index) in index.items() {
            for jar in &jar_index.jars {
                if !jar.path.is_empty() {
                    let name = jar.path.rsplit(['/', '!']).next().unwrap_or_default();
                    jars.entry(name.to_lowercase())
                        .or_insert_with(|| item.clone());
                }
                for module in &jar.mods {
                    mods.entry(compact(&module.id))
                        .or_insert_with(|| (item.clone(), module.id.clone()));
                }
                let mod_id = jar.primary_mod(loader).map(str::to_owned);
                for package in &jar.packages {
                    let owner = Some((item.clone(), mod_id.clone()));
                    abbreviations
                        .entry(abbreviate(package))
                        .and_modify(|current| {
                            if current.as_ref().map(|(i, _)| i) != Some(item) {
                                *current = None;
                            }
                        })
                        .or_insert(owner);
                }
            }
        }
        Self {
            index,
            jars,
            mods,
            abbreviations,
            fabric: matches!(loader, Some(Loader::Fabric | Loader::Quilt)),
        }
    }

    /// O dono de um mod pelo id.
    pub(crate) fn by_mod_id(&self, id: &str) -> Owner {
        let lower = id.to_lowercase();
        if NOT_MODS.contains(&lower.as_str()) {
            return Owner::Game;
        }
        match self.mods.get(&compact(id)) {
            Some((item, mod_id)) => Owner::Item {
                item: item.clone(),
                mod_id: Some(mod_id.clone()),
            },
            None => Owner::Mod(id.to_owned()),
        }
    }

    /// O dono de um jar pelo nome do arquivo (`ColdSweat-2.2.jar`).
    fn by_jar(&self, filename: &str) -> Option<Owner> {
        let lower = filename.to_lowercase();
        if NOT_MOD_JARS.iter().any(|prefix| lower.starts_with(prefix)) {
            return Some(Owner::Game);
        }
        self.jars.get(&lower).map(|item| Owner::Item {
            item: item.clone(),
            mod_id: None,
        })
    }

    /// O dono de uma classe (`me.x.Y`, aceita `/`), pelo índice de pacotes ou pelos pacotes do
    /// jogo.
    pub(crate) fn by_class(&self, class: &str) -> Option<Owner> {
        if let Some(owner) = self.index.class_owner(class).and_then(|o| o.owner()) {
            return Some(Owner::Item {
                item: owner.item.to_owned(),
                mod_id: owner.mod_id.map(str::to_owned),
            });
        }
        let dotted = class.replace('/', ".");
        GAME_PACKAGES
            .iter()
            .any(|prefix| dotted.starts_with(prefix))
            .then_some(Owner::Game)
    }

    /// O dono de uma config de mixin (`sodium.mixins.json`).
    pub(crate) fn by_mixin_config(&self, config: &str) -> Option<Owner> {
        let owner = self.index.mixin_owner(config)?;
        Some(Owner::Item {
            item: owner.owner.item.to_owned(),
            mod_id: owner.owner.mod_id.map(str::to_owned),
        })
    }

    /// Classe abreviada (`ne.ne.ne.co.NeoForgeMod`, `%c{2.}` do log4j).
    fn by_abbreviated_class(&self, class: &str) -> Option<Owner> {
        if GAME_ABBREVIATIONS
            .iter()
            .any(|prefix| class.starts_with(prefix))
        {
            return Some(Owner::Game);
        }
        let (package, _) = class.rsplit_once('.')?;
        let (item, mod_id) = self.abbreviations.get(package)?.clone()?;
        Some(Owner::Item { item, mod_id })
    }

    /// O dono pelo nome do logger. `None` = o logger não diz.
    pub(crate) fn by_logger(&self, logger: &str) -> Option<Owner> {
        let name = logger.split('/').next().unwrap_or(logger).trim();
        if name.is_empty() {
            return None;
        }
        let lower = name.to_lowercase();
        if GAME_LOGGERS.contains(&lower.as_str()) || lower.starts_with("mixin") {
            return Some(Owner::Game);
        }
        if name.contains('.') {
            if let Some(owner) = self.by_class(name) {
                return Some(owner);
            }
            let abbreviated = name
                .rsplit_once('.')
                .is_some_and(|(package, _)| package.split('.').all(|part| part.len() <= 2));
            return abbreviated
                .then(|| self.by_abbreviated_class(name))
                .flatten();
        }
        if NOT_MODS.contains(&lower.as_str()) {
            return Some(Owner::Game);
        }
        self.mods
            .get(&compact(name))
            .map(|(item, mod_id)| Owner::Item {
                item: item.clone(),
                mod_id: Some(mod_id.clone()),
            })
    }

    /// O dono pela config de mixin citada na primeira linha da mensagem (`Mixin apply failed
    /// x.mixins.json:…`, `Reference map '…' for x.mixins.json`) ou pelo mod que o Mixin cita
    /// junto (`for mod x`, `from mod x`).
    pub(crate) fn by_mentioned_mixin(&self, text: &str) -> Option<Owner> {
        if !text.contains(".json") {
            return None;
        }
        let first = text.split('\n').next().unwrap_or(text);
        MIXIN_CONFIG
            .captures_iter(first)
            .filter_map(|captures| captures.name("config"))
            .find_map(|config| self.by_mixin_config(config.as_str()))
            .or_else(|| {
                let id = MIXIN_MOD.captures(first)?.name("id")?;
                Some(self.by_mod_id(id.as_str())).filter(Owner::is_mod)
            })
    }

    /// O dono de um frame (`\tat …`), pelos sinais do próprio frame. `None` = sem sinal.
    pub(crate) fn by_frame(&self, frame: &str) -> Option<Owner> {
        if let Some(jar) = FORGE_JAR.captures(frame).and_then(|c| c.name("jar"))
            && let Some(owner) = self.by_jar(jar.as_str())
            && owner.is_mod()
        {
            return Some(owner);
        }
        if let Some(id) = NEOFORGE_MODULE.captures(frame).and_then(|c| c.name("id")) {
            let id = id.as_str();
            if !id.starts_with("java") && !id.starts_with("jdk") {
                let owner = self.by_mod_id(id);
                if owner.is_mod() {
                    return Some(owner);
                }
            }
        }
        if self.fabric
            && let Some(id) = FABRIC_HANDLER.captures(frame).and_then(|c| c.name("id"))
        {
            let owner = self.by_mod_id(id.as_str());
            if owner.is_mod() {
                return Some(owner);
            }
        }
        frame_class(frame).and_then(|class| self.by_class(class))
    }
}

/// A classe de um frame: `at TRANSFORMER/x@1/net.a.B.m(B.java:1)` → `net.a.B`.
fn frame_class(frame: &str) -> Option<&str> {
    let rest = frame.trim_start().strip_prefix("at ")?;
    let call = rest.split('(').next()?;
    let call = call.rsplit('/').next()?;
    let (class, _) = call.rsplit_once('.')?;
    Some(class)
}

/// O frame é uma linha `at …`.
pub(crate) fn is_frame(line: &str) -> bool {
    line.trim_start().starts_with("at ")
}

/// Id sem maiúsculas, espaços nem pontuação, para casar `Sodium` com `sodium` e `xaero-minimap`
/// com `xaerominimap`.
fn compact(id: &str) -> String {
    id.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// `net.neoforged.neoforge.common` → `ne.ne.ne.co`.
fn abbreviate(package: &str) -> String {
    package
        .split('.')
        .map(|part| part.get(..2).unwrap_or(part))
        .collect::<Vec<_>>()
        .join(".")
}

/// Configs listadas em "Mixins in Stacktrace:" (crash report do NeoForge, `MixinTrace`).
pub(crate) fn stacktrace_mixins(lines: &[&str]) -> Vec<String> {
    let Some(start) = lines
        .iter()
        .position(|line| line.trim_start().starts_with("Mixins in Stacktrace:"))
    else {
        return Vec::new();
    };
    lines[start + 1..]
        .iter()
        .take_while(|line| line.starts_with([' ', '\t']))
        .filter_map(|line| MIXIN_CONFIG.captures(line))
        .filter_map(|captures| captures.name("config").map(|m| m.as_str().to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classe_do_frame() {
        assert_eq!(
            frame_class("\tat TRANSFORMER/supplementaries@1.21-3.9.9/net.x.Y.m(Y.java:1)"),
            Some("net.x.Y")
        );
        assert_eq!(
            frame_class("\tat java.base/java.lang.Thread.run(Thread.java:1)"),
            Some("java.lang.Thread")
        );
        assert_eq!(frame_class("\tat a.b.C.<init>(C.java:1)"), Some("a.b.C"));
        assert_eq!(frame_class("Caused by: x"), None);
    }

    #[test]
    fn abreviacao_e_ids() {
        assert_eq!(abbreviate("net.neoforged.neoforge.common"), "ne.ne.ne.co");
        assert_eq!(compact("Xaero's-Minimap"), "xaerosminimap");
    }

    #[test]
    fn sem_indice_jogo_mod_e_desconhecido() {
        let dir = ModDirectory::empty();
        assert_eq!(dir.by_logger("minecraft/Minecraft"), Some(Owner::Game));
        assert_eq!(dir.by_logger("net.minecraft.class_3324"), Some(Owner::Game));
        assert_eq!(
            dir.by_logger("ne.ne.ne.co.NeoForgeMod/NEOFORGE-MOD"),
            Some(Owner::Game)
        );
        assert_eq!(
            dir.by_logger("cpw.mods.modlauncher.Launcher"),
            Some(Owner::Game)
        );
        assert_eq!(dir.by_logger("MixinExtras|Service"), Some(Owner::Game));
        assert_eq!(dir.by_logger("Sodium"), None);
        assert_eq!(dir.by_logger("me.x.y.Z"), None);
        assert_eq!(
            dir.by_frame("\tat TRANSFORMER/epicfight@21.1/yesman.X.m(X.java:1)"),
            Some(Owner::Mod("epicfight".into()))
        );
        assert_eq!(
            dir.by_frame("\tat TRANSFORMER/minecraft@1.21.1/net.minecraft.client.Minecraft.m(Minecraft.java:1)"),
            Some(Owner::Game)
        );
        assert_eq!(dir.by_frame("\tat foo.Bar.m(Bar.java:1)"), None);
    }

    #[test]
    fn mixins_in_stacktrace() {
        let lines = [
            "java.lang.NullPointerException: x",
            "\tat a.B.c(B.java:1)",
            "Mixins in Stacktrace:",
            "\tnet.minecraft.client.Minecraft:",
            "\t\tsodium.mixins.json:core.MinecraftMixin (sodium)",
            "outra coisa",
        ];
        assert_eq!(stacktrace_mixins(&lines), ["sodium.mixins.json"]);
    }
}
