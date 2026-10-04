//! Registro dos comandos do IPC (ARCHITECTURE §4.1).
//!
//! Registro acréscimo-apenas (ROADMAP §1): cada domínio declara `pub(crate) mod <domínio>;`
//! e acrescenta os comandos numa linha própria de `collect_commands!`; eventos globais, numa
//! linha de `collect_events!`.

pub(crate) mod app;
pub(crate) mod secrets;

use crate::events::{OperationUpdated, PackChanged};

/// Builder do `tauri-specta` com todos os comandos e eventos. Usado pelo app e pela geração
/// do `bindings.ts`.
pub(crate) fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            app::app_info,
            app::settings_get,
            app::settings_update,
            app::operations_list,
            app::operation_cancel,
            secrets::secrets_status,
            secrets::secrets_set,
            secrets::secrets_test,
            secrets::secrets_remove,
            secrets::secrets_backend_get,
            secrets::secrets_backend_set,
        ])
        .events(tauri_specta::collect_events![OperationUpdated, PackChanged,])
}
