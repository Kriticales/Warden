//! Contrato IPC para Meus packs, criação e importação.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;
use warden_catalog::{MinecraftVersionKind, Refresh};
use warden_core::{CancellationToken, PackId};
use warden_java::{JavaChoiceRequest, LoaderKind, PackJavaInput};
use warden_packwiz_cli::Packwiz;
use warden_project::ProjectErrorCode;
use warden_project::create::{CreatePack, CreatedPack};
use warden_project::hygiene::HygieneFinding;
use warden_project::open::{ImportPreview, ImportedPack};
use warden_project::registry::{PackRow, Registry};

use crate::commands::java::JavaPackSource;
use crate::error::AppError;
use crate::state::AppState;

/// Em build de debug, o E2E troca o diálogo nativo de pasta (que o `WebDriver` não alcança) por
/// esta pasta fixa.
#[cfg(debug_assertions)]
const E2E_PICK_FOLDER_ENV: &str = "WARDEN_E2E_PICK_FOLDER";

fn domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

fn packwiz(state: &AppState) -> Result<Packwiz, AppError> {
    let binary = Packwiz::locate_binary().map_err(|e| AppError::from_domain(&e))?;
    Ok(Packwiz::new(
        binary,
        state.paths.packwiz_cache_dir(),
        state.paths.packwiz_config_file(),
    ))
}

fn registered_path(state: &AppState, id: PackId) -> Result<PathBuf, AppError> {
    state
        .packs
        .get(id)
        .map(|record| record.path)
        .map_err(domain)
}

/// Lista de packs registrados, inclusive pastas perdidas e manifestos inválidos.
#[tauri::command]
#[specta::specta]
pub(crate) fn packs_list(state: State<'_, AppState>) -> Vec<PackRow> {
    state.packs.list()
}

/// Dados de um pack da lista.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_get(state: State<'_, AppState>, pack_id: PackId) -> Result<PackRow, AppError> {
    state
        .packs
        .list()
        .into_iter()
        .find(|row| row.id == pack_id)
        .ok_or_else(|| AppError::new(ProjectErrorCode::PackNotFound))
}

/// O que o assistente Criar pack mostra antes de o usuário digitar (SPEC T03, etapa 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateDefaults {
    /// Autor padrão: o nome do jogador configurado.
    pub(crate) author: String,
    /// Pasta onde os packs novos são criados.
    pub(crate) packs_dir: PathBuf,
}

/// Autor e pasta padrão do assistente Criar pack.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_create_defaults(state: State<'_, AppState>) -> CreateDefaults {
    create_defaults(&state)
}

fn create_defaults(state: &AppState) -> CreateDefaults {
    let settings = state.settings.get();
    CreateDefaults {
        author: settings.player_name.clone(),
        packs_dir: settings
            .packs_dir
            .clone()
            .unwrap_or_else(|| state.paths.default_packs_dir().to_path_buf()),
    }
}

/// Etapa "Nome e pasta": confere o nome e devolve a pasta final, recusando pasta com arquivos
/// antes de qualquer escrita (CA-T03-04). `destination` vazio = `<pasta dos packs>/<nome>`.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_create_check(
    state: State<'_, AppState>,
    name: String,
    destination: Option<PathBuf>,
) -> Result<PathBuf, AppError> {
    let defaults = create_defaults(&state);
    warden_project::create::resolve_destination(&name, destination.as_deref(), &defaults.packs_dir)
        .map_err(domain)
}

/// Cria um pack vazio com versões confirmadas pelo catálogo e ponto inicial no histórico.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_create(
    state: State<'_, AppState>,
    mut request: CreatePack,
) -> Result<CreatedPack, AppError> {
    let settings = state.settings.get();
    if request.author.trim().is_empty() {
        request.author = settings.player_name.clone();
    }
    let default_dir = settings
        .packs_dir
        .as_deref()
        .unwrap_or(state.paths.default_packs_dir());
    warden_project::create::validate(&request, default_dir).map_err(domain)?;
    let minecraft = state
        .catalog
        .minecraft_versions(Refresh::IfStale, None)
        .await
        .map_err(|e| AppError::from_domain(&e))?;
    if !minecraft
        .versions
        .iter()
        .any(|v| v.id == request.minecraft && v.kind == MinecraftVersionKind::Release)
    {
        return Err(AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "minecraft"));
    }
    if let (Some(loader), Some(version)) = (request.loader, request.loader_version.as_deref()) {
        let listed = state
            .catalog
            .loader_versions(loader, &request.minecraft, Refresh::IfStale, None)
            .await
            .map_err(|e| AppError::from_domain(&e))?;
        if listed.get(version).is_none() {
            return Err(AppError::new(ProjectErrorCode::InvalidLoaderVersion)
                .with_param("version", version));
        }
    }
    let cli = packwiz(&state)?;
    warden_project::create::create(
        &request,
        default_dir,
        &state.packs,
        &cli,
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// Verifica uma pasta packwiz sem escrever nela.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_import_preview(path: PathBuf) -> Result<ImportPreview, AppError> {
    ensure_directory(&path)?;
    warden_project::open::preview(&path).map_err(domain)
}

/// Abre e registra uma pasta packwiz. `add_controls` aceita os arquivos de controle padrão.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_import(
    state: State<'_, AppState>,
    path: PathBuf,
    add_controls: bool,
) -> Result<ImportedPack, AppError> {
    ensure_directory(&path)?;
    warden_project::open::import(&path, add_controls, &state.packs)
        .await
        .map_err(domain)
}

/// Localiza a nova pasta de um pack perdido sem trocar seu histórico local.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_relocate(
    state: State<'_, AppState>,
    pack_id: PackId,
    path: PathBuf,
) -> Result<(), AppError> {
    ensure_directory(&path)?;
    warden_project::open::preview(&path).map_err(domain)?;
    state.packs.get(pack_id).map_err(domain)?;
    if warden_project::open::project_id(&path).map_err(domain)? != Some(pack_id) {
        return Err(AppError::new(ProjectErrorCode::InvalidPack)
            .with_param("reason", "otherPack")
            .with_detail("identificador da pasta não corresponde ao registro"));
    }
    state.packs.relocate(pack_id, path).map_err(domain)
}

/// Retira apenas do registro local.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_forget(state: State<'_, AppState>, pack_id: PackId) -> Result<(), AppError> {
    state.packs.forget(pack_id).map_err(domain)
}

/// Itens que não devem entrar no pack distribuído.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_hygiene_scan(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Vec<HygieneFinding>, AppError> {
    warden_project::hygiene::scan(&registered_path(&state, pack_id)?).map_err(domain)
}

/// Remove os itens escolhidos após criar ponto de segurança.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_hygiene_fix(
    state: State<'_, AppState>,
    pack_id: PackId,
    paths: Vec<String>,
) -> Result<Vec<String>, AppError> {
    let _lock = state.locks.write(pack_id).await;
    let root = registered_path(&state, pack_id)?;
    let author = warden_packwiz::read_pack(&root)
        .map_err(|e| AppError::from_domain(&e))?
        .pack
        .value
        .author;
    warden_project::hygiene::fix(
        &root,
        &paths,
        &author,
        &packwiz(&state)?,
        &CancellationToken::new(),
    )
    .await
    .map_err(domain)
}

/// Move um pack para a Lixeira depois de digitar o nome exato.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_trash(
    state: State<'_, AppState>,
    pack_id: PackId,
    confirmation: String,
) -> Result<(), AppError> {
    let _lock = state.locks.write(pack_id).await;
    warden_project::trash::trash_pack(&state.packs, pack_id, &confirmation).map_err(domain)
}

/// Para que a pasta está sendo escolhida: decide o título do diálogo e onde ele abre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FolderPurpose {
    /// Pasta de um pack novo (Criar pack).
    CreateDestination,
    /// Pasta packwiz existente (Abrir ou importar…).
    OpenPack,
    /// Nova pasta de um pack que sumiu (Localizar…).
    Relocate,
}

impl FolderPurpose {
    const fn title(self) -> &'static str {
        match self {
            Self::CreateDestination => "Escolha a pasta do pack novo",
            Self::OpenPack => "Escolha a pasta do pack (a que tem o pack.toml)",
            Self::Relocate => "Escolha a nova pasta do pack",
        }
    }
}

/// Abre o diálogo nativo de pasta (ARCHITECTURE §20: diálogos são do Rust). `null` = o usuário
/// desistiu. O caminho escolhido ainda passa pelas validações do comando que o usa.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pack_choose_folder(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    purpose: FolderPurpose,
) -> Result<Option<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os(E2E_PICK_FOLDER_ENV).filter(|value| !value.is_empty()) {
        return Ok(Some(PathBuf::from(path)));
    }
    let start = create_defaults(&state).packs_dir;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut dialog = app
        .dialog()
        .file()
        .set_title(purpose.title())
        .set_parent(&window);
    if let Some(start) = start.ancestors().find(|dir| dir.is_dir()) {
        dialog = dialog.set_directory(start);
    }
    dialog.pick_folder(move |picked| {
        // Quem esperava pode ter ido embora (janela fechada); não há o que fazer.
        let _ = sender.send(picked.and_then(|path| path.as_path().map(Path::to_path_buf)));
    });
    receiver
        .await
        .map_err(|error| AppError::internal(format!("diálogo de pasta interrompido: {error}")))
}

/// "Mostrar na pasta": abre o Explorador com a pasta do pack selecionada.
#[tauri::command]
#[specta::specta]
pub(crate) fn pack_reveal_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<(), AppError> {
    let path = registered_path(&state, pack_id)?;
    if !path.is_dir() {
        return Err(AppError::new(ProjectErrorCode::FolderMissing));
    }
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|error| AppError::internal(format!("mostrar a pasta do pack: {error}")))
}

/// Os packs registrados para a tabela de Java de Configurações (handoff da L-01). Lê só o
/// `pack.toml` de cada pack (sem o git, ao contrário de `packs_list`); pack com a pasta sumida
/// ou o manifesto ilegível fica de fora, porque não dá para saber o Minecraft dele.
pub(crate) struct RegistryJavaPacks(pub(crate) Arc<Registry>);

impl JavaPackSource for RegistryJavaPacks {
    fn packs(&self) -> Result<Vec<PackJavaInput>, AppError> {
        Ok(self
            .0
            .records()
            .into_iter()
            .filter_map(|record| {
                let pack = warden_packwiz::read_pack(&record.path).ok()?;
                let manifest = pack.pack.value;
                let minecraft = manifest.minecraft_version()?.to_owned();
                let loaders = manifest.loaders();
                let (loader, version) = match loaders.first() {
                    Some((loader, version)) => (LoaderKind::from(*loader), Some(*version)),
                    None => (LoaderKind::Vanilla, None),
                };
                Some(PackJavaInput {
                    pack_id: record.id,
                    name: manifest.name.clone(),
                    request: JavaChoiceRequest::automatic(&minecraft, loader, version),
                })
            })
            .collect())
    }
}

fn ensure_directory(path: &Path) -> Result<(), AppError> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(AppError::new(ProjectErrorCode::InvalidInput).with_param("field", "path"));
    }
    Ok(())
}
