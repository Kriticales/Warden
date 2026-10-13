//! Comandos do domínio `capture` (ARCHITECTURE §4.1 e §8.3; SPEC T15; C-03): "O que mudou
//! durante o teste", no resultado do teste.
//!
//! - `capture_get`: a captura do último teste com o lado do pack lido agora (conflitos).
//! - `capture_identify`: os mods, resource packs e shaders colocados à mão, procurados pelo
//!   hash no Modrinth e, com a chave, na CurseForge (o arquivo nunca é copiado; a tela oferece
//!   adicionar pelo caminho normal, CA-T15-04).
//! - `capture_texts`: as três versões de um arquivo (início do teste, pack agora, jogo agora),
//!   para "Ver diferença de texto" e "Editar e combinar".
//! - `capture_apply`: "Trazer selecionados para o pack": só o escolhido, numa `PackTransaction`
//!   (atômica, com `packwiz refresh`); o `options.txt` chave a chave. Depois, o que ficou
//!   diferente do pack na instância volta à versão do pack.
//! - `capture_discard`: "Descartar": as configs do pack voltam à versão do pack no próximo
//!   teste.
//!
//! Finos: pegam a trava do pack e chamam a `warden-instance`. A gravação no pack é da
//! `warden-project` (`PackTransaction`).
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};
use warden_core::PackId;
use warden_instance::{
    CaptureApplyRequest, CaptureError, CaptureGroup, CaptureTexts, CaptureView, CaptureChange,
    InstanceDirs, capture_texts as read_texts, capture_view, discard, finish_apply, plan_apply,
};
use warden_project::add::local::{FileFacts, HashLookup, HashMatch, MAX_FILE_BYTES, identify};
use warden_project::search::{PackTarget, SourceId};
use warden_project::transaction::PackTransaction;

use crate::commands::add_curseforge;
use crate::commands::inventory::{notify_changed, pack_root, packwiz};
use crate::error::{AppError, AppErrorCode};
use crate::events::PackArea;
use crate::operations::OperationKind;
use crate::state::AppState;

/// "Trazer mudanças do teste para o pack".
const APPLY: OperationKind = OperationKind::new("capture.apply");
/// "Descartar mudanças do teste".
const DISCARD: OperationKind = OperationKind::new("capture.discard");
/// "Procurar mods colocados à mão".
const IDENTIFY: OperationKind = OperationKind::new("capture.identify");

/// O erro da captura em erro do app.
fn capture_error(error: CaptureError) -> AppError {
    match error {
        CaptureError::NotFound => AppError::new(AppErrorCode::CaptureNotFound),
        CaptureError::AlreadyResolved => AppError::new(AppErrorCode::CaptureResolved),
        CaptureError::Stale { path } => {
            AppError::new(AppErrorCode::CaptureStale).with_param("path", path)
        }
        CaptureError::InvalidItem { path } | CaptureError::NotMergeable { path } => {
            AppError::new(AppErrorCode::CaptureInvalid).with_param("path", path)
        }
        CaptureError::Instance(error) => AppError::from_domain(&error),
        other => AppError::internal(other.to_string()),
    }
}

/// O pack e a instância de teste dele.
fn places(state: &AppState, pack_id: PackId) -> Result<(PathBuf, InstanceDirs), AppError> {
    let root = pack_root(state, pack_id)?;
    Ok((root, InstanceDirs::for_pack(&state.paths, pack_id)))
}

/// Roda trabalho de disco fora do executor.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("tarefa interrompida: {error}")))?
}

/// A captura do último teste do pack (`None`: nenhum teste ainda, ou o jogo não chegou a abrir).
#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_get(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<Option<CaptureView>, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let (root, dirs) = places(&state, pack_id)?;
    blocking(move || capture_view(&dirs, &root).map_err(|error| AppError::from_domain(&error)))
        .await
}

/// Um arquivo colocado à mão, procurado pelo hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureMatch {
    /// Caminho na instância.
    pub(crate) path: String,
    /// O primeiro achado (Modrinth antes da CurseForge); `None`: não reconhecido.
    pub(crate) found: Option<HashMatch>,
}

/// O resultado de `capture_identify`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureIdentified {
    /// Um por arquivo procurado.
    pub(crate) items: Vec<CaptureMatch>,
    /// Fontes em que a procura falhou (sem internet, servidor fora do ar).
    pub(crate) failed: Vec<SourceId>,
}

/// Lê os arquivos novos de `mods/`, `resourcepacks/` e `shaderpacks/` da captura.
fn read_new_items(
    view: &CaptureView,
    game_dir: &Path,
) -> Result<Vec<(String, FileFacts)>, AppError> {
    let mut read = Vec::new();
    for item in &view.files {
        let file = &item.file;
        let wanted = matches!(file.group, CaptureGroup::Mod | CaptureGroup::Resource)
            && file.change != CaptureChange::Removed
            && file.size <= MAX_FILE_BYTES;
        if !wanted {
            continue;
        }
        let path = game_dir.join(&file.path);
        // Pasta (resource pack descompactado) ou arquivo que sumiu: sem hash para procurar.
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let name = file.path.rsplit('/').next().unwrap_or(&file.path);
        read.push((file.path.clone(), FileFacts::of(name, &bytes)));
    }
    Ok(read)
}

/// Procura pelo hash os mods, resource packs e shaders colocados à mão na instância.
#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_identify(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<CaptureIdentified, AppError> {
    let operation = state.operations.start(IDENTIFY, Some(pack_id), true);
    let result = async {
        let (target, read) = {
            let _lock = state.locks.read_for(pack_id, &operation).await;
            let (root, dirs) = places(&state, pack_id)?;
            let target = PackTarget::read(&root).map_err(|e| AppError::from_domain(&e))?;
            let read = blocking(move || {
                let Some(view) =
                    capture_view(&dirs, &root).map_err(|error| AppError::from_domain(&error))?
                else {
                    return Ok(Vec::new());
                };
                read_new_items(&view, &dirs.game_dir)
            })
            .await?;
            (target, read)
        };
        if read.is_empty() {
            return Ok(CaptureIdentified {
                items: Vec::new(),
                failed: Vec::new(),
            });
        }
        let facts: Vec<FileFacts> = read.iter().map(|(_, facts)| facts.clone()).collect();
        let curseforge = add_curseforge::lookup_client(&state);
        let identified = identify(
            HashLookup {
                modrinth: Some(&state.modrinth),
                curseforge: curseforge.as_ref(),
            },
            &facts,
            &target,
            operation.token(),
        )
        .await;
        operation.ensure_not_cancelled()?;
        Ok(CaptureIdentified {
            items: read
                .into_iter()
                .zip(identified.matches)
                .map(|((path, _), matches)| CaptureMatch {
                    path,
                    found: matches.into_iter().next(),
                })
                .collect(),
            failed: identified.failed,
        })
    }
    .await;
    operation.finish(result)
}

/// As três versões de um arquivo da captura.
#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_texts(
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
    path: String,
) -> Result<CaptureTexts, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let (root, dirs) = places(&state, pack_id)?;
    blocking(move || read_texts(&dirs, &root, &session_id, &path).map_err(capture_error)).await
}

/// O resultado de "Trazer selecionados para o pack".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureApplied {
    /// Quantas mudanças foram para o pack.
    pub(crate) count: u32,
    /// Arquivos do pack que mudaram.
    pub(crate) files: Vec<String>,
}

/// "Trazer selecionados para o pack": grava só o escolhido. Se o pack ou o jogo mudaram depois
/// que a lista foi mostrada, nada é gravado (`app.CAPTURE_STALE`).
#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: PackId,
    request: CaptureApplyRequest,
) -> Result<CaptureApplied, AppError> {
    let cli = packwiz(&state)?;
    let applied = apply_impl(&state, &cli, pack_id, request).await?;
    if !applied.files.is_empty() {
        notify_changed(&app, pack_id, vec![PackArea::Configs]);
    }
    Ok(applied)
}

/// A gravação de verdade (sem o aviso à interface).
async fn apply_impl(
    state: &AppState,
    cli: &warden_packwiz_cli::Packwiz,
    pack_id: PackId,
    request: CaptureApplyRequest,
) -> Result<CaptureApplied, AppError> {
    let handle = state.operations.start(APPLY, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let (root, dirs) = places(state, pack_id)?;
        let session_id = request.session_id.clone();
        let plan = {
            let (root, dirs) = (root.clone(), dirs.clone());
            blocking(move || plan_apply(&dirs, &root, &request).map_err(capture_error)).await?
        };
        let mut files: Vec<String> = plan.writes.keys().cloned().collect();
        files.extend(plan.deletes.iter().cloned());
        if !files.is_empty() {
            let mut transaction = PackTransaction::new(root.clone());
            for (path, bytes) in &plan.writes {
                transaction
                    .write(path, bytes.clone())
                    .map_err(|e| AppError::from_domain(&e))?;
            }
            for path in &plan.deletes {
                transaction
                    .delete(path)
                    .map_err(|e| AppError::from_domain(&e))?;
            }
            transaction
                .commit(cli, handle.token())
                .await
                .map_err(|e| AppError::from_domain(&e))?;
        }
        let count = plan.count;
        let restored = blocking(move || {
            finish_apply(&dirs, &root, &session_id, &plan).map_err(capture_error)
        })
        .await?;
        tracing::info!(
            mudancas = count,
            arquivos = files.len(),
            devolvidos = restored.len(),
            "mudanças do teste trazidas para o pack"
        );
        Ok(CaptureApplied { count, files })
    }
    .await;
    handle.finish(result)
}

/// "Descartar": as configs do pack que mudaram no jogo voltam à versão do pack no próximo
/// teste. Devolve quantos arquivos voltam.
#[tauri::command]
#[specta::specta]
pub(crate) async fn capture_discard(
    state: State<'_, AppState>,
    pack_id: PackId,
    session_id: String,
) -> Result<u32, AppError> {
    discard_impl(&state, pack_id, session_id).await
}

async fn discard_impl(
    state: &AppState,
    pack_id: PackId,
    session_id: String,
) -> Result<u32, AppError> {
    let handle = state.operations.start(DISCARD, Some(pack_id), false);
    let result = async {
        let _lock = state.locks.write_for(pack_id, &handle).await;
        let (root, dirs) = places(state, pack_id)?;
        let restored =
            blocking(move || discard(&dirs, &root, &session_id).map_err(capture_error)).await?;
        Ok(u32::try_from(restored.len()).unwrap_or(u32::MAX))
    }
    .await;
    handle.finish(result)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use serde_json::json;
    use warden_core::{CancellationToken, NoProgress};
    use warden_instance::{
        DownloadCache, CaptureDecision, MaterializeOptions, Outcome, CaptureResolution, Sources,
    };
    use warden_launcher::outcome::{CrashArtifacts, Outcome as GameOutcome};
    use warden_launcher::process::GameExit;
    use warden_project::configs::{ConfigOrigin, read_file, save_to_instance};

    use super::*;
    use crate::commands::inventory::tests::{packwiz_binary, registered_pack, test_packwiz};
    use crate::error::ErrorCode;
    use crate::state::tests::test_state;
    use crate::test_hooks::{ExitContext, HookContext, registered};
    use crate::test_session::TestMode;

    fn sha(bytes: &[u8]) -> String {
        use sha2::Digest as _;
        hex::encode(sha2::Sha256::digest(bytes))
    }

    /// Copia o pack para a instância de teste, como o Testar.
    async fn materialize(root: &Path, dirs: &InstanceDirs, cache: &Path) {
        let http =
            warden_http::HttpClient::new(warden_http::HttpConfig::for_version("teste")).unwrap();
        let sources = Sources {
            http,
            curseforge: None,
            cache: Arc::new(DownloadCache::open(cache).unwrap()),
        };
        let outcome = warden_instance::materialize(
            &sources,
            root,
            dirs,
            &MaterializeOptions::default(),
            &NoProgress,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(matches!(outcome, Outcome::Done(_)));
    }

    fn exit() -> GameExit {
        GameExit {
            outcome: GameOutcome::ClosedNormally,
            exit_code: Some(0),
            stop_requested: false,
            duration_ms: 1_000,
            artifacts: CrashArtifacts::default(),
        }
    }

    /// O Testar inteiro pelos ganchos: linha de base, o jogo (e o editor de configs da
    /// instância, CA-T12-04) mudando arquivos, a captura, o Testar recusado enquanto há
    /// mudanças para revisar, e "Trazer" gravando no pack pela `PackTransaction`.
    #[tokio::test]
    async fn ca_t12_04_e_trazer_para_o_pack() {
        let Some(binary) = packwiz_binary() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cli = test_packwiz(&state, binary);
        let (id, root) = registered_pack(&state, dir.path(), &cli, 0).await;
        let mut transaction = PackTransaction::new(root.clone());
        transaction
            .write("config/Ajustes do mod/ação.toml", b"a = 1\n".to_vec())
            .unwrap();
        transaction
            .write("options.txt", b"version:3465\nrenderDistance:8\n".to_vec())
            .unwrap();
        transaction.commit(&cli, &CancellationToken::new()).await.unwrap();
        let dirs = InstanceDirs::for_pack(&state.paths, id);
        materialize(&root, &dirs, &dir.path().join("cache")).await;

        let hooks = registered();
        let operation = state
            .operations
            .start(OperationKind::new("test.start"), Some(id), true);
        let context = HookContext {
            pack_id: id,
            pack_root: &root,
            dirs: &dirs,
            mode: TestMode::Normal,
            operation: &operation,
        };
        for hook in &hooks {
            hook.before_prepare(&context).await.unwrap();
            hook.before_launch(&context).await.unwrap();
        }

        // Com o jogo aberto: a pessoa edita a config da instância pelo editor de configs
        // (CA-T12-04) e o jogo grava a distância de renderização.
        let read = read_file(&dirs.game_dir, ConfigOrigin::Instance, "config/Ajustes do mod/ação.toml")
            .unwrap();
        save_to_instance(
            &dirs.game_dir,
            "config/Ajustes do mod/ação.toml",
            "a = 22\n",
            &read.hash,
        )
        .unwrap();
        fs::write(
            dirs.game_dir.join("options.txt"),
            "version:3700\nrenderDistance:16\n",
        )
        .unwrap();

        let session_dir = dir.path().join("sessoes/2026-10-13_14-20-00");
        let exit = exit();
        let exit_context = ExitContext {
            exit: &exit,
            session_dir: &session_dir,
        };
        let mut fields = serde_json::Map::new();
        for hook in &hooks {
            fields.extend(hook.after_exit(&context, &exit_context).await.unwrap());
        }
        assert_eq!(fields["capture"], json!({"changes": 2, "needsReview": true}));

        let view = capture_view(&dirs, &root).unwrap().unwrap();
        assert_eq!(view.session_id, "2026-10-13_14-20-00");
        let groups: Vec<(&str, CaptureGroup)> = view
            .files
            .iter()
            .map(|file| (file.file.path.as_str(), file.file.group))
            .collect();
        assert_eq!(
            groups,
            [
                ("config/Ajustes do mod/ação.toml", CaptureGroup::PackConfig),
                ("options.txt", CaptureGroup::Options),
            ]
        );

        // Testar de novo antes de decidir: recusado, com o teste das mudanças.
        let error = hooks[0].before_prepare(&context).await.unwrap_err();
        assert_eq!(error.code, ErrorCode::App(AppErrorCode::TestCapturePending));
        assert_eq!(error.params["sessionId"], "2026-10-13_14-20-00");

        let request = CaptureApplyRequest {
            session_id: view.session_id.clone(),
            files: vec![CaptureDecision {
                path: "config/Ajustes do mod/ação.toml".into(),
                resolution: CaptureResolution::Take,
                expected_pack_sha256: view.files[0].pack_sha256.clone(),
            }],
            option_keys: vec!["renderDistance".into()],
            expected_options_sha256: view.files[1].pack_sha256.clone(),
        };
        let applied = apply_impl(&state, &cli, id, request.clone()).await.unwrap();
        assert_eq!(applied.count, 2);
        assert_eq!(
            applied.files,
            ["config/Ajustes do mod/ação.toml", "options.txt"]
        );
        assert_eq!(
            fs::read_to_string(root.join("config/Ajustes do mod/ação.toml")).unwrap(),
            "a = 22\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("options.txt")).unwrap(),
            "version:3465\nrenderDistance:16\n"
        );
        // O índice foi atualizado pelo `packwiz refresh`.
        let index = fs::read_to_string(root.join("index.toml")).unwrap();
        assert!(index.contains(&sha(b"a = 22\n")), "{index}");

        hooks[0].before_prepare(&context).await.unwrap();
        let again = apply_impl(&state, &cli, id, request).await.unwrap_err();
        assert_eq!(again.code, ErrorCode::App(AppErrorCode::CaptureResolved));
        // A próxima cópia do pack passa sem pedir revisão.
        materialize(&root, &dirs, &dir.path().join("cache")).await;
    }

    /// "Descartar": o pack não muda e a config do pack volta à versão do pack na instância.
    #[tokio::test]
    async fn descartar_devolve_a_config_do_pack() {
        let Some(binary) = packwiz_binary() else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let cli = test_packwiz(&state, binary);
        let (id, root) = registered_pack(&state, dir.path(), &cli, 0).await;
        let mut transaction = PackTransaction::new(root.clone());
        transaction
            .write("config/a.toml", b"a = 1\n".to_vec())
            .unwrap();
        transaction.commit(&cli, &CancellationToken::new()).await.unwrap();
        let dirs = InstanceDirs::for_pack(&state.paths, id);
        materialize(&root, &dirs, &dir.path().join("cache")).await;
        let operation = state
            .operations
            .start(OperationKind::new("test.start"), Some(id), true);
        let context = HookContext {
            pack_id: id,
            pack_root: &root,
            dirs: &dirs,
            mode: TestMode::Normal,
            operation: &operation,
        };
        let hooks = registered();
        hooks[0].before_launch(&context).await.unwrap();
        fs::write(dirs.game_dir.join("config/a.toml"), "a = 22\n").unwrap();
        let exit = exit();
        let session_dir = dir.path().join("sessoes/s1");
        hooks[0]
            .after_exit(
                &context,
                &ExitContext {
                    exit: &exit,
                    session_dir: &session_dir,
                },
            )
            .await
            .unwrap();

        let missing = discard_impl(&state, id, "outro".into()).await.unwrap_err();
        assert_eq!(missing.code, ErrorCode::App(AppErrorCode::CaptureNotFound));
        assert_eq!(discard_impl(&state, id, "s1".into()).await.unwrap(), 1);
        assert_eq!(fs::read_to_string(root.join("config/a.toml")).unwrap(), "a = 1\n");
        hooks[0].before_prepare(&context).await.unwrap();
        materialize(&root, &dirs, &dir.path().join("cache")).await;
        assert_eq!(
            fs::read_to_string(dirs.game_dir.join("config/a.toml")).unwrap(),
            "a = 1\n"
        );
    }

    #[test]
    fn erros_da_captura_viram_codigos_do_app() {
        let code = |error| capture_error(error).code;
        assert_eq!(
            code(CaptureError::NotFound),
            ErrorCode::App(AppErrorCode::CaptureNotFound)
        );
        assert_eq!(
            code(CaptureError::AlreadyResolved),
            ErrorCode::App(AppErrorCode::CaptureResolved)
        );
        let stale = capture_error(CaptureError::Stale {
            path: "config/a.toml".into(),
        });
        assert_eq!(stale.code, ErrorCode::App(AppErrorCode::CaptureStale));
        assert_eq!(stale.params["path"], "config/a.toml");
        assert_eq!(
            code(CaptureError::NotMergeable {
                path: "config/a.toml".into()
            }),
            ErrorCode::App(AppErrorCode::CaptureInvalid)
        );
    }
}
