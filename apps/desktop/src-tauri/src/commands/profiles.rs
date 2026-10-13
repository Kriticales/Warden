//! Comandos dos perfis do teste (SPEC T11 "Perfis do teste"; ROADMAP L-08). O modelo e a
//! gravação ficam em [`crate::test_profiles`]; aqui ficam só a casca do IPC e a visão que a
//! interface mostra.
//!
//! - `test_profiles_get`: os perfis do pack, o ativo, a memória do computador (aviso acima de
//!   75%) e se a versão do Minecraft do pack aceita "Entrar direto no mundo de teste".
//! - `test_profile_create`, `test_profile_rename`, `test_profile_delete`: Novo perfil,
//!   Renomear e Apagar do diálogo Ajustes do teste (o Padrão não pode ser renomeado nem
//!   apagado).
//! - `test_profile_select`: o grupo "Perfil do teste" do menu ▾ do Testar.
//! - `test_profiles_save`: "Salvar ajustes" (todos os perfis que mudaram no diálogo).
//!
//! Nada disso toca no pack: tudo fica em `packs.json` (neste computador).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use serde::Serialize;
use tauri::State;
use warden_core::PackId;
use warden_launcher::QuickPlaySupport;
use warden_launcher::quick_play::quick_play_support;

use crate::commands::inventory::pack_root;
use crate::error::AppError;
use crate::state::AppState;
use crate::test_profiles::{self, ProfileFields, TestProfile};

/// O que o diálogo Ajustes do teste e o menu ▾ mostram dos perfis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestProfilesView {
    /// Id do perfil ativo (o que o Testar usa).
    pub(crate) active: String,
    /// Os perfis, com o Padrão (`default`) primeiro.
    pub(crate) profiles: Vec<TestProfile>,
    /// Memória total do computador, em MB (`null` se não deu para ler).
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) computer_memory_mb: Option<u64>,
    /// Versão do Minecraft do pack (`null` se o `pack.toml` não diz).
    pub(crate) minecraft: Option<String>,
    /// Se essa versão aceita entrar direto no mundo (Quick Play, 1.20+).
    pub(crate) quick_play: QuickPlaySupport,
}

async fn view(state: &AppState, pack_id: PackId) -> Result<TestProfilesView, AppError> {
    let profiles = test_profiles::profiles_of(state, pack_id)?;
    let minecraft = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(state, pack_id)?;
        warden_project::meta::read_meta(&root)
            .map_err(|error| AppError::from_domain(&error))?
            .minecraft
    };
    Ok(TestProfilesView {
        active: profiles.active,
        profiles: profiles.profiles,
        computer_memory_mb: crate::test_session::total_memory_mb(),
        quick_play: minecraft
            .as_deref()
            .map_or(QuickPlaySupport::Unknown, quick_play_support),
        minecraft,
    })
}

/// Os perfis do teste do pack.
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_profiles_get(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<TestProfilesView, AppError> {
    view(&state, pack_id).await
}

/// "Novo perfil": cria um perfil com o nome e os ajustes dados (o diálogo manda os que estão
/// na tela). O perfil novo não fica ativo. Erros: `project.INVALID_INPUT` (`field`),
/// `app.TEST_PROFILE_NAME_TAKEN`, `app.TEST_PROFILE_LIMIT`.
#[tauri::command]
#[specta::specta]
pub(crate) fn test_profile_create(
    state: State<'_, AppState>,
    pack_id: PackId,
    name: String,
    fields: ProfileFields,
) -> Result<TestProfile, AppError> {
    test_profiles::create_profile(&state, pack_id, &name, fields)
}

/// "Renomear". Erros: `app.TEST_PROFILE_DEFAULT_LOCKED`, `app.TEST_PROFILE_NOT_FOUND`,
/// `app.TEST_PROFILE_NAME_TAKEN`, `project.INVALID_INPUT`.
#[tauri::command]
#[specta::specta]
pub(crate) fn test_profile_rename(
    state: State<'_, AppState>,
    pack_id: PackId,
    profile_id: String,
    name: String,
) -> Result<(), AppError> {
    test_profiles::rename_profile(&state, pack_id, &profile_id, &name)
}

/// "Apagar". Se era o ativo, o Padrão passa a ser. Erros: `app.TEST_PROFILE_DEFAULT_LOCKED`,
/// `app.TEST_PROFILE_NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) fn test_profile_delete(
    state: State<'_, AppState>,
    pack_id: PackId,
    profile_id: String,
) -> Result<(), AppError> {
    test_profiles::delete_profile(&state, pack_id, &profile_id)
}

/// Escolhe o perfil que o Testar usa (menu ▾). Erros: `app.TEST_PROFILE_NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) fn test_profile_select(
    state: State<'_, AppState>,
    pack_id: PackId,
    profile_id: String,
) -> Result<(), AppError> {
    test_profiles::select_profile(&state, pack_id, &profile_id)
}

/// "Salvar ajustes": grava os perfis que mudaram no diálogo e devolve como ficaram.
#[tauri::command]
#[specta::specta]
pub(crate) async fn test_profiles_save(
    state: State<'_, AppState>,
    pack_id: PackId,
    profiles: Vec<TestProfile>,
) -> Result<TestProfilesView, AppError> {
    test_profiles::save_profiles(&state, pack_id, profiles)?;
    view(&state, pack_id).await
}
