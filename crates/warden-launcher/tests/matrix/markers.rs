//! Marcadores observados no S-R5-3 §3 e §5. As decisões usam a mensagem já decodificada
//! pelo `LogParser`, como a supervisão do jogo; os recortes de origem estão nesta pasta.

use warden_launcher::log::{LogLine, LogParser, LogSource};

#[derive(Debug, Default)]
pub(crate) struct Progress {
    pub(crate) ready_ms: Option<u64>,
    pub(crate) world_ms: Option<u64>,
    pub(crate) server_ready_ms: Option<u64>,
    pub(crate) failure: Option<&'static str>,
    sound: bool,
    atlas: bool,
    fml_loaded: bool,
}

impl Progress {
    pub(crate) fn observe(&mut self, minecraft: &str, loader: &str, line: &LogLine, server: bool) {
        let message = line.text.as_str();
        // Uma tela de erro também pode criar atlas e iniciar o som. Falha ganha prioridade,
        // inclusive quando só aparece depois dos marcadores visuais.
        if let Some(reason) = failure(message) {
            self.failure = Some(reason);
            return;
        }
        if server {
            if message.contains("Done (") && message.contains("For help, type") {
                self.server_ready_ms.get_or_insert(line.elapsed_ms);
            }
            if message.contains("logged in with entity id") {
                self.world_ms.get_or_insert(line.elapsed_ms);
            }
            return;
        }
        self.sound |= message.contains("Sound engine started");
        self.fml_loaded |= message.contains("Forge Mod Loader has successfully loaded");
        self.atlas |= if minecraft == "1.7.10" {
            message.contains("textures/blocks-atlas") && !message.contains("16x16")
        } else if minecraft == "1.12.2" {
            message.contains("textures-atlas")
        } else {
            message.contains("blocks.png-atlas")
        };
        let legacy_fml = loader == "forge" && matches!(minecraft, "1.7.10" | "1.12.2");
        if self.sound && self.atlas && (!legacy_fml || self.fml_loaded) {
            self.ready_ms.get_or_insert(line.elapsed_ms);
        }
        if message.contains("logged in with entity id") {
            self.world_ms.get_or_insert(line.elapsed_ms);
        }
    }

    pub(crate) fn passed(&self, server: bool) -> bool {
        self.failure.is_none()
            && self.ready_ms.is_some()
            && self.world_ms.is_some()
            && (!server || self.server_ready_ms.is_some())
    }
}

pub(crate) fn failure(text: &str) -> Option<&'static str> {
    const SIGNALS: &[&str] = &[
        "Error during pre-loading phase",
        "Missing or unsupported mandatory dependencies",
        "Cowardly refusing to send event",
        "Couldn't place player in world",
        "java.lang.OutOfMemoryError",
        "OutOfMemoryError",
        "Mixin apply failed",
        "Mixin apply for mod",
        "Incompatible mods found!",
        "MissingModsException",
        "DuplicateModsFoundException",
        "UnsupportedClassVersionError",
        "Unsupported major.minor version",
        "Could not find or load main class",
        "Error: Could not create the Java Virtual Machine",
        "#@!@# Game crashed!",
    ];
    if text.contains("SplashProgress") && text.contains("Minecraft Crash Report") {
        return None;
    }
    if text.contains("ClassCastException") && text.contains("URLClassLoader") {
        return Some("Java antigo incompatível");
    }
    SIGNALS
        .iter()
        .copied()
        .find(|signal| text.contains(signal))
        .or_else(|| {
            text.contains("---- Minecraft Crash Report ----")
                .then_some("---- Minecraft Crash Report ----")
        })
}

#[allow(dead_code)] // O smoke real recebe LogLine do GameProcess; só o golden lê recortes.
pub(crate) fn parse_fixture(raw: &str) -> Vec<LogLine> {
    let mut parser = LogParser::new(LogSource::Stdout);
    let mut output = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        output.extend(parser.push(line, index as u64));
    }
    output.extend(parser.finish(raw.lines().count() as u64));
    output
}
