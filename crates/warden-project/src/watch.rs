//! Mudanças externas no pack (ARCHITECTURE §15; ROADMAP A-05).
//!
//! [`PackWatcher`] vigia a pasta de um pack e avisa, sem repetir, quais [`Area`]s mudaram
//! quando outro programa (editor de texto, `git pull`, o jogo) mexe nos arquivos. As escritas
//! do próprio Warden não contam: quem escreve abre um [`WriteScope`] no [`WriteLedger`] e o
//! vigia descarta o que acontece durante o escopo e logo depois dele. Essas escritas já
//! avisam a interface por conta própria (`pack-changed`); contá-las de novo recarregaria a
//! tela duas vezes. Uma mudança externa que caia dentro de um escopo não se perde: o aviso
//! do próprio Warden faz a interface reler o disco depois da escrita.
//!
//! O vigia usa o `notify` e a sua própria espera: os eventos brutos viram um conjunto de áreas
//! e o aviso sai quando a pasta fica quieta por [`WatchConfig::debounce`] (ou, numa rajada
//! sem fim, em [`WatchConfig::max_wait`]), para um `git pull` gerar uma recarga só.

use std::collections::{BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind};
use warden_core::PackId;

use crate::{Error, ProjectErrorCode as Code, Result};

/// Parte do pack que uma mudança atinge (espelha `PackArea` do evento `pack-changed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Area {
    /// Mods, resource packs, shaders: o índice, os metafiles e os arquivos soltos.
    Inventory,
    /// Configs e demais arquivos do pack.
    Configs,
    /// Nome, autor e versões (`pack.toml`).
    Meta,
    /// Histórico de versões (repositório git do pack).
    History,
}

/// Todas as áreas, na ordem de [`Area`].
pub const ALL_AREAS: [Area; 4] = [Area::Inventory, Area::Configs, Area::Meta, Area::History];

/// Tempos do vigia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchConfig {
    /// Silêncio exigido antes de avisar (ARCHITECTURE §15: 500 ms).
    pub debounce: Duration,
    /// Espera máxima numa rajada contínua, para o aviso sair dentro dos 2 s do CA-T05-01.
    pub max_wait: Duration,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(500),
            max_wait: Duration::from_millis(1200),
        }
    }
}

/// Folga depois de uma escrita do Warden em que os eventos do sistema ainda são dela.
pub const DEFAULT_GRACE: Duration = Duration::from_millis(750);

#[derive(Debug, Default)]
struct Entry {
    active: usize,
    quiet_until: Option<Instant>,
}

/// Registro das escritas do Warden por pack ("fichas de escrita").
#[derive(Debug, Clone)]
pub struct WriteLedger {
    grace: Duration,
    packs: Arc<Mutex<HashMap<PackId, Entry>>>,
}

impl Default for WriteLedger {
    fn default() -> Self {
        Self::new(DEFAULT_GRACE)
    }
}

impl WriteLedger {
    /// Registro vazio com a folga dada.
    #[must_use]
    pub fn new(grace: Duration) -> Self {
        Self {
            grace,
            packs: Arc::default(),
        }
    }

    /// Marca o começo de uma escrita do Warden no pack; vale até o escopo ser solto, mais a
    /// folga.
    #[must_use]
    pub fn begin(&self, pack: PackId) -> WriteScope {
        self.entries().entry(pack).or_default().active += 1;
        WriteScope {
            ledger: self.clone(),
            pack,
        }
    }

    /// Se um evento ocorrido em `at` é consequência de uma escrita do Warden.
    #[must_use]
    pub fn covers(&self, pack: PackId, at: Instant) -> bool {
        self.entries().get(&pack).is_some_and(|entry| {
            entry.active > 0 || entry.quiet_until.is_some_and(|until| at <= until)
        })
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<PackId, Entry>> {
        self.packs.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Uma escrita do Warden em andamento (solta ao sair de escopo).
#[derive(Debug)]
pub struct WriteScope {
    ledger: WriteLedger,
    pack: PackId,
}

impl Drop for WriteScope {
    fn drop(&mut self) {
        let grace = self.ledger.grace;
        let mut entries = self.ledger.entries();
        if let Some(entry) = entries.get_mut(&self.pack) {
            entry.active = entry.active.saturating_sub(1);
            entry.quiet_until = Some(Instant::now() + grace);
        }
    }
}

/// Áreas atingidas por um caminho relativo à raiz do pack; vazio = nada que o Warden mostre.
#[must_use]
pub fn classify(relative: &Path) -> &'static [Area] {
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    let Some(&first) = parts.first() else {
        return &[];
    };
    let name = parts.last().copied().unwrap_or(first);
    if first == ".git" {
        // Só o que muda o histórico: HEAD e as referências. Objetos e índice são ruído.
        let moves_history = parts
            .get(1)
            .is_some_and(|part| *part == "HEAD" || *part == "refs");
        return if moves_history && !has_extension(name, "lock") {
            &[Area::History]
        } else {
            &[]
        };
    }
    if ["warden-tmp", "tmp", "lock"]
        .iter()
        .any(|extension| has_extension(name, extension))
    {
        return &[];
    }
    if parts.len() == 1 {
        match name {
            "pack.toml" => return &[Area::Meta, Area::Inventory],
            "index.toml" | ".packwizignore" => return &[Area::Inventory],
            ".gitignore" | ".gitattributes" => return &[],
            _ => {}
        }
    }
    if name.ends_with(".pw.toml") || matches!(first, "mods" | "resourcepacks" | "shaderpacks") {
        return &[Area::Inventory];
    }
    &[Area::Configs]
}

fn has_extension(name: &str, extension: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

enum Message {
    Changed(BTreeSet<Area>),
    Stop,
}

/// Vigia da pasta de um pack. Soltar o valor para o vigia.
pub struct PackWatcher {
    watcher: Option<RecommendedWatcher>,
    stop: Sender<Message>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for PackWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackWatcher").finish_non_exhaustive()
    }
}

impl PackWatcher {
    /// Começa a vigiar `root`. `on_change` roda numa thread do vigia, uma vez por rajada, com
    /// as áreas atingidas em ordem.
    pub fn start(
        pack: PackId,
        root: &Path,
        ledger: WriteLedger,
        config: WatchConfig,
        on_change: impl Fn(Vec<Area>) + Send + 'static,
    ) -> Result<Self> {
        if !root.is_dir() {
            return Err(Error::new(
                Code::FolderMissing,
                format!("a pasta do pack não existe: {}", root.display()),
            ));
        }
        let (sender, receiver) = mpsc::channel();
        let roots = roots_of(root);
        let callback_sender = sender.clone();
        let worker_ledger = ledger.clone();
        let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            let Ok(event) = result else {
                return;
            };
            let at = Instant::now();
            if ledger.covers(pack, at) {
                return;
            }
            let areas = areas_of(&event, &roots);
            if !areas.is_empty() {
                // O vigia já parou se o envio falhar: nada a fazer.
                let _ = callback_sender.send(Message::Changed(areas));
            }
        })
        .map_err(|error| watch_error(&error))?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|error| watch_error(&error))?;
        let probe = RootProbe {
            pack,
            root: root.to_path_buf(),
            ledger: worker_ledger,
        };
        let worker = std::thread::Builder::new()
            .name("warden-pack-watch".into())
            .spawn(move || run_worker(&receiver, config, &probe, &on_change))
            .map_err(|error| Error::new(Code::Internal, error.to_string()))?;
        Ok(Self {
            watcher: Some(watcher),
            stop: sender,
            worker: Some(worker),
        })
    }
}

impl Drop for PackWatcher {
    fn drop(&mut self) {
        // O vigia do sistema cai primeiro; depois a thread é avisada e espera-se por ela.
        drop(self.watcher.take());
        let _ = self.stop.send(Message::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn watch_error(error: &notify::Error) -> Error {
    Error::new(
        Code::Internal,
        format!("não foi possível vigiar a pasta do pack: {error}"),
    )
}

fn roots_of(root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![root.to_path_buf()];
    if let Ok(canonical) = root.canonicalize()
        && canonical != root
    {
        roots.push(canonical);
    }
    roots
}

fn areas_of(event: &Event, roots: &[PathBuf]) -> BTreeSet<Area> {
    let mut found = BTreeSet::new();
    if matches!(
        event.kind,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_))
    ) {
        return found;
    }
    for path in &event.paths {
        let Some(relative) = roots.iter().find_map(|root| path.strip_prefix(root).ok()) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            // A própria pasta do pack apagada ou renomeada: tudo mudou.
            if matches!(
                event.kind,
                EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
            ) {
                found.extend(ALL_AREAS);
            }
            continue;
        }
        found.extend(classify(relative));
    }
    found
}

/// Dados que o vigia usa para notar sozinho que a pasta do pack sumiu ou voltou: o Windows
/// não avisa quando a própria pasta vigiada é apagada ou renomeada.
struct RootProbe {
    pack: PackId,
    root: PathBuf,
    ledger: WriteLedger,
}

fn run_worker(
    receiver: &mpsc::Receiver<Message>,
    config: WatchConfig,
    probe: &RootProbe,
    on_change: &dyn Fn(Vec<Area>),
) {
    let mut pending: BTreeSet<Area> = BTreeSet::new();
    let mut first: Option<Instant> = None;
    let mut last = Instant::now();
    let mut root_present = true;
    loop {
        let wait = match first {
            None => config.debounce,
            Some(started) => {
                let deadline = (last + config.debounce).min(started + config.max_wait);
                deadline.saturating_duration_since(Instant::now())
            }
        };
        match receiver.recv_timeout(wait) {
            Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Message::Changed(areas)) => {
                pending.extend(areas);
                last = Instant::now();
                first.get_or_insert(last);
            }
            Err(RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                let present = probe.root.is_dir();
                if present != root_present {
                    root_present = present;
                    if !probe.ledger.covers(probe.pack, now) {
                        pending.extend(ALL_AREAS);
                        first.get_or_insert(now);
                    }
                }
                // O tempo de espera da rajada acabou (ou a pasta sumiu agora): avisa.
                if first.is_some() {
                    on_change(std::mem::take(&mut pending).into_iter().collect());
                    first = None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn areas(path: &str) -> Vec<Area> {
        classify(Path::new(path)).to_vec()
    }

    #[test]
    fn caminhos_do_pack_caem_na_area_certa() {
        assert_eq!(areas("pack.toml"), [Area::Meta, Area::Inventory]);
        assert_eq!(areas("index.toml"), [Area::Inventory]);
        assert_eq!(areas("mods/sodium.pw.toml"), [Area::Inventory]);
        assert_eq!(areas("mods/solto.jar"), [Area::Inventory]);
        assert_eq!(areas("config/create-common.toml"), [Area::Configs]);
        assert_eq!(areas("kubejs/server_scripts/a.js"), [Area::Configs]);
        assert_eq!(areas("options.txt"), [Area::Configs]);
        assert_eq!(areas(".git/HEAD"), [Area::History]);
        assert_eq!(areas(".git/refs/heads/main"), [Area::History]);
    }

    #[test]
    fn ruido_nao_vira_mudanca() {
        assert!(areas("").is_empty());
        assert!(areas(".git/objects/ab/cdef").is_empty());
        assert!(areas(".git/index").is_empty());
        assert!(areas(".git/refs/heads/main.lock").is_empty());
        assert!(areas("pack.toml.123-0.warden-tmp").is_empty());
        assert!(areas("config/a.toml.tmp").is_empty());
        assert!(areas(".gitignore").is_empty());
    }

    #[test]
    fn ledger_cobre_o_escopo_e_a_folga_depois_dele() {
        let ledger = WriteLedger::new(Duration::from_millis(200));
        let pack = PackId::new();
        let other = PackId::new();
        assert!(!ledger.covers(pack, Instant::now()));
        let scope = ledger.begin(pack);
        assert!(ledger.covers(pack, Instant::now()));
        assert!(!ledger.covers(other, Instant::now()));
        drop(scope);
        assert!(ledger.covers(pack, Instant::now()));
        let later = Instant::now() + Duration::from_millis(400);
        assert!(!ledger.covers(pack, later));
    }

    #[test]
    fn escopos_aninhados_so_terminam_com_o_ultimo() {
        let ledger = WriteLedger::new(Duration::ZERO);
        let pack = PackId::new();
        let first = ledger.begin(pack);
        let second = ledger.begin(pack);
        drop(first);
        assert!(ledger.covers(pack, Instant::now() + Duration::from_secs(1)));
        drop(second);
        assert!(!ledger.covers(pack, Instant::now() + Duration::from_secs(1)));
    }

    #[test]
    fn pasta_inexistente_e_recusada() {
        let error = PackWatcher::start(
            PackId::new(),
            Path::new("/nao/existe/mesmo"),
            WriteLedger::default(),
            WatchConfig::default(),
            |_| {},
        )
        .unwrap_err();
        assert_eq!(error.code, Code::FolderMissing);
    }
}
