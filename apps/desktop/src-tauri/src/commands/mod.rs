//! Registro dos comandos do IPC (ARCHITECTURE §4.1).
//!
//! **Não há lista para editar.** O `build.rs` (`build_registry.rs`) lê todos os
//! `commands/<domínio>.rs`, declara cada módulo e registra cada função marcada com
//! `#[tauri::command]` (na coluna 0), mais os eventos `#[tauri_specta(event_name = …)]` deste
//! diretório e de `events.rs`. Para um comando novo, basta escrevê-lo no arquivo do domínio
//! (ou criar `commands/<domínio>.rs`); duas branches que fazem isso não se tocam.

include!(concat!(env!("OUT_DIR"), "/commands_registry.rs"));

/// Builder do `tauri-specta` com todos os comandos e eventos. Usado pelo app e pela geração
/// do `bindings.ts`.
pub(crate) fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(commands())
        .events(events())
}
