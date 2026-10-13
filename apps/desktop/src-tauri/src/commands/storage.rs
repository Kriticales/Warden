//! Comandos do domínio `storage` (SPEC T21 → Armazenamento; ARCHITECTURE §4.1 e §17; A-06).
//!
//! - `storage_usage`: o espaço usado pelo cache, pelos arquivos do Minecraft (o armazenamento
//!   compartilhado do motor do launcher, sem os Javas), pelos Javas e pelas instâncias de
//!   teste. A medição percorre as pastas fora da thread do IPC, as quatro em paralelo, e não
//!   segue links simbólicos; um arquivo que some no meio da medição só deixa de contar.
//! - `storage_clear_cache`: "Limpar cache". Apaga só o que está dentro de `cache/` (downloads,
//!   cache do packwiz, cópias temporárias, índices…). Nunca toca em `instances/` (instâncias e
//!   mundos), em `shared/` nem nas pastas dos packs. Recusa com `app.STORAGE_GAME_RUNNING`
//!   enquanto houver um jogo aberto (ou em preparação) e com `app.STORAGE_OPERATION_RUNNING`
//!   enquanto houver outra tarefa em andamento. A conferência é feita no pedido; um teste
//!   começado nos segundos da limpeza (é preciso sair de Configurações para isso) pode perder
//!   um download em andamento, e o erro dele oferece Tentar de novo.
//!
//! O banco de metadados (`cache/metadata.sqlite`, com o Modrinth e o catálogo de versões) fica:
//! o app o mantém aberto do começo ao fim, e apagá-lo por baixo da conexão não libera nada no
//! Linux e falha no Windows. Ele é pequeno e se renova sozinho.

use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;
use warden_core::CoreErrorCode;

use crate::error::{AppError, AppErrorCode};
use crate::events::GameState;
use crate::state::AppState;

/// Arquivos de `cache/` que o app mantém abertos e a limpeza preserva (o banco de metadados e
/// os arquivos auxiliares do SQLite).
const KEPT_IN_CACHE: &[&str] = &[
    "metadata.sqlite",
    "metadata.sqlite-wal",
    "metadata.sqlite-shm",
    "metadata.sqlite-journal",
];

/// Espaço usado pelo Warden no disco, em bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageUsage {
    /// Cache (`cache/`): downloads, cache do packwiz e temporários. "Limpar cache" apaga.
    #[specta(type = specta_typescript::Number)]
    pub(crate) cache: u64,
    /// Arquivos do Minecraft e dos loaders (`shared/`, sem os Javas).
    #[specta(type = specta_typescript::Number)]
    pub(crate) minecraft: u64,
    /// Javas instalados pelo Warden (`shared/runtimes/`).
    #[specta(type = specta_typescript::Number)]
    pub(crate) javas: u64,
    /// Instâncias de teste, com os mundos (`instances/`).
    #[specta(type = specta_typescript::Number)]
    pub(crate) instances: u64,
}

/// Resultado de "Limpar cache".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CacheCleared {
    /// Bytes liberados.
    #[specta(type = specta_typescript::Number)]
    pub(crate) freed: u64,
    /// Itens que não deu para apagar (em uso por outro programa, sem permissão); ficam para a
    /// próxima limpeza.
    pub(crate) skipped: u32,
}

/// O espaço usado pelo cache, pelos arquivos do Minecraft, pelos Javas e pelas instâncias.
#[tauri::command]
#[specta::specta]
pub(crate) async fn storage_usage(state: State<'_, AppState>) -> Result<StorageUsage, AppError> {
    usage_impl(&state).await
}

/// "Limpar cache": apaga o cache sem tocar em instâncias, mundos, Javas, arquivos do Minecraft
/// nem nos packs.
#[tauri::command]
#[specta::specta]
pub(crate) async fn storage_clear_cache(
    state: State<'_, AppState>,
) -> Result<CacheCleared, AppError> {
    clear_impl(&state).await
}

async fn usage_impl(state: &AppState) -> Result<StorageUsage, AppError> {
    let dirs = UsageDirs::of(&state.paths);
    tauri::async_runtime::spawn_blocking(move || measure(&dirs))
        .await
        .map_err(|error| {
            AppError::internal(format!("medição do armazenamento interrompida: {error}"))
        })
}

async fn clear_impl(state: &AppState) -> Result<CacheCleared, AppError> {
    let running = state
        .operations
        .list()
        .into_iter()
        .filter(|operation| !operation.state.is_finished())
        .count();
    ensure_idle(state.tests.current().as_ref(), running)?;
    let cache = state.paths.cache_dir();
    let tmp = state.paths.tmp_dir();
    let protected = protected_paths(state);
    let cleared = tauri::async_runtime::spawn_blocking(move || {
        let cleared = clear_cache(&cache, &protected)?;
        // `cache/tmp/` é uma das pastas básicas que o app cria ao abrir.
        std::fs::create_dir_all(&tmp).map_err(|error| io_error("cache/tmp", &error))?;
        Ok::<_, AppError>(cleared)
    })
    .await
    .map_err(|error| AppError::internal(format!("limpeza do cache interrompida: {error}")))??;
    tracing::info!(
        freed = cleared.freed,
        skipped = cleared.skipped,
        "cache limpo"
    );
    Ok(cleared)
}

/// Recusa a limpeza com um jogo aberto (ou em preparação) ou com tarefas em andamento: o teste
/// e as tarefas usam o cache (downloads, cópias temporárias do pack).
fn ensure_idle(game: Option<&GameState>, running_operations: usize) -> Result<(), AppError> {
    if let Some(game) = game {
        return Err(AppError::new(AppErrorCode::StorageGameRunning)
            .with_param("packName", game.pack_name.clone()));
    }
    if running_operations > 0 {
        return Err(AppError::new(AppErrorCode::StorageOperationRunning)
            .with_param("count", running_operations.to_string()));
    }
    Ok(())
}

/// Pastas que a limpeza nunca apaga, mesmo que estejam (por escolha estranha do usuário)
/// dentro do cache: a de cada pack registrado e a pasta padrão dos packs.
fn protected_paths(state: &AppState) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = state.packs.list().into_iter().map(|pack| pack.path).collect();
    paths.push(
        state
            .settings
            .get()
            .packs_dir
            .unwrap_or_else(|| state.paths.default_packs_dir().to_path_buf()),
    );
    paths
}

/// As pastas medidas.
#[derive(Debug, Clone)]
struct UsageDirs {
    cache: PathBuf,
    shared: PathBuf,
    runtimes: PathBuf,
    instances: PathBuf,
}

impl UsageDirs {
    fn of(paths: &warden_core::AppPaths) -> Self {
        Self {
            cache: paths.cache_dir(),
            shared: paths.shared_dir(),
            runtimes: paths.runtimes_dir(),
            instances: paths.instances_dir(),
        }
    }
}

/// Mede as quatro partes em paralelo.
fn measure(dirs: &UsageDirs) -> StorageUsage {
    std::thread::scope(|scope| {
        let cache = scope.spawn(|| dir_size(&dirs.cache, None));
        let minecraft = scope.spawn(|| dir_size(&dirs.shared, Some(&dirs.runtimes)));
        let javas = scope.spawn(|| dir_size(&dirs.runtimes, None));
        let instances = scope.spawn(|| dir_size(&dirs.instances, None));
        StorageUsage {
            cache: cache.join().unwrap_or(0),
            minecraft: minecraft.join().unwrap_or(0),
            javas: javas.join().unwrap_or(0),
            instances: instances.join().unwrap_or(0),
        }
    })
}

/// Soma o tamanho dos arquivos sob `root` (pasta ausente = 0), sem seguir links simbólicos e
/// pulando `skip`. Um item que não dá para ler (sumiu, sem permissão) não conta.
fn dir_size(root: &Path, skip: Option<&Path>) -> u64 {
    let mut total = 0_u64;
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        if skip.is_some_and(|skip| path == skip) {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                pending.extend(entries.flatten().map(|entry| entry.path()));
            }
        } else if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }
    total
}

/// Apaga cada item de `cache` (menos [`KEPT_IN_CACHE`] e o que contiver uma pasta protegida).
fn clear_cache(cache: &Path, protected: &[PathBuf]) -> Result<CacheCleared, AppError> {
    let entries = match std::fs::read_dir(cache) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(CacheCleared {
                freed: 0,
                skipped: 0,
            });
        }
        Err(error) => return Err(io_error("cache", &error)),
    };
    let mut cleared = CacheCleared {
        freed: 0,
        skipped: 0,
    };
    for entry in entries {
        let entry = entry.map_err(|error| io_error("cache", &error))?;
        let path = entry.path();
        if KEPT_IN_CACHE
            .iter()
            .any(|kept| entry.file_name() == std::ffi::OsStr::new(kept))
        {
            continue;
        }
        if protected.iter().any(|keep| contains(&path, keep)) {
            tracing::warn!(
                item = %entry.file_name().to_string_lossy(),
                "item do cache contém uma pasta de packs e ficou"
            );
            cleared.skipped += 1;
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        // Como na medição: um link simbólico não ocupa o tamanho do destino.
        let size = if metadata.is_dir() {
            dir_size(&path, None)
        } else if metadata.is_file() {
            metadata.len()
        } else {
            0
        };
        // `remove_dir_all` não segue links simbólicos: um link é apagado, não o destino. No
        // Windows, um link para pasta (ou junção) sai com `remove_dir`, que também não entra nele.
        let removed = if metadata.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path).or_else(|error| {
                if metadata.is_symlink() {
                    std::fs::remove_dir(&path)
                } else {
                    Err(error)
                }
            })
        };
        match removed {
            Ok(()) => cleared.freed = cleared.freed.saturating_add(size),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                // Parte de uma pasta pode ter saído antes do erro: conta o que saiu.
                cleared.freed = cleared
                    .freed
                    .saturating_add(size.saturating_sub(dir_size(&path, None)));
                cleared.skipped += 1;
                tracing::warn!(
                    item = %entry.file_name().to_string_lossy(),
                    kind = ?error.kind(),
                    "item do cache não foi apagado"
                );
            }
        }
    }
    Ok(cleared)
}

/// Se `inner` é `outer` ou fica dentro dele, comparando os caminhos como estão e resolvidos
/// (links, `..`, maiúsculas no Windows).
fn contains(outer: &Path, inner: &Path) -> bool {
    if inner.starts_with(outer) {
        return true;
    }
    match (std::fs::canonicalize(outer), std::fs::canonicalize(inner)) {
        (Ok(outer), Ok(inner)) => inner.starts_with(outer),
        _ => false,
    }
}

/// Erro de disco com o caminho relativo à pasta de dados (nunca a pasta do usuário).
fn io_error(relative: &str, error: &io::Error) -> AppError {
    AppError::new(CoreErrorCode::Io)
        .with_param("path", relative)
        .with_detail(format!("{relative}: {:?}", error.kind()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use sha2::{Digest as _, Sha256};
    use warden_core::PackId;
    use warden_instance::{DownloadCache, InstanceDirs, MaterializeOptions, Sources};
    use warden_packwiz::{IndexEntry, PackIndex, PackManifest};
    use warden_project::registry::PackRecord;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::error::ErrorCode;
    use crate::events::GamePhase;
    use crate::operations::OperationKind;
    use crate::state::tests::test_state;

    /// Grava `size` bytes em `path` (criando as pastas).
    fn file(path: &Path, size: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![b'w'; size]).unwrap();
    }

    /// Medição direta, independente da do Warden: `find -type f` com o tamanho de cada arquivo
    /// (sem seguir links), quando o sistema tem o `find` do GNU.
    #[cfg(unix)]
    fn find_size(dir: &Path, prune: Option<&Path>) -> Option<u64> {
        let mut command = std::process::Command::new("find");
        command.arg(dir);
        if let Some(prune) = prune {
            command.args(["-path"]).arg(prune).args(["-prune", "-o"]);
        }
        let output = command.args(["-type", "f", "-printf", "%s\n"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        Some(
            String::from_utf8(output.stdout)
                .ok()?
                .lines()
                .map(|line| line.parse::<u64>().unwrap())
                .sum(),
        )
    }

    #[test]
    fn tamanhos_batem_com_a_medicao_direta_no_disco() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let paths = &state.paths;
        // O estado aberto já criou o banco de metadados no cache.
        let metadata = measure(&UsageDirs::of(paths)).cache;
        file(&paths.downloads_cache_dir().join("ab/abcd"), 1_000);
        file(&paths.packwiz_cache_dir().join("x.json"), 250);
        file(&paths.tmp_dir().join("java/parte.zip"), 3);
        file(&paths.shared_dir().join("versions/1.20.1/1.20.1.jar"), 20_000);
        file(&paths.shared_dir().join("libraries/a/b.jar"), 7_000);
        file(&paths.shared_dir().join("assets/objects/aa/aaaa"), 512);
        file(&paths.runtimes_dir().join("temurin-21/bin/java"), 40_000);
        file(&paths.runtimes_dir().join("temurin-8/lib/rt.jar"), 9_000);
        let pack = PackId::new();
        let game = InstanceDirs::for_pack(paths, pack).game_dir;
        file(&game.join("mods/a.jar"), 4_096);
        file(&game.join("saves/Mundo/level.dat"), 2_048);
        file(&paths.instance_dir(pack).join("state/manifest.json"), 100);
        // Um link para um arquivo grande não conta duas vezes.
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            paths.runtimes_dir().join("temurin-21/bin/java"),
            paths.cache_dir().join("link-para-java"),
        )
        .unwrap();

        let usage = measure(&UsageDirs::of(paths));
        assert_eq!(
            usage,
            StorageUsage {
                cache: metadata + 1_253,
                minecraft: 27_512,
                javas: 49_000,
                instances: 6_244,
            }
        );
        #[cfg(unix)]
        if let (Some(cache), Some(minecraft), Some(javas), Some(instances)) = (
            find_size(&paths.cache_dir(), None),
            find_size(&paths.shared_dir(), Some(&paths.runtimes_dir())),
            find_size(&paths.runtimes_dir(), None),
            find_size(&paths.instances_dir(), None),
        ) {
            assert_eq!(usage.cache, cache);
            assert_eq!(usage.minecraft, minecraft);
            assert_eq!(usage.javas, javas);
            assert_eq!(usage.instances, instances);
        }
    }

    #[test]
    fn sem_nada_baixado_tudo_e_zero() {
        // Pastas de dados que o app nunca abriu (nenhuma das quatro existe ainda).
        let dir = tempfile::tempdir().unwrap();
        let paths = warden_core::AppPaths::from_dev_root(dir.path()).unwrap();
        assert_eq!(
            measure(&UsageDirs::of(&paths)),
            StorageUsage {
                cache: 0,
                minecraft: 0,
                javas: 0,
                instances: 0,
            }
        );
    }

    #[tokio::test]
    async fn limpar_cache_mantem_instancias_mundos_javas_minecraft_e_packs() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let paths = &state.paths;
        // O estado aberto já criou o banco de metadados no cache.
        let metadata = measure(&UsageDirs::of(paths)).cache;
        assert!(metadata > 0 && paths.metadata_db_file().is_file());
        file(&paths.downloads_cache_dir().join("ab/abcd"), 1_000);
        file(&paths.downloads_cache_dir().join("index.sqlite"), 300);
        file(&paths.packwiz_cache_dir().join("x.json"), 250);
        file(&paths.cache_dir().join("staging/01J/pack.toml"), 40);
        file(&paths.tmp_dir().join("player-test-01J/a"), 10);
        let pack = PackId::new();
        let instance = paths.instance_dir(pack);
        let world = instance.join("minecraft/saves/Mundo/level.dat");
        file(&world, 2_048);
        file(&instance.join("minecraft/mods/a.jar"), 4_096);
        file(&instance.join("state/manifest.json"), 100);
        let java = paths.runtimes_dir().join("temurin-21/bin/java");
        file(&java, 500);
        let version = paths.shared_dir().join("versions/1.20.1/1.20.1.jar");
        file(&version, 700);
        let pack_root = dir.path().join("Meus packs/Vale");
        file(&pack_root.join("pack.toml"), 30);
        file(&pack_root.join("mods/a.pw.toml"), 30);
        // Um link no cache para a instância: apagar o link não apaga a instância.
        #[cfg(unix)]
        std::os::unix::fs::symlink(&instance, paths.cache_dir().join("link-instancia")).unwrap();

        let before = measure(&UsageDirs::of(paths));
        let cleared = clear_impl(&state).await.unwrap();
        let after = measure(&UsageDirs::of(paths));

        assert_eq!(cleared.skipped, 0);
        assert_eq!(cleared.freed, 1_000 + 300 + 250 + 40 + 10);
        assert_eq!(
            after.cache, metadata,
            "só o banco de metadados, aberto pelo app, fica"
        );
        assert_eq!(before.cache - after.cache, cleared.freed);
        assert_eq!(after.instances, before.instances);
        assert_eq!(after.javas, before.javas);
        assert_eq!(after.minecraft, before.minecraft);
        assert_eq!(std::fs::read(&world).unwrap().len(), 2_048, "o mundo continua");
        assert!(instance.join("minecraft/mods/a.jar").is_file());
        assert!(instance.join("state/manifest.json").is_file());
        assert!(java.is_file() && version.is_file());
        assert!(pack_root.join("pack.toml").is_file() && pack_root.join("mods/a.pw.toml").is_file());
        assert!(!paths.downloads_cache_dir().exists());
        assert!(paths.tmp_dir().is_dir(), "a pasta de temporários volta a existir");
        assert!(paths.metadata_db_file().is_file());
    }

    #[tokio::test]
    async fn pasta_de_packs_dentro_do_cache_nunca_e_apagada() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        // Pack registrado e pasta padrão dos packs dentro do cache (escolha estranha, mas
        // possível): os dois ficam.
        let pack_root = state.paths.cache_dir().join("meus-packs/Vale");
        file(&pack_root.join("pack.toml"), 30);
        state
            .packs
            .insert(PackRecord {
                id: PackId::new(),
                name: "Vale".into(),
                path: pack_root.clone(),
                last_test: None,
                extra: BTreeMap::new(),
            })
            .unwrap();
        let packs_dir = state.paths.cache_dir().join("padrao");
        file(&packs_dir.join("Outro/pack.toml"), 30);
        state.settings.set_packs_dir(Some(packs_dir.clone())).unwrap();
        file(&state.paths.downloads_cache_dir().join("ab/abcd"), 1_000);

        let cleared = clear_impl(&state).await.unwrap();
        assert_eq!(cleared.skipped, 2);
        assert_eq!(cleared.freed, 1_000);
        assert!(pack_root.join("pack.toml").is_file());
        assert!(packs_dir.join("Outro/pack.toml").is_file());
    }

    #[tokio::test]
    async fn recusa_com_tarefa_em_andamento_e_nao_apaga_nada() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let download = state.paths.downloads_cache_dir().join("ab/abcd");
        file(&download, 1_000);
        let handle = state
            .operations
            .start(OperationKind::new("teste.armazenamento"), None, true);
        let error = clear_impl(&state).await.unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::App(AppErrorCode::StorageOperationRunning)
        );
        assert_eq!(error.params["count"], "1");
        assert!(download.is_file());
        // Terminada a tarefa, a limpeza passa.
        handle.finish(Ok::<_, AppError>(())).unwrap();
        clear_impl(&state).await.unwrap();
        assert!(!download.exists());
    }

    #[test]
    fn recusa_com_jogo_aberto_com_o_nome_do_pack() {
        let game = GameState {
            pack_id: PackId::new(),
            pack_name: "Vale Sereno".into(),
            state: GamePhase::Running,
            session_id: None,
            operation_id: None,
            profile: None,
            started_at_ms: 0,
        };
        let error = ensure_idle(Some(&game), 0).unwrap_err();
        assert_eq!(error.code, ErrorCode::App(AppErrorCode::StorageGameRunning));
        assert_eq!(error.params["packName"], "Vale Sereno");
        // O jogo vem antes das tarefas (o teste aberto também é uma tarefa).
        let error = ensure_idle(Some(&game), 1).unwrap_err();
        assert_eq!(error.code, ErrorCode::App(AppErrorCode::StorageGameRunning));
        assert!(ensure_idle(None, 0).is_ok());
    }

    #[test]
    fn erro_de_disco_nao_mostra_a_pasta_do_usuario() {
        let error = io_error("cache", &io::Error::from(io::ErrorKind::PermissionDenied));
        assert_eq!(error.params["path"], "cache");
        let detail = error.detail.unwrap();
        assert!(detail.starts_with("cache:"), "{detail}");
    }

    fn sha256(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    /// Pack vanilla com dois mods baixados de `base`.
    fn write_pack(root: &Path, base: &str, jars: &[(&str, &[u8])]) {
        std::fs::create_dir_all(root.join("mods")).unwrap();
        let mut index = PackIndex::default();
        for (name, jar) in jars {
            let meta = format!(
                "name = \"{name}\"\nfilename = \"{name}.jar\"\nside = \"both\"\n\n[download]\n\
                 url = \"{base}/{name}.jar\"\nhash-format = \"sha256\"\nhash = \"{}\"\n",
                sha256(jar)
            );
            let path = format!("mods/{name}.pw.toml");
            std::fs::write(root.join(&path), &meta).unwrap();
            index.files.push(IndexEntry {
                file: path,
                hash: sha256(meta.as_bytes()),
                hash_format: String::new(),
                alias: String::new(),
                metafile: true,
                preserve: false,
            });
        }
        let index_text = index.to_toml_string();
        std::fs::write(root.join("index.toml"), &index_text).unwrap();
        let mut manifest = PackManifest::new("Vale", "1.20.1");
        manifest.version = "1.0.0".into();
        manifest.index.hash = sha256(index_text.as_bytes());
        std::fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
    }

    async fn copy_pack(state: &AppState, root: &Path, dirs: &InstanceDirs) -> warden_instance::Report {
        let sources = Sources {
            http: warden_http::HttpClient::new(warden_http::HttpConfig::for_version("0.0.0"))
                .unwrap(),
            curseforge: None,
            cache: Arc::new(DownloadCache::open(&state.paths.downloads_cache_dir()).unwrap()),
        };
        match warden_instance::materialize(
            &sources,
            root,
            dirs,
            &MaterializeOptions::default(),
            &warden_core::NoProgress,
            &warden_core::CancellationToken::new(),
        )
        .await
        .unwrap()
        {
            warden_instance::Outcome::Done(report) => report,
            warden_instance::Outcome::NeedsReview(files) => panic!("revisão pedida: {files:?}"),
        }
    }

    /// Critério da A-06: depois de limpar o cache, o próximo "Copiar o pack para o teste" não
    /// baixa o que já está na instância e baixa de novo só o que falta nela.
    #[tokio::test]
    async fn depois_de_limpar_o_proximo_teste_baixa_so_o_necessario() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let server = MockServer::start().await;
        let jar_a: &[u8] = b"jar do mod a";
        let jar_b: &[u8] = b"jar do mod b";
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/a.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(jar_a))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path("/b.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(jar_b))
            .mount(&server)
            .await;
        let root = dir.path().join("pack");
        write_pack(&root, &server.uri(), &[("a", jar_a), ("b", jar_b)]);
        let dirs = InstanceDirs::for_pack(&state.paths, PackId::new());
        let world = dirs.game_dir.join("saves/Mundo/level.dat");

        let first = copy_pack(&state, &root, &dirs).await;
        assert_eq!(first.downloaded, 2);
        file(&world, 128);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);

        let cleared = clear_impl(&state).await.unwrap();
        assert!(cleared.freed >= (jar_a.len() + jar_b.len()) as u64);

        // Tudo já está na instância: nada é baixado.
        let second = copy_pack(&state, &root, &dirs).await;
        assert_eq!(second.downloaded, 0);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);

        // Falta um jar na instância (e no cache, que foi limpo): só ele é baixado.
        std::fs::remove_file(dirs.game_dir.join("mods/b.jar")).unwrap();
        let third = copy_pack(&state, &root, &dirs).await;
        assert_eq!(third.downloaded, 1);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[2].url.path(), "/b.jar");
        assert_eq!(std::fs::read(dirs.game_dir.join("mods/a.jar")).unwrap(), jar_a);
        assert_eq!(std::fs::read(dirs.game_dir.join("mods/b.jar")).unwrap(), jar_b);
        assert_eq!(std::fs::read(&world).unwrap().len(), 128, "o mundo continua");
    }
}
