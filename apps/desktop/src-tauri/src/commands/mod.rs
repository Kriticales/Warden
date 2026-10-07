//! Registro dos comandos do IPC (ARCHITECTURE §4.1).
//!
//! Registro acréscimo-apenas (ROADMAP §1): cada domínio declara `pub(crate) mod <domínio>;`
//! e acrescenta os comandos numa linha própria de `collect_commands!`; eventos globais, numa
//! linha de `collect_events!`.

pub(crate) mod app;
pub(crate) mod catalog;
pub(crate) mod configs;
pub(crate) mod curseforge;
pub(crate) mod export;
pub(crate) mod inventory;
pub(crate) mod java;
pub(crate) mod pack_meta;
pub(crate) mod packs;
pub(crate) mod secrets;
pub(crate) mod window;

use crate::events::{OperationUpdated, PackChanged, TitleBarMaximize};

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
            catalog::catalog_minecraft_versions,
            catalog::catalog_loader_versions,
            java::java_choice,
            java::java_runtimes_list,
            java::java_runtimes_check_updates,
            java::java_runtime_remove,
            java::java_runtimes_remove_unused,
            packs::packs_list,
            packs::pack_get,
            packs::pack_create,
            packs::pack_import_preview,
            packs::pack_import,
            packs::pack_relocate,
            packs::pack_forget,
            packs::pack_hygiene_scan,
            packs::pack_hygiene_fix,
            packs::pack_trash,
            export::export_preview,
            export::export_exclude,
            export::export_run,
            export::export_reveal,
            app::settings_status,
            app::settings_choose_packs_dir,
            app::logs_reveal_folder,
            window::window_set_maximize_area,
            packs::pack_create_defaults,
            packs::pack_create_check,
            packs::pack_choose_folder,
            packs::pack_reveal_folder,
            inventory::inventory_list,
            inventory::item_details,
            inventory::items_set_side,
            inventory::items_remove_plan,
            inventory::items_remove,
            inventory::inventory_include_outside,
            inventory::item_open_file,
            pack_meta::pack_meta_get,
            pack_meta::pack_update_meta,
            pack_meta::pack_test_settings_get,
            pack_meta::pack_test_settings_set,
            pack_meta::instance_recreate,
            configs::config_tree,
            configs::config_read,
            configs::config_write,
        ])
        .events(tauri_specta::collect_events![
            OperationUpdated,
            PackChanged,
            TitleBarMaximize,
        ])
}
