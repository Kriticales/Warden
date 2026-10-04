//! Crate `warden-app`: a única que conhece o Tauri (ARCHITECTURE §1 e §3).
//!
//! Traduz entre o IPC e as crates de domínio: comandos finos, eventos e estado do app.
//! Esqueleto da F0-01; a F0-05 acrescenta estado, registros, configurações e cofre.

mod commands;
mod error;

use std::path::{Path, PathBuf};

pub use error::{AppError, AppErrorCode, ErrorCode};

/// Rótulo da janela principal no `tauri.conf.json`.
const MAIN_WINDOW: &str = "main";

/// Abre o app: registra os comandos e eventos e mostra a janela principal.
pub fn run() -> Result<(), tauri::Error> {
    let builder = commands::builder();
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            create_main_window(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
}

/// Cria a janela principal a partir do `tauri.conf.json` (onde ela tem `create: false`).
///
/// Em build de debug com `WARDEN_DATA_ROOT`, o perfil do WebView fica dentro dessa pasta:
/// sem isso, o Tauri usaria `%LOCALAPPDATA%\dev.kriticales.warden\EBWebView`, a pasta do
/// Warden instalado (ADR-0048; QUALITY §13.6).
fn create_main_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN_WINDOW)
        .ok_or("janela principal ausente no tauri.conf.json")?;
    let mut window = tauri::WebviewWindowBuilder::from_config(app.handle(), config)?;
    if let Some(dir) = dev_webview_dir() {
        window = window.data_directory(dir);
    }
    window.build()?;
    Ok(())
}

/// Pasta do perfil do WebView no desenvolvimento: `<WARDEN_DATA_ROOT>/data/EBWebView`.
#[cfg(debug_assertions)]
fn dev_webview_dir() -> Option<PathBuf> {
    let root = std::env::var_os("WARDEN_DATA_ROOT").filter(|value| !value.is_empty())?;
    Some(webview_dir_inside(Path::new(&root)))
}

/// Em build de release, `WARDEN_DATA_ROOT` não existe (ARCHITECTURE §13).
#[cfg(not(debug_assertions))]
fn dev_webview_dir() -> Option<PathBuf> {
    None
}

#[cfg(any(debug_assertions, test))]
fn webview_dir_inside(data_root: &Path) -> PathBuf {
    data_root.join("data").join("EBWebView")
}

/// Gera o `bindings.ts` (tipos e chamadas TypeScript) em `path`. Usado por
/// `cargo xtask bindings`; o arquivo gerado é versionado (ADR-0018).
pub fn export_bindings(path: &Path) -> Result<(), specta_typescript::Error> {
    commands::builder().export(specta_typescript::Typescript::default(), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfil_do_webview_fica_dentro_da_pasta_de_dados() {
        let root = Path::new("C:/dados-dev");
        assert_eq!(
            webview_dir_inside(root),
            root.join("data").join("EBWebView")
        );
    }

    #[test]
    fn janela_principal_existe_e_e_criada_pelo_codigo() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let windows = config["app"]["windows"].as_array().unwrap();
        let main = windows
            .iter()
            .find(|window| window["label"] == MAIN_WINDOW)
            .unwrap();
        assert_eq!(main["create"], false);
        assert_eq!(main["title"], "Warden");
    }
}
