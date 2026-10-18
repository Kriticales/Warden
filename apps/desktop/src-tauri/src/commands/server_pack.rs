//! Contrato IPC do pacote para servidor (E-03; ADR-0035; ARCHITECTURE §12.1; SPEC T19).
//!
//! - `server_pack_analyze`: o que cada variante vai levar (a com mods dentro, do estado atual;
//!   a pelo link, da última versão publicada), se a variante pelo link está disponível, a
//!   memória recomendada e se a EULA já foi aceita no Warden.
//! - `server_pack_run`: o diálogo nativo do destino, a geração (operação `export.serverPack`,
//!   com os downloads manuais da T20 quando a variante com mods dentro encontra mods
//!   bloqueados) e, com `validate`, a validação no servidor local da L-09 até "Done".
//! - `server_pack_reveal`: "Mostrar arquivo" de um pacote gerado nesta sessão.
//!
//! A validação (ARCHITECTURE §12.1 item 4) só roda com a EULA aceita no Warden: descompacta o
//! pacote em `cache/tmp/`, roda o bootstrap com `-s server` (pelo link), instala o servidor do
//! loader, abre com o `server.properties` controlado (só `127.0.0.1`) até "Done" e para com
//! `stop`. Se o servidor fechar antes, o resultado traz o "Por que travou" do servidor (o mod
//! apontado e o trecho do log, já redigido). A pasta temporária sai no fim, de qualquer jeito.
#![allow(clippy::needless_pass_by_value)] // argumentos pertencem ao contrato do Tauri

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;
use warden_core::{CancellationToken, PackId, ProgressSink as _};
use warden_export::{
    ExportSource, JavaRange, LoaderDownload, ManualDownloads, ServerPackAnalysis,
    ServerPackOptions, ServerPackRequest, ServerPackResult, ServerPackVariant, WaitFuture,
};
use warden_http::Url;
use warden_instance::{BlockedFile, DownloadCache, InstanceDirs};
use warden_java::{CompatibilityTable, JavaChoiceRequest, RuntimeLease};
use warden_launcher::{GameSpec, JavaRuntime, LoaderSpec};
use warden_packwiz::PackManifest;
use warden_packwiz_cli::Packwiz;
use warden_server::{ServerCommand, ServerDownload};
use warden_versioning::github::GithubEndpoints;
use warden_versioning::{PackRepo, Snapshot};

use super::inventory::{self, pack_root};
use crate::error::AppError;
use crate::operations::{OperationHandle, OperationKind};
use crate::server_session::{ServerCrash, diagnose_crash, eula_accepted_at};
use crate::state::AppState;

const SERVER_PACK_RUN: OperationKind = OperationKind::new("export.serverPack");

/// Etapa "Testando o pacote no servidor deste computador".
pub(crate) const STAGE_CHECK: &str = "serverPackCheck";
/// Texto da etapa.
const STAGE_CHECK_LABEL: &str = "pacoteServidor:etapa.testar";

/// Em build de debug, o E2E troca o diálogo nativo pelo caminho escrito neste arquivo (o mesmo
/// mecanismo de `export_run`).
#[cfg(debug_assertions)]
const E2E_PICK_FOLDER_ENV: &str = "WARDEN_E2E_PICK_FOLDER";

/// Pacotes gerados nesta sessão: "Mostrar arquivo" só abre um destes (ARCHITECTURE §20).
static GENERATED: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Mutex::default);

/// A variante pelo link: a versão publicada e o link do `pack.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerPackPublication {
    /// A última versão publicada.
    pub(crate) version: String,
    /// O link do pack (`…/main/pack.toml`).
    pub(crate) link: String,
    /// O que vai no pacote, lido dessa versão.
    pub(crate) analysis: ServerPackAnalysis,
}

/// Por que a variante pelo link não está disponível.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LinkUnavailable {
    /// O pack nunca foi publicado.
    NotPublished,
    /// O repositório é privado ("só backup"): o link não abre para o servidor.
    PrivateRepo,
}

/// O que a tela mostra antes de gerar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerPackInfo {
    /// O estado atual (a variante com mods dentro).
    pub(crate) current: ServerPackAnalysis,
    /// A publicação (a variante pelo link), quando há.
    pub(crate) published: Option<ServerPackPublication>,
    /// Sem publicação utilizável, o motivo.
    pub(crate) link_unavailable: Option<LinkUnavailable>,
    /// A memória recomendada do servidor, em MB.
    pub(crate) recommended_memory_mb: u32,
    /// Quando a EULA foi aceita no Warden (`None`: a validação pede o aceite antes).
    pub(crate) eula_accepted_at: Option<String>,
}

/// O que a pessoa pediu.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerPackRunRequest {
    /// As escolhas da geração.
    pub(crate) options: ServerPackOptions,
    /// "Testar no servidor deste computador depois de gerar".
    pub(crate) validate: bool,
}

/// O resultado da validação no servidor local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub(crate) enum ServerPackCheck {
    /// "Pacote para servidor testado: o servidor abriu em N s."
    Tested {
        /// Da abertura do processo ao "Done", em ms.
        #[specta(type = specta_typescript::Number)]
        ready_ms: u64,
    },
    /// O servidor fechou antes do "Done": o "Por que travou".
    Crashed {
        /// O mod apontado, a classe do cliente e o trecho do log.
        crash: ServerCrash,
    },
    /// A validação não chegou a abrir o servidor (download, instalação, tempo esgotado).
    Failed {
        /// O erro.
        error: AppError,
    },
}

/// O pacote gerado e, se pedida, a validação.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerPackRun {
    /// O pacote.
    pub(crate) result: ServerPackResult,
    /// A validação (`None`: não pedida).
    pub(crate) check: Option<ServerPackCheck>,
}

/// A última versão publicada e o link, ou por que não há.
fn publication(
    state: &AppState,
    pack_id: PackId,
    root: &Path,
) -> Result<Result<(String, Url), LinkUnavailable>, AppError> {
    let Some(link) = crate::commands::github::stored_link(state, pack_id)? else {
        return Ok(Err(LinkUnavailable::NotPublished));
    };
    let version = match PackRepo::open(root) {
        Ok(repo) => repo
            .last_published()
            .map_err(|error| AppError::from_domain(&error))?,
        Err(_) => None,
    };
    let Some(version) = version else {
        return Ok(Err(LinkUnavailable::NotPublished));
    };
    if link.private {
        return Ok(Err(LinkUnavailable::PrivateRepo));
    }
    let text = GithubEndpoints::from_env().pack_link(&link.repo());
    let url = Url::parse(&text).map_err(|error| AppError::internal(format!("link do pack: {error}")))?;
    Ok(Ok((version.to_string(), url)))
}

/// O que a tela mostra antes de gerar. Só lê o pack (e confere na CurseForge os mods que vão
/// pedir o download manual).
#[tauri::command]
#[specta::specta]
pub(crate) async fn server_pack_analyze(
    state: State<'_, AppState>,
    pack_id: PackId,
) -> Result<ServerPackInfo, AppError> {
    let _lock = state.locks.read(pack_id).await;
    let root = pack_root(&state, pack_id)?;
    let sources = crate::test_session::materialize_sources(&state)?;
    let cancel = CancellationToken::new();
    let current =
        warden_export::analyze_server_pack(&root, &ExportSource::Current, Some(&sources), &cancel)
            .await
            .map_err(|error| AppError::from_domain(&error))?;
    let (published, link_unavailable) = match publication(&state, pack_id, &root)? {
        Ok((version, link)) => {
            let analysis = warden_export::analyze_server_pack(
                &root,
                &ExportSource::Saved {
                    version: version.clone(),
                },
                None,
                &cancel,
            )
            .await
            .map_err(|error| AppError::from_domain(&error))?;
            (
                Some(ServerPackPublication {
                    version,
                    link: link.to_string(),
                    analysis,
                }),
                None,
            )
        }
        Err(reason) => (None, Some(reason)),
    };
    Ok(ServerPackInfo {
        recommended_memory_mb: warden_server::server_memory_mb(current.embedded.server_items),
        current,
        published,
        link_unavailable,
        eula_accepted_at: eula_accepted_at(&state),
    })
}

/// Abre o diálogo de destino, gera o pacote e, com `validate`, testa no servidor local.
/// `None` = a pessoa desistiu do diálogo.
#[tauri::command]
#[specta::specta]
pub(crate) async fn server_pack_run(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    pack_id: PackId,
    request: ServerPackRunRequest,
) -> Result<Option<ServerPackRun>, AppError> {
    let root = pack_root(&state, pack_id)?;
    let Some(destination) = pick_destination(&app, &window, &default_name(&root)).await? else {
        return Ok(None);
    };
    if !destination.is_absolute() {
        return Err(AppError::internal("o diálogo devolveu um caminho relativo"));
    }
    let packwiz = inventory::packwiz(&state)?;
    let operation = state.operations.start(SERVER_PACK_RUN, Some(pack_id), true);
    let result = run(
        &state,
        pack_id,
        &root,
        &destination,
        &request,
        &packwiz,
        &operation,
    )
    .await;
    operation.finish(result.map(Some))
}

/// A geração e a validação, dentro da operação. Com a validação pedida e a EULA ainda não
/// aceita no Warden, nada é gerado (`server.EULA_NOT_ACCEPTED`: a interface pergunta antes).
#[allow(clippy::too_many_lines)] // as etapas da geração, na ordem
pub(crate) async fn run(
    state: &AppState,
    pack_id: PackId,
    root: &Path,
    destination: &Path,
    request: &ServerPackRunRequest,
    packwiz: &Packwiz,
    operation: &OperationHandle,
) -> Result<ServerPackRun, AppError> {
    if request.validate && eula_accepted_at(state).is_none() {
        return Err(AppError::from_domain(&warden_server::Error::EulaNotAccepted));
    }
    let token = operation.token().clone();
    let lock = state.locks.read_for(pack_id, operation).await;
    let (source, link) = match request.options.variant {
        ServerPackVariant::Link => match publication(state, pack_id, root)? {
            Ok((version, link)) => (ExportSource::Saved { version }, Some(link)),
            Err(_) => {
                return Err(AppError::from_domain(&warden_export::Error::new(
                    warden_export::ExportErrorCode::ServerPackNotPublished,
                    "o pacote pelo link precisa do pack publicado",
                )));
            }
        },
        ServerPackVariant::Embedded => (ExportSource::Current, None),
    };
    let spec = pack_spec(root, &source)?;
    let loader = loader_download(state, &spec, operation).await?;
    let sources = crate::test_session::materialize_sources(state)?;
    let jars = match request.options.variant {
        ServerPackVariant::Link => Some(
            operation
                .run_cancellable(async {
                    state
                        .tests
                        .installer_cache(&state.paths)
                        .ensure(&sources.http, operation, &token)
                        .await
                        .map_err(|error| AppError::from_domain(&error))
                })
                .await?,
        ),
        ServerPackVariant::Embedded => None,
    };
    let manual = HubDownloads {
        state,
        pack_id,
        cache: Arc::clone(&sources.cache),
        operation,
    };
    let staging = state.paths.cache_dir().join("staging");
    let result = warden_export::export_server_pack(
        &ServerPackRequest {
            root,
            source: &source,
            destination,
            staging_parent: &staging,
            packwiz,
            options: &request.options,
            loader: &loader,
            java: java_range(&spec),
            link: link.as_ref(),
            installer: jars.as_ref(),
            sources: Some(&sources),
            manual: Some(&manual),
        },
        operation,
        &token,
    )
    .await
    .map_err(|error| {
        if token.is_cancelled() {
            AppError::cancelled()
        } else {
            AppError::from_domain(&error)
        }
    })?;
    drop(lock);
    GENERATED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(result.path.clone());
    tracing::info!(
        variant = ?result.variant,
        bytes = result.bytes,
        "pacote para servidor gerado"
    );
    let check = if request.validate {
        Some(
            check_package(state, pack_id, root, &result, &spec, link.as_ref(), operation)
                .await?,
        )
    } else {
        None
    };
    Ok(ServerPackRun { result, check })
}

/// O download manual da T20 durante a geração, com a mesma lista e a mesma tela do Testar.
struct HubDownloads<'a> {
    state: &'a AppState,
    pack_id: PackId,
    cache: Arc<DownloadCache>,
    operation: &'a OperationHandle,
}

impl ManualDownloads for HubDownloads<'_> {
    fn wait(&self, blocked: Vec<BlockedFile>) -> WaitFuture<'_> {
        Box::pin(async move {
            self.state
                .manual_downloads
                .wait(
                    self.pack_id,
                    blocked,
                    Arc::clone(&self.cache),
                    self.operation,
                )
                .await
                .map_err(|error| {
                    warden_export::Error::internal(format!("download manual: {}", error.code))
                })
        })
    }
}

/// O jogo do `pack.toml` da origem (o da versão publicada, pelo link).
fn pack_spec(root: &Path, source: &ExportSource) -> Result<GameSpec, AppError> {
    let bytes = match source {
        ExportSource::Current => std::fs::read(root.join("pack.toml"))
            .map_err(|error| AppError::internal(format!("ler o pack.toml: {error}")))?,
        ExportSource::Saved { version } => PackRepo::open(root)
            .and_then(|repo| repo.read_file_at(&Snapshot::Version(version.clone()), "pack.toml"))
            .map_err(|error| AppError::from_domain(&error))?
            .ok_or_else(|| AppError::internal("pack.toml ausente na versão publicada"))?,
    };
    let text = String::from_utf8(bytes)
        .map_err(|_| AppError::internal("o pack.toml não é UTF-8"))?;
    let manifest = PackManifest::parse(&text)
        .map_err(|error| AppError::from_domain(&error))?
        .value;
    let requirement = warden_instance::game_requirement(&manifest)
        .map_err(|error| AppError::from_domain(&error))?;
    crate::test_session::game_spec(&requirement)
}

/// De onde os scripts baixam o servidor do loader (os mesmos endereços do servidor local). Com o
/// servidor simulado do E2E, um endereço local que nunca é usado.
async fn loader_download(
    state: &AppState,
    spec: &GameSpec,
    operation: &OperationHandle,
) -> Result<LoaderDownload, AppError> {
    if state.tests.fake_server().is_some() {
        let url = Url::parse("http://127.0.0.1:9/warden-e2e/servidor.jar")
            .map_err(|error| AppError::internal(error.to_string()))?;
        return Ok(match spec.loader {
            LoaderSpec::Vanilla => LoaderDownload::Vanilla { url, sha1: None },
            LoaderSpec::Fabric { .. } | LoaderSpec::Quilt { .. } => LoaderDownload::Fabric { url },
            LoaderSpec::Forge { .. } | LoaderSpec::NeoForge { .. } => {
                LoaderDownload::Installer { url, sha1: None }
            }
        });
    }
    let installer = crate::server_session::installer(state)?;
    let download = operation
        .run_cancellable(async {
            installer
                .server_download(spec, operation.token())
                .await
                .map_err(|error| AppError::from_domain(&error))
        })
        .await?;
    Ok(match download {
        ServerDownload::Vanilla { url, sha1 } => LoaderDownload::Vanilla { url, sha1 },
        ServerDownload::Fabric { url } => LoaderDownload::Fabric { url },
        ServerDownload::Installer { url, sha1 } => LoaderDownload::Installer { url, sha1 },
    })
}

/// A faixa do Java dos scripts: o Java da política do Warden como mínimo e, quando uma regra de
/// loader obriga um Java (Forge até a 1.12.2 só abre no 8), o mesmo como máximo.
fn java_range(spec: &GameSpec) -> JavaRange {
    let request = JavaChoiceRequest::automatic(
        spec.minecraft.as_str(),
        crate::test_session::loader_kind(&spec.loader),
        spec.loader.version(),
    );
    let Ok(table) = CompatibilityTable::builtin() else {
        return JavaRange { min: 8, max: None };
    };
    let Ok(decision) = warden_java::policy::decide(&request, table) else {
        return JavaRange { min: 8, max: None };
    };
    let major = decision.requirement.major;
    let fixed = warden_java::MinecraftVersion::parse(spec.minecraft.as_str())
        .and_then(|minecraft| {
            table.loader_rule_for(&minecraft, request.loader, request.loader_version.as_deref())
        })
        .is_some();
    JavaRange {
        min: major,
        max: fixed.then_some(major),
    }
}

/// O Java da validação: o do jogo simulado no E2E, senão o da política (baixado se faltar).
async fn check_java(
    state: &AppState,
    spec: &GameSpec,
    operation: &OperationHandle,
) -> Result<(JavaRuntime, Option<RuntimeLease>), AppError> {
    if let Some(java) = state.tests.fake_java() {
        return Ok((java, None));
    }
    let request = JavaChoiceRequest::automatic(
        spec.minecraft.as_str(),
        crate::test_session::loader_kind(&spec.loader),
        spec.loader.version(),
    );
    let java_service = state.java.clone();
    let choice = tokio::task::spawn_blocking(move || {
        java_service
            .choose(&request)
            .map_err(|error| AppError::from_domain(&error))
    })
    .await
    .map_err(|error| AppError::internal(format!("escolher o Java: {error}")))??;
    let runtime = operation
        .run_cancellable(async {
            state
                .java
                .ensure_choice(&choice, operation, operation.token())
                .await
                .map_err(|error| AppError::from_domain(&error))
        })
        .await?;
    let lease = state.java.lease(&runtime.id);
    Ok((JavaRuntime::from_installed(&runtime), Some(lease)))
}

/// Apaga a pasta temporária da validação ao sair, de qualquer jeito.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(kind = ?error.kind(), "pasta temporária do pacote não apagada");
        }
    }
}

/// A validação no servidor local (ver o módulo). Erros que impedem abrir o servidor viram
/// [`ServerPackCheck::Failed`]; só o cancelamento e o aceite da EULA que faltou saem como erro.
async fn check_package(
    state: &AppState,
    pack_id: PackId,
    root: &Path,
    result: &ServerPackResult,
    spec: &GameSpec,
    link: Option<&Url>,
    operation: &OperationHandle,
) -> Result<ServerPackCheck, AppError> {
    if eula_accepted_at(state).is_none() {
        return Err(AppError::from_domain(&warden_server::Error::EulaNotAccepted));
    }
    operation.ensure_not_cancelled()?;
    operation.stage(STAGE_CHECK, STAGE_CHECK_LABEL);
    // Um servidor por vez no Warden: espera o que estiver aberto.
    while state.servers.current().is_some() {
        operation.ensure_not_cancelled()?;
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    let guard = state.servers.begin(pack_id);
    let outcome = check_inside(state, pack_id, root, result, spec, link, operation).await;
    guard.finish().await;
    match outcome {
        Ok(check) => Ok(check),
        Err(error) if operation.is_cancelled() => Err(error),
        Err(error) => {
            tracing::warn!(code = %error.code, "a validação do pacote não abriu o servidor");
            Ok(ServerPackCheck::Failed { error })
        }
    }
}

#[allow(clippy::too_many_arguments)] // as peças da validação
async fn check_inside(
    state: &AppState,
    pack_id: PackId,
    root: &Path,
    result: &ServerPackResult,
    spec: &GameSpec,
    link: Option<&Url>,
    operation: &OperationHandle,
) -> Result<ServerPackCheck, AppError> {
    let work = state
        .paths
        .tmp_dir()
        .join(format!("server-pack-{}", operation.id()));
    let temp = TempDir(work.clone());
    let dirs = InstanceDirs {
        game_dir: work.join("server"),
        state_dir: work.join("state"),
    };
    let zip_path = result.path.clone();
    let game_dir = dirs.game_dir.clone();
    tokio::task::spawn_blocking(move || warden_export::unpack_server_pack(&zip_path, &game_dir))
        .await
        .map_err(|error| AppError::internal(format!("descompactar o pacote: {error}")))?
        .map_err(|error| AppError::from_domain(&error))?;
    let (java, _lease) = check_java(state, spec, operation).await?;
    if let Some(link) = link {
        install_mods(&dirs.game_dir, &java, link, operation).await?;
    }
    let installed =
        crate::server_session::install(state, root, spec, &java, &dirs, operation).await?;
    operation.stage(STAGE_CHECK, STAGE_CHECK_LABEL);
    let record = state
        .packs
        .get(pack_id)
        .map_err(|error| AppError::from_domain(&error))?;
    let port = crate::server_session::configure(state, &dirs.game_dir, &record.name)?;
    let command = ServerCommand {
        java: java.clone(),
        memory_mb: result.memory_mb,
        extra_jvm_args: Vec::new(),
    }
    .build(&installed);
    let log_dir = work.join("log");
    let (running, mut events) = state
        .servers
        .spawn(&command, log_dir.join("output.log"), port)
        .await?;
    let handle = running.handle();
    let mut tail: Vec<String> = Vec::new();
    let token = operation.token().clone();
    let ready = tokio::select! {
        ready = warden_server::wait_ready(&mut events, crate::server_session::READY_TIMEOUT, |event| {
            if let warden_server::ServerEvent::Line(line) = event {
                tail.push(line.text.clone());
                if tail.len() > 400 {
                    tail.remove(0);
                }
            }
        }) => ready,
        () = token.cancelled() => {
            handle.kill().await;
            return Err(AppError::cancelled());
        }
    };
    let check = match ready {
        Ok(warden_server::ServerEvent::Ready { elapsed_ms, .. }) => {
            state.servers.update(|server| {
                server.phase = crate::commands::server::ServerPhase::Ready;
                server.ready_ms = Some(elapsed_ms);
            });
            state
                .servers
                .update(|server| server.phase = crate::commands::server::ServerPhase::Stopping);
            handle.stop().await;
            let _ = running.wait().await;
            ServerPackCheck::Tested {
                ready_ms: elapsed_ms,
            }
        }
        Ok(_) => return Err(AppError::internal("evento inesperado do servidor")),
        Err(warden_server::ReadyFailure::Exited(exit)) => {
            drop(running);
            let crash =
                diagnose_crash(state, pack_id, root, &log_dir, &dirs.game_dir, &exit, &tail).await;
            ServerPackCheck::Crashed { crash }
        }
        Err(failure @ warden_server::ReadyFailure::Timeout) => {
            handle.kill().await;
            return Err(AppError::from_domain(
                &failure.error(crate::server_session::READY_TIMEOUT),
            ));
        }
    };
    drop(temp);
    Ok(check)
}

/// Pelo link: o bootstrap do pacote com `-s server`, como o script faz, e o `server-overrides/`
/// por cima.
async fn install_mods(
    game_dir: &Path,
    java: &JavaRuntime,
    link: &Url,
    operation: &OperationHandle,
) -> Result<(), AppError> {
    let mut command = tokio::process::Command::new(&java.java);
    command
        .args([
            "-Djava.awt.headless=true",
            "-jar",
            "packwiz-installer-bootstrap.jar",
            "--bootstrap-no-update",
            "--bootstrap-main-jar",
            "packwiz-installer.jar",
            "-g",
            "-s",
            "server",
        ])
        .arg(link.as_str())
        .current_dir(game_dir)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = command.spawn().map_err(|error| {
        AppError::internal(format!("abrir o installer do pacote: {}", error.kind()))
    })?;
    let status = tokio::select! {
        status = child.wait() => status
            .map_err(|error| AppError::internal(format!("esperar o installer: {error}")))?,
        () = operation.token().cancelled() => {
            let _ = child.kill().await;
            return Err(AppError::cancelled());
        }
    };
    if !status.success() {
        return Err(AppError::from_domain(&warden_export::Error::new(
            warden_export::ExportErrorCode::ServerPackDownloadFailed,
            "o installer do pacote não baixou os mods pelo link",
        )));
    }
    let overrides = game_dir.join("server-overrides");
    let target = game_dir.to_path_buf();
    tokio::task::spawn_blocking(move || copy_overrides(&overrides, &target))
        .await
        .map_err(|error| AppError::internal(format!("copiar server-overrides: {error}")))?
}

fn copy_overrides(from: &Path, to: &Path) -> Result<(), AppError> {
    if !from.is_dir() {
        return Ok(());
    }
    let io = |error: std::io::Error| AppError::internal(format!("copiar server-overrides: {error}"));
    let mut pending = vec![from.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).map_err(io)? {
            let entry = entry.map_err(io)?;
            let path = entry.path();
            let kind = entry.file_type().map_err(io)?;
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                let relative = path
                    .strip_prefix(from)
                    .map_err(|error| AppError::internal(error.to_string()))?;
                let target = to.join(relative);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(io)?;
                }
                std::fs::copy(&path, &target).map_err(io)?;
            }
        }
    }
    Ok(())
}

/// "Mostrar arquivo": só aceita pacotes que `server_pack_run` gerou nesta sessão.
#[tauri::command]
#[specta::specta]
pub(crate) fn server_pack_reveal(app: AppHandle, path: PathBuf) -> Result<(), AppError> {
    let known = GENERATED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&path);
    if !known {
        return Err(AppError::internal(
            "caminho que não é um pacote gerado nesta sessão",
        ));
    }
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|error| AppError::internal(format!("mostrar o arquivo: {error}")))
}

/// `<pack>-<versão>-servidor.zip`, só com caracteres seguros para nome de arquivo.
fn default_name(root: &Path) -> String {
    let manifest = std::fs::read_to_string(root.join("pack.toml"))
        .ok()
        .and_then(|text| PackManifest::parse(&text).ok())
        .map(|parsed| parsed.value);
    let clean = |text: &str| -> String {
        text.chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, '.' | '_' | '-') {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_owned()
    };
    let (name, version) = manifest.map_or_else(
        || (String::new(), String::new()),
        |m| (clean(&m.name), clean(&m.version)),
    );
    let name = if name.is_empty() {
        "pack".to_owned()
    } else {
        name
    };
    if version.is_empty() {
        format!("{name}-servidor.zip")
    } else {
        format!("{name}-{version}-servidor.zip")
    }
}

async fn pick_destination(
    app: &AppHandle,
    window: &WebviewWindow,
    file_name: &str,
) -> Result<Option<PathBuf>, AppError> {
    #[cfg(debug_assertions)]
    if let Some(file) = std::env::var_os(E2E_PICK_FOLDER_ENV).filter(|value| !value.is_empty()) {
        let picked = std::fs::read_to_string(&file).unwrap_or_default();
        let picked = picked.trim();
        return Ok((!picked.is_empty()).then(|| PathBuf::from(picked)));
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_parent(window)
        .set_title("Salvar o pacote para servidor")
        .add_filter("Pacote para servidor (.zip)", &["zip"])
        .set_file_name(file_name)
        .save_file(move |picked| {
            let _ = sender.send(picked.and_then(|path| path.as_path().map(Path::to_path_buf)));
        });
    receiver
        .await
        .map_err(|error| AppError::internal(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_padrao_usa_pack_e_versao_sem_caracteres_perigosos() {
        let temp = tempfile::tempdir().unwrap();
        let mut manifest = PackManifest::new("Vale Sereno: o/melhor", "1.20.1");
        manifest.version = "1.4.2".into();
        std::fs::write(temp.path().join("pack.toml"), manifest.to_toml_string()).unwrap();
        assert_eq!(
            default_name(temp.path()),
            "Vale-Sereno--o-melhor-1.4.2-servidor.zip"
        );
        assert_eq!(default_name(&temp.path().join("nada")), "pack-servidor.zip");
    }

    #[test]
    fn faixa_do_java_segue_a_politica_e_fixa_o_forge_antigo() {
        let forge = |minecraft: &str, version: &str| {
            GameSpec::new(
                minecraft,
                LoaderSpec::Forge {
                    version: version.into(),
                },
            )
            .unwrap()
        };
        assert_eq!(
            java_range(&forge("1.12.2", "14.23.5.2860")),
            JavaRange {
                min: 8,
                max: Some(8)
            }
        );
        let modern = java_range(&forge("1.20.1", "47.3.0"));
        assert_eq!(modern.min, 17);
        assert_eq!(modern.max, None);
        let fabric = java_range(
            &GameSpec::new(
                "1.21.1",
                LoaderSpec::Fabric {
                    version: "0.16.5".into(),
                },
            )
            .unwrap(),
        );
        assert_eq!(fabric.min, 21);
    }
}
