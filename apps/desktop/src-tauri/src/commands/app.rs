//! Comandos do domínio `app`: informações, configurações e operações.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;
use warden_core::{AppPaths, CoreError, OperationId};

use crate::error::{AppError, AppErrorCode};
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

/// O que a interface precisa saber das configurações além do `settings.json` (T01, T21).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsStatus {
    /// O `settings.json` ainda não existe: o app abre na primeira execução (T01).
    pub(crate) first_run: bool,
    /// Pasta onde os packs novos são criados (a escolhida ou a padrão do sistema).
    pub(crate) packs_dir: PathBuf,
    /// Pasta padrão do sistema (Documentos\Warden).
    pub(crate) default_packs_dir: PathBuf,
}

impl SettingsStatus {
    fn of(state: &AppState) -> Self {
        let default_packs_dir = state.paths.default_packs_dir().to_path_buf();
        Self {
            first_run: !state.settings.exists(),
            packs_dir: state
                .settings
                .get()
                .packs_dir
                .unwrap_or_else(|| default_packs_dir.clone()),
            default_packs_dir,
        }
    }
}

/// Primeira execução e pasta dos packs em uso.
#[tauri::command]
#[specta::specta]
#[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn settings_status(state: State<'_, AppState>) -> Result<SettingsStatus, AppError> {
    Ok(SettingsStatus::of(&state))
}

/// Abre o diálogo nativo de pasta, valida a escolhida e a grava como pasta dos packs
/// (ARCHITECTURE §4.1: o caminho não vem da interface). `null` = o usuário desistiu.
#[tauri::command]
#[specta::specta]
pub(crate) async fn settings_choose_packs_dir(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Option<SettingsStatus>, AppError> {
    let current = SettingsStatus::of(&state).packs_dir;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Escolha a pasta dos packs")
        .set_parent(&window);
    if let Some(start) = nearest_existing_dir(&current) {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_folder(move |picked| {
        // Quem esperava pode ter ido embora (janela fechada); não há o que fazer.
        let _ = sender.send(picked.and_then(|path| path.as_path().map(Path::to_path_buf)));
    });
    let picked = receiver
        .await
        .map_err(|error| AppError::internal(format!("diálogo de pasta interrompido: {error}")))?;
    choose_packs_dir_impl(&state, picked)
}

fn choose_packs_dir_impl(
    state: &AppState,
    picked: Option<PathBuf>,
) -> Result<Option<SettingsStatus>, AppError> {
    let Some(picked) = picked else {
        return Ok(None);
    };
    let dir = validate_packs_dir(&picked, &state.paths)?;
    let stored = (dir != state.paths.default_packs_dir()).then_some(dir);
    state.settings.set_packs_dir(stored)?;
    tracing::info!("pasta dos packs alterada");
    Ok(Some(SettingsStatus::of(state)))
}

/// Por que uma pasta escolhida não serve para os packs (`params.reason` do
/// `app.SETTINGS_INVALID` com `field = packsDir`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PacksDirProblem {
    NotFound,
    NotDirectory,
    Symlink,
    InsideAppData,
    NotWritable,
}

impl PacksDirProblem {
    const fn reason(self) -> &'static str {
        match self {
            Self::NotFound => "notFound",
            Self::NotDirectory => "notDirectory",
            Self::Symlink => "symlink",
            Self::InsideAppData => "insideAppData",
            Self::NotWritable => "notWritable",
        }
    }

    fn into_error(self) -> AppError {
        AppError::new(AppErrorCode::SettingsInvalid)
            .with_param("field", "packsDir")
            .with_param("reason", self.reason())
    }
}

/// Confere a pasta escolhida: existe, é pasta de verdade (não link), fica fora das pastas do
/// próprio Warden e aceita gravação. Devolve o caminho absoluto.
fn validate_packs_dir(path: &Path, paths: &AppPaths) -> Result<PathBuf, AppError> {
    let path = std::path::absolute(path).map_err(|error| CoreError::io("ler", path, error))?;
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(PacksDirProblem::NotFound.into_error());
        }
        Err(error) => return Err(CoreError::io("ler", &path, error).into()),
    };
    if metadata.file_type().is_symlink() {
        return Err(PacksDirProblem::Symlink.into_error());
    }
    if !metadata.is_dir() {
        return Err(PacksDirProblem::NotDirectory.into_error());
    }
    if [paths.config_dir(), paths.data_dir()]
        .iter()
        .any(|dir| path.starts_with(dir))
    {
        return Err(PacksDirProblem::InsideAppData.into_error());
    }
    if !is_writable(&path) {
        return Err(PacksDirProblem::NotWritable.into_error());
    }
    Ok(path)
}

/// Tenta criar e apagar um arquivo vazio na pasta.
fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".warden-teste-de-escrita-{}", std::process::id()));
    let created = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .is_ok();
    if created && let Err(error) = std::fs::remove_file(&probe) {
        tracing::warn!(%error, "arquivo de teste de escrita não apagado");
    }
    created
}

/// A própria pasta ou a mais próxima acima dela que existe (o diálogo abre ali).
fn nearest_existing_dir(path: &Path) -> Option<&Path> {
    path.ancestors().find(|dir| dir.is_dir())
}

/// Abre a pasta dos registros no Explorador (Configurações → Privacidade e registros).
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn logs_reveal_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let dir = state.paths.logs_dir();
    std::fs::create_dir_all(&dir).map_err(|error| CoreError::io("criar a pasta", &dir, error))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|error| AppError::internal(format!("abrir a pasta de registros: {error}")))
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

    fn reason(error: &AppError) -> Option<&str> {
        error.params.get("reason").map(String::as_str)
    }

    #[test]
    fn status_diz_primeira_execucao_ate_gravar() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let status = SettingsStatus::of(&state);
        assert!(status.first_run);
        assert_eq!(status.packs_dir, state.paths.default_packs_dir());
        assert_eq!(status.default_packs_dir, state.paths.default_packs_dir());

        settings_update_impl(&state, SettingsPatch::default()).unwrap();
        assert!(!SettingsStatus::of(&state).first_run);
    }

    #[test]
    fn status_em_camel_case() {
        let status = SettingsStatus {
            first_run: true,
            packs_dir: PathBuf::from("packs"),
            default_packs_dir: PathBuf::from("padrao"),
        };
        let json = serde_json::to_value(status).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "firstRun": true,
                "packsDir": "packs",
                "defaultPacksDir": "padrao"
            })
        );
    }

    #[test]
    fn desistir_do_dialogo_nao_grava() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        assert_eq!(choose_packs_dir_impl(&state, None).unwrap(), None);
        assert!(!state.settings.exists());
    }

    #[test]
    fn pasta_valida_e_gravada_e_a_padrao_volta_a_null() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let chosen = dir.path().join("meus packs");
        std::fs::create_dir_all(&chosen).unwrap();

        let status = choose_packs_dir_impl(&state, Some(chosen.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(status.packs_dir, std::path::absolute(&chosen).unwrap());
        assert!(!status.first_run);
        let text = std::fs::read_to_string(state.paths.settings_file()).unwrap();
        assert!(text.contains("meus packs"), "{text}");
        // O teste de escrita não deixa rastro.
        assert_eq!(std::fs::read_dir(&chosen).unwrap().count(), 0);

        let default = state.paths.default_packs_dir().to_path_buf();
        std::fs::create_dir_all(&default).unwrap();
        let status = choose_packs_dir_impl(&state, Some(default.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(status.packs_dir, default);
        assert_eq!(state.settings.get().packs_dir, None);
    }

    #[test]
    fn pastas_que_nao_servem_sao_recusadas_sem_gravar() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let file = dir.path().join("arquivo.txt");
        std::fs::write(&file, b"x").unwrap();
        let inside = state.paths.data_dir().join("packs");
        std::fs::create_dir_all(&inside).unwrap();

        for (path, expected) in [
            (dir.path().join("nao-existe"), "notFound"),
            (file, "notDirectory"),
            (inside, "insideAppData"),
            (state.paths.config_dir().to_path_buf(), "insideAppData"),
        ] {
            let error = choose_packs_dir_impl(&state, Some(path.clone())).unwrap_err();
            assert_eq!(error.code, AppErrorCode::SettingsInvalid.into(), "{path:?}");
            assert_eq!(
                error.params.get("field").map(String::as_str),
                Some("packsDir")
            );
            assert_eq!(reason(&error), Some(expected), "{path:?}");
        }
        assert!(!state.settings.exists());
    }

    #[cfg(unix)]
    #[test]
    fn link_simbolico_e_recusado() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let target = dir.path().join("alvo");
        std::fs::create_dir_all(&target).unwrap();
        let link = dir.path().join("atalho");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let error = choose_packs_dir_impl(&state, Some(link)).unwrap_err();
        assert_eq!(reason(&error), Some("symlink"));
    }

    #[cfg(windows)]
    #[test]
    fn juncao_do_windows_e_recusada() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let target = dir.path().join("alvo");
        std::fs::create_dir_all(&target).unwrap();
        let link = dir.path().join("atalho");
        // Junção não exige privilégio de administrador (o link simbólico exige).
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .output()
            .unwrap();
        assert!(status.status.success(), "{status:?}");
        let error = choose_packs_dir_impl(&state, Some(link)).unwrap_err();
        assert_eq!(reason(&error), Some("symlink"));
    }

    #[test]
    fn dialogo_abre_na_pasta_existente_mais_proxima() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("a").join("b");
        assert_eq!(nearest_existing_dir(&missing), Some(dir.path()));
    }
}
