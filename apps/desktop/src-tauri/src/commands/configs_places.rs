//! Comandos da seção Configs com as três origens juntas (SPEC T12; ADR-0067): o pack, a
//! instância de teste e as configs pessoais.
//!
//! - `config_overview`: a lista juntada, com o grupo de cada arquivo (No pack, Só no teste, Só
//!   neste computador) e quantas chaves o teste tem diferentes do pack.
//! - `config_compare`: as diferenças de um arquivo, chave a chave, e os dois textos.
//! - `config_bring`: "Trazer para o pack" (o arquivo do teste inteiro, ou só chaves), pela
//!   `PackTransaction`.
//! - `config_revert`: "Voltar ao do pack" na instância de teste.
//! - `config_mark_personal` e `config_unmark_personal`: "Marcar como pessoal" e "Deixar de ser
//!   pessoal". Marcar uma config que está no pack tira do pack (a interface confirma antes).
//! - `config_keep_personal_out`: a migração do ADR-0067, quando o pack ainda leva o
//!   `options.txt` (ou outra pessoal): guarda a cópia neste computador e tira do pack.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use tauri::{AppHandle, State};
use warden_core::{PackId, atomic_write};
use warden_instance::PersonalConfigs;
use warden_project::config_places::{
    ConfigComparison, ConfigOverview, bring_bytes, compare_places, overview, revert_bytes,
};
use warden_project::configs::{ConfigSaved, hash_bytes};
use warden_project::transaction::PackTransaction;

use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// Trazer uma config (ou chaves) do teste para o pack.
pub(crate) const CONFIG_BRING: OperationKind = OperationKind::new("configs.bring");

/// Tirar configs pessoais do pack (marcar como pessoal, migração do `options.txt`).
pub(crate) const CONFIG_PERSONAL: OperationKind = OperationKind::new("configs.personal");

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

fn instance(error: warden_instance::Error) -> AppError {
    AppError::from_domain(&error)
}

fn joined<T>(result: Result<T, tauri::Error>) -> Result<T, AppError> {
    result.map_err(|error| AppError::internal(error.to_string()))
}

/// A pasta do jogo da instância de teste.
fn game_dir(state: &AppState, pack_id: PackId) -> std::path::PathBuf {
    state.paths.instance_dir(pack_id).join("minecraft")
}

/// As configs pessoais do pack.
pub(crate) fn personal_of(state: &AppState, pack_id: PackId) -> Result<PersonalConfigs, AppError> {
    PersonalConfigs::load(&state.paths.personal_dir(pack_id)).map_err(instance)
}

/// Os caminhos pessoais do pack (vazio se não der para ler: o teste segue como antes).
pub(crate) fn personal_paths(state: &AppState, pack_id: PackId) -> BTreeSet<String> {
    match personal_of(state, pack_id) {
        Ok(personal) => personal.paths(),
        Err(error) => {
            tracing::warn!(code = %error.code, "configs pessoais ilegíveis; nenhuma vale");
            BTreeSet::new()
        }
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, AppError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::internal(format!("ler a config: {error}"))),
    }
}

/// A lista juntada da seção Configs.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_overview(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<ConfigOverview, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let game = game_dir(&state, pack_id);
    let personal = personal_of(&state, pack_id)?.paths();
    joined(tauri::async_runtime::spawn_blocking(move || overview(&root, &game, &personal)).await)?
        .map_err(domain)
}

/// As diferenças de `path` entre o teste e o pack.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_compare(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<ConfigComparison, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let game = game_dir(&state, pack_id);
    joined(tauri::async_runtime::spawn_blocking(move || compare_places(&root, &game, &path)).await)?
        .map_err(domain)
}

/// "Trazer para o pack": grava no pack o arquivo do teste inteiro (`keys` = `None`) ou só o
/// valor dessas chaves.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_bring(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
    keys: Option<Vec<String>>,
) -> Result<ConfigSaved, AppError> {
    let cli = packwiz(&state)?;
    let handle = state.operations.start(CONFIG_BRING, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(&state, pack_id)?;
        let game = game_dir(&state, pack_id);
        if personal_of(&state, pack_id)?.contains(&path) {
            return Err(AppError::internal(format!(
                "{path} é uma config pessoal e não vai para o pack"
            )));
        }
        let (normalized, bytes) = bring_bytes(&root, &game, &path, keys.as_deref()).map_err(domain)?;
        let current = read_optional(&root.join(&normalized))?;
        let saved = ConfigSaved {
            hash: hash_bytes(&bytes),
            size: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            changed: current.as_deref() != Some(bytes.as_slice()),
        };
        if saved.changed {
            let mut transaction = PackTransaction::new(root);
            transaction.write(&normalized, bytes).map_err(domain)?;
            transaction
                .commit(&cli, handle.token())
                .await
                .map_err(domain)?;
        }
        Ok(saved)
    }
    .await;
    let result = handle.finish(result);
    if matches!(&result, Ok(saved) if saved.changed) {
        notify_changed(&app, pack_id, vec![PackArea::Configs]);
    }
    result
}

/// "Voltar ao do pack": grava na instância de teste o arquivo do pack inteiro (`keys` =
/// `None`) ou só o valor dessas chaves.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_revert(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
    keys: Option<Vec<String>>,
) -> Result<ConfigSaved, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let game = game_dir(&state, pack_id);
    joined(
        tauri::async_runtime::spawn_blocking(move || {
            let (normalized, bytes) =
                revert_bytes(&root, &game, &path, keys.as_deref()).map_err(domain)?;
            let target = game.join(&normalized);
            let current = read_optional(&target)?;
            let changed = current.as_deref() != Some(bytes.as_slice());
            if changed {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| AppError::internal(error.to_string()))?;
                }
                atomic_write(&target, &bytes).map_err(|error| AppError::from_domain(&error))?;
            }
            Ok(ConfigSaved {
                hash: hash_bytes(&bytes),
                size: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
                changed,
            })
        })
        .await,
    )?
}

/// Guarda a cópia pessoal de cada caminho (a da instância; sem ela, a do pack, se ainda não
/// há cópia guardada) e tira do pack os que estão nele, numa transação só.
async fn take_out_of_pack(
    state: &AppState,
    pack_id: PackId,
    personal: &PersonalConfigs,
    paths: &[String],
    cli: &warden_packwiz_cli::Packwiz,
    handle: &crate::operations::OperationHandle,
) -> Result<u32, AppError> {
    let root = pack_root(state, pack_id)?;
    let game = game_dir(state, pack_id);
    let mut transaction = PackTransaction::new(root.clone());
    let mut removed = 0_u32;
    for path in paths {
        let in_test = read_optional(&game.join(path))?;
        let in_pack = read_optional(&root.join(path))?;
        let copy = match (in_test, personal.stored(path).map_err(instance)?) {
            (Some(bytes), _) => Some(bytes),
            (None, Some(_)) => None,
            (None, None) => in_pack.clone(),
        };
        if let Some(bytes) = copy {
            personal.store(path, &bytes).map_err(instance)?;
        }
        if in_pack.is_some() {
            transaction.delete(path).map_err(domain)?;
            removed += 1;
        }
    }
    if removed > 0 {
        transaction
            .commit(cli, handle.token())
            .await
            .map_err(domain)?;
    }
    Ok(removed)
}

/// "Marcar como pessoal": `path` passa a ficar só neste computador. Se estava no pack, sai
/// dele (com a cópia guardada antes). Devolve quantos arquivos saíram do pack (0 ou 1).
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_mark_personal(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<u32, AppError> {
    let handle = state.operations.start(CONFIG_PERSONAL, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let mut personal = personal_of(&state, pack_id)?;
        personal.mark(&path).map_err(instance)?;
        let normalized: Vec<String> = personal
            .paths()
            .into_iter()
            .filter(|known| known == &path.replace('\\', "/"))
            .collect();
        let cli = packwiz(&state)?;
        take_out_of_pack(&state, pack_id, &personal, &normalized, &cli, &handle).await
    }
    .await;
    let result = handle.finish(result);
    if matches!(result, Ok(removed) if removed > 0) {
        notify_changed(&app, pack_id, vec![PackArea::Configs]);
    }
    result
}

/// "Deixar de ser pessoal": `path` volta a ser uma config comum (fica na instância de teste;
/// para ir aos jogadores, "Trazer para o pack"). A cópia guardada é apagada.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_unmark_personal(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<bool, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let mut personal = personal_of(&state, pack_id)?;
    personal.unmark(&path).map_err(instance)
}

/// Migração do ADR-0067: as configs pessoais que o pack ainda leva (`options.txt`) saem dele,
/// com a cópia guardada neste computador. Só depois de a pessoa ver o aviso e confirmar.
/// Devolve quantos arquivos saíram do pack.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_keep_personal_out(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    paths: Vec<String>,
) -> Result<u32, AppError> {
    let handle = state.operations.start(CONFIG_PERSONAL, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let personal = personal_of(&state, pack_id)?;
        let wanted: Vec<String> = paths
            .into_iter()
            .filter(|path| personal.contains(path))
            .collect();
        let cli = packwiz(&state)?;
        take_out_of_pack(&state, pack_id, &personal, &wanted, &cli, &handle).await
    }
    .await;
    let result = handle.finish(result);
    if matches!(result, Ok(removed) if removed > 0) {
        notify_changed(&app, pack_id, vec![PackArea::Configs]);
    }
    result
}

#[cfg(test)]
mod tests {
    use warden_core::CancellationToken;

    use super::*;
    use crate::commands::inventory::tests::{packwiz_binary, registered_pack, test_packwiz};
    use crate::state::tests::test_state;

    async fn refresh(root: &Path, cli: &warden_packwiz_cli::Packwiz) {
        PackTransaction::new(root.to_path_buf())
            .commit(cli, &CancellationToken::new())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn migracao_guarda_a_copia_e_tira_o_options_do_pack() {
        let Some(binary) = packwiz_binary() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cli = test_packwiz(&state, binary);
        let (id, root) = registered_pack(&state, dir.path(), &cli, 0).await;
        fs::write(root.join("options.txt"), "renderDistance:8\n").unwrap();
        refresh(&root, &cli).await;
        assert!(
            fs::read_to_string(root.join("index.toml"))
                .unwrap()
                .contains("options.txt")
        );
        let game = game_dir(&state, id);
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("options.txt"), "renderDistance:16\n").unwrap();

        let personal = personal_of(&state, id).unwrap();
        let handle = state.operations.start(CONFIG_PERSONAL, Some(id), false);
        let removed = take_out_of_pack(
            &state,
            id,
            &personal,
            &["options.txt".to_owned()],
            &cli,
            &handle,
        )
        .await
        .unwrap();
        handle.finish::<()>(Ok(())).unwrap();
        assert_eq!(removed, 1);
        assert!(!root.join("options.txt").exists());
        assert!(
            !fs::read_to_string(root.join("index.toml"))
                .unwrap()
                .contains("options.txt")
        );
        // A cópia guardada é a do teste (a mais nova), não a do pack.
        assert_eq!(
            personal.stored("options.txt").unwrap().as_deref(),
            Some(&b"renderDistance:16\n"[..])
        );
    }

    #[tokio::test]
    async fn sem_instancia_a_copia_guardada_e_a_do_pack() {
        let Some(binary) = packwiz_binary() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cli = test_packwiz(&state, binary);
        let (id, root) = registered_pack(&state, dir.path(), &cli, 0).await;
        fs::write(root.join("options.txt"), "lang:pt_br\n").unwrap();
        refresh(&root, &cli).await;
        let personal = personal_of(&state, id).unwrap();
        let handle = state.operations.start(CONFIG_PERSONAL, Some(id), false);
        take_out_of_pack(&state, id, &personal, &["options.txt".to_owned()], &cli, &handle)
            .await
            .unwrap();
        handle.finish::<()>(Ok(())).unwrap();
        assert_eq!(
            personal.stored("options.txt").unwrap().as_deref(),
            Some(&b"lang:pt_br\n"[..])
        );
        assert!(!root.join("options.txt").exists());
    }
}
