//! Testes da central dos downloads manuais (L-06): a espera do teste, o arquivo escolhido
//! conferido pelo hash, o cancelamento e a pasta Downloads observada.

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
    let (hub, _) = hub(Some(downloads.clone()));
    let pack = PackId::new();
    let a = blocked("mods/a.pw.toml", "a.jar", b"jar a");
    let (task, token) = spawn_wait(&hub, pack, vec![a.clone()], Arc::clone(&cache));
    until(|| !hub.list(pack).is_empty()).await;
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
    let (hub, _) = hub(Some(downloads.clone()));
    let pack = PackId::new();
    let a = blocked("mods/a.pw.toml", "a.jar", b"jar a");
    let (task, _) = spawn_wait(&hub, pack, vec![a.clone()], Arc::clone(&cache));
    until(|| !hub.list(pack).is_empty()).await;
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
