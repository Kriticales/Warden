//! Executável do Warden: só chama [`warden_app::run`].

// Em build de release no Windows, o app abre sem a janela de console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> Result<(), tauri::Error> {
    warden_app::run()
}
