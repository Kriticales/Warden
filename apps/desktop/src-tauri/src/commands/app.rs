//! Comandos do domínio `app`: informações, configurações e operações.

use serde::Serialize;
use tauri::State;
use warden_core::OperationId;

use crate::error::AppError;
use crate::operations::OperationSnapshot;
use crate::settings::{Settings, SettingsPatch};
use crate::state::AppState;

/// Sistema em que o app roda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Platform {
    /// Windows (WebView2).
    Windows,
    /// Linux (WebKitGTK; só na CI).
    Linux,
    /// Outro (não suportado).
    Other,
}

impl Platform {
    const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

/// Versão e commit do app em execução.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfo {
    /// Versão do app (a do `Cargo.toml` do workspace).
    pub(crate) version: String,
    /// Commit do código compilado (12 caracteres), quando conhecido.
    pub(crate) commit: Option<String>,
    /// Sistema em que o app roda.
    pub(crate) platform: Platform,
    /// Build de desenvolvimento (debug).
    pub(crate) debug_build: bool,
}

impl AppInfo {
    fn current() -> Self {
        let commit = env!("WARDEN_COMMIT");
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            commit: (!commit.is_empty()).then(|| commit.to_owned()),
            platform: Platform::current(),
            debug_build: cfg!(debug_assertions),
        }
    }
}

/// Informa a versão, o commit e a plataforma do app.
#[tauri::command]
#[specta::specta]
// Todo comando devolve `Result<T, AppError>`, mesmo quando hoje não falha (ARCHITECTURE §4.1).
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn app_info() -> Result<AppInfo, AppError> {
    Ok(AppInfo::current())
}

/// Configurações atuais.
#[tauri::command]
#[specta::specta]
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn settings_get(state: State<'_, AppState>) -> Result<Settings, AppError> {
    Ok(state.settings.get())
}

/// Altera as configurações e devolve como ficaram. O nível dos registros vale na hora.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn settings_update(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<Settings, AppError> {
    settings_update_impl(&state, patch)
}

fn settings_update_impl(state: &AppState, patch: SettingsPatch) -> Result<Settings, AppError> {
    let settings = state.settings.update(patch)?;
    if let Some(logging) = &state.logging {
        logging.set_level(settings.log_level);
    }
    tracing::info!("configurações alteradas");
    Ok(settings)
}

/// Operações em andamento e as últimas concluídas (painel Tarefas).
#[tauri::command]
#[specta::specta]
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn operations_list(
    state: State<'_, AppState>,
) -> Result<Vec<OperationSnapshot>, AppError> {
    Ok(state.operations.list())
}

/// Pede o cancelamento de uma operação.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn operation_cancel(
    state: State<'_, AppState>,
    operation_id: OperationId,
) -> Result<(), AppError> {
    state.operations.cancel(operation_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::LogLevel;
    use crate::state::tests::test_state;

    #[test]
    fn app_info_informa_versao_e_plataforma() {
        let info = app_info().unwrap();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        if let Some(commit) = &info.commit {
            assert!(!commit.is_empty());
        }
        assert_eq!(info.debug_build, cfg!(debug_assertions));
        #[cfg(windows)]
        assert_eq!(info.platform, Platform::Windows);
    }

    #[test]
    fn app_info_em_camel_case() {
        let info = AppInfo {
            version: "1.2.3".into(),
            commit: Some("abc".into()),
            platform: Platform::Windows,
            debug_build: false,
        };
        let json = serde_json::to_value(info).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "version": "1.2.3",
                "commit": "abc",
                "platform": "windows",
                "debugBuild": false
            })
        );
    }

    #[test]
    fn atualizar_configuracoes_grava() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let settings = settings_update_impl(
            &state,
            SettingsPatch {
                log_level: Some(LogLevel::Detailed),
                ..SettingsPatch::default()
            },
        )
        .unwrap();
        assert_eq!(settings.log_level, LogLevel::Detailed);
        assert!(state.settings.exists());
    }
}
