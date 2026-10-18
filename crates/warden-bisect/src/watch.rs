//! O veredito de uma rodada a partir da saída do jogo (ARCHITECTURE §9.7 item 8; S-R5-3 §3,
//! §5 e §14 itens 1 a 6).
//!
//! [`RoundWatcher`] recebe cada linha (a mensagem já decodificada pela supervisão, a mesma
//! do console, com XML do log4j e texto puro na mesma saída) e os tiques do relógio, e diz
//! quando a rodada terminou:
//!
//! - **pronto** e **entrou no mundo** são os do [`warden_perf::LoadTracker`] (L-10), conferidos
//!   nos golden logs da matriz: 1.7.10 com o atlas definitivo (não o `16x16`), 1.8–1.12.2 com
//!   `successfully loaded` + `textures-atlas` + som, 1.13+ com `blocks.png-atlas` + som; o
//!   mundo por `logged in with entity id` ou `joined the game`;
//! - **sinais de falha têm prioridade** sobre os de pronto ([`Anchor`]): no NeoForge e no Forge
//!   modernos a tela de erro de carregamento cria o atlas e liga o som, então `Error during
//!   pre-loading phase`, `Missing or unsupported mandatory dependencies` e `Cowardly refusing
//!   to send event` marcam a rodada como falha mesmo depois do "pronto";
//! - **Java errado** (`UnsupportedClassVersionError`, `Unsupported major.minor version`,
//!   `ClassCastException … URLClassLoader`) é problema de ambiente, não de mod;
//! - a **assinatura** da rodada é a âncora mais a primeira exceção depois dela (ou as
//!   dependências que faltam, nas falhas de dependência; ou a config de mixin e o alvo), com
//!   números trocados por `#` e sem caminhos; o ruído conhecido (401 do perfil offline,
//!   rede da Mojang, classes opcionais de mixin) não conta;
//! - **jogo vivo e calado:** depois de `Starting integrated minecraft server` (ou `Connected to
//!   a modded server`), sem linha nova e sem entrar no mundo por N s, a rodada "travou";
//! - **passou** = entrou no mundo (ou, sem mundo de teste, chegou ao menu) e ficou
//!   [`crate::estimate::STABILITY_MS`] sem sinal de falha.
//!
//! Quem chama encerra o jogo assim que recebe a [`Decision`]: várias falhas não fecham o jogo
//! sozinhas (S-R5-3 §4.6).

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use warden_perf::{LoadTracker, Loader};

use crate::estimate::STABILITY_MS;

/// Linhas lidas depois da âncora à procura da exceção.
const SIGNATURE_WINDOW_LINES: usize = 40;
/// Tempo esperando a exceção depois da âncora.
const SIGNATURE_WINDOW_MS: u64 = 2_000;
/// Mínimo do "jogo vivo e calado" (S-R5-3 §14 item 6).
pub const FREEZE_FLOOR_MS: u64 = 60_000;
/// Tamanho máximo do texto da assinatura.
const MAX_SIGNATURE_CHARS: usize = 240;

/// O que a rodada precisa alcançar para passar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Goal {
    /// Entrar no mundo (Quick Play) e ficar estável.
    World,
    /// Chegar ao menu principal e ficar estável (sem mundo de teste; só para "trava ao abrir").
    Menu,
}

/// Configuração de uma rodada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchConfig {
    /// Versão do Minecraft.
    pub minecraft: String,
    /// Loader.
    pub loader: Loader,
    /// O que é "passou".
    pub goal: Goal,
    /// Modo "aparece uma mensagem no log": o trecho procurado.
    pub log_line: Option<String>,
    /// Modo assistido: ao entrar no mundo, pergunta ao usuário.
    pub assisted: bool,
    /// Limite do "jogo vivo e calado".
    pub freeze_ms: u64,
    /// Tempo-limite da rodada inteira.
    pub max_ms: u64,
}

/// O sinal que decidiu a falha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    /// Java errado para a versão: problema de ambiente.
    JavaVersion,
    /// Tela de erro de carregamento (`Error during pre-loading phase`, `Cowardly refusing…`,
    /// `Failed to create mod instance`, `Mod loading has failed`).
    LoadingError,
    /// Dependência faltando (`Missing or unsupported mandatory dependencies`,
    /// `MissingModsException`, `Incompatible mods found!`).
    MissingDependency,
    /// `DuplicateModsFoundException`.
    DuplicateMods,
    /// `Mixin apply … failed`.
    Mixin,
    /// `Couldn't place player in world`: falhou ao entrar, sem crash.
    JoinWorld,
    /// `OutOfMemoryError` em qualquer thread.
    OutOfMemory,
    /// Crash report novo.
    CrashReport,
    /// Jogo vivo e calado depois de começar o mundo.
    Frozen,
    /// O processo saiu sozinho sem nenhum outro sinal.
    ProcessExit,
    /// A linha escolhida no modo "aparece uma mensagem no log".
    LogLine,
}

impl Anchor {
    /// Se é problema de ambiente (para a busca).
    #[must_use]
    pub fn is_environment(self) -> bool {
        self == Self::JavaVersion
    }

    fn id(self) -> &'static str {
        match self {
            Self::JavaVersion => "java",
            Self::LoadingError => "carregamento",
            Self::MissingDependency => "dependencia",
            Self::DuplicateMods => "duplicado",
            Self::Mixin => "mixin",
            Self::JoinWorld => "entrar",
            Self::OutOfMemory => "memoria",
            Self::CrashReport => "crash",
            Self::Frozen => "travou",
            Self::ProcessExit => "saiu",
            Self::LogLine => "linha",
        }
    }
}

/// Em que momento falhou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    /// Antes do menu principal ("trava ao abrir o jogo").
    Opening,
    /// Depois do menu ("trava ao entrar no mundo" ou já no mundo).
    World,
}

/// "O mod `from` precisa do mod `to`", lido do log.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct DependencyHint {
    /// Id do mod que precisa.
    pub from: String,
    /// Id do mod de que ele precisa.
    pub to: String,
}

/// Uma classe que não foi achada e, quando o log diz, o mod que a procurou.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MissingClass {
    /// Nome da classe com pontos (`net.mehvahdjukaar.moonlight.api.Foo`).
    pub class: String,
    /// Id do mod que a procurou (`Failed to create mod instance. ModID: …` ou o primeiro frame
    /// de mod da pilha).
    pub needed_by: Option<String>,
}

/// Uma falha da rodada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    /// O sinal.
    pub anchor: Anchor,
    /// Antes ou depois do menu.
    pub stage: Stage,
    /// A assinatura da rodada (âncora + primeira exceção, normalizada).
    pub signature: String,
    /// Milissegundos desde a abertura do processo.
    #[specta(type = specta_typescript::Number)]
    pub at_ms: u64,
    /// Dependências que o log diz faltarem.
    pub hints: Vec<DependencyHint>,
    /// Classes não achadas.
    pub missing_classes: Vec<MissingClass>,
}

/// Como a rodada terminou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Decision {
    /// Chegou ao objetivo e ficou estável.
    Passed {
        /// Até entrar no mundo (ou chegar ao menu).
        #[specta(type = specta_typescript::Number)]
        reached_ms: u64,
    },
    /// Falhou.
    Failed(Failure),
    /// Nenhum marcador no tempo-limite.
    Timeout,
}

/// Uma âncora vista, à espera da exceção que vem logo depois.
#[derive(Debug, Clone)]
struct Pending {
    anchor: Anchor,
    stage: Stage,
    at_ms: u64,
    lines: usize,
    /// O que já dá para a assinatura (mixin, linha) sem esperar a exceção.
    fixed: Option<String>,
    exception: Option<String>,
}

/// Acompanha uma rodada.
#[derive(Debug, Clone)]
pub struct RoundWatcher {
    config: WatchConfig,
    tracker: LoadTracker,
    pending: Option<Pending>,
    decided: Option<Decision>,
    asked: bool,
    started_world_ms: Option<u64>,
    last_line_ms: u64,
    hints: Vec<DependencyHint>,
    missing: Vec<MissingClass>,
    /// Mod citado por `Failed to create mod instance` (para a próxima `NoClassDefFoundError`).
    creating: Option<String>,
    /// Quando o arquivo do `--quickPlayPath` apareceu (sinal secundário de "entrou no mundo").
    world_file_ms: Option<u64>,
}

impl RoundWatcher {
    /// Começa uma rodada.
    #[must_use]
    pub fn new(config: WatchConfig) -> Self {
        let tracker = LoadTracker::new(&config.minecraft, config.loader);
        Self {
            config,
            tracker,
            pending: None,
            decided: None,
            asked: false,
            started_world_ms: None,
            last_line_ms: 0,
            hints: Vec::new(),
            missing: Vec::new(),
            creating: None,
            world_file_ms: None,
        }
    }

    /// A decisão, se já houver.
    #[must_use]
    pub fn decision(&self) -> Option<&Decision> {
        self.decided.as_ref()
    }

    /// Até carregar (menu), se carregou.
    #[must_use]
    pub fn loaded_ms(&self) -> Option<u64> {
        self.tracker.times().loaded_ms.map(u64::from)
    }

    /// Até entrar no mundo, se entrou (pelo log ou, como sinal secundário, pelo arquivo do
    /// `--quickPlayPath`).
    #[must_use]
    pub fn world_ms(&self) -> Option<u64> {
        self.tracker
            .times()
            .world_ms
            .map(u64::from)
            .or(self.world_file_ms)
    }

    /// O conteúdo do arquivo do `--quickPlayPath` (o jogo o grava 0,3–1 s depois de entrar no
    /// mundo; S-R5-3 §4.1). Devolve se ele confirma a entrada num mundo.
    pub fn quick_play_file(&mut self, json: &str, elapsed_ms: u64) -> bool {
        if !quick_play_entered(json) {
            return false;
        }
        self.world_file_ms.get_or_insert(elapsed_ms);
        true
    }

    /// Quando o mundo começou a abrir (`Starting integrated minecraft server`).
    #[must_use]
    pub fn started_world_ms(&self) -> Option<u64> {
        self.started_world_ms
    }

    /// Modo assistido: devolve `true` uma vez, quando o jogo entrou no mundo e é hora de
    /// perguntar.
    pub fn take_question(&mut self) -> bool {
        if self.config.assisted && !self.asked && self.decided.is_none() && self.reached() {
            self.asked = true;
            return true;
        }
        false
    }

    fn reached(&self) -> bool {
        match self.config.goal {
            Goal::World => self.world_ms().is_some(),
            Goal::Menu => self.loaded_ms().is_some() || self.world_ms().is_some(),
        }
    }

    fn stage(&self) -> Stage {
        if self.loaded_ms().is_some()
            || self.started_world_ms.is_some()
            || self.world_ms().is_some()
        {
            Stage::World
        } else {
            Stage::Opening
        }
    }

    /// Uma linha do jogo (`text` pode ter várias linhas: mensagem e pilha de um evento XML).
    pub fn observe(&mut self, text: &str, elapsed_ms: u64) -> Option<Decision> {
        self.last_line_ms = elapsed_ms;
        if self.decided.is_some() {
            return None;
        }
        let clean = strip_color_codes(text);
        self.collect_hints(&clean);
        if clean.contains("Starting integrated minecraft server")
            || clean.contains("Connected to a modded server")
        {
            self.started_world_ms.get_or_insert(elapsed_ms);
        }
        if self.pending.is_some() {
            self.feed_pending(&clean);
            return self.try_settle(elapsed_ms, false);
        }
        if let Some(wanted) = &self.config.log_line
            && !wanted.trim().is_empty()
            && clean.contains(wanted.trim())
        {
            self.pending = Some(Pending {
                anchor: Anchor::LogLine,
                stage: self.stage(),
                at_ms: elapsed_ms,
                lines: 0,
                fixed: Some(normalize(wanted.trim())),
                exception: None,
            });
            return self.try_settle(elapsed_ms, true);
        }
        for (index, line) in clean.lines().enumerate() {
            let Some(anchor) = anchor_of(line) else {
                continue;
            };
            let fixed = (anchor == Anchor::Mixin)
                .then(|| mixin_subject(line))
                .flatten();
            let mut pending = Pending {
                anchor,
                stage: self.stage(),
                at_ms: elapsed_ms,
                lines: 0,
                fixed,
                exception: None,
            };
            // A exceção pode estar no mesmo evento (pilha do XML) ou na própria linha.
            let rest: Vec<&str> = clean.lines().skip(index).collect();
            pending.exception = first_exception(&rest);
            self.pending = Some(pending);
            return self.try_settle(elapsed_ms, false);
        }
        self.tracker.observe(&clean, elapsed_ms);
        None
    }

    fn feed_pending(&mut self, text: &str) {
        let Some(pending) = &mut self.pending else {
            return;
        };
        pending.lines += 1;
        if pending.exception.is_none() {
            let lines: Vec<&str> = text.lines().collect();
            pending.exception = first_exception(&lines);
        }
    }

    /// Fecha a falha quando a assinatura está completa (ou a janela acabou).
    fn try_settle(&mut self, now_ms: u64, force: bool) -> Option<Decision> {
        let pending = self.pending.as_ref()?;
        let complete = pending.fixed.is_some()
            || pending.exception.is_some()
            || matches!(
                pending.anchor,
                Anchor::MissingDependency | Anchor::DuplicateMods
            ) && !self.hints.is_empty();
        let window_over = pending.lines >= SIGNATURE_WINDOW_LINES
            || now_ms.saturating_sub(pending.at_ms) >= SIGNATURE_WINDOW_MS;
        if !(complete || window_over || force) {
            return None;
        }
        let pending = self.pending.take()?;
        let signature = self.signature_of(&pending);
        let decision = Decision::Failed(Failure {
            anchor: pending.anchor,
            stage: pending.stage,
            signature,
            at_ms: pending.at_ms,
            hints: self.hints.clone(),
            missing_classes: self.missing.clone(),
        });
        self.decided = Some(decision.clone());
        Some(decision)
    }

    fn signature_of(&self, pending: &Pending) -> String {
        let detail = match pending.anchor {
            Anchor::MissingDependency | Anchor::DuplicateMods if !self.hints.is_empty() => {
                let mut hints: Vec<String> = self
                    .hints
                    .iter()
                    .map(|hint| format!("{}>{}", hint.from, hint.to))
                    .collect();
                hints.sort();
                hints.dedup();
                Some(hints.join(","))
            }
            _ => pending.fixed.clone().or_else(|| pending.exception.clone()),
        };
        let mut signature = pending.anchor.id().to_owned();
        if let Some(detail) = detail {
            signature.push_str(" | ");
            signature.push_str(&detail);
        }
        truncate(&signature, MAX_SIGNATURE_CHARS)
    }

    /// O relógio andou (`elapsed_ms` desde a abertura do processo).
    pub fn tick(&mut self, elapsed_ms: u64) -> Option<Decision> {
        if self.decided.is_some() {
            return None;
        }
        if self.pending.is_some() {
            return self.try_settle(elapsed_ms, false);
        }
        let reached = match self.config.goal {
            Goal::World => self.world_ms(),
            Goal::Menu => self.world_ms().or(self.loaded_ms()),
        };
        if let Some(reached) = reached
            && !self.config.assisted
            && elapsed_ms.saturating_sub(reached) >= STABILITY_MS
        {
            let decision = Decision::Passed {
                reached_ms: reached,
            };
            self.decided = Some(decision.clone());
            return Some(decision);
        }
        if let Some(started) = self.started_world_ms
            && self.world_ms().is_none()
        {
            let quiet = elapsed_ms.saturating_sub(self.last_line_ms.max(started));
            if quiet >= self.config.freeze_ms {
                let decision = Decision::Failed(Failure {
                    anchor: Anchor::Frozen,
                    stage: Stage::World,
                    signature: Anchor::Frozen.id().to_owned(),
                    at_ms: elapsed_ms,
                    hints: self.hints.clone(),
                    missing_classes: self.missing.clone(),
                });
                self.decided = Some(decision.clone());
                return Some(decision);
            }
        }
        if !self.config.assisted && elapsed_ms >= self.config.max_ms {
            self.decided = Some(Decision::Timeout);
            return Some(Decision::Timeout);
        }
        None
    }

    /// O processo saiu. `crash_report`: o texto do crash report novo, se houver.
    pub fn exit(
        &mut self,
        exit_code: Option<i32>,
        crash_report: Option<&str>,
        elapsed_ms: u64,
    ) -> Decision {
        if let Some(decided) = &self.decided {
            return decided.clone();
        }
        if self.pending.is_some()
            && let Some(decision) = self.try_settle(elapsed_ms, true)
        {
            return decision;
        }
        let decision = if let Some(report) = crash_report {
            let lines: Vec<&str> = report.lines().collect();
            let mut signature = Anchor::CrashReport.id().to_owned();
            if let Some(exception) = first_exception(&lines) {
                signature.push_str(" | ");
                signature.push_str(&exception);
            }
            Decision::Failed(Failure {
                anchor: Anchor::CrashReport,
                stage: self.stage(),
                signature: truncate(&signature, MAX_SIGNATURE_CHARS),
                at_ms: elapsed_ms,
                hints: self.hints.clone(),
                missing_classes: self.missing.clone(),
            })
        } else {
            // Saiu sozinho antes de passar (com ou sem código de erro): uma queda nativa,
            // por exemplo (S-R5-3 §5.3).
            let signature = match exit_code {
                Some(0) => format!("{} | 0", Anchor::ProcessExit.id()),
                _ => Anchor::ProcessExit.id().to_owned(),
            };
            Decision::Failed(Failure {
                anchor: Anchor::ProcessExit,
                stage: self.stage(),
                signature,
                at_ms: elapsed_ms,
                hints: self.hints.clone(),
                missing_classes: self.missing.clone(),
            })
        };
        self.decided = Some(decision.clone());
        decision
    }

    fn collect_hints(&mut self, text: &str) {
        for hint in dependency_hints(text) {
            if !self.hints.contains(&hint) {
                self.hints.push(hint);
            }
        }
        if let Some(found) = CREATING.captures(text).and_then(|caps| caps.get(1)) {
            self.creating = Some(found.as_str().to_owned());
        }
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            // "Error loading class" é de mixins opcionais de compatibilidade (ruído).
            if line.contains("Error loading class") {
                continue;
            }
            let Some(caps) = MISSING_CLASS.captures(line) else {
                continue;
            };
            let Some(class) = caps.get(2) else { continue };
            let class = class.as_str().replace('/', ".");
            let needed_by = self.creating.clone().or_else(|| {
                lines
                    .iter()
                    .skip(index + 1)
                    .take(8)
                    .find_map(|frame| MODULE_FRAME.captures(frame))
                    .and_then(|caps| caps.get(1))
                    .map(|module| module.as_str().to_owned())
                    .filter(|module| !is_platform_module(module))
            });
            let entry = MissingClass { class, needed_by };
            if !self.missing.iter().any(|known| known.class == entry.class) {
                self.missing.push(entry);
            }
        }
    }
}

/// Se o JSON do `--quickPlayPath` registra a entrada num mundo de um jogador
/// (`[{"type":"singleplayer","id":"…",…}]`).
#[must_use]
pub fn quick_play_entered(json: &str) -> bool {
    serde_json::from_str::<Vec<serde_json::Value>>(json).is_ok_and(|entries| {
        entries.iter().any(|entry| {
            entry.get("type").and_then(serde_json::Value::as_str) == Some("singleplayer")
                && entry
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
        })
    })
}

/// Módulos do jogo e do loader nas pilhas do FML (`TRANSFORMER/minecraft@1.21.1/…`).
fn is_platform_module(module: &str) -> bool {
    matches!(
        module,
        "minecraft" | "neoforge" | "forge" | "fml_loader" | "fmlcore" | "javafmllanguage"
    )
}

static CREATING: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"Failed to create mod instance\. ModID: ([\w.-]+)").ok());
static MISSING_CLASS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(NoClassDefFoundError|ClassNotFoundException): ([\w$./]+)").ok());
static MODULE_FRAME: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^\s*at TRANSFORMER/([\w.-]+)@").ok());

/// Regex compilada uma vez; padrão inválido seria bug (os testes compilam todas).
trait OptRegex {
    fn captures<'t>(&self, text: &'t str) -> Option<regex::Captures<'t>>;
    fn replace_all_text(&self, text: &str, with: &str) -> String;
    fn captures_all<'t>(&self, text: &'t str) -> Vec<regex::Captures<'t>>;
}

impl OptRegex for LazyLock<Option<Regex>> {
    fn captures<'t>(&self, text: &'t str) -> Option<regex::Captures<'t>> {
        self.as_ref()?.captures(text)
    }

    fn replace_all_text(&self, text: &str, with: &str) -> String {
        match self.as_ref() {
            Some(regex) => regex.replace_all(text, with).into_owned(),
            None => text.to_owned(),
        }
    }

    fn captures_all<'t>(&self, text: &'t str) -> Vec<regex::Captures<'t>> {
        self.as_ref()
            .map(|regex| regex.captures_iter(text).collect())
            .unwrap_or_default()
    }
}

/// Os códigos de cor do Forge (`§e`) atrapalham as expressões.
fn strip_color_codes(text: &str) -> String {
    if !text.contains('§') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character == '§' {
            chars.next();
        } else {
            out.push(character);
        }
    }
    out
}

/// A âncora de uma linha, em ordem de prioridade (ambiente primeiro).
#[must_use]
pub fn anchor_of(line: &str) -> Option<Anchor> {
    // O FML 1.7.10 escreve um crash report de mentira ao fechar a barra de progresso.
    if line.contains("SplashProgress") || line.contains("THIS IS NOT A ERROR") {
        return None;
    }
    if line.contains("UnsupportedClassVersionError")
        || line.contains("Unsupported major.minor version")
        || (line.contains("ClassCastException") && line.contains("URLClassLoader"))
    {
        return Some(Anchor::JavaVersion);
    }
    if line.contains("Mixin apply failed")
        || (line.contains("Mixin apply for mod") && line.contains("failed"))
    {
        return Some(Anchor::Mixin);
    }
    if line.contains("Missing or unsupported mandatory dependencies")
        || line.contains("MissingModsException")
        || line.contains("Incompatible mods found!")
    {
        return Some(Anchor::MissingDependency);
    }
    if line.contains("DuplicateModsFoundException") {
        return Some(Anchor::DuplicateMods);
    }
    if line.contains("Error during pre-loading phase")
        || line.contains("Cowardly refusing to send event")
        || line.contains("Failed to create mod instance")
        || line.contains("Mod loading has failed")
    {
        return Some(Anchor::LoadingError);
    }
    if line.contains("Couldn't place player in world") {
        return Some(Anchor::JoinWorld);
    }
    if line.contains("OutOfMemoryError") {
        return Some(Anchor::OutOfMemory);
    }
    if line.contains("---- Minecraft Crash Report ----") || line.contains("#@!@# Game crashed!") {
        return Some(Anchor::CrashReport);
    }
    None
}

static EXCEPTION: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"(?:^|[\s(:])((?:[a-zA-Z_$][\w$]*\.)+[A-Z][\w$]*(?:Exception|Error|Throwable))\b(?::\s*(.*))?")
        .ok()
});

/// Exceções que aparecem em todo jogo e não são a causa (S-R5-3 §5.2).
const NOISE: &[&str] = &[
    "InvalidCredentialsException",
    "MinecraftClientException",
    "MinecraftClientHttpException",
    "AuthenticationException",
    "UnknownHostException",
    "BadRequestException",
    "Error loading class",
    "Reference map",
    "Failed to verify authentication",
];

/// A primeira exceção (que não é ruído) nas linhas, normalizada.
fn first_exception(lines: &[&str]) -> Option<String> {
    lines.iter().find_map(|line| {
        let trimmed = line.trim_start();
        if trimmed.starts_with("at ") || NOISE.iter().any(|noise| line.contains(noise)) {
            return None;
        }
        let caps = EXCEPTION.captures(trimmed)?;
        let kind = caps.get(1)?.as_str();
        let message = caps.get(2).map_or("", |message| message.as_str()).trim();
        Some(if message.is_empty() {
            kind.to_owned()
        } else {
            normalize(&format!("{kind}: {message}"))
        })
    })
}

static MIXIN_CONFIG: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"([\w.-]+\.mixins?\.json)(?::([\w$.]+))?").ok());
static MIXIN_TARGET: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"->\s*([\w$./]+)").ok());

/// `config.mixins.json:Mixin -> alvo` de uma linha `Mixin apply … failed`.
fn mixin_subject(line: &str) -> Option<String> {
    let caps = MIXIN_CONFIG.captures(line)?;
    let mut subject = caps.get(1)?.as_str().to_owned();
    if let Some(mixin) = caps.get(2) {
        subject.push(':');
        subject.push_str(mixin.as_str());
    }
    if let Some(target) = MIXIN_TARGET.captures(line).and_then(|caps| caps.get(1)) {
        subject.push_str(" -> ");
        subject.push_str(target.as_str().trim_end_matches(':'));
    }
    Some(subject)
}

static PATHS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"(?:[A-Za-z]:\\|<DADOS>|<PERFIL>|\\\\|/)[^\s'"\]\)]*[\\/][^\s'"\]\)]*"#).ok()
});
static DIGITS: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\d+").ok());

/// Troca caminhos por `<caminho>` e números por `#` (a assinatura não pode mudar entre
/// execuções nem levar a pasta do usuário).
#[must_use]
pub fn normalize(text: &str) -> String {
    let no_paths = PATHS.replace_all_text(text, "<caminho>");
    let no_digits = DIGITS.replace_all_text(&no_paths, "#");
    no_digits.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

static NEO_REQUESTED: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"Mod ID: '([\w.-]+)', Requested by: '([\w.-]+)'").ok());
static MOD_REQUIRES: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\bMod ([a-z][\w.-]*) requires ([a-z][\w.-]*)").ok());
static FABRIC_REQUIRES: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"Mod '[^']*' \(([\w.-]+)\) \S+ requires .*? of (?:mod '[^']*' \(([\w.-]+)\)|([\w.-]+)),",
    )
    .ok()
});
static LEGACY_REQUIRES: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"Mod ([\w.-]+) \([^)]*\) requires \[([\w.-]+)@").ok());

/// As dependências que o texto diz faltarem: `Mod X requires Y` (NeoForge/Forge), a lista
/// `Mod ID: 'Y', Requested by: 'X'`, as linhas `requires … of mod '…' (y)` do Fabric e o
/// `MissingModsException` do Forge 1.12.2.
#[must_use]
pub fn dependency_hints(text: &str) -> Vec<DependencyHint> {
    let clean = strip_color_codes(text);
    let mut hints: Vec<DependencyHint> = Vec::new();
    let mut push = |from: &str, to: &str| {
        let hint = DependencyHint {
            from: from.to_owned(),
            to: to.to_owned(),
        };
        if from != to && !hints.contains(&hint) {
            hints.push(hint);
        }
    };
    for caps in NEO_REQUESTED.captures_all(&clean) {
        if let (Some(to), Some(from)) = (caps.get(1), caps.get(2)) {
            push(from.as_str(), to.as_str());
        }
    }
    for caps in LEGACY_REQUIRES.captures_all(&clean) {
        if let (Some(from), Some(to)) = (caps.get(1), caps.get(2)) {
            push(from.as_str(), to.as_str());
        }
    }
    for caps in MOD_REQUIRES.captures_all(&clean) {
        if let (Some(from), Some(to)) = (caps.get(1), caps.get(2)) {
            push(from.as_str(), to.as_str());
        }
    }
    for caps in FABRIC_REQUIRES.captures_all(&clean) {
        let to = caps.get(2).or_else(|| caps.get(3));
        if let (Some(from), Some(to)) = (caps.get(1), to) {
            push(from.as_str(), to.as_str());
        }
    }
    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressoes_compilam() {
        for regex in [
            &CREATING,
            &MISSING_CLASS,
            &MODULE_FRAME,
            &EXCEPTION,
            &MIXIN_CONFIG,
            &MIXIN_TARGET,
            &PATHS,
            &DIGITS,
            &NEO_REQUESTED,
            &MOD_REQUIRES,
            &FABRIC_REQUIRES,
            &LEGACY_REQUIRES,
        ] {
            assert!(regex.is_some());
        }
    }

    #[test]
    fn normaliza_numeros_e_caminhos() {
        assert_eq!(
            normalize(
                r"java.lang.IllegalStateException: falhou em C:\Users\fulano\x.jar na linha 42"
            ),
            "java.lang.IllegalStateException: falhou em <caminho> na linha #"
        );
        assert_eq!(normalize("em /home/fulano/jogo/a.txt"), "em <caminho>");
    }

    #[test]
    fn dicas_de_dependencia_de_cada_loader() {
        let hints = dependency_hints(
            "Mod ID: 'moonlight', Requested by: 'supplementaries', Expected range\n\
             Error during pre-loading phase: Mod supplementaries requires moonlight 1.21 or above\n\
             Mod §esupplementaries§r requires §6moonlight§r\n\
             Mod continuity requires fabric_api any\n\
              - Mod 'Comforts' (comforts) 6.4.0+1.20.1 requires any version of fabric-api, which is missing!\n\
              - Mod 'X' (x) 1 requires version 0.92.0 or later of mod 'Fabric API' (fabric-api), but only\n\
             MissingModsException: Mod quark (Quark) requires [autoreglib@[1.3-32,)]",
        );
        let pairs: Vec<(&str, &str)> = hints
            .iter()
            .map(|hint| (hint.from.as_str(), hint.to.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("supplementaries", "moonlight"),
                ("quark", "autoreglib"),
                ("continuity", "fabric_api"),
                ("comforts", "fabric-api"),
                ("x", "fabric-api"),
            ]
        );
    }
}
