//! O índice vivo de uma origem: lido em segundo plano e atualizado sozinho quando um arquivo
//! muda (ARCHITECTURE §10.1 e §15).
//!
//! Uma thread por índice faz todo o trabalho de disco: a primeira leitura (em lotes, para a
//! busca já responder com o que foi lido) e as atualizações. O observador (`notify`) só avisa
//! a thread, que espera os avisos pararem por um instante ([`LiveIndexOptions::debounce`]) e
//! então compara a lista da origem com o índice: relê o que é novo, mudou de tamanho ou de data
//! ou foi avisado, e tira o que sumiu. Quem chama recebe [`IndexChange`] quando algo mudou de
//! fato. Se o observador não puder ser ligado (pasta que ainda não existe) ou perder um aviso,
//! [`LiveIndex::refresh`] faz a mesma comparação sob pedido.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, RwLock, Weak};
use std::time::{Duration, Instant};

use notify::event::{MetadataKind, ModifyKind};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use warden_core::CancellationToken;

use super::model::{ConfigSearchResults, IndexStatus, SearchQuery, SourceFile};
use super::store::{ConfigIndex, ReadFile, read_file};

/// Arquivos lidos entre uma atualização do índice e outra na primeira leitura.
const BATCH: usize = 64;

/// A lista dos arquivos da origem: `Ok(None)` quando a origem não existe.
pub type Lister = Arc<dyn Fn(&Path) -> std::io::Result<Option<Vec<SourceFile>>> + Send + Sync>;

/// Se uma mudança neste caminho (relativo à origem, em partes) pode afetar a busca. Filtra o
/// que muda o tempo todo e não é config (logs do jogo, `.git/`, `mods/`).
pub type Relevance = Arc<dyn Fn(&[&str]) -> bool + Send + Sync>;

/// Chamado (na thread do índice) quando o índice muda.
pub type ChangeListener = Arc<dyn Fn(IndexChange) + Send + Sync>;

/// Como montar um [`LiveIndex`].
#[derive(Clone)]
pub struct LiveIndexOptions {
    /// Pasta da origem.
    pub root: PathBuf,
    /// Lista dos arquivos da origem.
    pub lister: Lister,
    /// Filtro dos avisos do observador.
    pub relevant: Relevance,
    /// Aviso de mudança.
    pub on_change: ChangeListener,
    /// Quanto tempo sem avisos antes de atualizar.
    pub debounce: Duration,
}

impl std::fmt::Debug for LiveIndexOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveIndexOptions")
            .field("debounce", &self.debounce)
            .finish_non_exhaustive()
    }
}

/// O índice mudou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexChange {
    /// Situação depois da mudança.
    pub status: IndexStatus,
    /// Arquivos que entraram, mudaram ou saíram.
    pub changed_files: u32,
    /// Foi a primeira leitura (não uma mudança nos arquivos).
    pub first: bool,
}

#[derive(Default)]
struct Pending {
    dirty: bool,
    forced: BTreeSet<String>,
    last_signal: Option<Instant>,
    stop: bool,
}

struct Shared {
    options: LiveIndexOptions,
    index: RwLock<ConfigIndex>,
    status: Mutex<IndexStatus>,
    pending: Mutex<Pending>,
    wake: Condvar,
    watcher: Mutex<Option<RecommendedWatcher>>,
    last_sync: Mutex<Option<Instant>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// O índice vivo de uma origem. Soltar o valor para a thread e o observador.
pub struct LiveIndex {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for LiveIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveIndex")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl LiveIndex {
    /// Começa a ler a origem em segundo plano e a observar a pasta.
    ///
    /// # Panics
    ///
    /// Se o sistema não deixar criar a thread do índice.
    #[must_use]
    pub fn start(options: LiveIndexOptions) -> Self {
        let shared = Arc::new(Shared {
            options,
            index: RwLock::new(ConfigIndex::default()),
            status: Mutex::new(IndexStatus {
                available: true,
                ready: false,
                indexed_files: 0,
                total_files: 0,
                entries: 0,
            }),
            pending: Mutex::new(Pending {
                dirty: true,
                ..Pending::default()
            }),
            wake: Condvar::new(),
            watcher: Mutex::new(None),
            last_sync: Mutex::new(None),
        });
        let worker = Arc::clone(&shared);
        #[allow(clippy::expect_used)] // sem thread não há índice; acontece só sem recursos
        std::thread::Builder::new()
            .name("warden-config-index".into())
            .spawn(move || worker.run())
            .expect("criar a thread do índice de configs");
        Self { shared }
    }

    /// Situação do índice.
    #[must_use]
    pub fn status(&self) -> IndexStatus {
        *lock(&self.shared.status)
    }

    /// Pasta da origem.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.shared.options.root
    }

    /// Busca no que já foi lido. `None` se `cancel` foi acionado no meio.
    #[must_use]
    pub fn search(
        &self,
        query: &SearchQuery,
        cancel: &CancellationToken,
    ) -> Option<ConfigSearchResults> {
        self.shared
            .index
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .search(query, cancel)
    }

    /// Pede uma comparação da origem com o índice, sem esperar os avisos do observador. Não
    /// faz nada se a última foi há menos de `min_interval`.
    pub fn refresh(&self, min_interval: Duration) {
        let recent = lock(&self.shared.last_sync).is_some_and(|at| at.elapsed() < min_interval);
        if !recent {
            self.shared.signal(None);
        }
    }

    /// Espera a primeira leitura terminar (até `timeout`). Devolve se terminou.
    #[must_use]
    pub fn wait_ready(&self, timeout: Duration) -> bool {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if self.status().ready {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.status().ready
    }
}

impl Drop for LiveIndex {
    fn drop(&mut self) {
        lock(&self.shared.pending).stop = true;
        self.shared.wake.notify_all();
    }
}

impl Shared {
    fn signal(&self, forced: Option<String>) {
        let mut pending = lock(&self.pending);
        pending.dirty = true;
        if let Some(path) = forced {
            pending.forced.insert(path);
            pending.last_signal = Some(Instant::now());
        }
        self.wake.notify_all();
    }

    fn stopped(&self) -> bool {
        lock(&self.pending).stop
    }

    fn run(self: Arc<Self>) {
        while let Some(forced) = self.next_round() {
            self.sync(&forced);
        }
        // A thread é a última dona do observador: soltá-lo aqui para de observar.
        lock(&self.watcher).take();
    }

    /// Espera um pedido e os avisos pararem. `None` = parar.
    fn next_round(&self) -> Option<BTreeSet<String>> {
        let debounce = self.options.debounce;
        let mut pending = lock(&self.pending);
        while !pending.dirty && !pending.stop {
            pending = self
                .wake
                .wait(pending)
                .unwrap_or_else(PoisonError::into_inner);
        }
        let waiting = Instant::now();
        loop {
            if pending.stop {
                return None;
            }
            let quiet = pending.last_signal.map_or(debounce, |at| at.elapsed());
            if quiet >= debounce || waiting.elapsed() >= debounce * 4 {
                break;
            }
            pending = self
                .wake
                .wait_timeout(pending, debounce.saturating_sub(quiet))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        pending.dirty = false;
        pending.last_signal = None;
        Some(std::mem::take(&mut pending.forced))
    }

    fn write_index(&self) -> std::sync::RwLockWriteGuard<'_, ConfigIndex> {
        self.index.write().unwrap_or_else(PoisonError::into_inner)
    }

    fn set_status(&self, change: impl FnOnce(&mut IndexStatus)) -> IndexStatus {
        let mut status = lock(&self.status);
        change(&mut status);
        *status
    }

    fn sync(self: &Arc<Self>, forced: &BTreeSet<String>) {
        let root = &self.options.root;
        let first = !lock(&self.status).ready;
        let listed = if root.is_dir() {
            (self.options.lister)(root)
        } else {
            Ok(None)
        };
        let files = match listed {
            Ok(Some(files)) => files,
            Ok(None) => {
                self.unavailable(first);
                return;
            }
            // Falha passageira na leitura da pasta: fica o que já está no índice.
            Err(_) => return,
        };
        self.ensure_watcher();
        let plan = self
            .index
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .plan(&files, forced);
        let total = count(files.len());
        self.set_status(|status| {
            status.available = true;
            status.total_files = total;
            if !status.ready {
                status.indexed_files = total.saturating_sub(count(plan.read.len()));
            }
        });
        let removed: Vec<ReadFile> = plan
            .remove
            .into_iter()
            .map(|path| ReadFile { path, index: None })
            .collect();
        let mut changed = self.write_index().apply(removed);
        for chunk in plan.read.chunks(BATCH) {
            if self.stopped() {
                return;
            }
            let batch: Vec<ReadFile> = chunk
                .iter()
                .map(|file| ReadFile {
                    path: file.path.clone(),
                    index: read_file(root, file),
                })
                .collect();
            let mut index = self.write_index();
            changed += index.apply(batch);
            let entries = count(index.entry_count());
            drop(index);
            self.set_status(|status| {
                if !status.ready {
                    status.indexed_files += count(chunk.len());
                }
                status.entries = entries;
            });
        }
        let entries = count(self.write_index().entry_count());
        let status = self.set_status(|status| {
            status.ready = true;
            status.indexed_files = status.total_files;
            status.entries = entries;
        });
        *lock(&self.last_sync) = Some(Instant::now());
        if first || changed > 0 {
            (self.options.on_change)(IndexChange {
                status,
                changed_files: count(changed),
                first,
            });
        }
    }

    /// A origem não existe (ou deixou de existir): índice vazio, sem observador.
    fn unavailable(&self, first: bool) {
        lock(&self.watcher).take();
        let removed = self.write_index().clear();
        let status = self.set_status(|status| {
            *status = IndexStatus {
                available: false,
                ready: true,
                indexed_files: 0,
                total_files: 0,
                entries: 0,
            };
        });
        *lock(&self.last_sync) = Some(Instant::now());
        if first || removed > 0 {
            (self.options.on_change)(IndexChange {
                status,
                changed_files: count(removed),
                first,
            });
        }
    }

    /// Liga o observador, se ainda não está ligado. Sem observador, as mudanças chegam só pelo
    /// [`LiveIndex::refresh`].
    fn ensure_watcher(self: &Arc<Self>) {
        let mut slot = lock(&self.watcher);
        if slot.is_some() {
            return;
        }
        let weak: Weak<Self> = Arc::downgrade(self);
        let root = self.options.root.clone();
        let created = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let (Ok(event), Some(shared)) = (event, weak.upgrade()) else {
                return;
            };
            if !is_relevant_kind(event.kind) {
                return;
            }
            for path in &event.paths {
                let Ok(relative) = path.strip_prefix(&root) else {
                    continue;
                };
                let parts: Vec<&str> = relative
                    .components()
                    .filter_map(|part| part.as_os_str().to_str())
                    .collect();
                if parts.is_empty() || !(shared.options.relevant)(&parts) {
                    continue;
                }
                shared.signal(Some(parts.join("/")));
            }
        });
        if let Ok(mut watcher) = created
            && watcher
                .watch(&self.options.root, RecursiveMode::Recursive)
                .is_ok()
        {
            *slot = Some(watcher);
        }
    }
}

/// Avisos que podem mudar o conteúdo de um arquivo. Leituras (inclusive as do próprio índice)
/// e mudanças de permissão ficam de fora.
fn is_relevant_kind(kind: EventKind) -> bool {
    match kind {
        EventKind::Modify(ModifyKind::Metadata(metadata)) => {
            matches!(metadata, MetadataKind::WriteTime | MetadataKind::Any)
        }
        EventKind::Access(_) => false,
        EventKind::Create(_)
        | EventKind::Remove(_)
        | EventKind::Modify(_)
        | EventKind::Any
        | EventKind::Other => true,
    }
}

#[cfg(test)]
mod tests {
    use notify::event::{AccessKind, CreateKind, DataChange, RemoveKind};

    use super::*;

    #[test]
    fn so_avisos_que_mudam_conteudo() {
        assert!(is_relevant_kind(EventKind::Create(CreateKind::File)));
        assert!(is_relevant_kind(EventKind::Remove(RemoveKind::Any)));
        assert!(is_relevant_kind(EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
        assert!(is_relevant_kind(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::WriteTime
        ))));
        assert!(!is_relevant_kind(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::AccessTime
        ))));
        assert!(!is_relevant_kind(EventKind::Access(AccessKind::Any)));
    }
}
