//! Tempos até "carregou" e até "entrou no mundo" (ARCHITECTURE §7.4 e §7.7; ADR-0038).
//!
//! [`LoadTracker`] recebe cada linha do jogo (a mensagem já decodificada pela supervisão, com
//! o tempo desde a abertura do processo) e marca o primeiro instante de cada marco. Os
//! marcadores vêm do spike S-R5-3 §3 e foram conferidos nos golden logs da matriz L-05 (20
//! combinações, de Forge 1.7.10 a Fabric 26.3):
//!
//! | Faixa | Carregou |
//! |---|---|
//! | 1.7.10 (e anteriores) | atlas de blocos **definitivo** (`textures/blocks-atlas`, nunca o `16x16` vazio) + `Sound engine started` |
//! | 1.8 a 1.12.2 | `textures-atlas` + `Sound engine started` |
//! | 1.13 em diante (e o calendário novo, 26.x) | `blocks.png-atlas` + `Sound engine started` |
//!
//! No Forge 1.7.10 e 1.12.2 também é preciso `Forge Mod Loader has successfully loaded` (a
//! barra do FML cria atlas antes de os mods terminarem). "Entrou no mundo" é `logged in with
//! entity id` (igual de 1.7.10 à mais nova; o servidor integrado escreve no log do cliente),
//! com `joined the game` como alternativa (cliente conectado a um servidor local).
//!
//! **Falha tem prioridade** sobre "carregou": no NeoForge e no Forge modernos a tela de erro de
//! carregamento também cria atlas e liga o som. Um sinal de falha antes de "carregou" impede o
//! marco; um sinal de **falha de carregamento** (dependência faltando, Mixin, mod duplicado…)
//! depois de "carregou" e antes de entrar no mundo apaga o marco (era a tela de erro). Um
//! travamento depois de carregar não apaga o tempo: o jogo carregou.
//!
//! Os tempos são do relógio do Warden (desde a abertura do processo), nunca do log: com o
//! Java 8u51 da Mojang o horário do log vem deslocado.

use serde::{Deserialize, Serialize};

/// O loader, para as regras do Forge antigo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Loader {
    /// Vanilla.
    Vanilla,
    /// Fabric.
    Fabric,
    /// Quilt.
    Quilt,
    /// Forge.
    Forge,
    /// NeoForge.
    NeoForge,
}

/// Os tempos de um teste, em milissegundos desde a abertura do processo.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct LoadTimes {
    /// Até o jogo carregar (menu principal).
    pub loaded_ms: Option<u32>,
    /// Até entrar no mundo (quando entrou).
    pub world_ms: Option<u32>,
}

/// Faixa de versão para o marcador de "carregou".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Range {
    /// 1.7.10 e anteriores.
    Legacy7,
    /// 1.8 a 1.12.2.
    Legacy12,
    /// 1.13 em diante.
    Modern,
}

impl Range {
    fn of(minecraft: &str) -> Self {
        let mut parts = minecraft.trim().split('.');
        let major = parts.next().and_then(|part| part.parse::<u32>().ok());
        let minor = parts.next().and_then(|part| {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u32>().ok()
        });
        match (major, minor) {
            (Some(1), Some(minor)) if minor <= 7 => Self::Legacy7,
            (Some(1), Some(minor)) if minor <= 12 => Self::Legacy12,
            // 1.13+, o calendário novo (26.x) e snapshots (`23w14a`).
            _ => Self::Modern,
        }
    }
}

/// Sinais de falha ao carregar: a tela de erro do loader (que também cria atlas e liga o som).
const LOAD_FAILURES: &[&str] = &[
    "Error during pre-loading phase",
    "Missing or unsupported mandatory dependencies",
    "Cowardly refusing to send event",
    "Mixin apply failed",
    "Mixin apply for mod",
    "Incompatible mods found!",
    "MissingModsException",
    "DuplicateModsFoundException",
    "UnsupportedClassVersionError",
    "Unsupported major.minor version",
    "Could not find or load main class",
    "Error: Could not create the Java Virtual Machine",
];

/// Sinais de travamento (em qualquer momento).
const CRASHES: &[&str] = &[
    "#@!@# Game crashed!",
    "Couldn't place player in world",
    "OutOfMemoryError",
];

/// Que tipo de falha uma linha anuncia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failure {
    /// Tela de erro do carregamento.
    Load,
    /// Travamento.
    Crash,
}

fn failure(text: &str) -> Option<Failure> {
    // O FML 1.7.10 escreve o cabeçalho de crash report ao fechar a barra de progresso, sem
    // travar (S-R5-3 §3.2).
    if text.contains("SplashProgress") && text.contains("Minecraft Crash Report") {
        return None;
    }
    if text.contains("ClassCastException") && text.contains("URLClassLoader") {
        // Java novo demais para o Forge antigo.
        return Some(Failure::Load);
    }
    if LOAD_FAILURES.iter().any(|signal| text.contains(signal)) {
        return Some(Failure::Load);
    }
    if CRASHES.iter().any(|signal| text.contains(signal))
        || text.contains("---- Minecraft Crash Report ----")
    {
        return Some(Failure::Crash);
    }
    None
}

/// Acompanha as linhas de um teste e marca "carregou" e "entrou no mundo".
#[derive(Debug, Clone)]
pub struct LoadTracker {
    range: Range,
    /// Forge 1.7.10 e 1.12.2: exige a linha do FML.
    needs_fml: bool,
    seen: Seen,
    times: LoadTimes,
    failed: bool,
}

/// As partes de "carregou" já vistas.
#[derive(Debug, Clone, Copy, Default)]
struct Seen {
    sound: bool,
    atlas: bool,
    fml_loaded: bool,
}

impl LoadTracker {
    /// Para o Minecraft e o loader do teste.
    #[must_use]
    pub fn new(minecraft: &str, loader: Loader) -> Self {
        let range = Range::of(minecraft);
        Self {
            range,
            needs_fml: loader == Loader::Forge && range != Range::Modern,
            seen: Seen::default(),
            times: LoadTimes::default(),
            failed: false,
        }
    }

    /// Os tempos até agora.
    #[must_use]
    pub fn times(&self) -> LoadTimes {
        self.times
    }

    /// Se apareceu um sinal de falha antes de entrar no mundo.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// Uma linha do jogo (`text` é a mensagem; `elapsed_ms`, o tempo desde a abertura do
    /// processo). Devolve se algum tempo mudou.
    pub fn observe(&mut self, text: &str, elapsed_ms: u64) -> bool {
        if self.times.world_ms.is_some() || self.failed {
            return false;
        }
        let before = self.times;
        if let Some(kind) = failure(text) {
            self.failed = true;
            if kind == Failure::Load {
                self.times.loaded_ms = None;
            }
            return self.times != before;
        }
        let elapsed = u32::try_from(elapsed_ms).unwrap_or(u32::MAX);
        if self.times.loaded_ms.is_none() {
            let seen = &mut self.seen;
            seen.sound |= text.contains("Sound engine started");
            seen.fml_loaded |= text.contains("Forge Mod Loader has successfully loaded");
            seen.atlas |= match self.range {
                Range::Legacy7 => text.contains("textures/blocks-atlas") && !text.contains("16x16"),
                Range::Legacy12 => text.contains("textures-atlas"),
                Range::Modern => text.contains("blocks.png-atlas"),
            };
            if seen.sound && seen.atlas && (!self.needs_fml || seen.fml_loaded) {
                self.times.loaded_ms = Some(elapsed);
            }
        }
        if text.contains("logged in with entity id") || text.contains("joined the game") {
            self.times.world_ms = Some(elapsed);
        }
        self.times != before
    }
}

/// A sessão de outra versão do pack com que o tempo é comparado.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviousLoad {
    /// A sessão (nome da pasta).
    pub session_id: String,
    /// A versão do pack daquela sessão.
    pub pack_version: String,
    /// Quanto ela levou para carregar.
    pub loaded_ms: u32,
}

/// Uma sessão anterior, como a comparação a vê.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PastSession<'a> {
    /// Nome da pasta.
    pub session_id: &'a str,
    /// Versão do pack testada.
    pub pack_version: Option<&'a str>,
    /// Tempo até carregar, se carregou.
    pub loaded_ms: Option<u32>,
}

/// A última sessão de **outra** versão do pack que carregou, para "18 s a mais que na 1.4.2".
/// `past` vem da mais nova para a mais antiga; sem versão no teste atual, não há comparação.
#[must_use]
pub fn previous_of_other_version<'a>(
    current_version: Option<&str>,
    past: impl IntoIterator<Item = PastSession<'a>>,
) -> Option<PreviousLoad> {
    let current = current_version
        .map(str::trim)
        .filter(|version| !version.is_empty())?;
    past.into_iter().find_map(|session| {
        let version = session.pack_version.map(str::trim)?;
        if version.is_empty() || version == current {
            return None;
        }
        Some(PreviousLoad {
            session_id: session.session_id.to_owned(),
            pack_version: version.to_owned(),
            loaded_ms: session.loaded_ms?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(tracker: &mut LoadTracker, lines: &[(&str, u64)]) {
        for (text, at) in lines {
            tracker.observe(text, *at);
        }
    }

    #[test]
    fn faixas_de_versao() {
        assert_eq!(Range::of("1.7.10"), Range::Legacy7);
        assert_eq!(Range::of("1.6.4"), Range::Legacy7);
        assert_eq!(Range::of("1.8.9"), Range::Legacy12);
        assert_eq!(Range::of("1.12.2"), Range::Legacy12);
        assert_eq!(Range::of("1.13"), Range::Modern);
        assert_eq!(Range::of("1.20.1"), Range::Modern);
        assert_eq!(Range::of("1.21.1-pre2"), Range::Modern);
        assert_eq!(Range::of("26.2"), Range::Modern);
        assert_eq!(Range::of("23w14a"), Range::Modern);
    }

    #[test]
    fn moderno_precisa_do_atlas_e_do_som() {
        let mut tracker = LoadTracker::new("1.20.1", Loader::Fabric);
        assert!(!tracker.observe("Sound engine started", 9_000));
        assert_eq!(tracker.times().loaded_ms, None);
        assert!(tracker.observe(
            "Created: 1024x1024x4 minecraft:textures/atlas/blocks.png-atlas",
            12_345
        ));
        assert_eq!(tracker.times().loaded_ms, Some(12_345));
        // O primeiro instante vale.
        tracker.observe(
            "Created: 512x512x4 minecraft:textures/atlas/blocks.png-atlas",
            20_000,
        );
        assert_eq!(tracker.times().loaded_ms, Some(12_345));
        assert!(tracker.observe(
            "Jogador[local:E:1] logged in with entity id 42 at (0, 64, 0)",
            30_000
        ));
        assert_eq!(
            tracker.times(),
            LoadTimes {
                loaded_ms: Some(12_345),
                world_ms: Some(30_000)
            }
        );
    }

    #[test]
    fn forge_antigo_precisa_do_fml() {
        let mut tracker = LoadTracker::new("1.12.2", Loader::Forge);
        feed(
            &mut tracker,
            &[
                ("Created: 512x512 textures-atlas", 1),
                ("Sound engine started", 2),
            ],
        );
        assert_eq!(tracker.times().loaded_ms, None);
        tracker.observe("Forge Mod Loader has successfully loaded 12 mods", 3);
        assert_eq!(tracker.times().loaded_ms, Some(3));
        // No Fabric da mesma faixa (se houvesse), o FML não conta.
        let mut vanilla = LoadTracker::new("1.12.2", Loader::Vanilla);
        feed(
            &mut vanilla,
            &[
                ("Created: 512x512 textures-atlas", 1),
                ("Sound engine started", 2),
            ],
        );
        assert_eq!(vanilla.times().loaded_ms, Some(2));
    }

    #[test]
    fn atlas_vazio_do_1_7_10_nao_conta() {
        let mut tracker = LoadTracker::new("1.7.10", Loader::Forge);
        feed(
            &mut tracker,
            &[
                ("Forge Mod Loader has successfully loaded 3 mods", 1),
                ("Sound engine started", 2),
                ("Created: 16x16 textures/blocks-atlas", 3),
            ],
        );
        assert_eq!(tracker.times().loaded_ms, None);
        tracker.observe("Created: 1024x512 textures/blocks-atlas", 4);
        assert_eq!(tracker.times().loaded_ms, Some(4));
    }

    #[test]
    fn tela_de_erro_apaga_o_carregou() {
        let mut tracker = LoadTracker::new("1.21.1", Loader::NeoForge);
        feed(
            &mut tracker,
            &[
                (
                    "Created: 1024x1024x0 minecraft:textures/atlas/blocks.png-atlas",
                    5,
                ),
                ("Sound engine started", 6),
            ],
        );
        assert_eq!(tracker.times().loaded_ms, Some(6));
        assert!(tracker.observe("Missing or unsupported mandatory dependencies:", 7));
        assert_eq!(tracker.times(), LoadTimes::default());
        assert!(tracker.failed());
        // Depois da falha nada mais conta.
        assert!(!tracker.observe("logged in with entity id 1", 8));
    }

    #[test]
    fn falha_antes_de_carregar_impede_o_marco() {
        let mut tracker = LoadTracker::new("1.20.1", Loader::Forge);
        feed(
            &mut tracker,
            &[
                ("Mixin apply failed foo.mixins.json:BarMixin", 1),
                ("Sound engine started", 2),
                (
                    "Created: 1024x1024x4 minecraft:textures/atlas/blocks.png-atlas",
                    3,
                ),
            ],
        );
        assert_eq!(tracker.times().loaded_ms, None);
    }

    #[test]
    fn travar_depois_de_carregar_mantem_o_tempo() {
        let mut tracker = LoadTracker::new("1.20.1", Loader::Fabric);
        feed(
            &mut tracker,
            &[
                ("Sound engine started", 1),
                (
                    "Created: 1024x1024x4 minecraft:textures/atlas/blocks.png-atlas",
                    2,
                ),
                ("#@!@# Game crashed! Crash report saved to: crash.txt", 3),
                ("logged in with entity id 1", 4),
            ],
        );
        assert_eq!(
            tracker.times(),
            LoadTimes {
                loaded_ms: Some(2),
                world_ms: None
            }
        );
        // Depois do mundo, falhas não mexem mais.
        let mut entered = LoadTracker::new("1.20.1", Loader::Fabric);
        feed(
            &mut entered,
            &[
                ("Sound engine started", 1),
                (
                    "Created: 1024x1024x4 minecraft:textures/atlas/blocks.png-atlas",
                    2,
                ),
                ("[CHAT] Jogador joined the game", 3),
                ("Mixin apply failed", 4),
            ],
        );
        assert_eq!(entered.times().world_ms, Some(3));
        assert_eq!(entered.times().loaded_ms, Some(2));
    }

    #[test]
    fn falso_crash_do_splash_1_7_10_nao_e_falha() {
        let mut tracker = LoadTracker::new("1.7.10", Loader::Forge);
        tracker.observe(
            "[SplashProgress] ---- Minecraft Crash Report ---- (barra de progresso)",
            1,
        );
        assert!(!tracker.failed());
    }

    #[test]
    fn comparacao_pega_a_ultima_de_outra_versao() {
        let past = [
            PastSession {
                session_id: "c",
                pack_version: Some("1.4.3"),
                loaded_ms: Some(90_000),
            },
            PastSession {
                session_id: "b",
                pack_version: Some("1.4.2"),
                loaded_ms: None,
            },
            PastSession {
                session_id: "a",
                pack_version: Some(" 1.4.2 "),
                loaded_ms: Some(84_000),
            },
        ];
        assert_eq!(
            previous_of_other_version(Some("1.4.3"), past),
            Some(PreviousLoad {
                session_id: "a".into(),
                pack_version: "1.4.2".into(),
                loaded_ms: 84_000,
            })
        );
        assert_eq!(previous_of_other_version(None, past), None);
        assert_eq!(previous_of_other_version(Some(""), past), None);
        assert_eq!(
            previous_of_other_version(Some("2.0"), past).map(|load| load.session_id),
            Some("c".to_owned())
        );
        assert_eq!(previous_of_other_version(Some("1.4.3"), []), None);
    }
}
