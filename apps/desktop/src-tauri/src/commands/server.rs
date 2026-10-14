//! Comandos do domínio `server`: o servidor local e o "Testar como servidor" (SPEC T27 e T21;
//! ARCHITECTURE §7.6; ROADMAP L-09).
//!
//! - `server_eula_accept` e `server_eula_revoke`: o aceite da EULA do Minecraft (com a data) em
//!   `settings.json`, dado no diálogo "Aceitar a EULA do Minecraft?" e revogado em
//!   Configurações → Teste (CA-T21-05). O estado atual vem no `settings_get`
//!   (`eulaAcceptedAt`).
//! - `server_plan`: o que o diálogo "Testar como servidor" mostra antes de abrir: a memória do
//!   servidor e do jogo contra a livre agora (CA-T27-03), os mods "Só cliente" que ficam de fora
//!   e se a EULA já foi aceita. O teste em si é o `test_start` com o modo `server`.
//! - `server_send_command`: "Enviar comando ao servidor" (stdin).
//! - `server_current` e o evento `server-state`: o servidor aberto agora.
//! - `server_session_log`: o console do servidor de uma sessão gravada.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_specta::Event;
use warden_core::PackId;
use warden_server::MemoryPlan;

use crate::error::AppError;
use crate::server_session::ServerSessions;
use crate::settings::Settings;
use crate::state::AppState;
use crate::test_session::{ConsoleLine, read_output_log, session_dir};

/// Situação do servidor local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ServerPhase {
    /// Instalando e copiando o pack ("Preparar o servidor").
    Preparing,
    /// Aberto, ainda carregando.
    Starting,
    /// Pronto (`Done`).
    Ready,
    /// Recebeu o `stop`.
    Stopping,
    /// Fechou (último aviso).
    Exited,
}

/// `server-state`: o servidor local mudou de situação (um por vez no Warden).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "server-state")]
pub(crate) struct ServerState {
    /// O pack.
    pub(crate) pack_id: PackId,
    /// Situação.
    pub(crate) phase: ServerPhase,
    /// A porta em `127.0.0.1`, depois de escolhida.
    pub(crate) port: Option<u16>,
    /// Início (milissegundos desde 1970).
    #[specta(type = specta_typescript::Number)]
    pub(crate) started_at_ms: u64,
    /// Do início do processo até o "pronto", em ms.
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) ready_ms: Option<u64>,
}

/// Liga o evento `server-state` (na abertura do app).
pub(crate) fn connect(sessions: &ServerSessions, app: AppHandle) {
    sessions.connect(std::sync::Arc::new(move |state: &ServerState| {
        if let Err(error) = state.clone().emit(&app) {
            tracing::warn!(%error, "evento server-state não enviado");
        }
    }));
}

/// "Aceito a EULA": grava o aceite com a data de agora.
#[tauri::command]
#[specta::specta]
pub(crate) fn server_eula_accept(state: State<'_, AppState>) -> Result<Settings, AppError> {
    tracing::info!("EULA do Minecraft aceita");
    state
        .settings
        .set_eula_accepted_at(Some(crate::server_session::now_rfc3339()))
}

/// "Revogar": apaga o aceite; o próximo servidor local pede de novo.
#[tauri::command]
#[specta::specta]
pub(crate) fn server_eula_revoke(state: State<'_, AppState>) -> Result<Settings, AppError> {
    tracing::info!("EULA do Minecraft revogada");
    state.settings.set_eula_accepted_at(None)
}

/// O que o diálogo "Testar como servidor" mostra antes de abrir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerPlan {
    /// A conta de memória (servidor + jogo contra a livre agora).
    pub(crate) memory: MemoryPlan,
    /// Os itens "Só cliente" que ficam de fora do servidor (até 50 nomes).
    pub(crate) client_only: Vec<String>,
    /// Quantos são.
    pub(crate) client_only_count: u32,
    /// Quando a EULA foi aceita (`None`: o diálogo da EULA vem antes).
    pub(crate) eula_accepted_at: Option<String>,
    /// O loader do pack tem servidor local (Quilt não tem na v1).
    pub(crate) loader_supported: bool,
}

/// A conta de memória e os itens que ficam de fora. `with_game`: abrir também o jogo (a opção
/// recomendada); `profile`: o perfil do teste (`None` = o ativo).
#[tauri::command]
#[specta::specta]
pub(crate) async fn server_plan(
    state: State<'_, AppState>,
    pack_id: PackId,
    profile: Option<String>,
    with_game: bool,
) -> Result<ServerPlan, AppError> {
    let root = crate::commands::inventory::pack_root(&state, pack_id)?;
    let profile = crate::test_profiles::resolve(&state, pack_id, profile.as_deref())?;
    let global = state.settings.get().test_memory;
    let eula_accepted_at = crate::server_session::eula_accepted_at(&state);
    let (mods, client_only, loader_supported) = tauri::async_runtime::spawn_blocking(move || {
        let read = warden_packwiz::read_pack(&root).map_err(|error| AppError::from_domain(&error))?;
        let mods = read.index.as_ref().map_or(0, |index| {
            index
                .value
                .files
                .iter()
                .filter(|entry| entry.file.starts_with("mods/"))
                .count()
        });
        let supported = !read.pack.value.versions.as_ref().is_some_and(|versions| {
            versions
                .keys()
                .any(|key| matches!(key.as_str(), "quilt" | "liteloader"))
        });
        Ok::<_, AppError>((mods, crate::server_session::client_only_items(&root), supported))
    })
    .await
    .map_err(|error| AppError::internal(format!("ler o pack: {error}")))??;
    let game_mb = with_game.then(|| {
        crate::test_session::choose_memory(
            profile.fields.memory,
            global,
            mods,
            crate::test_session::total_memory_mb(),
        )
        .mb
    });
    let memory = warden_server::memory_plan(
        warden_server::server_memory_mb(mods),
        game_mb,
        crate::server_session::free_memory_mb(),
    );
    Ok(ServerPlan {
        memory,
        client_only_count: u32::try_from(client_only.len()).unwrap_or(u32::MAX),
        client_only: client_only.into_iter().take(50).collect(),
        eula_accepted_at,
        loader_supported,
    })
}

/// "Enviar comando ao servidor". Erros: `server.NOT_RUNNING`, `server.INVALID_COMMAND`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn server_send_command(
    state: State<'_, AppState>,
    pack_id: PackId,
    command: String,
) -> Result<(), AppError> {
    tracing::info!(len = command.len(), "comando enviado ao servidor local");
    state.servers.send_command(pack_id, &command).await
}

/// O servidor local aberto (ou em preparação), se houver.
#[tauri::command]
#[specta::specta]
pub(crate) fn server_current(state: State<'_, AppState>) -> Option<ServerState> {
    state.servers.current()
}

/// O console do servidor de uma sessão gravada.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerLogView {
    /// As últimas linhas do log do servidor (até 50 000); vazio se a sessão não teve servidor
    /// ao lado do jogo.
    pub(crate) lines: Vec<ConsoleLine>,
    /// Se o começo do log ficou de fora.
    pub(crate) truncated: bool,
}

/// O console do servidor de uma sessão com o jogo e o servidor (a sessão "Só o servidor" tem
/// o servidor no console principal). Erros: `app.TEST_SESSION_NOT_FOUND`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn server_session_log(
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
) -> Result<ServerLogView, AppError> {
    let dir = session_dir(&state.paths, pack_id, &session_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = crate::test_session::server_log_path(dir.path());
        let (lines, truncated) = read_output_log(&path)
            .map_err(|error| AppError::internal(format!("ler o log do servidor: {error}")))?;
        Ok(ServerLogView { lines, truncated })
    })
    .await
    .map_err(|error| AppError::internal(format!("ler o log do servidor: {error}")))?
}
