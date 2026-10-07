//! Comandos do domínio `versioning`: Salvar versão (SPEC T16) e Histórico (SPEC T17; V-02).
//!
//! Finos: pegam a trava do pack, chamam a `warden-versioning` (em `spawn_blocking`, porque a
//! libgit2 é bloqueante) e a `PackTransaction` (que grava `pack.toml` e `CHANGELOG.md` e
//! atualiza o índice). O que o Warden grava no histórico é sempre local: nada vai ao GitHub
//! (isso é da V-03).
//!
//! - `history_get`: alterações não salvas e versões salvas, com o estado de cada uma.
//! - `version_save_preview` / `version_validate` / `version_save`: o diálogo Salvar versão.
//! - `version_set_final`: marcar e desmarcar a versão final (não muda nenhum arquivo do pack).
//! - `version_changes`: "Ver diferenças para o estado atual".
//! - `version_restore`: "Voltar para esta versão" (transacional, com ponto de segurança).
//! - `safety_points_list` / `safety_point_recover`: os pontos de segurança (P1).
//! - `unsaved_discard`: "Descartar" a alteração de um arquivo de config (P1).
//!
//! Os nomes legíveis das versões vêm do cache do Modrinth (a rede só é usada para o que o
//! cache não tem); versões da CurseForge e arquivos locais saem com o nome do arquivo.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use warden_core::PackId;
use warden_modrinth::ModrinthClient;
use warden_packwiz::edit::{PackTextField, set_pack_text};
use warden_project::ProjectErrorCode;
use warden_project::meta::read_meta;
use warden_project::transaction::PackTransaction;
use warden_versioning::{
    ChangeFacts, ChangeSet, FileNames, Identity, ItemChange, ItemChangeKind, Moment, PackRepo,
    RestoreReport, RestoreTarget, SafetyPoint, SaveVersion, SavedVersion, Snapshot,
    VersionNameResolver, VersionRef, VersionSuggestion, changelog, suggest_version,
};

use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::AppError;
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Salvar versão".
const SAVE: OperationKind = OperationKind::new("versioning.save");
/// "Voltar para esta versão" e "Recuperar" um ponto de segurança.
const RESTORE: OperationKind = OperationKind::new("versioning.restore");
/// "Descartar" a alteração de um arquivo.
const DISCARD: OperationKind = OperationKind::new("versioning.discard");

/// Maior fuso aceito (UTC+14).
const MAX_UTC_OFFSET_MINUTES: i32 = 14 * 60;

fn domain(error: warden_versioning::Error) -> AppError {
    AppError::from_domain(&error)
}

fn project_domain(error: warden_project::Error) -> AppError {
    AppError::from_domain(&error)
}

fn invalid(field: &str) -> AppError {
    AppError::new(ProjectErrorCode::InvalidInput).with_param("field", field)
}

/// Roda uma operação bloqueante da libgit2 fora do executor assíncrono.
async fn blocking<T, F>(work: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, warden_versioning::Error> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("operação de versões: {error}")))?
        .map_err(domain)
}

/// Agora, no fuso do computador (que a interface informa; `utc_offset_minutes` é o
/// deslocamento em relação ao UTC, positivo a leste).
fn moment_now(utc_offset_minutes: i32) -> Result<Moment, AppError> {
    if !(-MAX_UTC_OFFSET_MINUTES..=MAX_UTC_OFFSET_MINUTES).contains(&utc_offset_minutes) {
        return Err(invalid("utcOffsetMinutes"));
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        });
    Ok(Moment::new(seconds, utc_offset_minutes))
}

/// Quem assina as versões: o autor do `pack.toml` (o e-mail do GitHub chega com a V-03).
fn identity(root: &Path) -> Identity {
    let author = read_meta(root).map(|meta| meta.author).unwrap_or_default();
    Identity::for_pack(&author, None)
}

/// Nomes legíveis já resolvidos, para o `VersionNameResolver` da `warden-versioning`.
struct NameMap(HashMap<VersionRef, String>);

impl VersionNameResolver for NameMap {
    fn version_names(&self, versions: &[VersionRef]) -> Vec<Option<String>> {
        versions
            .iter()
            .map(|version| self.0.get(version).cloned())
            .collect()
    }
}

/// Itens do changelog que vêm do Modrinth.
fn modrinth_refs(changes: &ChangeSet) -> Vec<(String, VersionRef)> {
    changes
        .items
        .iter()
        .flat_map(|item| [item.old.as_ref(), item.new.as_ref()])
        .flatten()
        .filter_map(|version| match &version.reference {
            VersionRef::Modrinth { version_id, .. } => {
                Some((version_id.clone(), version.reference.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Versões do Modrinth pelo ID: cliente (cache primeiro); sem rede, só o cache.
async fn modrinth_version_numbers(
    client: &ModrinthClient,
    ids: &[String],
) -> HashMap<String, String> {
    if ids.is_empty() {
        return HashMap::new();
    }
    let versions = match client.versions(ids, None).await {
        Ok(versions) => versions,
        Err(_) => match client.cache() {
            Some(cache) => cache.versions(ids, false).await.unwrap_or_default(),
            None => Vec::new(),
        },
    };
    versions
        .into_iter()
        .filter(|version| !version.version_number.trim().is_empty())
        .map(|version| (version.id, version.version_number))
        .collect()
}

/// Projetos do Modrinth de geração de mundo (categoria `worldgen`) entre os itens
/// adicionados ou removidos. Sem rede nem cache, o conjunto sai vazio.
async fn worldgen_projects(client: &ModrinthClient, changes: &ChangeSet) -> HashSet<String> {
    let ids: Vec<String> = changes
        .items
        .iter()
        .filter(|item| matches!(item.kind, ItemChangeKind::Added | ItemChangeKind::Removed))
        .filter_map(|item| item.project_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    if ids.is_empty() {
        return HashSet::new();
    }
    let projects = match client.projects(&ids, None).await {
        Ok(projects) => projects,
        Err(_) => match client.cache() {
            Some(cache) => cache
                .projects(&ids)
                .await
                .map(|found| found.into_iter().map(|cached| cached.value).collect())
                .unwrap_or_default(),
            None => Vec::new(),
        },
    };
    projects
        .into_iter()
        .filter(|project| {
            project
                .categories
                .iter()
                .chain(&project.additional_categories)
                .any(|category| category == "worldgen")
        })
        .map(|project| project.id)
        .collect()
}

/// O que mudou entre dois estados do pack, com as versões dos itens do Modrinth em nome
/// legível (`1.4.2`) e as demais pelo nome do arquivo.
async fn changes_between(
    modrinth: Option<&ModrinthClient>,
    root: &Path,
    old: Snapshot,
    new: Snapshot,
) -> Result<ChangeSet, AppError> {
    let first = {
        let (root, old, new) = (root.to_path_buf(), old.clone(), new.clone());
        blocking(move || PackRepo::open(&root)?.changes(&old, &new, &FileNames)).await?
    };
    let refs = modrinth_refs(&first);
    let Some(modrinth) = modrinth else {
        return Ok(first);
    };
    if refs.is_empty() {
        return Ok(first);
    }
    let ids: Vec<String> = refs
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let numbers = modrinth_version_numbers(modrinth, &ids).await;
    if numbers.is_empty() {
        return Ok(first);
    }
    let names = NameMap(
        refs.into_iter()
            .filter_map(|(id, reference)| numbers.get(&id).map(|name| (reference, name.clone())))
            .collect(),
    );
    let root = root.to_path_buf();
    blocking(move || PackRepo::open(&root)?.changes(&old, &new, &names)).await
}

/// Estado do pack contra a última versão salva (ou contra o vazio, antes da primeira).
async fn unsaved_changes(
    modrinth: Option<&ModrinthClient>,
    root: &Path,
) -> Result<ChangeSet, AppError> {
    let base = {
        let root = root.to_path_buf();
        blocking(move || PackRepo::open(&root)?.last_version()).await?
    };
    let base = base.map_or(Snapshot::Empty, |version| {
        Snapshot::Version(version.to_string())
    });
    changes_between(modrinth, root, base, Snapshot::WorkingTree).await
}

// ---------------------------------------------------------------------------------------
// Histórico
// ---------------------------------------------------------------------------------------

/// O que a seção Histórico mostra.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryView {
    /// Alterações não salvas, contra a última versão salva.
    pub(crate) unsaved: ChangeSet,
    /// Última versão salva (`None` antes da primeira).
    pub(crate) last_version: Option<String>,
    /// Versões salvas da linha atual do pack, da mais nova para a mais antiga.
    pub(crate) versions: Vec<SavedVersion>,
}

/// Alterações não salvas e versões salvas do pack.
#[tauri::command]
#[specta::specta]
pub(crate) async fn history_get(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<HistoryView, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    history_view(Some(&state.modrinth), &root).await
}

async fn history_view(
    modrinth: Option<&ModrinthClient>,
    root: &Path,
) -> Result<HistoryView, AppError> {
    let (last_version, versions) = {
        let root = root.to_path_buf();
        blocking(move || {
            let repo = PackRepo::open(&root)?;
            let last = repo.last_version()?.map(|version| version.to_string());
            let mut versions = repo.versions()?;
            // Etiquetas de outras linhas de um pack importado não são o histórico deste pack.
            versions.retain(|version| version.reachable);
            Ok((last, versions))
        })
        .await?
    };
    let unsaved = unsaved_changes(modrinth, root).await?;
    Ok(HistoryView {
        unsaved,
        last_version,
        versions,
    })
}

/// "Ver diferenças para o estado atual": o que mudou da versão escolhida até agora.
#[tauri::command]
#[specta::specta]
pub(crate) async fn version_changes(
    state: State<'_, AppState>,
    pack_id: PackId,
    version: String,
) -> Result<ChangeSet, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    changes_between(
        Some(&state.modrinth),
        &root,
        Snapshot::Version(version),
        Snapshot::WorkingTree,
    )
    .await
}

// ---------------------------------------------------------------------------------------
// Salvar versão
// ---------------------------------------------------------------------------------------

/// O que o diálogo Salvar versão mostra antes de salvar.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavePreview {
    /// O que mudou desde a última versão salva.
    pub(crate) changes: ChangeSet,
    /// Versão sugerida e o motivo (`None` quando nada mudou).
    pub(crate) suggestion: Option<VersionSuggestion>,
    /// Changelog automático em Markdown (sem as notas do usuário).
    pub(crate) body: String,
    /// Última versão salva da linha atual.
    pub(crate) last_version: Option<String>,
    /// Maior versão já salva no repositório: a nova precisa ser maior que ela.
    pub(crate) highest_version: Option<String>,
    /// Há mods removidos que podem ter conteúdo nos mundos ("Atenção" no changelog).
    pub(crate) world_warning: bool,
}

/// Versão sugerida e changelog automático das alterações não salvas.
#[tauri::command]
#[specta::specta]
pub(crate) async fn version_save_preview(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<SavePreview, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    save_preview(Some(&state.modrinth), &root).await
}

async fn save_preview(
    modrinth: Option<&ModrinthClient>,
    root: &Path,
) -> Result<SavePreview, AppError> {
    let changes = unsaved_changes(modrinth, root).await?;
    let worldgen = match modrinth {
        Some(client) => worldgen_projects(client, &changes).await,
        None => HashSet::new(),
    };
    let facts = ChangeFacts::from_changes(&changes, |item: &ItemChange| {
        item.project_id
            .as_ref()
            .is_some_and(|id| worldgen.contains(id))
    });
    let pack_version = read_meta(root).map(|meta| meta.version).unwrap_or_default();
    let (last_version, highest_version) = {
        let root = root.to_path_buf();
        blocking(move || {
            let repo = PackRepo::open(&root)?;
            Ok((
                repo.last_version()?.map(|version| version.to_string()),
                repo.highest_version()?,
            ))
        })
        .await?
    };
    let suggestion = suggest_version(highest_version.as_ref(), &pack_version, &facts);
    let world_warning = changes.removed_world_mods().next().is_some();
    Ok(SavePreview {
        body: changelog::render_body("", &changes),
        world_warning,
        highest_version: highest_version.map(|version| version.to_string()),
        changes,
        suggestion,
        last_version,
    })
}

/// Confere se `version` pode ser salva agora: `SemVer` válido e maior que todas as já salvas
/// (CA-T16-04). O erro traz a explicação.
#[tauri::command]
#[specta::specta]
pub(crate) async fn version_validate(
    state: State<'_, AppState>,
    pack_id: PackId,
    version: String,
) -> Result<(), AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    blocking(move || {
        PackRepo::open(&root)?.validate_new_version(version.trim())?;
        Ok(())
    })
    .await
}

/// Pedido de "Salvar versão".
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveRequest {
    /// Número da versão (`1.5.0`).
    pub(crate) version: String,
    /// Notas do usuário, que entram no topo do changelog.
    pub(crate) notes: String,
    /// Já marcar como versão final.
    pub(crate) mark_final: bool,
    /// Fuso do computador em minutos a leste do UTC (a data do `CHANGELOG.md` é a de lá).
    pub(crate) utc_offset_minutes: i32,
}

/// Maior tamanho das notas, em caracteres.
const MAX_NOTES_CHARS: usize = 20_000;

/// Salva a versão: grava a versão no `pack.toml`, acrescenta a entrada no topo do
/// `CHANGELOG.md`, atualiza o índice e registra no histórico (commit + tag). Local: nada vai ao
/// GitHub.
#[tauri::command]
#[specta::specta]
pub(crate) async fn version_save(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    request: SaveRequest,
) -> Result<SavedVersion, AppError> {
    let cli = packwiz(&state)?;
    let when = moment_now(request.utc_offset_minutes)?;
    if request.notes.chars().count() > MAX_NOTES_CHARS {
        return Err(invalid("notes"));
    }
    let handle = state.operations.start(SAVE, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(&state, pack_id)?;
        save_impl(
            Some(&state.modrinth),
            &cli,
            &root,
            &request,
            when,
            handle.token(),
        )
        .await
    }
    .await;
    let result = handle.finish(result);
    notify_changed(&app, pack_id, vec![PackArea::History, PackArea::Meta]);
    result
}

async fn save_impl(
    modrinth: Option<&ModrinthClient>,
    cli: &warden_packwiz_cli::Packwiz,
    root: &Path,
    request: &SaveRequest,
    when: Moment,
    token: &warden_core::CancellationToken,
) -> Result<SavedVersion, AppError> {
    let version = request.version.trim().to_owned();
    // Confere tudo o que dá para conferir antes de mexer em qualquer arquivo.
    let parsed = {
        let (root, version) = (root.to_path_buf(), version.clone());
        blocking(move || {
            let repo = PackRepo::open(&root)?;
            repo.check_writable()?;
            repo.validate_new_version(&version)
        })
        .await?
    };
    let changes = unsaved_changes(modrinth, root).await?;
    if changes.is_empty() {
        let root = root.to_path_buf();
        let last = blocking(move || PackRepo::open(&root)?.last_version()).await?;
        return Err(domain(warden_versioning::Error::NothingChanged {
            last: last.map(|version| version.to_string()),
        }));
    }

    let version = parsed.to_string();
    let entry = changelog::render_entry(&version, &when.date(), &request.notes, &changes);
    let tag_message = changelog::render_body(&request.notes, &changes);

    // Os arquivos de controle antes e depois, para desfazer se o histórico falhar.
    let pack_text = std::fs::read_to_string(root.join("pack.toml"))
        .map_err(|error| AppError::internal(format!("ler o pack.toml: {error}")))?;
    let changelog_before = match std::fs::read(root.join("CHANGELOG.md")) {
        Ok(bytes) => Some(String::from_utf8(bytes).map_err(|_| {
            AppError::new(ProjectErrorCode::InvalidPack)
                .with_detail("o CHANGELOG.md não está em UTF-8")
        })?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(AppError::internal(format!("ler o CHANGELOG.md: {error}")));
        }
    };
    let manifest_version = read_meta(root).map_err(project_domain)?.version;

    let mut transaction = PackTransaction::new(root.to_path_buf());
    transaction
        .write(
            "CHANGELOG.md",
            changelog::prepend_entry(changelog_before.as_deref(), &entry).into_bytes(),
        )
        .map_err(project_domain)?;
    if manifest_version != version {
        let edited =
            set_pack_text(&pack_text, PackTextField::Version, &version).map_err(|error| {
                AppError::new(ProjectErrorCode::InvalidPack).with_detail(error.to_string())
            })?;
        transaction
            .write("pack.toml", edited.into_bytes())
            .map_err(project_domain)?;
    }
    transaction
        .commit(cli, token)
        .await
        .map_err(project_domain)?;

    let saved = {
        let root = root.to_path_buf();
        let identity = identity(&root);
        let request = request.clone();
        blocking(move || {
            PackRepo::open(&root)?.save_version(&SaveVersion {
                version: &version,
                tag_message: &tag_message,
                identity: &identity,
                when,
                mark_final: request.mark_final,
            })
        })
        .await
    };
    match saved {
        Ok(saved) => Ok(saved),
        Err(error) => {
            // O histórico não aceitou: devolve o `pack.toml` e o `CHANGELOG.md` ao que eram, para
            // uma nova tentativa não repetir a entrada.
            let mut undo = PackTransaction::new(root.to_path_buf());
            let restored = match &changelog_before {
                Some(text) => undo.write("CHANGELOG.md", text.clone().into_bytes()),
                None => undo.delete("CHANGELOG.md"),
            }
            .and_then(|()| undo.write("pack.toml", pack_text.into_bytes()));
            if restored.is_ok()
                && let Err(undo_error) = undo.commit_without_refresh().await
            {
                tracing::warn!(%undo_error, "não foi possível desfazer a entrada do CHANGELOG.md");
            }
            Err(error)
        }
    }
}

// ---------------------------------------------------------------------------------------
// Versão final
// ---------------------------------------------------------------------------------------

/// Marca ou desmarca uma versão salva como versão final. Não muda nenhum arquivo do pack
/// (CA-T16-05). Uma versão já publicada não pode deixar de ser final.
#[tauri::command]
#[specta::specta]
pub(crate) async fn version_set_final(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    version: String,
    is_final: bool,
) -> Result<(), AppError> {
    let result = async {
        let _lock = state.locks.write(pack_id).await;
        let root = pack_root(&state, pack_id)?;
        blocking(move || {
            let repo = PackRepo::open(&root)?;
            repo.check_writable()?;
            repo.set_final(&version, is_final)
        })
        .await
    }
    .await;
    notify_changed(&app, pack_id, vec![PackArea::History]);
    result
}

// ---------------------------------------------------------------------------------------
// Voltar para uma versão e pontos de segurança
// ---------------------------------------------------------------------------------------

/// Volta o pack ao destino, como operação em Tarefas, com a trava de escrita.
async fn restore_core(
    state: &AppState,
    pack_id: PackId,
    target: RestoreTarget,
    when: Moment,
) -> Result<RestoreReport, AppError> {
    let handle = state.operations.start(RESTORE, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(state, pack_id)?;
        let report = {
            let root = root.clone();
            let identity = identity(&root);
            blocking(move || PackRepo::open(&root)?.restore(&target, &identity, when)).await?
        };
        // O nome do pack pode ser outro na versão de destino: o registro local acompanha.
        if let Ok(meta) = read_meta(&root) {
            let _ = state.packs.rename(pack_id, &meta.name);
        }
        Ok(report)
    }
    .await;
    handle.finish(result)
}

async fn restore_impl(
    app: &AppHandle,
    state: &AppState,
    pack_id: PackId,
    target: RestoreTarget,
    utc_offset_minutes: i32,
) -> Result<RestoreReport, AppError> {
    let when = moment_now(utc_offset_minutes)?;
    let result = restore_core(state, pack_id, target, when).await;
    // Volta tudo: itens, configs, informações e histórico.
    notify_changed(app, pack_id, Vec::new());
    result
}

/// "Voltar para esta versão": o pack passa a ficar igual à versão escolhida (arquivos que não
/// existiam nela são removidos). O estado de antes fica num ponto de segurança; o histórico
/// não é apagado.
#[tauri::command]
#[specta::specta]
pub(crate) async fn version_restore(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    version: String,
    utc_offset_minutes: i32,
) -> Result<RestoreReport, AppError> {
    restore_impl(
        &app,
        &state,
        pack_id,
        RestoreTarget::Version(version),
        utc_offset_minutes,
    )
    .await
}

/// Pontos de segurança do pack, do mais novo para o mais antigo.
#[tauri::command]
#[specta::specta]
pub(crate) async fn safety_points_list(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Vec<SafetyPoint>, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    blocking(move || PackRepo::open(&root)?.safety_points()).await
}

/// "Recuperar" um ponto de segurança: o pack volta ao estado guardado (e o estado de agora vira
/// outro ponto de segurança).
#[tauri::command]
#[specta::specta]
pub(crate) async fn safety_point_recover(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    name: String,
    utc_offset_minutes: i32,
) -> Result<RestoreReport, AppError> {
    restore_impl(
        &app,
        &state,
        pack_id,
        RestoreTarget::SafetyPoint(name),
        utc_offset_minutes,
    )
    .await
}

// ---------------------------------------------------------------------------------------
// Descartar a alteração de um arquivo
// ---------------------------------------------------------------------------------------

/// "Descartar" a alteração não salva de um arquivo de config: ele volta ao que era na última
/// versão salva (ou é apagado, se não existia nela). Só vale para configs; itens (mods,
/// resource packs e shaders) se desfazem pelo Mods ou voltando a uma versão.
#[tauri::command]
#[specta::specta]
pub(crate) async fn unsaved_discard(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    path: String,
) -> Result<(), AppError> {
    let cli = packwiz(&state)?;
    let result = discard_core(&state, &cli, pack_id, &path).await;
    notify_changed(&app, pack_id, vec![PackArea::Configs, PackArea::History]);
    result
}

async fn discard_core(
    state: &AppState,
    cli: &warden_packwiz_cli::Packwiz,
    pack_id: PackId,
    path: &str,
) -> Result<(), AppError> {
    let handle = state.operations.start(DISCARD, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let root = pack_root(state, pack_id)?;
        let (last, changes) = {
            let root = root.clone();
            blocking(move || {
                let repo = PackRepo::open(&root)?;
                repo.check_writable()?;
                let last = repo.last_version()?;
                let base = last.as_ref().map_or(Snapshot::Empty, |version| {
                    Snapshot::Version(version.to_string())
                });
                let changes = repo.changes(&base, &Snapshot::WorkingTree, &FileNames)?;
                Ok((last, changes))
            })
            .await?
        };
        // Sem versão salva não há para onde voltar, e só configs podem ser descartadas.
        let Some(last) = last else {
            return Err(invalid("path"));
        };
        if !changes.configs.iter().any(|config| config == path) {
            return Err(invalid("path"));
        }
        let original = {
            let (root, path) = (root.clone(), path.to_owned());
            let snapshot = Snapshot::Version(last.to_string());
            blocking(move || PackRepo::open(&root)?.read_file_at(&snapshot, &path)).await?
        };
        let mut transaction = PackTransaction::new(root);
        match original {
            Some(bytes) => transaction.write(path, bytes),
            None => transaction.delete(path),
        }
        .map_err(project_domain)?;
        transaction
            .commit(cli, handle.token())
            .await
            .map(|_| ())
            .map_err(project_domain)
    }
    .await;
    handle.finish(result)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;
    use warden_core::CancellationToken;
    use warden_packwiz_cli::Packwiz;
    use warden_versioning::{InitialPoint, SafetyReason};

    use super::*;
    use crate::commands::inventory::tests::{packwiz_binary, registered_pack, test_packwiz};
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;

    struct Fixture {
        _dir: TempDir,
        state: AppState,
        cli: Packwiz,
        id: PackId,
        root: PathBuf,
    }

    fn when() -> Moment {
        // 2026-09-30 14:00 em Brasília.
        Moment::new(1_790_787_600, -180)
    }

    /// Um pack com `count` mods do Modrinth, registrado e com o repositório criado ("Pack criado").
    async fn fixture(count: usize) -> Option<Fixture> {
        let binary = packwiz_binary()?;
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cli = test_packwiz(&state, binary);
        let (id, root) = registered_pack(&state, dir.path(), &cli, count).await;
        PackRepo::init(
            &root,
            InitialPoint::Created,
            &Identity::for_pack("Kriticales", None),
            when(),
        )
        .unwrap();
        Some(Fixture {
            _dir: dir,
            state,
            cli,
            id,
            root,
        })
    }

    fn request(version: &str, notes: &str, mark_final: bool) -> SaveRequest {
        SaveRequest {
            version: version.to_owned(),
            notes: notes.to_owned(),
            mark_final,
            utc_offset_minutes: -180,
        }
    }

    impl Fixture {
        async fn refresh(&self) {
            PackTransaction::new(self.root.clone())
                .commit(&self.cli, &CancellationToken::new())
                .await
                .unwrap();
        }

        /// Grava (ou troca a versão de) o mod `n`: o projeto é o mesmo, a versão e o arquivo mudam.
        async fn write_mod(&self, n: usize, release: &str) {
            let file = warden_packwiz::ModrinthFile {
                title: format!("Mod {n}"),
                project_id: format!("PROJ{n:04}"),
                version_id: format!("VERS{n:04}-{release}"),
                filename: format!("mod-{n}-{release}.jar"),
                url: format!("https://cdn.modrinth.com/data/PROJ{n:04}/mod-{n}-{release}.jar"),
                hash_format: warden_packwiz::HashFormat::Sha512,
                hash: format!("{n:02x}").repeat(64),
            };
            let metafile = warden_packwiz::Metafile::modrinth(&file, warden_packwiz::Side::Both);
            fs::write(
                self.root.join(format!("mods/mod-{n}.pw.toml")),
                metafile.to_toml_string(),
            )
            .unwrap();
            self.refresh().await;
        }

        async fn write_config(&self, path: &str, text: &str) {
            let file = self.root.join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
            self.refresh().await;
        }

        async fn save(
            &self,
            version: &str,
            notes: &str,
            mark_final: bool,
        ) -> Result<SavedVersion, AppError> {
            save_impl(
                None,
                &self.cli,
                &self.root,
                &request(version, notes, mark_final),
                when(),
                &CancellationToken::new(),
            )
            .await
        }

        fn repo(&self) -> PackRepo {
            PackRepo::open(&self.root).unwrap()
        }

        /// Todos os arquivos da pasta (menos o `.git`), com o conteúdo.
        fn tree(&self) -> BTreeMap<String, Vec<u8>> {
            fn walk(
                dir: &std::path::Path,
                root: &std::path::Path,
                out: &mut BTreeMap<String, Vec<u8>>,
            ) {
                for entry in fs::read_dir(dir).unwrap() {
                    let path = entry.unwrap().path();
                    if path.file_name().is_some_and(|name| name == ".git") {
                        continue;
                    }
                    if path.is_dir() {
                        walk(&path, root, out);
                    } else {
                        let relative = path
                            .strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/");
                        out.insert(relative, fs::read(&path).unwrap());
                    }
                }
            }
            let mut out = BTreeMap::new();
            walk(&self.root, &self.root, &mut out);
            out
        }
    }

    /// Linhas de item (`- …`) de um trecho do changelog.
    fn bullets(text: &str) -> Vec<&str> {
        text.lines()
            .filter_map(|line| line.strip_prefix("- "))
            .collect()
    }

    #[tokio::test]
    async fn ca_t16_03_salvar_grava_versao_changelog_e_deixa_o_historico_limpo() {
        let Some(f) = fixture(2).await else {
            return;
        };
        let saved = f.save("1.0.0", "Primeira versão", false).await.unwrap();
        assert_eq!(saved.version, "1.0.0");
        assert!(!saved.is_final && !saved.is_published);

        // O pack.toml tem a versão nova e o CHANGELOG.md, a entrada no topo, com as notas.
        assert_eq!(read_meta(&f.root).unwrap().version, "1.0.0");
        let log = fs::read_to_string(f.root.join("CHANGELOG.md")).unwrap();
        assert!(log.starts_with("# Changelog\n\n## 1.0.0 — 2026-09-30\n\nPrimeira versão\n"));
        assert!(log.contains("### Mods adicionados"));
        assert!(log.contains("- Mod 0 mod-0-1.0.0.jar"));

        // `git status` limpo e a versão no histórico, com o changelog na tag.
        assert!(f.repo().unsaved_changes().unwrap().is_empty());
        let view = history_view(None, &f.root).await.unwrap();
        assert_eq!(view.last_version.as_deref(), Some("1.0.0"));
        assert_eq!(view.versions.len(), 1);
        assert!(view.versions[0].message.contains("Primeira versão"));
        assert!(view.versions[0].message.contains("### Mods adicionados"));
        assert!(view.unsaved.files.is_empty());

        // Um `packwiz refresh` depois de salvar não muda nada: o índice já estava certo.
        f.refresh().await;
        assert!(f.repo().unsaved_changes().unwrap().is_empty());
        // Salvar é local: o repositório não tem nenhum remoto.
        let config = fs::read_to_string(f.root.join(".git/config")).unwrap();
        assert!(!config.contains("[remote"));
    }

    #[tokio::test]
    async fn ca_t16_01_dois_mods_novos_e_uma_atualizacao_sugerem_menor_e_listam_so_isso() {
        let Some(f) = fixture(2).await else {
            return;
        };
        f.save("1.0.0", "", false).await.unwrap();
        f.write_mod(10, "1.0.0").await;
        f.write_mod(11, "1.0.0").await;
        f.write_mod(0, "2.0.0").await;

        let preview = save_preview(None, &f.root).await.unwrap();
        let suggestion = preview.suggestion.unwrap();
        assert_eq!(suggestion.version, "1.1.0");
        assert_eq!(suggestion.bump, Some(warden_versioning::Bump::Minor));
        assert_eq!(preview.last_version.as_deref(), Some("1.0.0"));
        assert_eq!(preview.highest_version.as_deref(), Some("1.0.0"));
        assert!(!preview.world_warning);
        let added = preview
            .changes
            .items_of(ItemChangeKind::Added)
            .map(|item| item.name.clone())
            .collect::<Vec<_>>();
        assert_eq!(added, ["Mod 10", "Mod 11"]);
        let updated = preview
            .changes
            .items_of(ItemChangeKind::Updated)
            .map(|item| item.name.clone())
            .collect::<Vec<_>>();
        assert_eq!(updated, ["Mod 0"]);
        assert_eq!(preview.changes.items.len(), 3);
        // Exatamente os três no changelog (sem o nome legível: o teste não usa a rede).
        let lines = bullets(&preview.body);
        assert_eq!(lines.len(), 3, "{}", preview.body);
        assert!(lines.contains(&"Mod 0 mod-0-1.0.0.jar → mod-0-2.0.0.jar"));
    }

    #[tokio::test]
    async fn a_versao_legivel_do_item_vem_do_resolvedor() {
        let Some(f) = fixture(1).await else {
            return;
        };
        f.save("1.0.0", "", false).await.unwrap();
        f.write_mod(0, "2.0.0").await;
        let first = f
            .repo()
            .changes(
                &Snapshot::Version("1.0.0".into()),
                &Snapshot::WorkingTree,
                &FileNames,
            )
            .unwrap();
        let refs = modrinth_refs(&first);
        let ids: Vec<&str> = refs.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["VERS0000", "VERS0000-2.0.0"]);

        let readable = NameMap(
            refs.into_iter()
                .map(|(id, reference)| {
                    let label = if id.ends_with("2.0.0") {
                        "0.6.0"
                    } else {
                        "0.5.1"
                    };
                    (reference, label.to_owned())
                })
                .collect(),
        );
        let resolved = f
            .repo()
            .changes(
                &Snapshot::Version("1.0.0".into()),
                &Snapshot::WorkingTree,
                &readable,
            )
            .unwrap();
        let body = changelog::render_body("", &resolved);
        assert!(body.contains("- Mod 0 0.5.1 → 0.6.0"), "{body}");
    }

    #[tokio::test]
    async fn ca_t16_02_remover_mod_de_lado_ambos_sugere_maior_e_inclui_atencao() {
        let Some(f) = fixture(3).await else {
            return;
        };
        f.save("1.0.0", "", false).await.unwrap();
        fs::remove_file(f.root.join("mods/mod-1.pw.toml")).unwrap();
        f.refresh().await;

        let preview = save_preview(None, &f.root).await.unwrap();
        assert_eq!(preview.suggestion.unwrap().version, "2.0.0");
        assert!(preview.world_warning);
        assert!(preview.body.contains("### Atenção"), "{}", preview.body);

        f.save("2.0.0", "", false).await.unwrap();
        let log = fs::read_to_string(f.root.join("CHANGELOG.md")).unwrap();
        assert!(log.contains("### Atenção"));
        assert!(log.contains("### Mods removidos"));
    }

    #[tokio::test]
    async fn sem_mudancas_nao_ha_sugestao_e_salvar_recusa() {
        let Some(f) = fixture(1).await else {
            return;
        };
        f.save("1.0.0", "", false).await.unwrap();
        let preview = save_preview(None, &f.root).await.unwrap();
        assert!(preview.suggestion.is_none());
        assert!(preview.changes.is_empty());

        let before = f.tree();
        let error = f.save("1.0.1", "", false).await.unwrap_err();
        assert_eq!(
            error.code,
            ErrorCode::Versioning(warden_versioning::VersioningErrorCode::NothingChanged)
        );
        assert_eq!(error.params["last"], "1.0.0");
        assert_eq!(f.tree(), before);
    }

    #[tokio::test]
    async fn ca_t16_04_versao_menor_ou_igual_a_ultima_e_recusada_sem_tocar_em_nada() {
        let Some(f) = fixture(1).await else {
            return;
        };
        f.save("1.2.0", "", false).await.unwrap();
        f.write_mod(5, "1.0.0").await;
        let before = f.tree();

        for (version, code) in [
            (
                "1.0.0",
                warden_versioning::VersioningErrorCode::VersionNotGreater,
            ),
            (
                "1.2.0",
                warden_versioning::VersioningErrorCode::VersionExists,
            ),
            (
                "um.dois",
                warden_versioning::VersioningErrorCode::InvalidVersion,
            ),
        ] {
            let error = f.save(version, "", false).await.unwrap_err();
            assert_eq!(error.code, ErrorCode::Versioning(code), "{version}");
            assert_eq!(f.tree(), before, "{version} não pode mudar nenhum arquivo");
        }
        // A explicação da recusa traz os dois números (o frontend monta a frase).
        let error = f.save("1.0.0", "", false).await.unwrap_err();
        assert_eq!(error.params["version"], "1.0.0");
        assert_eq!(error.params["last"], "1.2.0");
    }

    #[tokio::test]
    async fn ca_t16_05_versao_final_nao_publica_nada_e_desmarcar_nao_muda_arquivos() {
        let Some(f) = fixture(1).await else {
            return;
        };
        let saved = f.save("1.0.0", "", true).await.unwrap();
        assert!(saved.is_final && !saved.is_published);
        let view = history_view(None, &f.root).await.unwrap();
        assert!(view.versions[0].is_final && !view.versions[0].is_published);

        let before = f.tree();
        f.repo().set_final("1.0.0", false).unwrap();
        assert!(!f.repo().versions().unwrap()[0].is_final);
        f.repo().set_final("1.0.0", true).unwrap();
        f.repo().set_final("1.0.0", false).unwrap();
        assert_eq!(f.tree(), before, "marcar e desmarcar não mexe no pack");
        let config = fs::read_to_string(f.root.join(".git/config")).unwrap();
        assert!(!config.contains("[remote"), "nada vai para o GitHub");
    }

    #[tokio::test]
    async fn falha_no_historico_devolve_pack_toml_e_changelog_ao_que_eram() {
        let Some(f) = fixture(1).await else {
            return;
        };
        let before = f.tree();
        // Uma trava na tag faz o historico recusar depois de os arquivos já terem sido gravados.
        let lock = f.root.join(".git/refs/tags/v1.0.0.lock");
        fs::create_dir_all(lock.parent().unwrap()).unwrap();
        fs::write(&lock, "").unwrap();

        assert!(f.save("1.0.0", "", false).await.is_err());
        assert_eq!(f.tree(), before, "nenhum arquivo do pack pode ter mudado");
        assert!(!f.root.join("CHANGELOG.md").exists());
        assert_eq!(f.repo().last_version().unwrap(), None);

        // Sem a trava, salvar funciona e o changelog tem uma entrada só.
        fs::remove_file(&lock).unwrap();
        f.save("1.0.0", "", false).await.unwrap();
        let log = fs::read_to_string(f.root.join("CHANGELOG.md")).unwrap();
        assert_eq!(log.matches("## 1.0.0").count(), 1);
    }

    #[tokio::test]
    async fn ca_t17_voltar_restaura_a_arvore_e_o_ponto_de_seguranca_recupera_o_estado_anterior() {
        let Some(f) = fixture(2).await else {
            return;
        };
        f.write_config("config/a.toml", "a = 1\n").await;
        f.save("1.0.0", "", false).await.unwrap();
        let at_1_0_0 = f.tree();

        f.write_mod(10, "1.0.0").await;
        f.write_config("config/a.toml", "a = 2\n").await;
        f.save("1.1.0", "", true).await.unwrap();
        // Alterações não salvas por cima: um arquivo novo e um mod a menos.
        f.write_config("config/novo.toml", "x = 1\n").await;
        fs::remove_file(f.root.join("mods/mod-1.pw.toml")).unwrap();
        f.refresh().await;
        let before_restore = f.tree();
        assert_ne!(before_restore, at_1_0_0);

        // CA-T17-01: a árvore fica igual à da 1.0.0, inclusive sem os arquivos criados depois.
        let report = restore_core(
            &f.state,
            f.id,
            RestoreTarget::Version("1.0.0".into()),
            when(),
        )
        .await
        .unwrap();
        assert_eq!(f.tree(), at_1_0_0);
        assert!(!f.root.join("config/novo.toml").exists());
        assert!(!f.root.join("mods/mod-10.pw.toml").exists());
        assert!(report.deleted.contains(&"config/novo.toml".to_owned()));
        assert_eq!(report.safety_point.reason, "antes de voltar para 1.0.0");
        // O histórico não foi apagado: as duas versões continuam lá.
        let view = history_view(None, &f.root).await.unwrap();
        assert_eq!(
            view.versions
                .iter()
                .map(|v| v.version.as_str())
                .collect::<Vec<_>>(),
            ["1.1.0", "1.0.0"]
        );
        // Voltar aparece em Tarefas.
        let kinds: Vec<String> = f
            .state
            .operations
            .list()
            .into_iter()
            .map(|operation| operation.kind.as_str().to_owned())
            .collect();
        assert!(kinds.contains(&"versioning.restore".to_owned()));

        // CA-T17-03: o estado de antes se recupera pelo ponto de segurança.
        let points = f.repo().safety_points().unwrap();
        assert_eq!(points.len(), 1);
        restore_core(
            &f.state,
            f.id,
            RestoreTarget::SafetyPoint(points[0].name.clone()),
            when(),
        )
        .await
        .unwrap();
        assert_eq!(f.tree(), before_restore);
    }

    #[tokio::test]
    async fn salvar_depois_de_voltar_cria_uma_versao_nova_a_partir_dali() {
        let Some(f) = fixture(2).await else {
            return;
        };
        f.save("1.0.0", "", false).await.unwrap();
        f.write_mod(10, "1.0.0").await;
        f.save("1.1.0", "", false).await.unwrap();
        restore_core(
            &f.state,
            f.id,
            RestoreTarget::Version("1.0.0".into()),
            when(),
        )
        .await
        .unwrap();

        // Voltar não move a linha do tempo: o que mudou é "mod 10 saiu" contra a 1.1.0.
        let preview = save_preview(None, &f.root).await.unwrap();
        assert_eq!(preview.highest_version.as_deref(), Some("1.1.0"));
        let suggested = preview.suggestion.unwrap().version;
        assert_eq!(suggested, "2.0.0");
        // 1.0.1 não passa: precisa ser maior que a mais nova já salva.
        assert!(f.save("1.0.1", "", false).await.is_err());
        f.save(&suggested, "", false).await.unwrap();
    }

    #[tokio::test]
    async fn descartar_volta_a_config_ao_que_era_e_recusa_o_que_nao_e_config() {
        let Some(f) = fixture(1).await else {
            return;
        };
        f.write_config("config/a.toml", "a = 1\n").await;
        // Antes da primeira versão não há para onde voltar.
        let error = discard_core(&f.state, &f.cli, f.id, "config/a.toml")
            .await
            .unwrap_err();
        assert_eq!(error.params["field"], "path");

        f.save("1.0.0", "", false).await.unwrap();
        let saved = f.tree();
        f.write_config("config/a.toml", "a = 99\n").await;
        f.write_config("config/novo.toml", "n = 1\n").await;

        discard_core(&f.state, &f.cli, f.id, "config/a.toml")
            .await
            .unwrap();
        assert_eq!(
            fs::read_to_string(f.root.join("config/a.toml")).unwrap(),
            "a = 1\n"
        );
        discard_core(&f.state, &f.cli, f.id, "config/novo.toml")
            .await
            .unwrap();
        assert!(!f.root.join("config/novo.toml").exists());
        // Voltou tudo ao que era na 1.0.0, índice incluído.
        assert_eq!(f.tree(), saved);
        assert!(f.repo().unsaved_changes().unwrap().is_empty());

        // Item (mod), arquivo sem alteração e caminho fora do pack não são descartáveis.
        for path in [
            "mods/mod-0.pw.toml",
            "config/a.toml",
            "../fora.txt",
            "pack.toml",
        ] {
            let error = discard_core(&f.state, &f.cli, f.id, path)
                .await
                .unwrap_err();
            assert_eq!(error.params["field"], "path", "{path}");
        }
    }

    #[tokio::test]
    async fn pontos_de_seguranca_de_outras_acoes_aparecem_na_lista() {
        let Some(f) = fixture(1).await else {
            return;
        };
        f.repo()
            .create_safety_point(
                &SafetyReason::RemoveItems { count: 2 },
                &Identity::for_pack("", None),
                when(),
            )
            .unwrap();
        let points = blocking({
            let root = f.root.clone();
            move || PackRepo::open(&root)?.safety_points()
        })
        .await
        .unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].reason, "antes de remover 2 itens");
    }

    #[test]
    fn o_fuso_fora_de_24_horas_e_recusado() {
        assert!(moment_now(-180).is_ok());
        assert!(moment_now(14 * 60).is_ok());
        assert!(moment_now(-14 * 60).is_ok());
        for bad in [15 * 60, -15 * 60, i32::MAX] {
            let error = moment_now(bad).unwrap_err();
            assert_eq!(error.params["field"], "utcOffsetMinutes");
        }
        assert_eq!(moment_now(-180).unwrap().offset_minutes, -180);
    }

    #[test]
    fn a_identidade_usa_o_autor_do_pack_ou_warden() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = {
            let mut manifest = warden_packwiz::PackManifest::new("Vale", "1.20.1");
            manifest.author = "Kriticales".into();
            manifest
        };
        fs::write(dir.path().join("pack.toml"), manifest.to_toml_string()).unwrap();
        assert_eq!(identity(dir.path()).name, "Kriticales");
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(identity(empty.path()).name, "Warden");
    }
}
