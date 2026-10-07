//! Vigia das mudanças externas dos packs abertos (ARCHITECTURE §15; ROADMAP A-05).
//!
//! Um vigia por pack aberto, contado por abertura: a tela do pack liga ao montar e desliga ao
//! sair (`pack_watch_start`/`pack_watch_stop`). A detecção em si é de
//! [`warden_project::watch`]; aqui ficam o registro dos vigias e a ligação ao evento
//! `pack-changed`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use warden_core::PackId;
use warden_project::watch::{Area, PackWatcher, WatchConfig, WriteLedger};

use crate::error::AppError;
use crate::events::PackArea;

/// O que acontece quando uma mudança externa é detectada.
pub type OnExternalChange = Box<dyn Fn(PackId, Vec<PackArea>) + Send + 'static>;

struct Entry {
    opens: usize,
    // Só é solto, nunca lido: soltar para a vigia.
    _watcher: PackWatcher,
}

/// Os vigias dos packs abertos.
#[derive(Default)]
pub struct PackWatchers {
    entries: Mutex<HashMap<PackId, Entry>>,
}

impl std::fmt::Debug for PackWatchers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackWatchers").finish_non_exhaustive()
    }
}

impl PackWatchers {
    /// Vazio.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Uma abertura a mais do pack; o primeiro começa a vigiar.
    pub fn start(
        &self,
        pack: PackId,
        root: &Path,
        ledger: &WriteLedger,
        config: WatchConfig,
        on_change: OnExternalChange,
    ) -> Result<(), AppError> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = entries.get_mut(&pack) {
            entry.opens += 1;
            return Ok(());
        }
        let watcher = PackWatcher::start(pack, root, ledger.clone(), config, move |areas| {
            on_change(pack, areas.into_iter().map(pack_area).collect());
        })
        .map_err(|error| AppError::from_domain(&error))?;
        entries.insert(
            pack,
            Entry {
                opens: 1,
                _watcher: watcher,
            },
        );
        Ok(())
    }

    /// Uma abertura a menos; a última para de vigiar.
    pub fn stop(&self, pack: PackId) {
        let removed = {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            match entries.get_mut(&pack) {
                Some(entry) if entry.opens > 1 => {
                    entry.opens -= 1;
                    None
                }
                Some(_) => entries.remove(&pack),
                None => None,
            }
        };
        // Soltar espera a thread do vigia: fora da trava.
        drop(removed);
    }

    /// Quantos packs estão sendo vigiados (testes).
    #[cfg(test)]
    pub(crate) fn watching(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

fn pack_area(area: Area) -> PackArea {
    match area {
        Area::Inventory => PackArea::Inventory,
        Area::Configs => PackArea::Configs,
        Area::Meta => PackArea::Meta,
        Area::History => PackArea::History,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn listener() -> (OnExternalChange, mpsc::Receiver<(PackId, Vec<PackArea>)>) {
        let (sender, receiver) = mpsc::channel();
        let on_change: OnExternalChange = Box::new(move |pack, areas| {
            let _ = sender.send((pack, areas));
        });
        (on_change, receiver)
    }

    #[test]
    fn edicao_externa_chega_com_o_pack_e_as_areas() {
        let dir = tempfile::tempdir().unwrap();
        let pack = PackId::new();
        let watchers = PackWatchers::new();
        let (on_change, receiver) = listener();
        watchers
            .start(
                pack,
                dir.path(),
                &WriteLedger::default(),
                WatchConfig::default(),
                on_change,
            )
            .unwrap();

        std::fs::write(dir.path().join("options.txt"), "fov:70\n").unwrap();

        let (changed, areas) = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(changed, pack);
        assert_eq!(areas, [PackArea::Configs]);
    }

    #[test]
    fn varias_aberturas_compartilham_o_vigia_ate_a_ultima_sair() {
        let dir = tempfile::tempdir().unwrap();
        let pack = PackId::new();
        let watchers = PackWatchers::new();
        for _ in 0..2 {
            let (on_change, _) = listener();
            watchers
                .start(
                    pack,
                    dir.path(),
                    &WriteLedger::default(),
                    WatchConfig::default(),
                    on_change,
                )
                .unwrap();
        }
        assert_eq!(watchers.watching(), 1);
        watchers.stop(pack);
        assert_eq!(watchers.watching(), 1);
        watchers.stop(pack);
        assert_eq!(watchers.watching(), 0);
        watchers.stop(pack);
    }

    #[test]
    fn pasta_sumida_nao_comeca_a_vigiar() {
        let watchers = PackWatchers::new();
        let (on_change, _) = listener();
        let result = watchers.start(
            PackId::new(),
            Path::new("/nao/existe/mesmo"),
            &WriteLedger::default(),
            WatchConfig::default(),
            on_change,
        );
        assert!(result.is_err());
        assert_eq!(watchers.watching(), 0);
    }
}
