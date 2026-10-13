//! Busca em todas as configs (C-05; SPEC T12; ARCHITECTURE §10.1).
//!
//! - `config_search`: busca no índice da origem (pack ou instância de teste). Na primeira
//!   chamada para uma origem, o índice começa a ser lido em segundo plano e a resposta já traz
//!   a situação ([`IndexStatus`]) e o que foi lido até ali. Busca vazia só prepara o índice (a
//!   tela chama ao abrir, para o índice estar pronto quando a pessoa digitar). Cada busca
//!   cancela a anterior da mesma origem (`cancelled = true` na resposta da cancelada).
//! - `config-index-changed`: o índice terminou a primeira leitura ou mudou porque um arquivo
//!   mudou. Mudança de arquivo também vira `pack-changed` (Configs e, no pack, Histórico), para
//!   a árvore, o arquivo aberto e as alterações não salvas serem relidos sem reiniciar o app
//!   (CA-T12-07).
//!
//! Os índices ficam só em memória, no [`ConfigSearchHub`] do estado: no máximo
//! [`MAX_INDEXES`] origens abertas; a menos usada é solta (para a thread e o observador).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_specta::Event as _;
use warden_configs::index::{
    ConfigSearchResults, IndexChange, IndexStatus, LiveIndex, LiveIndexOptions, SearchQuery,
    SourceFile,
};
use warden_core::{CancellationToken, PackId, TEMP_SUFFIX};
use warden_project::configs::{ConfigOrigin, list_files};

use crate::commands::configs::origin_root;
use crate::commands::inventory::notify_changed;
use crate::error::AppError;
use crate::events::PackArea;
use crate::state::AppState;

/// Origens com índice aberto ao mesmo tempo.
pub(crate) const MAX_INDEXES: usize = 4;

/// Espera depois do último aviso do observador antes de atualizar o índice.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// Uma busca confere a origem de novo (sem esperar o observador) se a última comparação foi há
/// mais que isto: cobre avisos perdidos e a instância criada depois de o índice abrir.
const REFRESH_INTERVAL: Duration = Duration::from_secs(3);

/// Maior busca aceita (caracteres); o resto é ignorado.
const MAX_QUERY_CHARS: usize = 200;

/// Pastas de itens do pack: o packwiz as gerencia e não são configs.
const ITEM_DIRS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];

/// Resposta de `config_search`.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigSearchResponse {
    /// Situação do índice no momento da busca.
    pub status: IndexStatus,
    /// Resultados (`None` na busca vazia e na cancelada).
    pub results: Option<ConfigSearchResults>,
    /// Uma busca mais nova da mesma origem chegou antes desta terminar.
    pub cancelled: bool,
}

/// `config-index-changed`: o índice de uma origem terminou a primeira leitura ou mudou.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "config-index-changed")]
pub(crate) struct ConfigIndexChanged {
    /// Pack.
    pub pack_id: PackId,
    /// Origem do índice.
    pub origin: ConfigOrigin,
    /// Situação depois da mudança.
    pub status: IndexStatus,
    /// Arquivos que entraram, mudaram ou saíram (0 na primeira leitura de uma origem vazia).
    pub changed_files: u32,
    /// Foi a primeira leitura, não uma mudança de arquivo.
    pub first: bool,
}

/// Quem recebe as mudanças de um índice (o app emite eventos; os testes guardam).
pub(crate) type ChangeSink = Arc<dyn Fn(ConfigIndexChanged) + Send + Sync>;

struct Slot {
    pack_id: PackId,
    origin: ConfigOrigin,
    index: Arc<LiveIndex>,
}

/// Os índices abertos e a busca em andamento de cada origem (só memória).
#[derive(Default)]
pub(crate) struct ConfigSearchHub {
    /// Do mais usado para o menos.
    slots: Mutex<Vec<Slot>>,
    searches: Mutex<HashMap<(PackId, ConfigOrigin), CancellationToken>>,
}

impl std::fmt::Debug for ConfigSearchHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigSearchHub")
            .field("indexes", &lock(&self.slots).len())
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ConfigSearchHub {
    /// O índice da origem, aberto agora se ainda não estava (ou se a pasta mudou). Um índice
    /// que já existia confere a origem de novo se a última comparação ficou para trás.
    pub(crate) fn index(
        &self,
        pack_id: PackId,
        origin: ConfigOrigin,
        root: PathBuf,
        sink: ChangeSink,
    ) -> Arc<LiveIndex> {
        let mut slots = lock(&self.slots);
        if let Some(position) = slots.iter().position(|slot| {
            slot.pack_id == pack_id && slot.origin == origin && slot.index.root() == root
        }) {
            let slot = slots.remove(position);
            let index = Arc::clone(&slot.index);
            slots.insert(0, slot);
            drop(slots);
            index.refresh(REFRESH_INTERVAL);
            return index;
        }
        slots.retain(|slot| !(slot.pack_id == pack_id && slot.origin == origin));
        let index = Arc::new(start_index(pack_id, origin, root, sink));
        slots.insert(
            0,
            Slot {
                pack_id,
                origin,
                index: Arc::clone(&index),
            },
        );
        slots.truncate(MAX_INDEXES);
        index
    }

    /// Começa uma busca na origem: cancela a anterior, que ainda pode estar rodando.
    pub(crate) fn begin_search(&self, pack_id: PackId, origin: ConfigOrigin) -> CancellationToken {
        let token = CancellationToken::new();
        if let Some(previous) = lock(&self.searches).insert((pack_id, origin), token.clone()) {
            previous.cancel();
        }
        token
    }
}

/// Os arquivos da origem, como o editor os lista (C-02): o índice nunca lê o que a árvore
/// não mostra.
fn lister(origin: ConfigOrigin) -> warden_configs::index::Lister {
    Arc::new(move |root: &Path| {
        let list = list_files(root, origin).map_err(std::io::Error::other)?;
        if !list.available {
            return Ok(None);
        }
        Ok(Some(
            list.files
                .into_iter()
                .map(|file| SourceFile {
                    path: file.path,
                    size: u64::from(file.size),
                    modified_ms: file.modified_at_ms,
                    binary: file.binary,
                })
                .collect(),
        ))
    })
}

/// Se um aviso do observador interessa à busca. Fica de fora o que muda o tempo todo e não é
/// config: `.git/` e outros ocultos, `mods/`, os logs e os mundos do jogo, os temporários das
/// gravações do Warden.
fn is_relevant(origin: ConfigOrigin, parts: &[&str]) -> bool {
    let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
        return false;
    };
    if last.to_ascii_lowercase().ends_with(TEMP_SUFFIX) {
        return false;
    }
    let is = |name: &str, expected: &str| name.eq_ignore_ascii_case(expected);
    match origin {
        ConfigOrigin::Pack => {
            !first.starts_with('.') && !ITEM_DIRS.iter().any(|dir| is(first, dir))
        }
        ConfigOrigin::Instance => {
            is(first, "config")
                || is(first, "defaultconfigs")
                || (parts.len() == 1 && last.to_ascii_lowercase().starts_with("options"))
                || (is(first, "saves") && (parts.len() < 3 || is(parts[2], "serverconfig")))
        }
    }
}

fn start_index(
    pack_id: PackId,
    origin: ConfigOrigin,
    root: PathBuf,
    sink: ChangeSink,
) -> LiveIndex {
    LiveIndex::start(LiveIndexOptions {
        root,
        lister: lister(origin),
        relevant: Arc::new(move |parts: &[&str]| is_relevant(origin, parts)),
        on_change: Arc::new(move |change: IndexChange| {
            sink(ConfigIndexChanged {
                pack_id,
                origin,
                status: change.status,
                changed_files: change.changed_files,
                first: change.first,
            });
        }),
        debounce: DEBOUNCE,
    })
}

/// Emite a mudança e, se um arquivo mudou, o `pack-changed` das áreas que mostram configs.
fn app_sink(app: AppHandle) -> ChangeSink {
    Arc::new(move |change: ConfigIndexChanged| {
        if let Err(error) = change.clone().emit(&app) {
            tracing::warn!(%error, "não foi possível avisar a interface da mudança no índice");
        }
        if !change.first && change.changed_files > 0 {
            let areas = match change.origin {
                ConfigOrigin::Pack => vec![PackArea::Configs, PackArea::History],
                ConfigOrigin::Instance => vec![PackArea::Configs],
            };
            notify_changed(&app, change.pack_id, areas);
        }
    })
}

/// Busca em todas as configs da origem. Busca vazia só prepara o índice e devolve a situação.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_search(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    origin: ConfigOrigin,
    query: String,
    mod_id: Option<String>,
) -> Result<ConfigSearchResponse, AppError> {
    let root = origin_root(&state, pack_id, origin)?;
    search_impl(
        &state.config_search,
        pack_id,
        origin,
        root,
        &query,
        mod_id,
        app_sink(app),
    )
    .await
}

async fn search_impl(
    hub: &ConfigSearchHub,
    pack_id: PackId,
    origin: ConfigOrigin,
    root: PathBuf,
    query: &str,
    mod_id: Option<String>,
    sink: ChangeSink,
) -> Result<ConfigSearchResponse, AppError> {
    let index = hub.index(pack_id, origin, root, sink);
    let text: String = query.trim().chars().take(MAX_QUERY_CHARS).collect();
    if text.is_empty() {
        return Ok(ConfigSearchResponse {
            status: index.status(),
            results: None,
            cancelled: false,
        });
    }
    let token = hub.begin_search(pack_id, origin);
    let search = SearchQuery::new(text).with_mod(mod_id.filter(|id| !id.is_empty()));
    tauri::async_runtime::spawn_blocking(move || {
        let status = index.status();
        let results = index.search(&search, &token);
        ConfigSearchResponse {
            status,
            cancelled: results.is_none(),
            results,
        }
    })
    .await
    .map_err(|error| AppError::internal(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Instant;

    use super::*;

    fn sink() -> (ChangeSink, Arc<Mutex<Vec<ConfigIndexChanged>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let store = Arc::clone(&seen);
        (
            Arc::new(move |change| lock(&store).push(change)),
            seen,
        )
    }

    /// Repete a busca até o índice ficar pronto e `check` aceitar a resposta.
    async fn search_until(
        hub: &ConfigSearchHub,
        pack_id: PackId,
        root: &Path,
        text: &str,
        sink: &ChangeSink,
        check: impl Fn(&ConfigSearchResponse) -> bool,
    ) -> ConfigSearchResponse {
        let started = Instant::now();
        loop {
            let response = search_impl(
                hub,
                pack_id,
                ConfigOrigin::Pack,
                root.to_path_buf(),
                text,
                None,
                Arc::clone(sink),
            )
            .await
            .unwrap();
            if response.status.ready && check(&response) {
                return response;
            }
            assert!(started.elapsed() < Duration::from_secs(10), "{response:?}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[tokio::test]
    async fn busca_no_pack_so_o_que_o_editor_mostra_e_ve_a_edicao_por_fora() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("pack");
        fs::create_dir_all(root.join("config")).unwrap();
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::write(root.join("pack.toml"), "name = \"spawnPack\"\n").unwrap();
        fs::write(root.join("mods/x.pw.toml"), "name = \"spawnMod\"\n").unwrap();
        fs::write(root.join("config/mobs-common.toml"), "spawnRate = 5\n").unwrap();
        let hub = ConfigSearchHub::default();
        let pack_id = PackId::new();
        let (sink, seen) = sink();

        // Vazia: só prepara.
        let prepared = search_impl(
            &hub,
            pack_id,
            ConfigOrigin::Pack,
            root.clone(),
            "  ",
            None,
            Arc::clone(&sink),
        )
        .await
        .unwrap();
        assert!(prepared.results.is_none() && !prepared.cancelled);

        let found = search_until(&hub, pack_id, &root, "spawn", &sink, |r| {
            r.results.as_ref().is_some_and(|r| r.total_hits > 0)
        })
        .await;
        let results = found.results.unwrap();
        // O `pack.toml` e o `.pw.toml` não são do editor: não entram na busca.
        let paths: Vec<&str> = results.groups.iter().map(|g| g.path.as_str()).collect();
        assert_eq!(paths, ["config/mobs-common.toml"]);
        assert_eq!(results.groups[0].mod_id.as_deref(), Some("mobs"));

        fs::write(root.join("config/mobs-common.toml"), "despawnRate = 5\n").unwrap();
        search_until(&hub, pack_id, &root, "despawn", &sink, |r| {
            r.results.as_ref().is_some_and(|r| r.total_hits == 1)
        })
        .await;
        let changes = lock(&seen).clone();
        assert!(changes[0].first && changes[0].pack_id == pack_id);
        assert!(changes.iter().any(|c| !c.first && c.changed_files == 1));
    }

    #[test]
    fn indice_reaproveitado_e_o_menos_usado_solto() {
        let dir = tempfile::tempdir().unwrap();
        let hub = ConfigSearchHub::default();
        let (sink, _) = sink();
        let first = PackId::new();
        let a = hub.index(first, ConfigOrigin::Pack, dir.path().into(), Arc::clone(&sink));
        let again = hub.index(first, ConfigOrigin::Pack, dir.path().into(), Arc::clone(&sink));
        assert!(Arc::ptr_eq(&a, &again));
        // Outra pasta para a mesma origem (pack movido): índice novo.
        let moved = hub.index(
            first,
            ConfigOrigin::Pack,
            dir.path().join("x"),
            Arc::clone(&sink),
        );
        assert!(!Arc::ptr_eq(&a, &moved));
        assert_eq!(lock(&hub.slots).len(), 1);
        for _ in 0..MAX_INDEXES + 2 {
            let _ = hub.index(
                PackId::new(),
                ConfigOrigin::Instance,
                dir.path().into(),
                Arc::clone(&sink),
            );
        }
        let slots = lock(&hub.slots);
        assert_eq!(slots.len(), MAX_INDEXES);
        assert!(slots.iter().all(|slot| slot.pack_id != first));
    }

    #[test]
    fn busca_nova_cancela_a_anterior_da_mesma_origem() {
        let hub = ConfigSearchHub::default();
        let pack = PackId::new();
        let first = hub.begin_search(pack, ConfigOrigin::Pack);
        let other = hub.begin_search(pack, ConfigOrigin::Instance);
        let second = hub.begin_search(pack, ConfigOrigin::Pack);
        assert!(first.is_cancelled());
        assert!(!other.is_cancelled() && !second.is_cancelled());
    }

    #[test]
    fn avisos_que_interessam_a_cada_origem() {
        let pack = |path: &str| is_relevant(ConfigOrigin::Pack, &path.split('/').collect::<Vec<_>>());
        assert!(pack("config/a.toml"));
        assert!(pack("options.txt"));
        assert!(pack("kubejs/server_scripts/x.js"));
        assert!(!pack(".git/index"));
        assert!(!pack("mods/x.pw.toml"));
        assert!(!pack("config/a.toml.1234-0.warden-tmp"));
        let instance =
            |path: &str| is_relevant(ConfigOrigin::Instance, &path.split('/').collect::<Vec<_>>());
        assert!(instance("config/a.toml"));
        assert!(instance("defaultconfigs/b.toml"));
        assert!(instance("options.txt"));
        assert!(instance("saves/Mundo"));
        assert!(instance("saves/Mundo/serverconfig/c.toml"));
        assert!(!instance("saves/Mundo/region/r.0.0.mca"));
        assert!(!instance("logs/latest.log"));
        assert!(!instance("mods/a.jar"));
        assert!(!is_relevant(ConfigOrigin::Pack, &[]));
    }
}
