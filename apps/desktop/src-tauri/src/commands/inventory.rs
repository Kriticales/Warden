//! Comandos do domínio `inventory` (ARCHITECTURE §4.1; SPEC T06 e T07; P1-08).
//!
//! - `inventory_list`: a lista de Mods (mods, resource packs e shaders), guiada pelo índice e
//!   tolerante a erros, com a versão legível do cache do Modrinth.
//! - `item_details`: o painel de detalhes (Modrinth do cache; CurseForge ao vivo, nada gravado).
//! - `items_set_side`, `items_remove_plan`, `items_remove`: lado em lote e remover.
//! - `inventory_include_outside`: "Incluir no pack" dos arquivos que estão na pasta mas fora do
//!   índice (um `packwiz refresh`).
//! - `item_open_file`: "Abrir no editor de texto" de um arquivo inválido.
//!
//! Finos: validam a entrada, pegam a trava do pack e chamam a `warden-project`. Toda escrita
//! passa pela trava de escrita do pack (a segunda espera na fila e aparece em Tarefas) e avisa
//! a interface com `pack-changed`.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::path::PathBuf;

use secrecy::SecretString;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt as _;
use tauri_specta::Event as _;
use warden_core::{AppPaths, PackId, resolve_inside};
use warden_curseforge::CurseforgeClient;
use warden_http::{HttpClient, HttpConfig};
use warden_modrinth::{MetadataCache, ModrinthClient};
use warden_packwiz_cli::Packwiz;
use warden_project::ProjectErrorCode;
use warden_project::details::ItemDetails;
use warden_project::inventory::Inventory;
use warden_project::remove::RemovalPlan;
use warden_project::side::SideChoice;
use warden_project::transaction::PackTransaction;
use warden_secrets::SecretKind;

use crate::error::AppError;
use crate::events::{PackArea, PackChanged};
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Alterar lado" (um ou vários itens).
pub(crate) const SET_SIDE: OperationKind = OperationKind::new("inventory.setSide");
/// "Remover" (um ou vários itens).
pub(crate) const REMOVE: OperationKind = OperationKind::new("inventory.remove");
/// "Incluir no pack" (arquivos fora do índice).
pub(crate) const INCLUDE: OperationKind = OperationKind::new("inventory.include");

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

fn http() -> Result<HttpClient, AppError> {
    HttpClient::new(HttpConfig::for_version(env!("CARGO_PKG_VERSION")))
        .map_err(|error| AppError::from_domain(&error))
}

/// Abre o cliente do Modrinth do app, com o cache em `cache/metadata.sqlite`. Se o cache não
/// abrir (disco), o cliente funciona com um cache em memória e o problema fica registrado.
pub(crate) fn open_modrinth(paths: &AppPaths) -> Result<ModrinthClient, AppError> {
    let cache = match MetadataCache::open(&paths.metadata_db_file()) {
        Ok(cache) => cache,
        Err(error) => {
            tracing::warn!(%error, "cache do Modrinth indisponível; usando cache em memória");
            MetadataCache::in_memory(None).map_err(|error| AppError::from_domain(&error))?
        }
    };
    ModrinthClient::new(http()?, Some(cache)).map_err(|error| AppError::from_domain(&error))
}

/// Cliente da CurseForge com a chave do usuário (`None`: sem chave). Criado a cada uso, sem
/// cache em disco (termos da CurseForge, SPEC T07).
fn curseforge(state: &AppState) -> Result<CurseforgeClient, AppError> {
    let key: Option<SecretString> = state
        .secrets
        .get(SecretKind::Curseforge)
        .map_err(|error| AppError::from_domain(&error))?;
    CurseforgeClient::new(http()?, key).map_err(|error| AppError::from_domain(&error))
}

pub(crate) fn packwiz(state: &AppState) -> Result<Packwiz, AppError> {
    let binary = Packwiz::locate_binary().map_err(|e| AppError::from_domain(&e))?;
    Ok(Packwiz::new(
        binary,
        state.paths.packwiz_cache_dir(),
        state.paths.packwiz_config_file(),
    ))
}

/// Pasta de um pack registrado que ainda existe.
pub(crate) fn pack_root(state: &AppState, pack_id: PackId) -> Result<PathBuf, AppError> {
    let root = state.packs.get(pack_id).map_err(domain)?.path;
    if !root.is_dir() {
        return Err(AppError::new(ProjectErrorCode::FolderMissing));
    }
    Ok(root)
}

/// Avisa a interface de que uma parte do pack mudou (invalida as queries dessa área).
pub(crate) fn notify_changed(app: &AppHandle, pack_id: PackId, areas: Vec<PackArea>) {
    if let Err(error) = (PackChanged { pack_id, areas }).emit(app) {
        tracing::warn!(%error, "não foi possível avisar a interface da mudança do pack");
    }
}

fn ensure_paths(paths: &[String]) -> Result<(), AppError> {
    if paths.is_empty() {
        return Err(AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "paths"));
    }
    Ok(())
}

/// A lista de Mods: todos os itens do índice (mods, resource packs, shaders), os arquivos
/// inválidos e os que estão fora do índice. Sempre lida do disco.
#[tauri::command]
#[specta::specta]
pub(crate) async fn inventory_list(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Inventory, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    warden_project::inventory::inventory(&root, Some(&state.modrinth))
        .await
        .map_err(domain)
}

/// Detalhes de um item (painel lateral). `path`: o caminho do item na lista.
#[tauri::command]
#[specta::specta]
pub(crate) async fn item_details(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<ItemDetails, AppError> {
    let curseforge = curseforge(&state)?;
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    warden_project::details::item_details(
        &root,
        &path,
        Some(&state.modrinth),
        Some(&curseforge),
        None,
    )
    .await
    .map_err(domain)
}

/// Muda o lado de um ou vários itens: uma escrita só, com um único `packwiz refresh`.
/// Devolve os arquivos alterados.
#[tauri::command]
#[specta::specta]
pub(crate) async fn items_set_side(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    paths: Vec<String>,
    side: SideChoice,
) -> Result<Vec<String>, AppError> {
    let cli = packwiz(&state)?;
    let changed = set_side_impl(&state, &cli, pack_id, &paths, side).await?;
    if !changed.is_empty() {
        notify_changed(&app, pack_id, vec![PackArea::Inventory]);
    }
    Ok(changed)
}

pub(crate) async fn set_side_impl(
    state: &AppState,
    cli: &Packwiz,
    pack_id: PackId,
    paths: &[String],
    side: SideChoice,
) -> Result<Vec<String>, AppError> {
    ensure_paths(paths)?;
    let handle = state.operations.start(SET_SIDE, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(state, pack_id)?;
        warden_project::side::set_sides(&root, paths, side, cli, handle.token())
            .await
            .map_err(domain)
    }
    .await;
    handle.finish(result)
}

/// O que "Remover" vai apagar e quem depende dos itens (diálogo de confirmação).
#[tauri::command]
#[specta::specta]
pub(crate) async fn items_remove_plan(
    state: State<'_, AppState>,
    pack_id: PackId,
    paths: Vec<String>,
) -> Result<RemovalPlan, AppError> {
    ensure_paths(&paths)?;
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let inventory = warden_project::inventory::inventory(&root, Some(&state.modrinth))
        .await
        .map_err(domain)?;
    let dependencies =
        warden_project::remove::direct_dependencies(&inventory, Some(&state.modrinth)).await;
    warden_project::remove::removal_plan(&inventory, &paths, &dependencies).map_err(domain)
}

/// Remove os itens (apaga o `.pw.toml` ou o arquivo local e atualiza o índice). Devolve os
/// arquivos apagados.
#[tauri::command]
#[specta::specta]
pub(crate) async fn items_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    paths: Vec<String>,
) -> Result<Vec<String>, AppError> {
    let cli = packwiz(&state)?;
    let removed = remove_impl(&state, &cli, pack_id, &paths).await?;
    notify_changed(&app, pack_id, vec![PackArea::Inventory]);
    Ok(removed)
}

pub(crate) async fn remove_impl(
    state: &AppState,
    cli: &Packwiz,
    pack_id: PackId,
    paths: &[String],
) -> Result<Vec<String>, AppError> {
    ensure_paths(paths)?;
    let handle = state.operations.start(REMOVE, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(state, pack_id)?;
        warden_project::remove::remove(&root, paths, cli, handle.token())
            .await
            .map_err(domain)
    }
    .await;
    handle.finish(result)
}

/// "Incluir no pack": põe no índice os arquivos que estão nas pastas do pack mas fora dele
/// (um `packwiz refresh`, que respeita o `.packwizignore`).
#[tauri::command]
#[specta::specta]
pub(crate) async fn inventory_include_outside(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<(), AppError> {
    let cli = packwiz(&state)?;
    let handle = state.operations.start(INCLUDE, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(&state, pack_id)?;
        PackTransaction::new(root)
            .commit(&cli, handle.token())
            .await
            .map(|_| ())
            .map_err(domain)
    }
    .await;
    let result = handle.finish(result);
    notify_changed(&app, pack_id, vec![PackArea::Inventory]);
    result
}

/// "Abrir no editor de texto": abre um arquivo do pack no programa padrão do sistema.
#[tauri::command]
#[specta::specta]
pub(crate) fn item_open_file(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<(), AppError> {
    let file = file_inside(&state, pack_id, &path)?;
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|error| AppError::internal(format!("abrir o arquivo do pack: {error}")))
}

fn file_inside(state: &AppState, pack_id: PackId, path: &str) -> Result<PathBuf, AppError> {
    let root = pack_root(state, pack_id)?;
    let file = resolve_inside(&root, path)
        .map_err(|_| AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "path"))?;
    if !file.is_file() {
        return Err(AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "path"));
    }
    Ok(file)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    use warden_project::registry::PackRecord;

    use super::*;
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;

    /// O sidecar do packwiz compilado (`cargo xtask build-packwiz`), ou `None`. Com
    /// `WARDEN_REQUIRE_EXTERNALS=1`, a falta dele é erro.
    pub(crate) fn packwiz_binary() -> Option<PathBuf> {
        let name = if cfg!(windows) {
            "packwiz-x86_64-pc-windows-msvc.exe"
        } else {
            "packwiz-x86_64-unknown-linux-gnu"
        };
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(name);
        if path.is_file() {
            return Some(path);
        }
        assert_ne!(
            std::env::var_os("WARDEN_REQUIRE_EXTERNALS").as_deref(),
            Some(std::ffi::OsStr::new("1")),
            "sidecar packwiz ausente"
        );
        None
    }

    /// O packwiz real com as pastas do estado de teste.
    pub(crate) fn test_packwiz(state: &AppState, binary: PathBuf) -> Packwiz {
        Packwiz::new(
            binary,
            state.paths.packwiz_cache_dir(),
            state.paths.packwiz_config_file(),
        )
    }

    /// Um pack com `count` mods do Modrinth (metafiles e índice como o packwiz grava),
    /// registrado no estado. Devolve o ID e a pasta.
    pub(crate) async fn registered_pack(
        state: &AppState,
        dir: &Path,
        cli: &Packwiz,
        count: usize,
    ) -> (PackId, PathBuf) {
        let root = dir.join("pack");
        fs::create_dir_all(root.join("mods")).unwrap();
        let manifest = warden_packwiz::PackManifest::new("Vale", "1.20.1");
        fs::write(root.join("pack.toml"), manifest.to_toml_string()).unwrap();
        fs::write(
            root.join("index.toml"),
            warden_packwiz::PackIndex::default().to_toml_string(),
        )
        .unwrap();
        for n in 0..count {
            let file = warden_packwiz::ModrinthFile {
                title: format!("Mod {n}"),
                project_id: format!("PROJ{n:04}"),
                version_id: format!("VERS{n:04}"),
                filename: format!("mod-{n}-1.0.0.jar"),
                url: format!("https://cdn.modrinth.com/data/PROJ{n:04}/mod-{n}.jar"),
                hash_format: warden_packwiz::HashFormat::Sha512,
                hash: "ab".repeat(64),
            };
            let metafile = warden_packwiz::Metafile::modrinth(&file, warden_packwiz::Side::Both);
            fs::write(
                root.join(format!("mods/mod-{n}.pw.toml")),
                metafile.to_toml_string(),
            )
            .unwrap();
        }
        PackTransaction::new(root.clone())
            .commit(cli, &warden_core::CancellationToken::new())
            .await
            .unwrap();
        let record = PackRecord {
            id: PackId::new(),
            name: "Vale".into(),
            path: root.clone(),
            last_test: None,
            extra: BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        (id, root)
    }

    #[test]
    fn pack_sem_pasta_e_caminho_vazio_explicam_o_problema() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let record = PackRecord {
            id: PackId::new(),
            name: "Sumiu".into(),
            path: dir.path().join("sumiu"),
            last_test: None,
            extra: BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        let missing = pack_root(&state, id).unwrap_err();
        assert_eq!(
            missing.code,
            ErrorCode::Project(ProjectErrorCode::FolderMissing)
        );
        let unknown = pack_root(&state, PackId::new()).unwrap_err();
        assert_eq!(
            unknown.code,
            ErrorCode::Project(ProjectErrorCode::PackNotFound)
        );
        let empty = ensure_paths(&[]).unwrap_err();
        assert_eq!(empty.params["field"], "paths");
    }

    #[test]
    fn abrir_arquivo_recusa_caminho_fora_do_pack() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let root = dir.path().join("pack");
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::write(root.join("mods/a.pw.toml"), "x").unwrap();
        fs::write(dir.path().join("fora.txt"), "x").unwrap();
        let record = PackRecord {
            id: PackId::new(),
            name: "Vale".into(),
            path: root.clone(),
            last_test: None,
            extra: BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        assert_eq!(
            file_inside(&state, id, "mods/a.pw.toml").unwrap(),
            root.join("mods/a.pw.toml")
        );
        for bad in ["../fora.txt", "mods/nao-existe.pw.toml", "mods"] {
            let error = file_inside(&state, id, bad).unwrap_err();
            assert_eq!(error.params["field"], "path", "{bad}");
        }
    }

    /// CA-T05-02: duas escritas disparadas em sequência rápida no mesmo pack rodam uma depois
    /// da outra (a segunda espera a trava) e o `index.toml` termina válido e coerente.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn escritas_seguidas_no_mesmo_pack_rodam_em_ordem_sem_corromper_o_indice() {
        let Some(binary) = packwiz_binary() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(test_state(dir.path()));
        let cli = Arc::new(test_packwiz(&state, binary));
        let (id, root) = registered_pack(&state, dir.path(), &cli, 6).await;
        // "Alterar lado" dos cinco primeiros e "Remover" do sexto, disparados juntos.
        let all: Vec<String> = (0..5).map(|n| format!("mods/mod-{n}.pw.toml")).collect();

        let (side_state, side_cli) = (Arc::clone(&state), Arc::clone(&cli));
        let side_paths = all.clone();
        let side = tokio::spawn(async move {
            set_side_impl(&side_state, &side_cli, id, &side_paths, SideChoice::Client).await
        });
        let (remove_state, remove_cli) = (Arc::clone(&state), Arc::clone(&cli));
        let remove = tokio::spawn(async move {
            let paths = ["mods/mod-5.pw.toml".to_owned()];
            remove_impl(&remove_state, &remove_cli, id, &paths).await
        });
        let (side, remove) = (side.await.unwrap(), remove.await.unwrap());
        assert!(remove.unwrap().contains(&"mods/mod-5.pw.toml".to_owned()));
        // Os dois terminaram, um depois do outro (a trava de escrita pôs o segundo na fila).
        assert_eq!(
            side.unwrap()
                .iter()
                .filter(|p| p.ends_with(".pw.toml"))
                .count(),
            5
        );
        let read = warden_packwiz::read_pack(&root).unwrap();
        let index = read.index.as_ref().unwrap();
        assert_eq!(index.value.metafiles().len(), 5);
        assert!(!root.join("mods/mod-5.pw.toml").exists());
        assert!(read.errors().next().is_none());
        for (_, metafile) in read.valid_metafiles() {
            assert_eq!(metafile.side, warden_packwiz::Side::Client);
        }
        // O índice confere com o que o packwiz geraria: um refresh não muda nada.
        let before = fs::read(root.join("index.toml")).unwrap();
        PackTransaction::new(root.clone())
            .commit(&cli, &warden_core::CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(fs::read(root.join("index.toml")).unwrap(), before);
        // As duas escritas aparecem em Tarefas como concluídas.
        let kinds: Vec<String> = state
            .operations
            .list()
            .into_iter()
            .map(|op| op.kind.as_str().to_owned())
            .collect();
        assert!(kinds.contains(&"inventory.setSide".to_owned()));
        assert!(kinds.contains(&"inventory.remove".to_owned()));
    }
}
