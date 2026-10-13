//! Comandos do formulário de configs (SPEC T12, parte P1; ARCHITECTURE §10.1; C-04).
//!
//! - `config_form`: o formulário do texto atual do editor (do pack ou da instância: os rótulos
//!   vêm dos mesmos jars) (as camadas da `warden-configs::meta`
//!   com os rótulos dos jars do pack), ou o motivo de abrir só em texto.
//! - `config_form_apply`: aplica as edições do formulário ao texto e devolve o texto novo; a
//!   gravação continua pelo `config_write` (diferença antes de gravar, alteração externa).
//! - `config_preserve` e `config_set_preserve`: "Não substituir se o jogador já tiver".
//! - `config_copy_to_defaultconfigs`: "Copiar para defaultconfigs/".
//!
//! Rótulos (camada 4): os jars da instância de teste (`minecraft/mods/`) e os guardados no
//! próprio pack (`mods/*.jar`). Os rótulos de cada jar ficam num cache na memória, pela data e
//! pelo tamanho do arquivo: a primeira abertura do formulário lê os jars em paralelo; as
//! seguintes só leem os que mudaram.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use tauri::{AppHandle, State};
use warden_configs::meta::{ConfigForm, FormEdit, apply_form_edits, build_form};
use warden_core::PackId;
use warden_jarmeta::Limits;
use warden_jarmeta::lang::{ConfigLang, read_config_lang_file};
use warden_packwiz_cli::Packwiz;
use warden_project::preserve::{
    CopyOutcome, PreserveState, copy_to_defaultconfigs, preserve_of, set_preserve,
};

use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// Ligar ou desligar o `preserve` de uma config.
pub(crate) const CONFIG_PRESERVE: OperationKind = OperationKind::new("configs.preserve");

/// Copiar uma config para `defaultconfigs/`.
pub(crate) const CONFIG_COPY_DEFAULTS: OperationKind =
    OperationKind::new("configs.copyDefaultconfigs");

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

fn joined<T>(result: Result<T, tauri::Error>) -> Result<T, AppError> {
    result.map_err(|error| AppError::internal(error.to_string()))
}

/// O formulário de `path` com o texto atual do editor (que pode ter alterações não salvas).
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_form(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
    text: String,
) -> Result<ConfigForm, AppError> {
    let jars = {
        let _lock = state.locks.read(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        let game = state.paths.instance_dir(pack_id).join("minecraft");
        jar_files(&[game.join("mods"), root.join("mods")])
    };
    joined(
        tauri::async_runtime::spawn_blocking(move || {
            let labels = labels_for(&jars);
            build_form(&path, text.as_bytes(), Some(&labels))
        })
        .await,
    )
}

/// Aplica as edições do formulário ao texto e devolve o texto novo. Só os bytes dos valores
/// mudam; se a conferência falhar, nada volta (`EDIT_NOT_CONFIRMED`) e a interface abre o texto.
#[tauri::command]
#[specta::specta]
pub(crate) fn config_form_apply(
    path: String,
    text: String,
    edits: Vec<FormEdit>,
) -> Result<String, AppError> {
    apply_form_edits(&path, &text, &edits).map_err(|error| AppError::from_domain(&error))
}

/// Se a config do pack está no índice e com "Não substituir se o jogador já tiver".
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_preserve(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<PreserveState, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    joined(tauri::async_runtime::spawn_blocking(move || preserve_of(&root, &path)).await)?
        .map_err(domain)
}

/// Liga ou desliga "Não substituir se o jogador já tiver". `false` quando já estava assim.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_set_preserve(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
    preserve: bool,
) -> Result<bool, AppError> {
    let cli = packwiz(&state)?;
    let result = preserve_impl(&state, &cli, pack_id, &path, preserve).await;
    if matches!(result, Ok(true)) {
        notify_changed(&app, pack_id, vec![PackArea::Configs]);
    }
    result
}

/// Copia um `config/*-server.toml` para `defaultconfigs/`. Sem `overwrite`, um destino
/// diferente não é tocado e a resposta pede a confirmação.
#[tauri::command]
#[specta::specta]
pub(crate) async fn config_copy_to_defaultconfigs(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
    overwrite: bool,
) -> Result<CopyOutcome, AppError> {
    let cli = packwiz(&state)?;
    let result = copy_impl(&state, &cli, pack_id, &path, overwrite).await;
    if matches!(result, Ok(CopyOutcome::Copied { .. })) {
        notify_changed(&app, pack_id, vec![PackArea::Configs]);
    }
    result
}

/// A gravação do `preserve`, com a operação e a trava de escrita do pack.
async fn preserve_impl(
    state: &AppState,
    cli: &Packwiz,
    pack_id: PackId,
    path: &str,
    preserve: bool,
) -> Result<bool, AppError> {
    let handle = state.operations.start(CONFIG_PRESERVE, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(state, pack_id)?;
        set_preserve(&root, path, preserve, cli, handle.token())
            .await
            .map_err(domain)
    }
    .await;
    handle.finish(result)
}

/// A cópia para `defaultconfigs/`, com a operação e a trava de escrita do pack.
async fn copy_impl(
    state: &AppState,
    cli: &Packwiz,
    pack_id: PackId,
    path: &str,
    overwrite: bool,
) -> Result<CopyOutcome, AppError> {
    let handle = state
        .operations
        .start(CONFIG_COPY_DEFAULTS, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(state, pack_id)?;
        copy_to_defaultconfigs(&root, path, overwrite, cli, handle.token())
            .await
            .map_err(domain)
    }
    .await;
    handle.finish(result)
}

// ---------------------------------------------------------------------------------------------
// Rótulos dos jars
// ---------------------------------------------------------------------------------------------

/// Um jar e o que identifica a versão dele no disco.
#[derive(Debug, Clone, PartialEq, Eq)]
struct JarFile {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}

/// Os `.jar` diretamente dentro das pastas (sem descer em subpastas nem seguir links).
fn jar_files(dirs: &[PathBuf]) -> Vec<JarFile> {
    let mut found = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_jar = path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"));
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if !is_jar || !kind.is_file() {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            found.push(JarFile {
                path,
                size: metadata.len(),
                modified: metadata.modified().ok(),
            });
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

type LabelCache = Mutex<HashMap<PathBuf, (JarFile, Arc<ConfigLang>)>>;

fn cache() -> &'static LabelCache {
    static CACHE: OnceLock<LabelCache> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// Os rótulos de todos os jars, lendo só os que não estão no cache (em paralelo). Um jar
/// ilegível fica sem rótulos; o formulário funciona do mesmo jeito.
fn labels_for(jars: &[JarFile]) -> ConfigLang {
    let missing: Vec<&JarFile> = {
        let cached = cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        jars.iter()
            .filter(|jar| cached.get(&jar.path).is_none_or(|(known, _)| known != *jar))
            .collect()
    };
    let read = read_all(&missing);
    let mut cached = cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    for (jar, lang) in read {
        cached.insert(jar.path.clone(), (jar.clone(), Arc::new(lang)));
    }
    let mut merged = ConfigLang::default();
    for jar in jars {
        if let Some((_, lang)) = cached.get(&jar.path) {
            merged.merge(lang.as_ref().clone());
        }
    }
    merged
}

fn read_one(path: &Path, limits: &Limits) -> ConfigLang {
    read_config_lang_file(path, limits).unwrap_or_else(|error| {
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned());
        tracing::warn!(%error, jar = ?name, "jar ilegível; o formulário fica sem os rótulos dele");
        ConfigLang::default()
    })
}

fn read_all<'j>(jars: &[&'j JarFile]) -> Vec<(&'j JarFile, ConfigLang)> {
    if jars.is_empty() {
        return Vec::new();
    }
    let threads = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .min(8);
    let chunk = jars.len().div_ceil(threads).max(1);
    let limits = Limits::default();
    std::thread::scope(|scope| {
        let handles: Vec<_> = jars
            .chunks(chunk)
            .map(|part| {
                let limits = &limits;
                scope.spawn(move || {
                    part.iter()
                        .map(|jar| (*jar, read_one(&jar.path, limits)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use warden_configs::meta::{FieldKey, FormItem, FormValue};
    use warden_project::ProjectErrorCode;

    use super::*;
    use crate::commands::inventory::tests::{packwiz_binary, registered_pack, test_packwiz};
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;

    fn jar(path: &Path, lang: &str) {
        let file = fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(
                "assets/inmis/lang/en_us.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(lang.as_bytes()).unwrap();
        writer.finish().unwrap();
    }

    #[test]
    fn rotulos_dos_jars_com_cache_por_data_e_tamanho() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        fs::create_dir_all(mods.join("sub")).unwrap();
        let path = mods.join("inmis.jar");
        jar(&path, r#"{"text.autoconfig.inmis.option.a": "Primeiro"}"#);
        fs::write(mods.join("notas.txt"), "x").unwrap();
        fs::write(mods.join("quebrado.jar"), "não é zip").unwrap();
        jar(&mods.join("sub/escondido.jar"), r#"{"text.autoconfig.inmis.option.b": "B"}"#);

        let jars = jar_files(&[mods.clone(), dir.path().join("falta")]);
        assert_eq!(jars.len(), 2);
        let labels = labels_for(&jars);
        assert_eq!(labels.get("text.autoconfig.inmis.option.a").unwrap().text, "Primeiro");
        assert_eq!(labels.get("text.autoconfig.inmis.option.b"), None);

        // Arquivo trocado (outro tamanho): relido.
        jar(&path, r#"{"text.autoconfig.inmis.option.a": "Segundo rótulo"}"#);
        let labels = labels_for(&jar_files(&[mods]));
        assert_eq!(
            labels.get("text.autoconfig.inmis.option.a").unwrap().text,
            "Segundo rótulo"
        );
    }

    #[tokio::test]
    async fn formulario_usa_os_rotulos_dos_jars_da_instancia() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let root = dir.path().join("pack");
        fs::create_dir_all(&root).unwrap();
        let record = warden_project::registry::PackRecord {
            id: PackId::new(),
            name: "Vale".into(),
            path: root,
            last_test: None,
            extra: std::collections::BTreeMap::new(),
        };
        let id = record.id;
        state.packs.insert(record).unwrap();
        let mods = state.paths.instance_dir(id).join("minecraft/mods");
        fs::create_dir_all(&mods).unwrap();
        jar(
            &mods.join("inmis.jar"),
            r#"{"text.autoconfig.inmis.option.enableTrinkets": "Enable Trinkets"}"#,
        );
        let jars = jar_files(&[mods]);
        let labels = labels_for(&jars);
        let form = build_form(
            "config/inmis.json",
            b"{\"enableTrinkets\": true}",
            Some(&labels),
        );
        let FormItem::Field(field) = &form.items[0] else {
            panic!("esperava um campo: {form:?}");
        };
        assert_eq!(field.label.as_ref().unwrap().text, "Enable Trinkets");

        let text = config_form_apply(
            "config/inmis.json".into(),
            "{\"enableTrinkets\": true}".into(),
            vec![FormEdit {
                path: vec![FieldKey::Name("enableTrinkets".into())],
                value: FormValue::Bool(false),
            }],
        )
        .unwrap();
        assert_eq!(text, "{\"enableTrinkets\": false}");
        let error = config_form_apply(
            "config/inmis.json".into(),
            "{\"a\": 1}".into(),
            vec![FormEdit {
                path: vec![FieldKey::Name("b".into())],
                value: FormValue::Integer("2".into()),
            }],
        )
        .unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::Configs(warden_configs::ConfigsErrorCode::KeyNotFound)
        );
    }

    #[tokio::test]
    async fn preserve_e_defaultconfigs_pela_trava_do_pack() {
        let Some(binary) = packwiz_binary() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cli = test_packwiz(&state, binary);
        let (id, root) = registered_pack(&state, dir.path(), &cli, 0).await;
        fs::write(root.join("options.txt"), "fov:0.5\n").unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("config/a-server.toml"), "a = 1\n").unwrap();
        warden_project::transaction::PackTransaction::new(root.clone())
            .commit(&cli, &warden_core::CancellationToken::new())
            .await
            .unwrap();

        assert!(!preserve_of(&root, "options.txt").unwrap().preserve);
        assert!(preserve_impl(&state, &cli, id, "options.txt", true).await.unwrap());
        assert!(!preserve_impl(&state, &cli, id, "options.txt", true).await.unwrap());
        assert!(preserve_of(&root, "options.txt").unwrap().preserve);

        let copied = copy_impl(&state, &cli, id, "config/a-server.toml", false)
            .await
            .unwrap();
        assert_eq!(
            copied,
            CopyOutcome::Copied {
                target: "defaultconfigs/a-server.toml".into()
            }
        );
        let error = copy_impl(&state, &cli, id, "options.txt", false)
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Project(ProjectErrorCode::InvalidInput));
        let error = preserve_impl(&state, &cli, PackId::new(), "options.txt", true)
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Project(ProjectErrorCode::PackNotFound));
    }
}
