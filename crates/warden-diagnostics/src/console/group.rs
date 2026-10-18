//! O agrupamento incremental: cada linha (ou evento com stack trace) vira uma ocorrência de um
//! item (mesmo dono, nível, logger e forma), e cada item fica no grupo do dono.
//!
//! A memória tem teto, como o console linha a linha (SPEC T13: até 50.000 linhas na tela).
//! Todas as ocorrências contam (contagem, primeira e última hora), e o texto de cada uma fica
//! guardado até a sessão juntar [`MAX_KEPT`] ocorrências ou [`MAX_TEXT_BYTES`] de texto. Daí
//! em diante cada item guarda a primeira vez, as que já tinha do começo e as [`EDGE`] últimas
//! (as do meio dão lugar às novas), e nada mais cresce. A sessão guarda no máximo
//! [`MAX_ITEMS`] itens: uma forma nova que não cabe mais conta no grupo, mas não vira item (a
//! tela avisa quantas ficaram de fora). O log completo continua no `output.log` ("Salvar em
//! arquivo…" lê de lá).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use super::attribute::{ModDirectory, Owner, is_frame, stacktrace_mixins};
use super::noise::NoiseCatalog;
use super::normalize::{display, fill, normalize};
use super::{
    GroupKind, GroupSummary, ItemSeverity, ItemView, LineLevel, LogEntry, OriginalLine, Stamp,
};
use crate::postcrash::stack::is_stack_line;

/// Linhas que um item pode juntar (um stack trace enorme é cortado em vários itens).
const MAX_ENTRY_LINES: u32 = 2_000;
/// Últimas ocorrências que cada item guarda depois do teto da sessão.
pub(crate) const EDGE: usize = 50;
/// Ocorrências com o texto guardado na sessão (o teto do console linha a linha). Depois dele,
/// só um item que ainda não tem nenhuma das últimas guarda mais uma (no máximo uma por item).
pub(crate) const MAX_KEPT: usize = 50_000;
/// Itens distintos na sessão. Os de informação param em [`MAX_INFO_ITEMS`], para sobrar lugar
/// para erros, avisos e stack traces.
pub(crate) const MAX_ITEMS: usize = 50_000;
/// Itens distintos de informação na sessão.
pub(crate) const MAX_INFO_ITEMS: usize = 40_000;
/// Bytes de texto guardado (formas e valores) na sessão; os itens de informação param em
/// [`MAX_INFO_TEXT_BYTES`].
pub(crate) const MAX_TEXT_BYTES: usize = 32 << 20;
/// Bytes de texto guardado em que os itens novos de informação param.
pub(crate) const MAX_INFO_TEXT_BYTES: usize = 24 << 20;
/// Donos de stack trace guardados em cache.
const MAX_TRACE_CACHE: usize = 4_096;

/// Hora de uma ocorrência, compacta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawStamp {
    None,
    /// Segundos desde a meia-noite, escritos pelo jogo (`19:53:26`).
    Clock(u32),
    /// Carimbo do evento XML (milissegundos desde 1970).
    Epoch(i64),
}

impl RawStamp {
    fn of(time: Option<&str>, timestamp_ms: Option<i64>) -> Self {
        if let Some(ms) = timestamp_ms {
            return Self::Epoch(ms);
        }
        time.and_then(clock_seconds).map_or(Self::None, Self::Clock)
    }

    fn view(self) -> Stamp {
        match self {
            Self::None => Stamp::default(),
            Self::Clock(seconds) => Stamp {
                at_ms: None,
                time: Some(format!(
                    "{:02}:{:02}:{:02}",
                    seconds / 3600,
                    seconds / 60 % 60,
                    seconds % 60
                )),
            },
            #[allow(clippy::cast_precision_loss)] // milissegundos de uma data cabem num f64
            Self::Epoch(ms) => Stamp {
                at_ms: Some(ms as f64),
                time: None,
            },
        }
    }
}

/// O último `HH:MM:SS` do texto da hora (`19:53:26`, `04Oct2026 19:53:26.123`).
fn clock_seconds(time: &str) -> Option<u32> {
    let bytes = time.as_bytes();
    (0..bytes.len().saturating_sub(7)).rev().find_map(|i| {
        let part = &bytes[i..i + 8];
        let digits = [0, 1, 3, 4, 6, 7].iter().all(|&j| part[j].is_ascii_digit());
        if !digits || part[2] != b':' || part[5] != b':' {
            return None;
        }
        let number = |a: usize| u32::from(part[a] - b'0') * 10 + u32::from(part[a + 1] - b'0');
        Some(number(0) * 3600 + number(3) * 60 + number(6))
    })
}

/// Grupo: o dono das linhas.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum GroupId {
    Item(String),
    Mod(String),
    Game,
    Unknown,
}

impl GroupId {
    fn key(&self) -> String {
        match self {
            Self::Item(item) => format!("item:{item}"),
            Self::Mod(id) => format!("mod:{id}"),
            Self::Game => "game".to_owned(),
            Self::Unknown => "unknown".to_owned(),
        }
    }

    const fn kind(&self) -> GroupKind {
        match self {
            Self::Item(_) => GroupKind::Item,
            Self::Mod(_) => GroupKind::Mod,
            Self::Game => GroupKind::Game,
            Self::Unknown => GroupKind::Unknown,
        }
    }
}

fn split_owner(owner: Owner) -> (GroupId, Option<String>) {
    match owner {
        Owner::Item { item, mod_id } => (GroupId::Item(item), mod_id),
        Owner::Mod(id) => (GroupId::Mod(id.to_lowercase()), Some(id)),
        Owner::Game => (GroupId::Game, None),
        Owner::Unknown => (GroupId::Unknown, None),
    }
}

#[derive(Debug)]
struct Group {
    id: GroupId,
    mod_id: Option<String>,
    entries: u32,
    errors: u32,
    warnings: u32,
    repeated: u32,
    items: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ItemKey {
    group: u32,
    level: Option<LineLevel>,
    logger: Option<u32>,
    template: Arc<str>,
}

#[derive(Debug)]
struct Occurrence {
    seq: u32,
    stamp: RawStamp,
    values: Box<str>,
}

#[derive(Debug)]
struct Item {
    group: u32,
    level: Option<LineLevel>,
    logger: Option<u32>,
    template: Arc<str>,
    /// Quantas vezes apareceu.
    count: u32,
    /// Hora da última vez.
    last: RawStamp,
    /// As ocorrências do começo (a primeira sempre).
    head: Vec<Occurrence>,
    /// As últimas ocorrências, depois das de `head` (só depois do teto da sessão).
    tail: VecDeque<Occurrence>,
    severity: ItemSeverity,
    trace: bool,
    extra_lines: u32,
    mod_frame: Option<u32>,
    noise: Option<u16>,
}

impl Item {
    fn first(&self) -> Option<&Occurrence> {
        self.head.first()
    }

    /// As ocorrências guardadas, na ordem.
    fn kept(&self) -> impl Iterator<Item = &Occurrence> {
        self.head.iter().chain(self.tail.iter())
    }

    /// A ocorrência guardada número `index` (na ordem de [`Self::kept`]).
    fn kept_at(&self, index: usize) -> Option<&Occurrence> {
        self.head
            .get(index)
            .or_else(|| self.tail.get(index.checked_sub(self.head.len())?))
    }
}

/// Gravidade e "é stack trace" de uma entrada.
fn classify(level: Option<LineLevel>, text: &str) -> (ItemSeverity, bool) {
    let trace = text.split('\n').skip(1).any(is_frame);
    let severity = match level {
        Some(LineLevel::Error | LineLevel::Fatal) => ItemSeverity::Error,
        Some(LineLevel::Warn) => ItemSeverity::Warning,
        None if trace => ItemSeverity::Error,
        _ => ItemSeverity::Info,
    };
    (severity, trace)
}

/// Onde a última ocorrência ficou (para desfazer).
#[derive(Debug)]
enum Placed {
    /// No começo do item.
    Head,
    /// No fim do item.
    Tail,
    /// No fim do item, no lugar desta (a mais nova do começo, que saiu).
    Steal(Occurrence),
    /// No fim do item, no lugar desta (a mais antiga do fim, que saiu).
    Ring(Occurrence),
    /// Só contou (o texto não coube).
    Counted,
    /// Não coube como item: só contou no grupo. Gravidade e "é problema fora do ruído".
    Hidden(ItemSeverity, bool),
}

/// Uma entrada ainda aberta (pode receber as linhas do stack trace que seguem).
#[derive(Debug, Clone)]
struct OpenEntry {
    seq: u32,
    level: Option<LineLevel>,
    logger: Option<u32>,
    stamp: RawStamp,
    text: String,
    lines: u32,
}

/// O que a última gravação fez, para desfazer quando chegam linhas de continuação.
#[derive(Debug)]
struct LastCommit {
    entry: OpenEntry,
    group: u32,
    /// O item (`None` quando a entrada não coube como item).
    item: Option<u32>,
    placed: Placed,
    /// A hora da última vez antes desta.
    previous_last: RawStamp,
    created_item: bool,
    created_group: bool,
}

/// Dono e frame de mod de uma entrada com várias linhas (cache pela forma).
type TraceOwner = (Owner, Option<u32>);

/// O console agrupado de uma sessão, alimentado aos poucos.
#[derive(Debug)]
pub struct ConsoleGrouper {
    directory: ModDirectory,
    noise: Option<&'static NoiseCatalog>,
    groups: Vec<Group>,
    group_index: HashMap<GroupId, u32>,
    items: Vec<Item>,
    item_index: HashMap<ItemKey, u32>,
    loggers: Vec<Arc<str>>,
    logger_index: HashMap<Arc<str>, u32>,
    logger_owner: HashMap<u32, Option<Owner>>,
    trace_owner: HashMap<(Option<u32>, Arc<str>), TraceOwner>,
    problems: Vec<u32>,
    entries: u32,
    lines: u32,
    /// Ocorrências com o texto guardado.
    kept: usize,
    /// Bytes de texto guardado (formas e valores).
    text_bytes: usize,
    /// Entradas que não couberam como item.
    hidden: u32,
    /// Dessas, as de erro, aviso ou stack trace fora do ruído conhecido.
    hidden_problems: u32,
    last: Option<LastCommit>,
}

impl ConsoleGrouper {
    /// Agrupador vazio com os mods do pack.
    #[must_use]
    pub fn new(directory: ModDirectory) -> Self {
        Self {
            directory,
            noise: super::noise::embedded().ok(),
            groups: Vec::new(),
            group_index: HashMap::new(),
            items: Vec::new(),
            item_index: HashMap::new(),
            loggers: Vec::new(),
            logger_index: HashMap::new(),
            logger_owner: HashMap::new(),
            trace_owner: HashMap::new(),
            problems: Vec::new(),
            entries: 0,
            lines: 0,
            kept: 0,
            text_bytes: 0,
            hidden: 0,
            hidden_problems: 0,
            last: None,
        }
    }

    /// Entradas que não couberam como item no teto de memória (contam nos grupos, mas não
    /// aparecem na lista).
    #[must_use]
    pub const fn hidden(&self) -> u32 {
        self.hidden
    }

    /// Dessas, as que apareceriam em "Só problemas".
    #[must_use]
    pub const fn hidden_problems(&self) -> u32 {
        self.hidden_problems
    }

    /// Ocorrências com o texto original guardado (para "Copiar").
    #[must_use]
    pub const fn kept(&self) -> usize {
        self.kept
    }

    /// Linhas recebidas (com as de stack trace).
    #[must_use]
    pub const fn lines(&self) -> u32 {
        self.lines
    }

    /// Entradas (uma linha do log ou um evento com o stack trace).
    #[must_use]
    pub const fn entries(&self) -> u32 {
        self.entries
    }

    /// Itens distintos.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Recebe um lote de linhas, na ordem. As linhas de stack trace sem nível (texto puro)
    /// juntam-se à linha anterior, mesmo vindo no lote seguinte.
    pub fn push_batch(&mut self, entries: &[LogEntry<'_>]) {
        let mut open: Option<OpenEntry> = None;
        for entry in entries {
            self.lines = self.lines.saturating_add(1);
            let target = open.as_ref().or(self.last.as_ref().map(|last| &last.entry));
            if target.is_some_and(|target| continues(entry, target)) {
                if open.is_none() {
                    open = self.uncommit();
                }
                if let Some(open) = open.as_mut() {
                    open.text.push('\n');
                    open.text.push_str(entry.text);
                    open.lines += 1;
                }
                continue;
            }
            if let Some(done) = open.take() {
                self.commit(done);
            }
            open = Some(OpenEntry {
                seq: entry.seq,
                level: entry.level,
                logger: entry.logger.map(|logger| self.intern(logger)),
                stamp: RawStamp::of(entry.time, entry.timestamp_ms),
                text: entry.text.to_owned(),
                lines: 1 + u32::try_from(entry.text.matches('\n').count()).unwrap_or(0),
            });
        }
        if let Some(done) = open {
            self.commit(done);
        }
    }

    fn intern(&mut self, logger: &str) -> u32 {
        if let Some(&id) = self.logger_index.get(logger) {
            return id;
        }
        let id = u32::try_from(self.loggers.len()).unwrap_or(u32::MAX);
        let name: Arc<str> = Arc::from(logger);
        self.loggers.push(Arc::clone(&name));
        self.logger_index.insert(name, id);
        id
    }

    fn logger_name(&self, id: Option<u32>) -> Option<&str> {
        id.and_then(|id| self.loggers.get(id as usize))
            .map(AsRef::as_ref)
    }

    /// Dono pelo logger (com o cache por logger) e pelo `[classe:método:linha]` do STDOUT.
    fn owner_by_logger(&mut self, logger: Option<u32>, text: &str) -> Option<Owner> {
        let id = logger?;
        let name = self.logger_name(Some(id))?;
        if name.eq_ignore_ascii_case("stdout") || name.eq_ignore_ascii_case("stderr") {
            let class = text
                .strip_prefix('[')
                .and_then(|rest| rest.split(':').next())
                .filter(|class| class.contains('.') && !class.contains(' '));
            return class.and_then(|class| self.directory.by_class(class));
        }
        if let Some(owner) = self.logger_owner.get(&id) {
            return owner.clone();
        }
        let owner = self.directory.by_logger(name);
        self.logger_owner.insert(id, owner.clone());
        owner
    }

    fn attribute(&mut self, entry: &OpenEntry, template: &Arc<str>) -> TraceOwner {
        if entry.lines <= 1 {
            let owner = self
                .directory
                .by_mentioned_mixin(&entry.text)
                .or_else(|| self.owner_by_logger(entry.logger, &entry.text))
                .unwrap_or(Owner::Unknown);
            return (owner, None);
        }
        let cache_key = (entry.logger, Arc::clone(template));
        if let Some(cached) = self.trace_owner.get(&cache_key) {
            return cached.clone();
        }
        let lines: Vec<&str> = entry.text.split('\n').collect();
        let mut saw_game = false;
        let mut found: Option<TraceOwner> = None;
        for (index, line) in lines.iter().enumerate().skip(1) {
            if !is_frame(line) {
                continue;
            }
            match self.directory.by_frame(line) {
                Some(owner) if owner.is_mod() => {
                    found = Some((owner, u32::try_from(index).ok()));
                    break;
                }
                Some(_) => saw_game = true,
                None => {}
            }
        }
        let found = found
            .or_else(|| {
                stacktrace_mixins(&lines)
                    .iter()
                    .find_map(|config| self.directory.by_mixin_config(config))
                    .map(|owner| (owner, None))
            })
            .or_else(|| {
                self.directory
                    .by_mentioned_mixin(lines[0])
                    .map(|owner| (owner, None))
            })
            .or_else(|| {
                self.owner_by_logger(entry.logger, &entry.text)
                    .filter(|owner| owner.is_mod() || !saw_game)
                    .map(|owner| (owner, None))
            })
            .unwrap_or(if saw_game {
                (Owner::Game, None)
            } else {
                (Owner::Unknown, None)
            });
        if self.trace_owner.len() < MAX_TRACE_CACHE {
            self.trace_owner.insert(cache_key, found.clone());
        }
        found
    }

    /// Cabe mais um item com este texto?
    fn room_for_item(&self, problem: bool, bytes: usize) -> bool {
        let (items, text) = if problem {
            (MAX_ITEMS, MAX_TEXT_BYTES)
        } else {
            (MAX_INFO_ITEMS, MAX_INFO_TEXT_BYTES)
        };
        self.items.len() < items && self.text_bytes + bytes <= text
    }

    #[allow(clippy::too_many_lines)] // a gravação inteira, com o teto de memória
    fn commit(&mut self, entry: OpenEntry) {
        let normalized = normalize(&entry.text);
        let template: Arc<str> = Arc::from(normalized.template);
        let (owner, mod_frame) = self.attribute(&entry, &template);
        let (group_id, mod_id) = split_owner(owner);
        let (group, created_group) = if let Some(&index) = self.group_index.get(&group_id) {
            (index, false)
        } else {
            let index = u32::try_from(self.groups.len()).unwrap_or(u32::MAX);
            self.group_index.insert(group_id.clone(), index);
            self.groups.push(Group {
                id: group_id,
                mod_id: None,
                entries: 0,
                errors: 0,
                warnings: 0,
                repeated: 0,
                items: Vec::new(),
            });
            (index, true)
        };
        if let Some(mod_id) = mod_id {
            self.groups[group as usize].mod_id.get_or_insert(mod_id);
        }
        let values = normalized.values.into_boxed_str();
        let key = ItemKey {
            group,
            level: entry.level,
            logger: entry.logger,
            template: Arc::clone(&template),
        };
        let (item, created_item, severity) = if let Some(&index) = self.item_index.get(&key) {
            (Some(index), false, self.items[index as usize].severity)
        } else {
            let (severity, trace) = classify(entry.level, &entry.text);
            let problem = trace || severity != ItemSeverity::Info;
            let first_line = entry.text.split('\n').next().unwrap_or_default();
            let noise = self.noise.and_then(|catalog| catalog.matching(first_line));
            if self.room_for_item(problem, template.len() + values.len()) {
                let index = u32::try_from(self.items.len()).unwrap_or(u32::MAX);
                self.text_bytes += template.len();
                let new = Item {
                    group,
                    level: entry.level,
                    logger: entry.logger,
                    template,
                    count: 0,
                    last: RawStamp::None,
                    head: Vec::new(),
                    tail: VecDeque::new(),
                    severity,
                    trace,
                    extra_lines: entry.lines.saturating_sub(1),
                    mod_frame,
                    noise,
                };
                if problem {
                    self.problems.push(index);
                }
                self.items.push(new);
                self.item_index.insert(key, index);
                self.groups[group as usize].items.push(index);
                (Some(index), true, severity)
            } else {
                // Não cabe mais um item: a entrada só conta no grupo.
                let visible_problem = problem && noise.is_none();
                let group_data = &mut self.groups[group as usize];
                group_data.entries += 1;
                match severity {
                    ItemSeverity::Error => group_data.errors += 1,
                    ItemSeverity::Warning => group_data.warnings += 1,
                    ItemSeverity::Info => {}
                }
                self.hidden += 1;
                if visible_problem {
                    self.hidden_problems += 1;
                }
                self.entries += 1;
                self.last = Some(LastCommit {
                    entry,
                    group,
                    item: None,
                    placed: Placed::Hidden(severity, visible_problem),
                    previous_last: RawStamp::None,
                    created_item: false,
                    created_group,
                });
                return;
            }
        };
        let Some(item) = item else { return };
        let room = self.kept < MAX_KEPT && self.text_bytes < MAX_TEXT_BYTES;
        let occurrence = Occurrence {
            seq: entry.seq,
            stamp: entry.stamp,
            values,
        };
        let size = occurrence.values.len();
        let data = &mut self.items[item as usize];
        let previous_last = data.last;
        data.count += 1;
        data.last = entry.stamp;
        let count = data.count;
        let placed = if data.head.is_empty() || (room && data.tail.is_empty()) {
            data.head.push(occurrence);
            Placed::Head
        } else if room {
            data.tail.push_back(occurrence);
            Placed::Tail
        } else if data.tail.len() < EDGE
            && data.head.len() > 1
            && let Some(newest) = data.head.pop()
        {
            // Sem lugar: a mais nova do começo dá lugar à nova, no fim.
            data.tail.push_back(occurrence);
            Placed::Steal(newest)
        } else if let Some(oldest) = data.tail.pop_front() {
            data.tail.push_back(occurrence);
            Placed::Ring(oldest)
        } else if self.kept < MAX_KEPT + MAX_ITEMS {
            // Item só com a primeira vez: guarda também a última.
            data.tail.push_back(occurrence);
            Placed::Tail
        } else {
            Placed::Counted
        };
        match &placed {
            Placed::Head | Placed::Tail => {
                self.kept += 1;
                self.text_bytes += size;
            }
            Placed::Steal(gone) | Placed::Ring(gone) => {
                self.text_bytes = self.text_bytes + size - gone.values.len();
            }
            Placed::Counted | Placed::Hidden(..) => {}
        }
        let group_data = &mut self.groups[group as usize];
        group_data.entries += 1;
        match severity {
            ItemSeverity::Error => group_data.errors += 1,
            ItemSeverity::Warning => group_data.warnings += 1,
            ItemSeverity::Info => {}
        }
        group_data.repeated += match count {
            2 => 2,
            n if n > 2 => 1,
            _ => 0,
        };
        self.entries += 1;
        self.last = Some(LastCommit {
            entry,
            group,
            item: Some(item),
            placed,
            previous_last,
            created_item,
            created_group,
        });
    }

    /// Desfaz a última gravação e devolve a entrada, para juntar mais linhas a ela.
    fn uncommit(&mut self) -> Option<OpenEntry> {
        let last = self.last.take()?;
        let group = last.group;
        let severity = match (last.item, &last.placed) {
            (_, Placed::Hidden(severity, visible_problem)) => {
                self.hidden -= 1;
                if *visible_problem {
                    self.hidden_problems -= 1;
                }
                *severity
            }
            (Some(item), _) => {
                let data = &mut self.items[item as usize];
                let count = data.count;
                data.count -= 1;
                data.last = last.previous_last;
                match last.placed {
                    Placed::Head => {
                        if let Some(removed) = data.head.pop() {
                            self.kept -= 1;
                            self.text_bytes -= removed.values.len();
                        }
                    }
                    Placed::Tail => {
                        if let Some(removed) = data.tail.pop_back() {
                            self.kept -= 1;
                            self.text_bytes -= removed.values.len();
                        }
                    }
                    Placed::Steal(newest) => {
                        if let Some(removed) = data.tail.pop_back() {
                            self.text_bytes =
                                self.text_bytes + newest.values.len() - removed.values.len();
                        }
                        data.head.push(newest);
                    }
                    Placed::Ring(oldest) => {
                        if let Some(removed) = data.tail.pop_back() {
                            self.text_bytes =
                                self.text_bytes + oldest.values.len() - removed.values.len();
                        }
                        data.tail.push_front(oldest);
                    }
                    Placed::Counted | Placed::Hidden(..) => {}
                }
                let severity = data.severity;
                self.groups[group as usize].repeated -= match count {
                    2 => 2,
                    n if n > 2 => 1,
                    _ => 0,
                };
                if last.created_item
                    && let Some(removed) = self.items.pop()
                {
                    self.text_bytes -= removed.template.len();
                    if self.problems.last() == Some(&item) {
                        self.problems.pop();
                    }
                    self.groups[group as usize].items.pop();
                    self.item_index.remove(&ItemKey {
                        group,
                        level: removed.level,
                        logger: removed.logger,
                        template: removed.template,
                    });
                }
                severity
            }
            (None, _) => return None,
        };
        let group_data = &mut self.groups[group as usize];
        group_data.entries -= 1;
        match severity {
            ItemSeverity::Error => group_data.errors -= 1,
            ItemSeverity::Warning => group_data.warnings -= 1,
            ItemSeverity::Info => {}
        }
        self.entries -= 1;
        if last.created_group
            && let Some(removed) = self.groups.pop()
        {
            self.group_index.remove(&removed.id);
        }
        Some(last.entry)
    }

    /// Os grupos, mods primeiro (mais erros, avisos e linhas antes), depois "Jogo e loader" e
    /// "Desconhecido". Com busca, só os grupos com itens que casam, e as contagens dos itens
    /// que casam.
    #[must_use]
    pub fn groups(&self, query: &str) -> Vec<GroupSummary> {
        let query = query.trim().to_lowercase();
        let mut out: Vec<GroupSummary> = self
            .groups
            .iter()
            .filter(|group| group.entries > 0)
            .filter_map(|group| {
                let matching = if query.is_empty() {
                    u32::try_from(group.items.len()).unwrap_or(u32::MAX)
                } else {
                    let count = group
                        .items
                        .iter()
                        .filter(|&&item| self.matches(item, &query))
                        .count();
                    u32::try_from(count).unwrap_or(u32::MAX)
                };
                (matching > 0).then(|| GroupSummary {
                    key: group.id.key(),
                    kind: group.id.kind(),
                    item: match &group.id {
                        GroupId::Item(item) => Some(item.clone()),
                        _ => None,
                    },
                    mod_id: group.mod_id.clone(),
                    entries: group.entries,
                    errors: group.errors,
                    warnings: group.warnings,
                    repeated: group.repeated,
                    items: matching,
                })
            })
            .collect();
        let rank = |kind: GroupKind| match kind {
            GroupKind::Item | GroupKind::Mod => 0,
            GroupKind::Game => 1,
            GroupKind::Unknown => 2,
        };
        out.sort_by(|a, b| {
            rank(a.kind)
                .cmp(&rank(b.kind))
                .then(b.errors.cmp(&a.errors))
                .then(b.warnings.cmp(&a.warnings))
                .then(b.entries.cmp(&a.entries))
                .then(a.key.cmp(&b.key))
        });
        out
    }

    /// Os itens de um grupo (erros e stack traces primeiro, depois avisos, depois os mais
    /// repetidos), até `limit`, e quantos casam no total.
    #[must_use]
    pub fn group_items(&self, key: &str, query: &str, limit: usize) -> (Vec<ItemView>, u32) {
        let query = query.trim().to_lowercase();
        let Some(group) = self.groups.iter().find(|group| group.id.key() == key) else {
            return (Vec::new(), 0);
        };
        let mut ids: Vec<u32> = group
            .items
            .iter()
            .copied()
            .filter(|&item| query.is_empty() || self.matches(item, &query))
            .collect();
        let total = u32::try_from(ids.len()).unwrap_or(u32::MAX);
        let rank = |item: &Item| match (item.severity, item.trace) {
            (ItemSeverity::Error, _) | (_, true) => 0,
            (ItemSeverity::Warning, false) => 1,
            (ItemSeverity::Info, false) => 2,
        };
        ids.sort_by(|&a, &b| {
            let (x, y) = (&self.items[a as usize], &self.items[b as usize]);
            rank(x)
                .cmp(&rank(y))
                .then(y.count.cmp(&x.count))
                .then(a.cmp(&b))
        });
        let views = ids
            .into_iter()
            .take(limit)
            .map(|item| self.view(item))
            .collect();
        (views, total)
    }

    /// "Só problemas": erros, avisos e stack traces que não são ruído conhecido, na ordem em
    /// que apareceram, até `limit`, e quantos casam no total.
    #[must_use]
    pub fn problems(&self, query: &str, limit: usize) -> (Vec<ItemView>, u32) {
        let query = query.trim().to_lowercase();
        let ids: Vec<u32> = self
            .problems
            .iter()
            .copied()
            .filter(|&item| self.items[item as usize].noise.is_none())
            .filter(|&item| query.is_empty() || self.matches(item, &query))
            .collect();
        let total = u32::try_from(ids.len()).unwrap_or(u32::MAX);
        let views = ids
            .into_iter()
            .take(limit)
            .map(|item| self.view(item))
            .collect();
        (views, total)
    }

    /// As linhas originais de um item (todas as ocorrências) ou de um grupo (de todos os itens,
    /// na ordem do log), até `limit`.
    #[must_use]
    pub fn original_lines(&self, target: &LinesTarget, limit: Option<usize>) -> Vec<OriginalLine> {
        let limit = limit.unwrap_or(usize::MAX);
        let mut picked: Vec<(u32, u32, usize)> = Vec::new();
        let take_item = |item: u32, picked: &mut Vec<(u32, u32, usize)>| {
            if let Some(data) = self.items.get(item as usize) {
                for (index, occurrence) in data.kept().enumerate() {
                    picked.push((occurrence.seq, item, index));
                }
            }
        };
        match target {
            LinesTarget::Item(item) => take_item(*item, &mut picked),
            LinesTarget::Group(key) => {
                if let Some(group) = self.groups.iter().find(|group| group.id.key() == *key) {
                    for &item in &group.items {
                        take_item(item, &mut picked);
                    }
                }
                picked.sort_unstable();
            }
        }
        picked
            .into_iter()
            .take(limit)
            .filter_map(|(seq, item, index)| {
                let data = &self.items[item as usize];
                let occurrence = data.kept_at(index)?;
                Some(OriginalLine {
                    seq,
                    level: data.level,
                    logger: self.logger_name(data.logger).map(str::to_owned),
                    stamp: occurrence.stamp.view(),
                    text: fill(&data.template, &occurrence.values),
                })
            })
            .collect()
    }

    /// O texto do item na lista: o original quando aparece uma vez; a forma quando repete.
    fn first_line(item: &Item) -> String {
        let full = match (item.count, item.first()) {
            (1, Some(first)) => fill(&item.template, &first.values),
            _ => display(&item.template),
        };
        match full.split_once('\n') {
            Some((first, _)) => first.to_owned(),
            None => full,
        }
    }

    fn matches(&self, item: u32, query: &str) -> bool {
        let data = &self.items[item as usize];
        Self::first_line(data).to_lowercase().contains(query)
            || self
                .logger_name(data.logger)
                .is_some_and(|logger| logger.to_lowercase().contains(query))
    }

    fn view(&self, item: u32) -> ItemView {
        let data = &self.items[item as usize];
        let group = &self.groups[data.group as usize];
        let mod_frame = data.mod_frame.and_then(|index| {
            let first = data.first()?;
            fill(&data.template, &first.values)
                .split('\n')
                .nth(index as usize)
                .map(|line| line.trim().to_owned())
        });
        ItemView {
            id: item,
            group: group.id.key(),
            severity: data.severity,
            level: data.level,
            logger: self.logger_name(data.logger).map(str::to_owned),
            text: Self::first_line(data),
            count: data.count,
            first: data.first().map_or_else(Stamp::default, |o| o.stamp.view()),
            last: data.last.view(),
            noise: data
                .noise
                .zip(self.noise)
                .map(|(index, catalog)| catalog.id(index).to_owned()),
            trace: data.trace,
            extra_lines: data.extra_lines,
            mod_frame,
        }
    }
}

/// De onde copiar as linhas originais.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinesTarget {
    /// Um item (pelo id de [`ItemView::id`]).
    Item(u32),
    /// Um grupo (pela chave de [`GroupSummary::key`]).
    Group(String),
}

/// A linha continua a entrada anterior? Só linhas sem nível (texto puro): frames, `Caused by`,
/// `... N more` sempre; a linha da exceção e linhas recuadas só depois de aviso, erro ou de
/// outra linha de stack trace.
fn continues(entry: &LogEntry<'_>, target: &OpenEntry) -> bool {
    if entry.level.is_some() || entry.text.trim().is_empty() || target.lines >= MAX_ENTRY_LINES {
        return false;
    }
    let trimmed = entry.text.trim_start();
    if trimmed.starts_with("at ")
        || trimmed.starts_with("... ")
        || trimmed.starts_with("Caused by")
        || trimmed.starts_with("Suppressed:")
        || trimmed.starts_with("Wrapped by:")
    {
        return true;
    }
    let problem = target.lines > 1 || target.level.is_some_and(|level| level >= LineLevel::Warn);
    problem && (is_stack_line(entry.text) || entry.text.starts_with(['\t', ' ']))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry<'a>(
        seq: u32,
        level: Option<LineLevel>,
        logger: Option<&'a str>,
        text: &'a str,
    ) -> LogEntry<'a> {
        LogEntry {
            seq,
            level,
            logger,
            time: Some("18:02:16"),
            timestamp_ms: None,
            text,
        }
    }

    #[test]
    fn hora_do_jogo() {
        assert_eq!(clock_seconds("19:53:26"), Some(19 * 3600 + 53 * 60 + 26));
        assert_eq!(
            clock_seconds("04Oct2026 19:53:26.123"),
            Some(19 * 3600 + 53 * 60 + 26)
        );
        assert_eq!(clock_seconds("ontem"), None);
        assert_eq!(
            RawStamp::Clock(3661).view().time.as_deref(),
            Some("01:01:01")
        );
    }

    #[test]
    fn stack_trace_em_texto_puro_mesmo_entre_lotes() {
        let mut grouper = ConsoleGrouper::new(ModDirectory::empty());
        let error = Some(LineLevel::Error);
        grouper.push_batch(&[
            entry(
                0,
                Some(LineLevel::Info),
                Some("minecraft/Minecraft"),
                "Abrindo",
            ),
            entry(1, error, Some("minecraft/Minecraft"), "Falhou"),
            entry(2, None, None, "java.lang.RuntimeException: x"),
        ]);
        assert_eq!(grouper.entries(), 2);
        grouper.push_batch(&[
            entry(
                3,
                None,
                None,
                "\tat TRANSFORMER/epicfight@21.1/yesman.X.m(X.java:1)",
            ),
            entry(4, None, None, "\t... 3 more"),
            entry(
                5,
                Some(LineLevel::Info),
                Some("minecraft/Minecraft"),
                "Abrindo",
            ),
        ]);
        assert_eq!(grouper.entries(), 3);
        assert_eq!(grouper.lines(), 6);
        let groups = grouper.groups("");
        assert_eq!(groups[0].key, "mod:epicfight");
        assert_eq!(groups[0].errors, 1);
        let (items, total) = grouper.group_items("mod:epicfight", "", 10);
        assert_eq!(total, 1);
        assert!(items[0].trace);
        assert_eq!(items[0].extra_lines, 3);
        assert_eq!(items[0].text, "Falhou");
        assert_eq!(
            items[0].mod_frame.as_deref(),
            Some("at TRANSFORMER/epicfight@21.1/yesman.X.m(X.java:1)")
        );
        let game = groups.iter().find(|g| g.key == "game").unwrap();
        assert_eq!((game.entries, game.repeated, game.items), (2, 2, 1));
        let original = grouper.original_lines(&LinesTarget::Item(items[0].id), None);
        assert_eq!(
            original[0].text,
            "Falhou\njava.lang.RuntimeException: x\n\tat TRANSFORMER/epicfight@21.1/yesman.X.m(X.java:1)\n\t... 3 more"
        );
        let (problems, _) = grouper.problems("", 10);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn excecao_depois_de_info_nao_se_junta() {
        let mut grouper = ConsoleGrouper::new(ModDirectory::empty());
        grouper.push_batch(&[
            entry(0, Some(LineLevel::Info), None, "ok"),
            entry(1, None, None, "java.lang.IllegalStateException: y"),
            entry(
                2,
                None,
                None,
                "\tat java.base/java.lang.Thread.run(Thread.java:1)",
            ),
        ]);
        assert_eq!(grouper.entries(), 2);
        let (problems, _) = grouper.problems("", 10);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].severity, ItemSeverity::Error);
        assert_eq!(problems[0].group, "game");
    }

    #[test]
    fn desfazer_devolve_o_estado() {
        let mut grouper = ConsoleGrouper::new(ModDirectory::empty());
        grouper.push_batch(&[entry(0, Some(LineLevel::Warn), Some("x"), "Aviso 1")]);
        grouper.push_batch(&[entry(1, Some(LineLevel::Warn), Some("x"), "Aviso 2")]);
        let before = grouper.groups("");
        assert_eq!(before[0].repeated, 2);
        grouper.push_batch(&[entry(2, None, None, "\tdetalhe")]);
        let after = grouper.groups("");
        assert_eq!(after[0].repeated, 0);
        assert_eq!(after[0].items, 2);
        assert_eq!(after[0].warnings, 2);
        assert_eq!(grouper.item_count(), 2);
    }

    /// Uma palavra diferente para cada número (formas que nunca repetem).
    fn word(mut n: usize) -> String {
        let mut out = String::new();
        loop {
            out.push(char::from(b'a' + u8::try_from(n % 26).unwrap_or(0)));
            n /= 26;
            if n == 0 {
                return out;
            }
        }
    }

    fn seqs(grouper: &ConsoleGrouper, item: u32) -> Vec<u32> {
        grouper
            .original_lines(&LinesTarget::Item(item), None)
            .iter()
            .map(|line| line.seq)
            .collect()
    }

    /// Depois do teto da sessão, o item continua contando, guarda a primeira vez, o começo e as
    /// [`EDGE`] últimas; e desfazer (uma linha de stack trace que chega depois) devolve tudo.
    #[test]
    fn teto_guarda_o_comeco_e_as_ultimas() {
        let mut grouper = ConsoleGrouper::new(ModDirectory::empty());
        let texts: Vec<String> = (0..MAX_KEPT + 10).map(|n| format!("Aviso {n}")).collect();
        let warn = Some(LineLevel::Warn);
        let entries: Vec<LogEntry<'_>> = texts
            .iter()
            .enumerate()
            .map(|(n, text)| entry(u32::try_from(n).unwrap_or(0), warn, Some("x"), text))
            .collect();
        grouper.push_batch(&entries);
        assert_eq!(grouper.kept(), MAX_KEPT);
        let id = grouper.problems("", 1).0[0].id;
        let kept = seqs(&grouper, id);
        assert_eq!(kept.len(), MAX_KEPT);
        assert_eq!(kept[0], 0);
        // As 10 depois do teto tomaram o lugar das 10 mais novas do começo.
        assert_eq!(
            kept[MAX_KEPT - 11..],
            [
                49_989, 50_000, 50_001, 50_002, 50_003, 50_004, 50_005, 50_006, 50_007, 50_008,
                50_009
            ]
        );

        // Uma linha de stack trace junta-se à última: desfaz a troca e devolve a do começo.
        grouper.push_batch(&[entry(99_999, None, None, "\tat a.B.c(B.java:1)")]);
        let kept = seqs(&grouper, id);
        assert_eq!(kept.len(), MAX_KEPT);
        assert_eq!(kept[MAX_KEPT - 10..MAX_KEPT - 8], [49_990, 50_000]);
        assert_eq!(*kept.last().unwrap_or(&0), 50_008);
        // A entrada com o stack trace é um item novo, com a primeira vez guardada.
        assert_eq!(grouper.kept(), MAX_KEPT + 1);

        // Mais 200: o fim guarda só as EDGE últimas, e a contagem continua certa.
        let more: Vec<String> = (0..200).map(|n| format!("Aviso {n}")).collect();
        let entries: Vec<LogEntry<'_>> = more
            .iter()
            .enumerate()
            .map(|(n, text)| {
                entry(
                    200_000 + u32::try_from(n).unwrap_or(0),
                    warn,
                    Some("x"),
                    text,
                )
            })
            .collect();
        grouper.push_batch(&entries);
        let (items, _) = grouper.problems("Aviso", 5);
        let item = items
            .iter()
            .find(|item| item.id == id)
            .unwrap_or_else(|| panic!("{items:?}"));
        assert_eq!(item.count, u32::try_from(MAX_KEPT + 9 + 200).unwrap_or(0));
        let kept = seqs(&grouper, id);
        assert_eq!(kept.len(), MAX_KEPT);
        let tail: Vec<u32> = (200_000 + 200 - u32::try_from(EDGE).unwrap_or(0)..200_200).collect();
        assert_eq!(kept[MAX_KEPT - EDGE..], tail[..]);

        // Desfazer a troca no fim (a mais antiga volta).
        grouper.push_batch(&[entry(300_000, None, None, "\tat a.B.c(B.java:1)")]);
        let kept = seqs(&grouper, id);
        assert_eq!(kept[MAX_KEPT - EDGE], 200_149);
        assert_eq!(*kept.last().unwrap_or(&0), 200_198);
    }

    /// Formas novas além do teto de itens contam no grupo, mas não viram item; os erros têm
    /// lugar reservado e continuam em "Só problemas".
    #[test]
    fn formas_alem_do_teto_contam_sem_virar_item() {
        let mut grouper = ConsoleGrouper::new(ModDirectory::empty());
        let texts: Vec<String> = (0..MAX_INFO_ITEMS + 10)
            .map(|n| format!("Modelo {}", word(n)))
            .collect();
        let info = Some(LineLevel::Info);
        let entries: Vec<LogEntry<'_>> = texts
            .iter()
            .enumerate()
            .map(|(n, text)| entry(u32::try_from(n).unwrap_or(0), info, Some("x"), text))
            .collect();
        grouper.push_batch(&entries);
        assert_eq!(grouper.item_count(), MAX_INFO_ITEMS);
        assert_eq!((grouper.hidden(), grouper.hidden_problems()), (10, 0));
        let groups = grouper.groups("");
        assert_eq!(
            groups[0].entries,
            u32::try_from(MAX_INFO_ITEMS + 10).unwrap_or(0)
        );

        // A última forma que ficou de fora vira stack trace: agora cabe (lugar dos problemas).
        grouper.push_batch(&[entry(900_000, None, None, "\tat a.B.c(B.java:1)")]);
        assert_eq!((grouper.hidden(), grouper.hidden_problems()), (9, 0));
        let (problems, total) = grouper.problems("", 5);
        assert_eq!(total, 1);
        assert!(problems[0].trace);
        let groups = grouper.groups("");
        assert_eq!(
            groups[0].entries,
            u32::try_from(MAX_INFO_ITEMS + 10).unwrap_or(0)
        );
        assert_eq!(groups[0].errors, 0);
        assert_eq!(
            grouper.entries(),
            u32::try_from(MAX_INFO_ITEMS + 10).unwrap_or(0)
        );
    }

    #[test]
    fn busca_e_limite() {
        let mut grouper = ConsoleGrouper::new(ModDirectory::empty());
        let texts: Vec<String> = (0..30_u8)
            .map(|i| format!("linha {i} item-{}", char::from(b'a' + i % 26)))
            .collect();
        let entries: Vec<LogEntry<'_>> = texts
            .iter()
            .enumerate()
            .map(|(i, text)| entry(u32::try_from(i).unwrap(), Some(LineLevel::Info), None, text))
            .collect();
        grouper.push_batch(&entries);
        let (items, total) = grouper.group_items("unknown", "", 5);
        assert_eq!((items.len(), total), (5, 26));
        let (items, total) = grouper.group_items("unknown", "ITEM-C", 5);
        assert_eq!(total, 1);
        assert_eq!(items[0].count, 2);
        assert_eq!(items[0].text, "linha {n} item-c");
        assert_eq!(grouper.groups("item-c")[0].items, 1);
        assert!(grouper.groups("nada disso").is_empty());
    }
}
