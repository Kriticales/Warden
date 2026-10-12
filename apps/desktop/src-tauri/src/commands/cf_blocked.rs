//! Downloads manuais da CurseForge durante o teste (SPEC T20; ROADMAP L-06).
//!
//! Quando a etapa "Copiar o pack para o teste" encontra mods que o autor bloqueou para apps de
//! terceiros, o teste não falha: [`ManualDownloadsHub::wait`] publica a lista, observa a pasta
//! Downloads (P1) e espera até **todos** os itens estarem no cache de downloads; aí o teste
//! copia de novo o pack (agora com os arquivos achados no cache) e continua sozinho. Cancelar
//! o teste para a espera e o vigia.
//!
//! Comandos:
//! - `cf_blocked_list`: a lista do pack, com o estado de cada item (vazia fora da espera);
//! - `cf_blocked_provide_file`: "Selecionar arquivo baixado…": o diálogo nativo e a conferência
//!   do hash antes de qualquer cópia; arquivo errado fica marcado na lista ("Este não é o
//!   arquivo certo.") e nada é copiado;
//! - `cf_blocked_open_page`: "Abrir página de download" no navegador. O endereço fica no
//!   backend: não vai para a interface, para mensagens nem para os registros.
//!
//! Evento `manual-downloads-changed`: a lista de um pack mudou (a interface relê).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;
use tauri_specta::Event;
use tokio::sync::watch;
use warden_core::{PackId, Progress, ProgressSink as _};
use warden_instance::{
    BlockedFile, DownloadCache, DownloadsWatch, FoundVia, ManualDownloadItem, ManualDownloads,
    WatchContext, display_name, provide_file,
};

use crate::error::{AppError, AppErrorCode};
use crate::operations::OperationHandle;
use crate::state::AppState;

/// Etapa "Esperando os downloads manuais" (dentro de "Copiar o pack para o teste").
pub(crate) const STAGE_MANUAL: &str = "syncPack.manual";
/// Texto da etapa.
const STAGE_MANUAL_LABEL: &str = "downloadsManuais:etapa";

/// Em build de debug, o E2E troca o diálogo de escolher arquivo (que o `WebDriver` não
/// alcança) pelo caminho escrito neste arquivo.
const E2E_PICK_FILE_ENV: &str = "WARDEN_E2E_PICK_FILE";
/// Em build de debug, a pasta observada no lugar da pasta Downloads do sistema (E2E).
const DOWNLOADS_DIR_ENV: &str = "WARDEN_DOWNLOADS_DIR";

/// `manual-downloads-changed`: a lista de downloads manuais do pack mudou (apareceu, um item
/// mudou de estado ou acabou).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "manual-downloads-changed")]
pub(crate) struct ManualDownloadsChanged {
    /// O pack.
    pub(crate) pack_id: PackId,
}

/// Quem avisa a interface.
type Emit = Arc<dyn Fn(PackId) + Send + Sync>;

/// A lista de um pack que está esperando arquivos.
struct Waiting {
    list: ManualDownloads,
    changed: watch::Sender<u64>,
    cache: Arc<DownloadCache>,
}

/// Ligação com o app: o evento e a pasta Downloads.
#[derive(Default)]
struct Connection {
    emit: Option<Emit>,
    downloads_dir: Option<PathBuf>,
}

/// As listas de downloads manuais em espera, por pack (só memória).
#[derive(Default)]
pub(crate) struct ManualDownloadsHub {
    waiting: Mutex<HashMap<PackId, Waiting>>,
    connection: Mutex<Connection>,
}

impl std::fmt::Debug for ManualDownloadsHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManualDownloadsHub")
            .field("packs", &self.lock().keys().copied().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

fn unpoisoned<'a, T>(
    guard: Result<MutexGuard<'a, T>, PoisonError<MutexGuard<'a, T>>>,
) -> MutexGuard<'a, T> {
    guard.unwrap_or_else(PoisonError::into_inner)
}

impl ManualDownloadsHub {
    fn lock(&self) -> MutexGuard<'_, HashMap<PackId, Waiting>> {
        unpoisoned(self.waiting.lock())
    }

    /// Liga o evento e a pasta Downloads observada (`None`: não observa).
    pub(crate) fn connect(&self, emit: Emit, downloads_dir: Option<PathBuf>) {
        let mut connection = unpoisoned(self.connection.lock());
        connection.emit = Some(emit);
        connection.downloads_dir = downloads_dir;
    }

    fn emit(&self, pack_id: PackId) {
        let emit = unpoisoned(self.connection.lock()).emit.clone();
        if let Some(emit) = emit {
            emit(pack_id);
        }
    }

    fn downloads_dir(&self) -> Option<PathBuf> {
        unpoisoned(self.connection.lock()).downloads_dir.clone()
    }

    /// A lista do pack (vazia fora da espera).
    pub(crate) fn list(&self, pack_id: PackId) -> Vec<ManualDownloadItem> {
        self.lock()
            .get(&pack_id)
            .map(|waiting| waiting.list.items())
            .unwrap_or_default()
    }

    /// O item esperado e o cache. Erro: `app.MANUAL_DOWNLOAD_NOT_WAITING`.
    fn expected(
        &self,
        pack_id: PackId,
        metafile: &str,
    ) -> Result<(BlockedFile, Arc<DownloadCache>), AppError> {
        self.lock()
            .get(&pack_id)
            .and_then(|waiting| {
                waiting
                    .list
                    .get(metafile)
                    .map(|file| (file.clone(), Arc::clone(&waiting.cache)))
            })
            .ok_or_else(|| AppError::new(AppErrorCode::ManualDownloadNotWaiting))
    }

    /// Muda o item e devolve como ficou. Avisa a interface e a espera se algo mudou.
    fn update(
        &self,
        pack_id: PackId,
        metafile: &str,
        change: impl FnOnce(&mut ManualDownloads) -> bool,
    ) -> Option<ManualDownloadItem> {
        let (item, changed) = {
            let mut guard = self.lock();
            let waiting = guard.get_mut(&pack_id)?;
            let changed = change(&mut waiting.list);
            if changed {
                waiting.changed.send_modify(|version| *version += 1);
            }
            (waiting.list.item(metafile), changed)
        };
        if changed {
            self.emit(pack_id);
        }
        item
    }

    /// O arquivo escolhido para um item. Hash certo: vai para o cache e o item fica
    /// encontrado. Hash diferente: nada é copiado e o item fica com "arquivo errado".
    /// Erros: `app.MANUAL_DOWNLOAD_NOT_WAITING`, falha de disco ao ler o arquivo.
    pub(crate) async fn provide(
        &self,
        pack_id: PackId,
        metafile: &str,
        file: PathBuf,
    ) -> Result<ManualDownloadItem, AppError> {
        let (expected, cache) = self.expected(pack_id, metafile)?;
        let chosen = display_name(&file);
        let result = tokio::task::spawn_blocking(move || provide_file(&cache, &expected, &file))
            .await
            .map_err(|error| AppError::internal(format!("conferência interrompida: {error}")))?;
        let item = match result {
            Ok(_) => {
                tracing::info!("download manual escolhido e conferido");
                self.update(pack_id, metafile, |list| {
                    list.mark_found(metafile, FoundVia::Selected)
                })
            }
            Err(warden_instance::Error::ManualFileMismatch { .. }) => {
                tracing::info!("arquivo escolhido para download manual recusado (hash diferente)");
                self.update(pack_id, metafile, |list| list.mark_wrong(metafile, chosen))
            }
            Err(error) => return Err(AppError::from_domain(&error)),
        };
        item.ok_or_else(|| AppError::new(AppErrorCode::ManualDownloadNotWaiting))
    }

    /// Publica a lista e espera até todos os itens estarem no cache (`Ok`) ou o teste ser
    /// cancelado (`CANCELLED`). Enquanto espera, observa a pasta Downloads. No fim, de qualquer
    /// jeito, a lista some e o vigia para.
    pub(crate) async fn wait(
        self: &Arc<Self>,
        pack_id: PackId,
        blocked: Vec<BlockedFile>,
        cache: Arc<DownloadCache>,
        handle: &OperationHandle,
    ) -> Result<(), AppError> {
        let (changed, mut receiver) = watch::channel(0_u64);
        let list = ManualDownloads::new(blocked);
        tracing::info!(count = list.items().len(), "esperando downloads manuais da CurseForge");
        self.lock().insert(
            pack_id,
            Waiting {
                list,
                changed,
                cache: Arc::clone(&cache),
            },
        );
        let _registered = Registered {
            hub: self,
            pack_id,
        };
        // O vigia começa antes de a lista aparecer na tela: nada que chegue depois escapa.
        let _watch = self.watch_downloads(pack_id, cache);
        self.emit(pack_id);
        handle.stage(STAGE_MANUAL, STAGE_MANUAL_LABEL);
        loop {
            let (found, total, complete) = {
                let guard = self.lock();
                let list = guard.get(&pack_id).map(|waiting| &waiting.list);
                let total = list.map_or(0, |list| list.items().len());
                let missing = list.map_or(0, |list| list.waiting().len());
                (total - missing, total, missing == 0)
            };
            handle.progress(Progress::items(found as u64, Some(total as u64)));
            if complete {
                tracing::info!("downloads manuais completos; o teste continua");
                return Ok(());
            }
            tokio::select! {
                biased;
                () = handle.token().cancelled() => {
                    return Err(AppError::cancelled().in_operation(handle.id()));
                }
                changed = receiver.changed() => {
                    if changed.is_err() {
                        return Err(AppError::internal("lista de downloads manuais perdida"));
                    }
                }
            }
        }
    }

    /// O vigia da pasta Downloads (P1), se houver pasta. Falhar ao observar não impede o
    /// "Selecionar arquivo baixado…"; o registro não leva o caminho.
    fn watch_downloads(
        self: &Arc<Self>,
        pack_id: PackId,
        cache: Arc<DownloadCache>,
    ) -> Option<DownloadsWatch> {
        let folder = self.downloads_dir()?;
        let waiting_hub = Arc::clone(self);
        let found_hub = Arc::clone(self);
        let context = WatchContext {
            cache,
            waiting: Arc::new(move || {
                waiting_hub
                    .lock()
                    .get(&pack_id)
                    .map(|waiting| waiting.list.waiting())
                    .unwrap_or_default()
            }),
            found: Arc::new(move |item: &BlockedFile| {
                found_hub.update(pack_id, &item.metafile, |list| {
                    list.mark_found(&item.metafile, FoundVia::DownloadsFolder)
                });
            }),
        };
        match DownloadsWatch::start(&folder, context) {
            Ok(watch) => Some(watch),
            Err(error) => {
                use warden_core::DomainError as _;
                tracing::warn!(code = ?error.code(), "a pasta Downloads não pôde ser observada");
                None
            }
        }
    }

    /// O endereço da página do item (só para o navegador).
    fn page_url(&self, pack_id: PackId, metafile: &str) -> Result<Option<String>, AppError> {
        let (expected, _) = self.expected(pack_id, metafile)?;
        Ok(expected.page_url)
    }
}

/// Tira a lista do pack quando a espera termina, de qualquer jeito.
struct Registered<'a> {
    hub: &'a ManualDownloadsHub,
    pack_id: PackId,
}

impl Drop for Registered<'_> {
    fn drop(&mut self) {
        let removed = self.hub.lock().remove(&self.pack_id).is_some();
        if removed {
            self.hub.emit(self.pack_id);
        }
    }
}

/// Liga a central ao app: o evento `manual-downloads-changed` e a pasta Downloads do sistema
/// (em debug, `WARDEN_DOWNLOADS_DIR` a substitui).
pub(crate) fn connect(hub: &ManualDownloadsHub, app: AppHandle, system_downloads: Option<PathBuf>) {
    let downloads = downloads_override().or(system_downloads);
    hub.connect(
        Arc::new(move |pack_id| {
            if let Err(error) = (ManualDownloadsChanged { pack_id }).emit(&app) {
                tracing::warn!(%error, "evento manual-downloads-changed não enviado");
            }
        }),
        downloads,
    );
}

fn downloads_override() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        std::env::var_os(DOWNLOADS_DIR_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// A lista de downloads manuais do pack (vazia quando o teste não está esperando nenhum).
#[tauri::command]
#[specta::specta]
pub(crate) fn cf_blocked_list(state: State<'_, AppState>, pack_id: PackId) -> Vec<ManualDownloadItem> {
    state.manual_downloads.list(pack_id)
}

/// "Selecionar arquivo baixado…": abre o diálogo de arquivo na pasta Downloads e confere o
/// arquivo escolhido pelo hash. `null` = a pessoa desistiu. Arquivo errado volta com o estado
/// `wrongFile` (nada é copiado). Erros: `app.MANUAL_DOWNLOAD_NOT_WAITING`, falha ao ler o
/// arquivo.
#[tauri::command]
#[specta::specta]
pub(crate) async fn cf_blocked_provide_file(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    pack_id: PackId,
    metafile: String,
) -> Result<Option<ManualDownloadItem>, AppError> {
    let hub = Arc::clone(&state.manual_downloads);
    let (expected, _) = hub.expected(pack_id, &metafile)?;
    let Some(file) = pick_file(&app, &window, &hub, &expected).await? else {
        return Ok(None);
    };
    hub.provide(pack_id, &metafile, file).await.map(Some)
}

/// O diálogo nativo de arquivo (ARCHITECTURE §20: diálogos são do Rust).
async fn pick_file(
    app: &AppHandle,
    window: &WebviewWindow,
    hub: &ManualDownloadsHub,
    expected: &BlockedFile,
) -> Result<Option<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_PICK_FILE_ENV).filter(|value| !value.is_empty()) {
        let picked = std::fs::read_to_string(&file).unwrap_or_default();
        let picked = picked.trim();
        return Ok((!picked.is_empty()).then(|| PathBuf::from(picked)));
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut dialog = app
        .dialog()
        .file()
        .set_title(format!("Escolha o arquivo baixado: {}", expected.file_name))
        .set_parent(window);
    if let Some(extension) = Path::new(&expected.file_name)
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned())
    {
        dialog = dialog
            .add_filter(format!("Arquivo .{extension}"), &[extension.as_str()])
            .add_filter("Todos os arquivos", &["*"]);
    }
    if let Some(folder) = hub.downloads_dir().filter(|folder| folder.is_dir()) {
        dialog = dialog.set_directory(folder);
    }
    dialog.pick_file(move |picked| {
        // Quem esperava pode ter ido embora (janela fechada); não há o que fazer.
        let _ = sender.send(picked.and_then(|path| path.as_path().map(Path::to_path_buf)));
    });
    receiver
        .await
        .map_err(|error| AppError::internal(format!("diálogo de arquivo interrompido: {error}")))
}

/// "Abrir página de download": a página do arquivo no site da CurseForge, no navegador.
/// Erros: `app.MANUAL_DOWNLOAD_NOT_WAITING`, `app.MANUAL_DOWNLOAD_NO_PAGE`.
#[tauri::command]
#[specta::specta]
pub(crate) fn cf_blocked_open_page(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    metafile: String,
) -> Result<(), AppError> {
    let url = state
        .manual_downloads
        .page_url(pack_id, &metafile)?
        .filter(|url| is_curseforge_page(url))
        .ok_or_else(|| AppError::new(AppErrorCode::ManualDownloadNoPage))?;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|_| AppError::internal("abrir a página de download no navegador"))
}

/// Só páginas `https` do site da CurseForge.
fn is_curseforge_page(url: &str) -> bool {
    warden_http::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| host == "curseforge.com" || host.ends_with(".curseforge.com"))
    })
}

#[cfg(test)]
mod tests {
    // Testes da central dos downloads manuais (L-06): a espera do teste, o arquivo escolhido
    // conferido pelo hash, o cancelamento e a pasta Downloads observada.

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use warden_core::CancellationToken;
    use warden_instance::ManualDownloadStatus;
    use warden_packwiz::HashFormat;
    use warden_packwiz::hash::hash_bytes;

    use super::*;
    use crate::operations::{OperationKind, OperationRegistry};

    fn blocked(metafile: &str, file_name: &str, data: &[u8]) -> BlockedFile {
        BlockedFile {
            metafile: metafile.to_owned(),
            name: format!("Mod {file_name}"),
            file_name: file_name.to_owned(),
            project_id: 448_233,
            file_id: 6_000_001,
            page_url: Some(
                "https://www.curseforge.com/minecraft/mc-mods/entityculling/files/6000001".to_owned(),
            ),
            hash_format: "sha1".to_owned(),
            hash: hash_bytes(HashFormat::Sha1, data),
        }
    }

    /// A central com um contador de eventos.
    fn hub(downloads: Option<PathBuf>) -> (Arc<ManualDownloadsHub>, Arc<AtomicUsize>) {
        let hub = Arc::new(ManualDownloadsHub::default());
        let events = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&events);
        hub.connect(
            Arc::new(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
            downloads,
        );
        (hub, events)
    }

    /// Começa a espera numa tarefa; devolve a tarefa e o token do teste.
    fn spawn_wait(
        hub: &Arc<ManualDownloadsHub>,
        pack: PackId,
        items: Vec<BlockedFile>,
        cache: Arc<DownloadCache>,
    ) -> (
        tokio::task::JoinHandle<Result<(), AppError>>,
        CancellationToken,
    ) {
        let registry = OperationRegistry::new(Arc::new(|_| {}));
        let handle = registry.start(OperationKind::new("test.start"), Some(pack), true);
        let token = handle.token().clone();
        let hub = Arc::clone(hub);
        let task = tokio::spawn(async move {
            let result = hub.wait(pack, items, cache, &handle).await;
            handle.finish(result)
        });
        (task, token)
    }

    async fn until(what: impl Fn() -> bool) {
        for _ in 0..300 {
            if what() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(what(), "a condição não aconteceu a tempo");
    }

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ca_t20_01_arquivo_errado_recusado_e_o_certo_libera_o_teste() {
        let dir = temp_dir();
        let cache = Arc::new(DownloadCache::open(&dir.path().join("cache")).unwrap());
        let (hub, events) = hub(None);
        let pack = PackId::new();
        let a = blocked("mods/a.pw.toml", "a-1.0.jar", b"jar a");
        let b = blocked("mods/b.pw.toml", "b-1.0.jar", b"jar b");
        let (task, _) = spawn_wait(&hub, pack, vec![a.clone(), b.clone()], Arc::clone(&cache));
        until(|| hub.list(pack).len() == 2).await;
        assert!(
            hub.list(pack)
                .iter()
                .all(|item| item.status == ManualDownloadStatus::Waiting && item.has_page)
        );

        // Pasta com espaço e acento, como no Windows.
        let folder = dir.path().join("Meus Downloads").join("Mods é ação");
        std::fs::create_dir_all(&folder).unwrap();
        let wrong = folder.join("baixei errado (1).jar");
        std::fs::write(&wrong, b"outro jar").unwrap();
        let item = hub.provide(pack, &a.metafile, wrong).await.unwrap();
        assert_eq!(
            item.status,
            ManualDownloadStatus::WrongFile {
                chosen_file: "baixei errado (1).jar".to_owned()
            }
        );
        assert!(
            cache.find(&a.expected().unwrap()).unwrap().is_none(),
            "nada foi para o cache"
        );
        assert!(!task.is_finished(), "o teste continua esperando");

        let right_a = folder.join("a-1.0.jar");
        std::fs::write(&right_a, b"jar a").unwrap();
        let item = hub.provide(pack, &a.metafile, right_a).await.unwrap();
        assert_eq!(
            item.status,
            ManualDownloadStatus::Found {
                via: FoundVia::Selected
            }
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!task.is_finished(), "falta o b: o teste só continua com todos");

        let right_b = folder.join("b-1.0.jar");
        std::fs::write(&right_b, b"jar b").unwrap();
        hub.provide(pack, &b.metafile, right_b).await.unwrap();
        task.await.unwrap().unwrap();
        assert!(hub.list(pack).is_empty(), "a lista some quando o teste continua");
        assert!(cache.find(&b.expected().unwrap()).unwrap().is_some());
        // Lista publicada, arquivo errado, a, b e o fim.
        assert!(events.load(Ordering::SeqCst) >= 5);

        let error = hub
            .provide(pack, &a.metafile, folder.join("a-1.0.jar"))
            .await
            .unwrap_err();
        assert_eq!(
            error.code,
            crate::ErrorCode::App(AppErrorCode::ManualDownloadNotWaiting)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancelar_o_teste_para_a_espera_e_tira_a_lista() {
        let dir = temp_dir();
        let cache = Arc::new(DownloadCache::open(&dir.path().join("cache")).unwrap());
        let downloads = dir.path().join("Downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        let (hub, events) = hub(Some(downloads.clone()));
        let pack = PackId::new();
        let a = blocked("mods/a.pw.toml", "a.jar", b"jar a");
        let (task, token) = spawn_wait(&hub, pack, vec![a.clone()], Arc::clone(&cache));
        // O evento sai depois de o vigia começar.
        until(|| events.load(Ordering::SeqCst) >= 1).await;
        token.cancel();
        let error = task.await.unwrap().unwrap_err();
        assert_eq!(
            error.code,
            crate::ErrorCode::Core(warden_core::CoreErrorCode::Cancelled)
        );
        assert!(hub.list(pack).is_empty());
        // O vigia parou junto: um arquivo certo que chega depois não entra no cache.
        std::fs::write(downloads.join("a.jar"), b"jar a").unwrap();
        tokio::time::sleep(warden_instance::SETTLE * 3).await;
        assert!(cache.find(&a.expected().unwrap()).unwrap().is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn arquivo_que_chega_na_pasta_downloads_libera_o_teste() {
        let dir = temp_dir();
        let cache = Arc::new(DownloadCache::open(&dir.path().join("cache")).unwrap());
        let downloads = dir.path().join("Downloads do usuário");
        std::fs::create_dir_all(&downloads).unwrap();
        let (hub, events) = hub(Some(downloads.clone()));
        let pack = PackId::new();
        let a = blocked("mods/a.pw.toml", "a.jar", b"jar a");
        let (task, _) = spawn_wait(&hub, pack, vec![a.clone()], Arc::clone(&cache));
        // O evento sai depois de o vigia começar.
        until(|| events.load(Ordering::SeqCst) >= 1).await;
        std::fs::write(downloads.join("nome qualquer.jar"), b"jar a").unwrap();
        tokio::time::timeout(Duration::from_secs(15), task)
            .await
            .expect("o teste continuou sozinho")
            .unwrap()
            .unwrap();
        assert!(cache.find(&a.expected().unwrap()).unwrap().is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pasta_downloads_que_nao_existe_nao_impede_a_espera() {
        let dir = temp_dir();
        let cache = Arc::new(DownloadCache::open(&dir.path().join("cache")).unwrap());
        let (hub, _) = hub(Some(dir.path().join("nao-existe")));
        let pack = PackId::new();
        let a = blocked("mods/a.pw.toml", "a.jar", b"jar a");
        let (task, _) = spawn_wait(&hub, pack, vec![a.clone()], Arc::clone(&cache));
        until(|| !hub.list(pack).is_empty()).await;
        let file = dir.path().join("a.jar");
        std::fs::write(&file, b"jar a").unwrap();
        hub.provide(pack, &a.metafile, file).await.unwrap();
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn sem_espera_nao_ha_lista_nem_pagina() {
        let (hub, _) = hub(None);
        let pack = PackId::new();
        assert!(hub.list(pack).is_empty());
        let error = hub.page_url(pack, "mods/a.pw.toml").unwrap_err();
        assert_eq!(
            error.code,
            crate::ErrorCode::App(AppErrorCode::ManualDownloadNotWaiting)
        );
    }

    #[test]
    fn so_abre_paginas_do_site_da_curseforge() {
        assert!(is_curseforge_page(
            "https://www.curseforge.com/minecraft/mc-mods/x/files/1"
        ));
        assert!(is_curseforge_page("https://curseforge.com/projects/1"));
        assert!(!is_curseforge_page("http://www.curseforge.com/x"));
        assert!(!is_curseforge_page("https://curseforge.com.exemplo.com/x"));
        assert!(!is_curseforge_page("https://exemplo.com/curseforge.com"));
        assert!(!is_curseforge_page("file:///C:/Users/x"));
        assert!(!is_curseforge_page("não é endereço"));
    }

    #[test]
    fn evento_em_camel_case() {
        let pack: PackId = "01J9ZQ0000000000000000000A".parse().unwrap();
        assert_eq!(
            serde_json::to_value(ManualDownloadsChanged { pack_id: pack }).unwrap(),
            serde_json::json!({ "packId": "01J9ZQ0000000000000000000A" })
        );
    }
}
