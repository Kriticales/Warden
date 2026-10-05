//! Cache **só em memória** das respostas da CurseForge (termos da API, §3.1(e); ARCHITECTURE
//! §17): nada daqui vai para o disco, e tudo some quando o app fecha.
//!
//! - Busca: 5 minutos (ARCHITECTURE §17.1).
//! - Projetos, arquivos e descrições: 30 minutos (a sessão de edição de um pack).
//! - Cada tabela guarda no máximo [`MAX_ENTRIES`] itens; ao passar, os vencidos saem primeiro
//!   e depois os mais antigos.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use warden_http::Timer;

use crate::model::{File, Mod, SearchResults};
use crate::query::SearchQuery;

/// Validade da busca.
pub const SEARCH_TTL: Duration = Duration::from_mins(5);
/// Validade de projetos, arquivos e descrições.
pub const ITEM_TTL: Duration = Duration::from_mins(30);
/// Itens por tabela.
pub const MAX_ENTRIES: usize = 4096;

struct Entry<V> {
    value: V,
    stored_at: Duration,
}

struct Table<K, V> {
    entries: HashMap<K, Entry<V>>,
    ttl: Duration,
}

impl<K: Eq + Hash + Clone, V: Clone> Table<K, V> {
    fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    fn get(&self, key: &K, now: Duration) -> Option<V> {
        let entry = self.entries.get(key)?;
        (now.saturating_sub(entry.stored_at) < self.ttl).then(|| entry.value.clone())
    }

    fn put(&mut self, key: K, value: V, now: Duration) {
        if self.entries.len() >= MAX_ENTRIES && !self.entries.contains_key(&key) {
            let ttl = self.ttl;
            self.entries
                .retain(|_, entry| now.saturating_sub(entry.stored_at) < ttl);
            if self.entries.len() >= MAX_ENTRIES {
                let mut ages: Vec<Duration> =
                    self.entries.values().map(|entry| entry.stored_at).collect();
                ages.sort_unstable();
                let cutoff = ages[ages.len() / 2];
                self.entries.retain(|_, entry| entry.stored_at > cutoff);
            }
        }
        self.entries.insert(
            key,
            Entry {
                value,
                stored_at: now,
            },
        );
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

struct Tables {
    searches: Table<SearchQuery, SearchResults>,
    mods: Table<u64, Mod>,
    files: Table<u64, File>,
    descriptions: Table<u64, String>,
}

/// O cache em memória. Barato de clonar: os clones dividem as tabelas.
#[derive(Clone)]
pub struct MemoryCache {
    tables: Arc<Mutex<Tables>>,
    timer: Arc<dyn Timer>,
}

impl std::fmt::Debug for MemoryCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Só os tamanhos: o conteúdo é da CurseForge.
        let (searches, mods, files, descriptions) = self.sizes();
        f.debug_struct("MemoryCache")
            .field("searches", &searches)
            .field("mods", &mods)
            .field("files", &files)
            .field("descriptions", &descriptions)
            .finish()
    }
}

impl MemoryCache {
    /// Cache vazio, com o relógio dado (testes: [`warden_http::ManualTimer`]).
    #[must_use]
    pub fn new(timer: Arc<dyn Timer>) -> Self {
        Self {
            tables: Arc::new(Mutex::new(Tables {
                searches: Table::new(SEARCH_TTL),
                mods: Table::new(ITEM_TTL),
                files: Table::new(ITEM_TTL),
                descriptions: Table::new(ITEM_TTL),
            })),
            timer,
        }
    }

    fn with<T>(&self, work: impl FnOnce(&mut Tables, Duration) -> T) -> T {
        let now = self.timer.now();
        let mut tables = self.tables.lock().unwrap_or_else(PoisonError::into_inner);
        work(&mut tables, now)
    }

    /// Quantos itens há em cada tabela (busca, projetos, arquivos, descrições).
    #[must_use]
    pub fn sizes(&self) -> (usize, usize, usize, usize) {
        self.with(|t, _| {
            (
                t.searches.len(),
                t.mods.len(),
                t.files.len(),
                t.descriptions.len(),
            )
        })
    }

    /// Esvazia tudo (ao trocar a chave, por exemplo).
    pub fn clear(&self) {
        self.with(|t, _| {
            t.searches.entries.clear();
            t.mods.entries.clear();
            t.files.entries.clear();
            t.descriptions.entries.clear();
        });
    }

    pub(crate) fn search(&self, query: &SearchQuery) -> Option<SearchResults> {
        self.with(|t, now| t.searches.get(query, now))
    }

    pub(crate) fn put_search(&self, query: SearchQuery, results: SearchResults) {
        self.with(|t, now| {
            for project in &results.mods {
                t.mods.put(project.id, project.clone(), now);
            }
            t.searches.put(query, results, now);
        });
    }

    pub(crate) fn project(&self, id: u64) -> Option<Mod> {
        self.with(|t, now| t.mods.get(&id, now))
    }

    pub(crate) fn put_projects(&self, projects: &[Mod]) {
        self.with(|t, now| {
            for project in projects {
                t.mods.put(project.id, project.clone(), now);
            }
        });
    }

    pub(crate) fn file(&self, id: u64) -> Option<File> {
        self.with(|t, now| t.files.get(&id, now))
    }

    pub(crate) fn put_files(&self, files: &[File]) {
        self.with(|t, now| {
            for file in files {
                t.files.put(file.id, file.clone(), now);
            }
        });
    }

    pub(crate) fn description(&self, id: u64) -> Option<String> {
        self.with(|t, now| t.descriptions.get(&id, now))
    }

    pub(crate) fn put_description(&self, id: u64, text: String) {
        self.with(|t, now| t.descriptions.put(id, text, now));
    }
}

#[cfg(test)]
mod tests {
    use warden_http::ManualTimer;

    use super::*;

    fn project(id: u64) -> Mod {
        Mod {
            id,
            ..Mod::default()
        }
    }

    #[test]
    fn vence_pelo_tempo() {
        let timer = ManualTimer::new();
        let cache = MemoryCache::new(Arc::new(timer.clone()));
        let query = SearchQuery::new("jei");
        cache.put_search(
            query.clone(),
            SearchResults {
                mods: vec![project(1)],
                pagination: crate::Pagination::default(),
            },
        );
        assert!(cache.search(&query).is_some());
        assert!(cache.project(1).is_some(), "a busca alimenta os projetos");
        timer.advance(SEARCH_TTL);
        assert!(cache.search(&query).is_none());
        assert!(cache.project(1).is_some());
        timer.advance(ITEM_TTL);
        assert!(cache.project(1).is_none());
        cache.put_description(1, "x".into());
        cache.put_files(&[File {
            id: 5,
            ..File::default()
        }]);
        assert_eq!(cache.description(1).as_deref(), Some("x"));
        assert!(cache.file(5).is_some());
        assert_eq!(cache.sizes(), (1, 1, 1, 1));
        cache.clear();
        assert_eq!(cache.sizes(), (0, 0, 0, 0));
        assert!(format!("{cache:?}").contains("mods: 0"));
    }

    #[test]
    fn tamanho_limitado() {
        let timer = ManualTimer::new();
        let cache = MemoryCache::new(Arc::new(timer.clone()));
        for id in 0..(MAX_ENTRIES as u64 + 10) {
            timer.advance(Duration::from_millis(1));
            cache.put_projects(&[project(id)]);
        }
        let (_, mods, _, _) = cache.sizes();
        assert!(mods <= MAX_ENTRIES, "{mods}");
        assert!(mods >= MAX_ENTRIES / 2 - 1, "{mods}");
        assert!(
            cache.project(MAX_ENTRIES as u64 + 9).is_some(),
            "o mais novo fica"
        );
        assert!(cache.project(0).is_none(), "o mais antigo sai");
        // Com a tabela cheia, os vencidos saem primeiro.
        let cache = MemoryCache::new(Arc::new(timer.clone()));
        let full: Vec<Mod> = (0..MAX_ENTRIES as u64).map(project).collect();
        cache.put_projects(&full);
        assert_eq!(cache.sizes().1, MAX_ENTRIES);
        timer.advance(ITEM_TTL);
        cache.put_projects(&[project(u64::MAX)]);
        assert_eq!(cache.sizes().1, 1);
    }
}
