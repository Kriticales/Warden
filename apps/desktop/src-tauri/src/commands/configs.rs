//! Comandos do domínio `configs` (ARCHITECTURE §4.1 e §10; SPEC T12; C-02): o editor de texto
//! das configs do pack e da instância de teste.
//!
//! - `config_tree`: os arquivos da origem escolhida (Pack ou Instância).
//! - `config_read`: o texto de um arquivo com o `hash` do conteúdo.
//! - `config_write`: grava o texto do usuário, exigindo o `hash` lido (concorrência otimista;
//!   `FILE_CHANGED_ON_DISK` se o arquivo mudou por fora). No pack passa pela `PackTransaction`
//!   (atômica, com `packwiz refresh`); na instância é uma gravação atômica.
//!
//! Finos: pegam a trava do pack e chamam a `warden-project`. A busca, o formulário e os padrões
//! chegam com a C-04, a C-05 e a C-06 (acréscimo neste mesmo domínio).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::path::PathBuf;

use tauri::{AppHandle, State};
use warden_core::PackId;
use warden_packwiz_cli::Packwiz;
use warden_project::configs::{
    ConfigContent, ConfigFileList, ConfigOrigin, ConfigSaved, list_files, read_file,
    save_to_instance, save_to_pack,
};

use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// Gravar uma config (pack ou instância).
pub(crate) const CONFIG_WRITE: OperationKind = OperationKind::new("configs.write");

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

/// A pasta da origem: o pack, ou a pasta do jogo da instância de teste (`minecraft/`).
fn origin_root(
    state: &AppState,
    pack_id: PackId,
    origin: ConfigOrigin,
) -> Result<PathBuf, AppError> {
    match origin {
        ConfigOrigin::Pack => pack_root(state, pack_id),
        ConfigOrigin::Instance => {
            // Confere que o pack existe, mesmo quando só a instância é lida.
            state.packs.get(pack_id).map_err(domain)?;
            Ok(state.paths.instance_dir(pack_id).join("minecraft"))
        }
    }
}

/// Os arquivos da origem, em ordem de caminho. A instância só existe depois do primeiro teste
/// (`available = false` antes disso).
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_tree(
    state: State<'_, AppState>,
    pack_id: PackId,
    origin: ConfigOrigin,
) -> Result<ConfigFileList, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = origin_root(&state, pack_id, origin)?;
    tauri::async_runtime::spawn_blocking(move || list_files(&root, origin))
        .await
        .map_err(|error| AppError::internal(error.to_string()))?
        .map_err(domain)
}

/// Lê um arquivo: texto, `hash`, fim de linha e o motivo de só leitura, quando houver.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_read(
    state: State<'_, AppState>,
    pack_id: PackId,
    origin: ConfigOrigin,
    path: String,
) -> Result<ConfigContent, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = origin_root(&state, pack_id, origin)?;
    tauri::async_runtime::spawn_blocking(move || read_file(&root, origin, &path))
        .await
        .map_err(|error| AppError::internal(error.to_string()))?
        .map_err(domain)
}

/// Grava o texto do arquivo. `expected_hash` é o `hash` da última leitura: se o arquivo mudou
/// no disco desde então, nada é gravado (`FILE_CHANGED_ON_DISK`). "Sobrescrever" é reler o
/// arquivo e gravar com o `hash` novo, por escolha explícita da pessoa.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_write(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    origin: ConfigOrigin,
    path: String,
    text: String,
    expected_hash: String,
) -> Result<ConfigSaved, AppError> {
    let cli = match origin {
        ConfigOrigin::Pack => Some(packwiz(&state)?),
        ConfigOrigin::Instance => None,
    };
    let saved = write_impl(
        &state,
        cli.as_ref(),
        pack_id,
        origin,
        &path,
        &text,
        &expected_hash,
    )
    .await;
    if matches!(&saved, Ok(saved) if saved.changed) && origin == ConfigOrigin::Pack {
        notify_changed(&app, pack_id, vec![PackArea::Configs]);
    }
    saved
}

/// A gravação de verdade. `cli` é o packwiz do pack (`None` na instância, que não tem índice).
async fn write_impl(
    state: &AppState,
    cli: Option<&Packwiz>,
    pack_id: PackId,
    origin: ConfigOrigin,
    path: &str,
    text: &str,
    expected_hash: &str,
) -> Result<ConfigSaved, AppError> {
    let handle = state.operations.start(CONFIG_WRITE, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = origin_root(state, pack_id, origin)?;
        match cli {
            Some(cli) => save_to_pack(&root, path, text, expected_hash, cli, handle.token())
                .await
                .map_err(domain),
            None => save_to_instance(&root, path, text, expected_hash).map_err(domain),
        }
    }
    .await;
    handle.finish(result)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use warden_project::ProjectErrorCode;

    use super::*;
    use crate::commands::inventory::tests::{packwiz_binary, registered_pack, test_packwiz};
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;

    fn code(error: &AppError) -> ErrorCode {
        error.code
    }

    #[tokio::test]
    async fn grava_no_pack_com_hash_e_recusa_o_hash_velho() {
        let Some(binary) = packwiz_binary() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cli = test_packwiz(&state, binary);
        let (id, root) = registered_pack(&state, dir.path(), &cli, 0).await;
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("config/a.toml"), "a = 1\n").unwrap();
        refresh(&root, &cli).await;

        let read = read_file(
            &origin_root(&state, id, ConfigOrigin::Pack).unwrap(),
            ConfigOrigin::Pack,
            "config/a.toml",
        )
        .unwrap();
        let saved = write_impl(
            &state,
            Some(&cli),
            id,
            ConfigOrigin::Pack,
            "config/a.toml",
            "a = 2\n",
            &read.hash,
        )
        .await
        .unwrap();
        assert!(saved.changed);
        assert_eq!(
            fs::read_to_string(root.join("config/a.toml")).unwrap(),
            "a = 2\n"
        );

        // O hash da leitura antiga não vale mais.
        let error = write_impl(
            &state,
            Some(&cli),
            id,
            ConfigOrigin::Pack,
            "config/a.toml",
            "a = 3\n",
            &read.hash,
        )
        .await
        .unwrap_err();
        assert_eq!(
            code(&error),
            ErrorCode::Project(ProjectErrorCode::FileChangedOnDisk)
        );
        assert_eq!(
            fs::read_to_string(root.join("config/a.toml")).unwrap(),
            "a = 2\n"
        );
    }

    #[tokio::test]
    async fn grava_na_instancia_sem_packwiz() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let root = dir.path().join("pack");
        fs::create_dir_all(&root).unwrap();
        let record = warden_project::registry::PackRecord {
            id: PackId::new(),
            name: "Vale".into(),
            path: root,
            last_test: None,
            extra: std::collections::BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        let game = state.paths.instance_dir(id).join("minecraft");
        fs::create_dir_all(game.join("config")).unwrap();
        fs::write(game.join("config/a.toml"), "a = 1\n").unwrap();

        let list = list_files(&game, ConfigOrigin::Instance).unwrap();
        assert_eq!(list.files.len(), 1);
        let read = read_file(&game, ConfigOrigin::Instance, "config/a.toml").unwrap();
        let saved = write_impl(
            &state,
            None,
            id,
            ConfigOrigin::Instance,
            "config/a.toml",
            "a = 2\n",
            &read.hash,
        )
        .await
        .unwrap();
        assert!(saved.changed);
        assert_eq!(
            fs::read_to_string(game.join("config/a.toml")).unwrap(),
            "a = 2\n"
        );
    }

    #[test]
    fn pack_desconhecido_e_pasta_da_instancia() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let error = origin_root(&state, PackId::new(), ConfigOrigin::Instance).unwrap_err();
        assert_eq!(
            code(&error),
            ErrorCode::Project(ProjectErrorCode::PackNotFound)
        );
    }

    /// Roda o `packwiz refresh` para o índice conhecer o arquivo criado pelo teste.
    async fn refresh(root: &std::path::Path, cli: &warden_packwiz_cli::Packwiz) {
        warden_project::transaction::PackTransaction::new(root.to_path_buf())
            .commit(cli, &warden_core::CancellationToken::new())
            .await
            .unwrap();
    }
}
