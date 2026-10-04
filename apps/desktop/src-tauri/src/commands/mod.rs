//! Registro dos comandos do IPC (ARCHITECTURE §4.1).
//!
//! Registro acréscimo-apenas (ROADMAP §1): cada domínio declara `pub(crate) mod <domínio>;`
//! e acrescenta os comandos numa linha própria de `collect_commands!`.

pub(crate) mod app;

/// Builder do `tauri-specta` com todos os comandos e eventos. Usado pelo app e pela geração
/// do `bindings.ts`.
pub(crate) fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![app::app_info,])
}
