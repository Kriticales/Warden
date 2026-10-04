//! Crate `warden-app`: a única que conhece o Tauri (ARCHITECTURE §1 e §3).
//!
//! Traduz entre o IPC e as crates de domínio: comandos finos, eventos e estado do app.
//! Esqueleto da F0-01; a F0-05 acrescenta estado, registros, configurações e cofre.

mod commands;
mod error;

use std::path::Path;

pub use error::{AppError, AppErrorCode, ErrorCode};

/// Abre o app: registra os comandos e eventos e mostra a janela principal.
pub fn run() -> Result<(), tauri::Error> {
    let builder = commands::builder();
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
}

/// Gera o `bindings.ts` (tipos e chamadas TypeScript) em `path`. Usado por
/// `cargo xtask bindings`; o arquivo gerado é versionado (ADR-0018).
pub fn export_bindings(path: &Path) -> Result<(), specta_typescript::Error> {
    commands::builder().export(specta_typescript::Typescript::default(), path)
}
