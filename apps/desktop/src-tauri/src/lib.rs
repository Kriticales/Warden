//! Crate `warden-app`: a única que conhece o Tauri (ARCHITECTURE §1 e §3).
//!
//! Traduz entre o IPC e as crates de domínio: comandos finos, eventos, estado do app,
//! configurações, registro de operações, travas por pack e registros (ARCHITECTURE §4, §5,
//! §13 a §16 e §20).
//!
//! Ao abrir ([`run`]):
//! 1. decide as pastas: as do sistema ou, só em build de debug, as de `WARDEN_DATA_ROOT`
//!    (ADR-0048); com essa pasta, a instância única vale só entre cópias com a mesma pasta;
//! 2. liga a instância única (`tauri-plugin-single-instance`): abrir uma segunda cópia foca a
//!    primeira;
//! 3. liga os registros, o gancho de pânico, as configurações, o cofre e o registro de
//!    operações;
//! 4. cria a janela principal com o perfil do WebView dentro dos dados locais e, no Windows,
//!    com a barra de título própria (`window_chrome`).

mod commands;
mod error;
pub mod events;
pub mod locks;
pub mod logging;
pub mod operations;
pub mod settings;
pub mod state;
mod window_chrome;

/// Testes do gerador do registro de comandos (o mesmo arquivo roda no `build.rs`).
#[cfg(test)]
#[path = "../build_registry.rs"]
mod build_registry;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::Manager as _;
use tauri_specta::Event as _;
use warden_core::{AppPaths, SystemDirs};
use warden_secrets::BackendOverride;

pub use commands::secrets::{SecretTestResult, SecretTester, SecretTesters};
pub use error::{AppError, AppErrorCode, ErrorCode};

use crate::events::OperationUpdated;
use crate::state::AppState;

/// Rótulo da janela principal no `tauri.conf.json`.
const MAIN_WINDOW: &str = "main";

/// Abre o app: registra os comandos e eventos e mostra a janela principal.
pub fn run() -> Result<(), tauri::Error> {
    logging::install_panic_hook();
    let dev_root = AppPaths::dev_root_from_env();
    let mut context = tauri::generate_context!();
    if let Some(root) = &dev_root {
        // Instância única por pasta de desenvolvimento: o plugin usa o identificador do app
        // como nome da trava (ARCHITECTURE §13).
        context.config_mut().identifier = dev_identifier(&context.config().identifier, root);
    }

    let builder = commands::builder();
    tauri::Builder::default()
        // A instância única vem primeiro, para a segunda cópia sair antes de abrir qualquer
        // outra coisa.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }))
        .plugin(
            // O `tauri-plugin-log` só recebe os registros do frontend; quem grava é o
            // `tracing` (logging.rs), pela fachada `log`.
            tauri_plugin_log::Builder::new().skip_logger().build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            setup(app, dev_root.as_deref())?;
            Ok(())
        })
        .run(context)
}

/// Pastas, registros, estado e janela.
fn setup(app: &mut tauri::App, dev_root: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let paths = app_paths(app, dev_root)?;
    let logging = match logging::init(&paths.logs_dir(), settings::LogLevel::Normal) {
        Ok(logging) => Some(logging),
        Err(error) => {
            // Sem registros em arquivo o app ainda funciona; o motivo vai para o terminal.
            #[allow(clippy::print_stderr)] // os registros ainda não existem
            {
                eprintln!("registros desligados: {error}");
            }
            None
        }
    };
    logging::set_panic_app(app.handle().clone());
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        commit = env!("WARDEN_COMMIT"),
        dev_root = ?paths.dev_root(),
        "Warden abrindo"
    );

    let keyring_override =
        BackendOverride::from_env(paths.dev_root()).map_err(AppError::internal)?;
    if let Some(BackendOverride::TestFile(dir)) = &keyring_override {
        tracing::info!(dir = %dir.display(), "cofre de teste em arquivo no lugar do cofre do sistema");
    }
    let handle = app.handle().clone();
    let notifier = Arc::new(move |snapshot: &operations::OperationSnapshot| {
        if let Err(error) = OperationUpdated(snapshot.clone()).emit(&handle) {
            tracing::warn!(%error, "evento operation-updated não enviado");
        }
    });
    let webview_dir = paths.webview_dir();
    let (state, outcome) = AppState::open(paths, keyring_override, notifier, logging)?;
    tracing::info!(?outcome, "configurações lidas");
    app.manage(state);

    create_main_window(app, webview_dir)?;
    Ok(())
}

/// Pastas do app: as do sistema (pelo Tauri) ou as de desenvolvimento.
fn app_paths(
    app: &tauri::App,
    dev_root: Option<&Path>,
) -> Result<AppPaths, Box<dyn std::error::Error>> {
    if let Some(root) = dev_root {
        return Ok(AppPaths::from_dev_root(root)?);
    }
    let resolver = app.path();
    let system = SystemDirs {
        config: resolver.app_config_dir()?,
        local_data: resolver.app_local_data_dir()?,
        documents: resolver.document_dir().ok(),
    };
    Ok(AppPaths::from_system(system))
}

/// Identificador do app de desenvolvimento: o do app seguido de um resumo da pasta, para que
/// cópias com pastas diferentes não se enxerguem nem enxerguem o Warden instalado.
fn dev_identifier(base: &str, root: &Path) -> String {
    let normalized = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    let mut text = normalized.to_string_lossy().replace('\\', "/");
    text = text.trim_end_matches('/').to_owned();
    if cfg!(windows) {
        text = text.to_lowercase();
    }
    format!("{base}.dev-{:016x}", fnv1a(text.as_bytes()))
}

/// FNV-1a de 64 bits: estável entre compilações (o `DefaultHasher` não promete isso).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Traz a janela principal para a frente (segunda cópia aberta).
fn focus_main_window(app: &tauri::AppHandle) {
    tracing::info!("segunda cópia do Warden aberta; focando a janela existente");
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        // Falhas aqui só deixam a janela onde está.
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Cria a janela principal a partir do `tauri.conf.json` (onde ela tem `create: false`), com o
/// perfil do WebView em `<dados locais>/EBWebView`: no app instalado é a mesma pasta padrão do
/// Tauri; no desenvolvimento fica dentro de `WARDEN_DATA_ROOT`, longe da pasta do Warden
/// instalado (ADR-0048; QUALITY §13.6).
fn create_main_window(
    app: &tauri::App,
    webview_dir: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN_WINDOW)
        .ok_or("janela principal ausente no tauri.conf.json")?;
    let window = tauri::WebviewWindowBuilder::from_config(app.handle(), config)?
        .data_directory(webview_dir)
        // No Windows a barra de título é a do Warden (UI-01; `window_chrome`).
        .decorations(!window_chrome::CUSTOM_TITLE_BAR)
        .build()?;
    window_chrome::attach(&window);
    Ok(())
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
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_dev_root(dir.path()).unwrap();
        assert_eq!(
            paths.webview_dir(),
            dir.path().join("data").join("EBWebView")
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

    /// Critério 6 da F0-05 (parte unitária): cada pasta de desenvolvimento tem um
    /// identificador próprio, diferente do Warden instalado; a mesma pasta, escrita de outro
    /// jeito, dá o mesmo.
    #[test]
    fn f0_05_ca6_identificador_por_pasta_de_desenvolvimento() {
        let base = "dev.kriticales.warden";
        let a = dev_identifier(base, Path::new("C:/dados/a"));
        let b = dev_identifier(base, Path::new("C:/dados/b"));
        assert_ne!(a, b);
        assert_ne!(a, base);
        assert!(a.starts_with("dev.kriticales.warden.dev-"));
        assert_eq!(a, dev_identifier(base, Path::new("C:/dados/a/")));
        #[cfg(windows)]
        {
            assert_eq!(a, dev_identifier(base, Path::new("C:\\dados\\a")));
            assert_eq!(a, dev_identifier(base, Path::new("c:\\DADOS\\A")));
        }
    }

    #[test]
    fn fnv1a_conhecido() {
        // Vetores de teste da especificação do FNV.
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
