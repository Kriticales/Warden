//! O Testar (SPEC T13; ROADMAP L-04): do clique em Testar ao jogo fechado.
//!
//! Etapas, cada uma com o progresso na operação `test.start` (painel Tarefas e canal do
//! comando):
//! 1. ganchos `before_prepare` ([`crate::test_hooks`]; "Verificar o pack", da D-03);
//! 2. **Preparar o Minecraft**: escolhe e, se faltar, baixa o Java (`warden-java`, ADR-0029)
//!    e instala o jogo e o loader com a versão exata do `pack.toml` (`warden-launcher`);
//! 3. **Copiar o pack para o teste**: a materialização da `warden-instance` (só o que mudou;
//!    remove o que saiu; nunca toca nos mundos);
//! 4. ganchos `before_launch` ("Verificação final", da D-03; linha de base da C-03);
//! 5. **Abrir o jogo**: memória automática, argumentos do teste (com o G1 no perfil padrão),
//!    jogador offline; o processo é supervisionado pela `warden-launcher` e as linhas vão para
//!    o console em lotes de até 50 ms;
//! 6. o jogo fechou: ganchos `after_exit`, `session.json` com os campos do Testar
//!    ([`records`]), retenção das sessões e o "último teste" de Meus packs.
//!
//! Um jogo por vez no Warden inteiro: [`TestSessions`] guarda o teste ativo, avisa cada
//! mudança pelo evento `game-state` e recusa um segundo teste com `app.GAME_ALREADY_RUNNING`.

mod console;
mod fake;
mod launch_plan;
mod records;
mod system;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use warden_core::{AppPaths, CancellationToken, PackId, ProgressSink as _};
use warden_instance::{
    DownloadCache, GameRequirement, InstanceDirs, MaterializeOptions, OnModified, Sources,
};
use warden_java::{JavaChoiceRequest, LoaderKind, RuntimeLease};
use warden_launcher::outcome::Outcome;
use warden_launcher::process::{
    GameEvent, GameEvents, GameExit, GameHandle, GameProcess, SpawnOptions,
};
use warden_launcher::session::{
    KEEP_CLOSED_SESSIONS, SessionDir, SessionRecord, list_sessions, prune_closed,
};
use warden_launcher::{
    CatalogForgeResolver, EngineDirs, GameSpec, InstalledGame, JavaRuntime, LauncherEngine,
    LoaderSpec, OfflineProfile, PortableMcEngine,
};

pub(crate) use self::console::{ConsoleBuffer, read_output_log};
pub use self::console::{ConsoleLevel, ConsoleLine, ConsoleOrigin, ConsoleStream};
pub(crate) use self::launch_plan::{choose_memory, launch_options};
pub(crate) use self::records::{TestSessionSummary, last_test_value, pack_tree_hash};

use self::fake::FakeGame;
use self::launch_plan::{JAVA8_WARN_ABOVE_MB, PlanInput};
use self::records::{PackTree, ProfileSignature, SessionExtras};
use self::system::MachineFingerprint;
use crate::error::{AppError, AppErrorCode};
use crate::events::{GamePhase, GameState, OperationEvent};
use crate::operations::{OperationHandle, OperationKind};
use crate::state::AppState;
use crate::test_hooks::{ExitContext, HookContext, TestHooks};

/// Tipo da operação do teste.
pub(crate) const TEST_START: OperationKind = OperationKind::new("test.start");

/// Etapa "Abrir o jogo" (as outras vêm da `warden-java`, da `warden-launcher` e da
/// `warden-instance`).
pub(crate) const STAGE_LAUNCH: &str = "test.launch";
/// Texto da etapa "Abrir o jogo".
const STAGE_LAUNCH_LABEL: &str = "teste:etapa.abrir";
/// Etapa "Escolhendo o Java".
pub(crate) const STAGE_JAVA: &str = "test.java";
/// Texto da etapa "Escolhendo o Java".
const STAGE_JAVA_LABEL: &str = "teste:etapa.java";

/// Intervalo dos lotes de linhas do console (ARCHITECTURE §4.3).
const CONSOLE_BATCH: Duration = Duration::from_millis(50);

/// Quanto o "Fechar o Warden" espera o jogo fechar e a sessão ser gravada.
pub(crate) const QUIT_WAIT: Duration = Duration::from_secs(10);

/// Modo do teste (ARCHITECTURE §4.1). Só o normal existe nesta versão; os outros chegam com
/// as tarefas donas (L-07, L-12, L-09).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TestMode {
    /// Teste normal.
    #[default]
    Normal,
    /// Testar como o jogador recebe (L-07).
    AsPlayer,
    /// Testar com perfil de desempenho (L-12).
    Performance,
    /// Testar como servidor (L-09).
    Server,
}

/// O pedido de teste.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct TestRequest {
    /// Modo.
    pub(crate) mode: TestMode,
    /// Perfil do teste (`None` = Padrão; os perfis são da L-08).
    pub(crate) profile: Option<String>,
    /// Substituir pela versão do pack os arquivos que mudaram na instância durante o teste
    /// anterior (resposta ao `app.TEST_INSTANCE_CHANGED`).
    pub(crate) replace_instance_changes: bool,
}

/// O teste ativo.
struct Active {
    state: GameState,
    cancel: Option<CancellationToken>,
    game: Option<GameHandle>,
    console: ConsoleBuffer,
    notify: Notify,
}

/// Quem recebe cada mudança do `game-state`.
pub(crate) type Notify = Arc<dyn Fn(&GameState) + Send + Sync>;

/// O jogo do Warden (um por vez) e o motor do launcher.
pub(crate) struct TestSessions {
    active: Mutex<Option<Active>>,
    busy: watch::Sender<bool>,
    engine: Arc<dyn LauncherEngine>,
    fake: Option<FakeGame>,
}

impl std::fmt::Debug for TestSessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestSessions")
            .field("active", &self.current().map(|state| state.pack_id))
            .field("fake", &self.fake.is_some())
            .finish_non_exhaustive()
    }
}

impl TestSessions {
    /// O motor do portablemc nas pastas do app, com o catálogo para achar as versões do Forge.
    pub(crate) fn new(paths: &AppPaths, catalog: &warden_catalog::Catalog) -> Self {
        let engine = PortableMcEngine::new(
            EngineDirs::for_app(paths),
            Some(Arc::new(CatalogForgeResolver::new(Arc::new(
                catalog.clone(),
            )))),
        );
        Self::with_engine(Arc::new(engine), FakeGame::from_env())
    }

    fn with_engine(engine: Arc<dyn LauncherEngine>, fake: Option<FakeGame>) -> Self {
        if fake.is_some() {
            tracing::warn!("jogo simulado do E2E ligado: o Testar não abre o Minecraft");
        }
        Self {
            active: Mutex::new(None),
            busy: watch::channel(false).0,
            engine,
            fake,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Active>> {
        self.active.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// O jogo aberto (ou em preparação), se houver.
    pub(crate) fn current(&self) -> Option<GameState> {
        self.lock().as_ref().map(|active| active.state.clone())
    }

    /// As linhas do console do teste ativo do pack (vazio sem teste ativo nesse pack).
    pub(crate) fn live_console(&self, pack_id: PackId) -> Vec<ConsoleLine> {
        self.lock()
            .as_ref()
            .filter(|active| active.state.pack_id == pack_id)
            .map(|active| active.console.snapshot())
            .unwrap_or_default()
    }

    /// Começa um teste. Erros: `app.GAME_ALREADY_RUNNING`, com o pack do jogo aberto.
    fn begin(&self, state: GameState, notify: Notify) -> Result<ActiveGuard<'_>, AppError> {
        let mut active = self.lock();
        if let Some(open) = active.as_ref() {
            return Err(already_running(&open.state));
        }
        notify(&state);
        *active = Some(Active {
            state,
            cancel: None,
            game: None,
            console: ConsoleBuffer::default(),
            notify,
        });
        drop(active);
        self.busy.send_replace(true);
        Ok(ActiveGuard { sessions: self })
    }

    /// Muda o teste ativo e avisa.
    fn update(&self, change: impl FnOnce(&mut Active)) {
        let notice = {
            let mut guard = self.lock();
            let Some(active) = guard.as_mut() else {
                return;
            };
            let before = active.state.clone();
            change(active);
            (active.state != before).then(|| (active.state.clone(), Arc::clone(&active.notify)))
        };
        if let Some((state, notify)) = notice {
            notify(&state);
        }
    }

    /// Guarda e numera linhas do console; devolve as numeradas.
    fn push_console(&self, lines: Vec<ConsoleLine>) -> Vec<ConsoleLine> {
        self.lock()
            .as_mut()
            .map(|active| active.console.push(lines))
            .unwrap_or_default()
    }

    /// "Parar jogo" (ou cancelar a preparação) do teste do pack. Volta quando o jogo saiu.
    /// Erros: `app.NO_GAME_RUNNING`.
    pub(crate) async fn stop(&self, pack_id: PackId) -> Result<(), AppError> {
        let (game, cancel) = {
            let guard = self.lock();
            let active = guard
                .as_ref()
                .filter(|active| active.state.pack_id == pack_id)
                .ok_or_else(|| AppError::new(AppErrorCode::NoGameRunning))?;
            (active.game.clone(), active.cancel.clone())
        };
        match (game, cancel) {
            (Some(game), _) => game.stop().await,
            (None, Some(cancel)) => cancel.cancel(),
            (None, None) => {}
        }
        Ok(())
    }

    /// Fecha o que estiver aberto e espera o teste terminar (sessão gravada), por até `wait`.
    /// Devolve se terminou a tempo.
    pub(crate) async fn stop_all(&self, wait: Duration) -> bool {
        if let Some(state) = self.current() {
            let _ = self.stop(state.pack_id).await;
        }
        let mut busy = self.busy.subscribe();
        tokio::time::timeout(wait, busy.wait_for(|busy| !busy))
            .await
            .is_ok()
    }
}

/// Erro "já existe um jogo em execução".
fn already_running(state: &GameState) -> AppError {
    AppError::new(AppErrorCode::GameAlreadyRunning)
        .with_param("packId", state.pack_id.to_string())
        .with_param("packName", state.pack_name.clone())
}

/// Libera o "um jogo por vez" quando o teste termina, de qualquer jeito.
struct ActiveGuard<'a> {
    sessions: &'a TestSessions,
}

impl Drop for ActiveGuard<'_> {
    fn drop(&mut self) {
        let finished = self.sessions.lock().take();
        if let Some(mut active) = finished {
            active.state.state = GamePhase::Exited;
            (active.notify)(&active.state);
        }
        self.sessions.busy.send_replace(false);
    }
}

/// Para onde vão os eventos do teste.
#[derive(Clone)]
pub(crate) struct TestSink {
    /// O canal do comando.
    pub(crate) events: Arc<dyn Fn(OperationEvent) + Send + Sync>,
    /// O evento `game-state`.
    pub(crate) game_state: Notify,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Testa o pack: prepara, abre o jogo e espera ele fechar. Devolve a sessão gravada.
pub(crate) async fn run_test(
    state: &AppState,
    pack_id: PackId,
    request: TestRequest,
    sink: TestSink,
) -> Result<TestSessionSummary, AppError> {
    if request.mode != TestMode::Normal {
        return Err(AppError::new(AppErrorCode::TestModeUnavailable).with_param(
            "mode",
            serde_json::to_value(request.mode)
                .ok()
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
                .unwrap_or_default(),
        ));
    }
    let record = state
        .packs
        .get(pack_id)
        .map_err(|error| AppError::from_domain(&error))?;
    let initial = GameState {
        pack_id,
        pack_name: record.name.clone(),
        state: GamePhase::Preparing,
        session_id: None,
        operation_id: None,
        profile: request.profile.clone(),
        started_at_ms: now_ms(),
    };
    let tests = &state.tests;
    let _active = tests.begin(initial, Arc::clone(&sink.game_state))?;
    let events = Arc::clone(&sink.events);
    let handle = state
        .operations
        .start(TEST_START, Some(pack_id), true)
        .with_events(move |event| events(event));
    let token = handle.token().clone();
    tests.update(|active| {
        active.state.operation_id = Some(handle.id());
        active.cancel = Some(token);
    });
    let result = Runner {
        state,
        pack_id,
        request,
        sink,
        handle: &handle,
    }
    .run()
    .await;
    handle.finish(result)
}

/// O que a preparação descobriu.
struct Prepared {
    pack_root: PathBuf,
    dirs: InstanceDirs,
    spec: GameSpec,
    pack_version: Option<String>,
    pack_tree: Option<PackTree>,
    mod_count: usize,
    game: InstalledGame,
    java: JavaRuntime,
    java_id: Option<String>,
    _lease: Option<RuntimeLease>,
}

/// Um teste em andamento.
struct Runner<'a> {
    state: &'a AppState,
    pack_id: PackId,
    request: TestRequest,
    sink: TestSink,
    handle: &'a OperationHandle,
}

impl Runner<'_> {
    async fn run(self) -> Result<TestSessionSummary, AppError> {
        let pack_root = crate::commands::inventory::pack_root(self.state, self.pack_id)?;
        let dirs = InstanceDirs::for_pack(&self.state.paths, self.pack_id);
        let hooks = crate::test_hooks::registered();
        self.run_hooks(&hooks, &pack_root, &dirs, Moment::BeforePrepare)
            .await?;
        let prepared = self.prepare(pack_root, dirs).await?;
        self.run_hooks(
            &hooks,
            &prepared.pack_root,
            &prepared.dirs,
            Moment::BeforeLaunch,
        )
        .await?;
        self.play(&hooks, prepared).await
    }

    async fn run_hooks(
        &self,
        hooks: &[Arc<dyn TestHooks>],
        pack_root: &Path,
        dirs: &InstanceDirs,
        moment: Moment,
    ) -> Result<(), AppError> {
        let context = self.hook_context(pack_root, dirs);
        for hook in hooks {
            self.handle.ensure_not_cancelled()?;
            let result = match moment {
                Moment::BeforePrepare => hook.before_prepare(&context).await,
                Moment::BeforeLaunch => hook.before_launch(&context).await,
            };
            result.inspect_err(|error| {
                tracing::info!(hook = hook.name(), code = %error.code, "gancho interrompeu o teste");
            })?;
        }
        Ok(())
    }

    fn hook_context<'b>(&'b self, pack_root: &'b Path, dirs: &'b InstanceDirs) -> HookContext<'b> {
        HookContext {
            pack_id: self.pack_id,
            pack_root,
            dirs,
            mode: self.request.mode,
            operation: self.handle,
        }
    }

    /// Passos 2 e 3: Java, Minecraft e a cópia do pack.
    async fn prepare(&self, pack_root: PathBuf, dirs: InstanceDirs) -> Result<Prepared, AppError> {
        let root = pack_root.clone();
        let info = blocking(move || read_pack_info(&root)).await?;
        let spec = game_spec(&info.requirement)?;
        let fake = self.state.tests.fake.clone();

        let settings = crate::commands::pack_meta::read_test_settings(self.state, self.pack_id)?;
        let (java, java_id, lease) = if let Some(fake) = &fake {
            (fake.java(), None, None)
        } else {
            self.handle.stage(STAGE_JAVA, STAGE_JAVA_LABEL);
            let request = JavaChoiceRequest {
                user_choice: settings.java.clone(),
                ..JavaChoiceRequest::automatic(
                    &info.requirement.minecraft,
                    loader_kind(&spec.loader),
                    spec.loader.version(),
                )
            };
            let java_service = self.state.java.clone();
            let choice = blocking(move || Ok(java_service.choose(&request)?)).await?;
            let runtime = self
                .handle
                .run_cancellable(async {
                    Ok(self
                        .state
                        .java
                        .ensure_choice(&choice, self.handle, self.handle.token())
                        .await?)
                })
                .await?;
            let lease = self.state.java.lease(&runtime.id);
            (
                JavaRuntime::from_installed(&runtime),
                Some(runtime.id.to_string()),
                Some(lease),
            )
        };

        let game = if let Some(fake) = &fake {
            fake.installed(&spec, &pack_root)
        } else {
            self.handle
                .run_cancellable(async {
                    self.state
                        .tests
                        .engine
                        .install(&spec, &java, self.handle, self.handle.token())
                        .await
                        .map_err(|error| AppError::from_domain(&error))
                })
                .await?
        };

        self.sync_instance(&pack_root, &dirs).await?;
        Ok(Prepared {
            pack_root,
            dirs,
            spec,
            pack_version: info.version,
            pack_tree: info.tree,
            mod_count: info.mod_count,
            game,
            java,
            java_id,
            _lease: lease,
        })
    }

    /// Passo 3: copia o pack para a instância de teste.
    async fn sync_instance(&self, pack_root: &Path, dirs: &InstanceDirs) -> Result<(), AppError> {
        let _read = self.state.locks.read_for(self.pack_id, self.handle).await;
        let sources = materialize_sources(self.state)?;
        let options = MaterializeOptions {
            on_modified: if self.request.replace_instance_changes {
                OnModified::Overwrite
            } else {
                OnModified::Pause
            },
            ..MaterializeOptions::default()
        };
        let outcome = warden_instance::materialize(
            &sources,
            pack_root,
            dirs,
            &options,
            self.handle,
            self.handle.token(),
        )
        .await
        .map_err(|error| AppError::from_domain(&error))?;
        let report = match outcome {
            warden_instance::Outcome::Done(report) => report,
            warden_instance::Outcome::NeedsReview(files) => {
                let list: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
                return Err(AppError::new(AppErrorCode::TestInstanceChanged)
                    .with_param("count", files.len().to_string())
                    .with_param(
                        "files",
                        list.iter().take(20).copied().collect::<Vec<_>>().join("\n"),
                    )
                    .with_detail(list.join("\n")));
            }
        };
        if let Some(error) = report.incomplete_error() {
            return Err(AppError::from_domain(&error));
        }
        if !report.blocked.is_empty() {
            let names: Vec<&str> = report
                .blocked
                .iter()
                .map(|file| file.name.as_str())
                .collect();
            return Err(AppError::new(AppErrorCode::TestManualDownloads)
                .with_param("count", names.len().to_string())
                .with_param("names", names.join(", ")));
        }
        tracing::info!(
            written = report.written,
            downloaded = report.downloaded,
            removed = report.removed,
            unchanged = report.unchanged,
            "instância de teste sincronizada"
        );
        Ok(())
    }

    /// Passo 5: a linha de comando com a memória, os argumentos e o jogador do teste.
    fn plan_launch(&self, prepared: &Prepared) -> Result<Launch, AppError> {
        let global = self.state.settings.get();
        let settings = crate::commands::pack_meta::read_test_settings(self.state, self.pack_id)?;
        let machine = MachineFingerprint::current();
        let memory = choose_memory(
            settings.memory,
            global.test_memory,
            prepared.mod_count,
            machine.memory_mb,
        );
        if prepared.java.major <= 8 && memory.mb > JAVA8_WARN_ABOVE_MB {
            self.handle.warn(
                AppError::new(AppErrorCode::TestMemoryHighJava8)
                    .with_param("memoryMb", memory.mb.to_string()),
            );
        }
        let player = OfflineProfile::from_name(&global.player_name)
            .map_err(|error| AppError::from_domain(&error))?;
        let options = launch_options(PlanInput {
            game_dir: prepared.dirs.game_dir.clone(),
            state_dir: prepared.dirs.state_dir.clone(),
            java: prepared.java.clone(),
            memory_mb: memory.mb,
            user_jvm_args: settings.jvm_args.clone(),
            player,
        });
        let signature = ProfileSignature::new(
            memory,
            settings
                .java
                .as_ref()
                .map_or_else(|| "auto".to_owned(), ToString::to_string),
            prepared.java.major,
            options.extra_jvm_args.clone(),
        );
        let command = self
            .state
            .tests
            .engine
            .command(&prepared.game, &options)
            .map_err(|error| AppError::from_domain(&error))?;
        Ok(Launch {
            command,
            memory,
            signature,
            machine,
        })
    }

    /// Passos 5 e 6: abre o jogo, acompanha e grava a sessão.
    async fn play(
        &self,
        hooks: &[Arc<dyn TestHooks>],
        prepared: Prepared,
    ) -> Result<TestSessionSummary, AppError> {
        self.handle.ensure_not_cancelled()?;
        self.handle.stage(STAGE_LAUNCH, STAGE_LAUNCH_LABEL);
        let launch = self.plan_launch(&prepared)?;
        let sessions_dir = prepared.dirs.state_dir.join("sessions");
        let first_launch = list_sessions(&sessions_dir).map_or(true, |sessions| {
            sessions.iter().all(|session| session.record.is_none())
        });
        let session = SessionDir::create(&sessions_dir, SystemTime::now())
            .map_err(|error| AppError::from_domain(&error))?;
        let session_id = session_name(&session);
        let (process, events) = GameProcess::spawn(
            &launch.command,
            SpawnOptions {
                output_log: Some(session.output_log()),
                stdin: false,
            },
        )
        .await
        .map_err(|error| AppError::from_domain(&error))?;
        let started_at = process.started_at();
        let started_ms = started_at.duration_since(UNIX_EPOCH).map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        });
        tracing::info!(pid = process.pid(), %session_id, game = %prepared.spec, "jogo aberto");

        let game = process.handle();
        self.state.tests.update(|active| {
            active.state.state = GamePhase::Running;
            active.state.session_id = Some(session_id.clone());
            active.game = Some(game.clone());
        });
        self.console(vec![ConsoleLine::warden(
            "console.abrindo",
            ConsoleLevel::Info,
            started_ms,
            [
                ("jogo", prepared.spec.to_string()),
                ("java", prepared.java.major.to_string()),
                ("memoriaMb", launch.memory.mb.to_string()),
            ],
        )]);

        // Cancelar a operação (painel Tarefas) com o jogo aberto é o mesmo que "Parar jogo".
        let stop_on_cancel = {
            let token = self.handle.token().clone();
            tokio::spawn(async move {
                token.cancelled().await;
                game.stop().await;
            })
        };
        let exit = self.supervise(events, started_ms).await;
        stop_on_cancel.abort();
        let exit = match exit {
            Some(exit) => exit,
            None => process
                .wait()
                .await
                .map_err(|error| AppError::from_domain(&error))?,
        };
        self.console(vec![exit_line(&exit, now_ms())]);
        let hook_fields = self.after_exit(hooks, &prepared, &exit, &session).await;

        let extras = SessionExtras {
            session_id: session_id.clone(),
            pack_id: self.pack_id,
            started_at_ms: started_ms,
            mode: self.request.mode,
            profile: self
                .request
                .profile
                .clone()
                .unwrap_or_else(|| "default".into()),
            pack_tree: prepared.pack_tree.clone(),
            memory: launch.memory,
            java_runtime: prepared.java_id.clone(),
            mod_count: prepared.mod_count,
            profile_signature: launch.signature,
            machine: launch.machine,
            first_launch,
        };
        let mut record = SessionRecord::from_exit(
            started_at,
            &exit,
            &prepared.spec,
            prepared.java.major,
            prepared.pack_version.clone(),
            &prepared.dirs.game_dir,
        );
        extras.apply(&mut record);
        record.extra.extend(hook_fields);
        let summary = TestSessionSummary::from_record(&session_id, &record);
        self.finish_session(&session, &record, &sessions_dir, exit.outcome)?;
        Ok(summary)
    }

    /// Passo 6: ganchos `after_exit`. Um erro vira aviso; a sessão é gravada do mesmo jeito.
    async fn after_exit(
        &self,
        hooks: &[Arc<dyn TestHooks>],
        prepared: &Prepared,
        exit: &GameExit,
        session: &SessionDir,
    ) -> serde_json::Map<String, serde_json::Value> {
        let context = self.hook_context(&prepared.pack_root, &prepared.dirs);
        let mut fields = serde_json::Map::new();
        for hook in hooks {
            let exit_context = ExitContext {
                exit,
                session_dir: session.path(),
            };
            match hook.after_exit(&context, &exit_context).await {
                Ok(more) => fields.extend(more),
                Err(error) => self.handle.warn(error),
            }
        }
        fields
    }

    /// Lê os eventos do jogo até ele sair, mandando as linhas em lotes.
    async fn supervise(&self, mut events: GameEvents, started_ms: u64) -> Option<GameExit> {
        let mut pending: Vec<ConsoleLine> = Vec::new();
        let mut ticker = tokio::time::interval(CONSOLE_BATCH);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Some(GameEvent::Line(line)) => {
                        pending.push(ConsoleLine::from_game(line, Some(started_ms)));
                    }
                    Some(GameEvent::Exited(exit)) => {
                        self.console(std::mem::take(&mut pending));
                        return Some(exit);
                    }
                    None => {
                        self.console(std::mem::take(&mut pending));
                        return None;
                    }
                },
                _ = ticker.tick() => {
                    if !pending.is_empty() {
                        self.console(std::mem::take(&mut pending));
                    }
                }
            }
        }
    }

    /// Guarda as linhas no console e manda pelo canal.
    fn console(&self, lines: Vec<ConsoleLine>) {
        if lines.is_empty() {
            return;
        }
        let numbered = self.state.tests.push_console(lines);
        (self.sink.events)(OperationEvent::Console { lines: numbered });
    }

    /// Grava o `session.json`, aplica a retenção e o "último teste" de Meus packs.
    fn finish_session(
        &self,
        session: &SessionDir,
        record: &SessionRecord,
        sessions_dir: &Path,
        outcome: Outcome,
    ) -> Result<(), AppError> {
        session
            .write_record(record)
            .map_err(|error| AppError::from_domain(&error))?;
        match prune_closed(sessions_dir, KEEP_CLOSED_SESSIONS) {
            Ok(removed) if !removed.is_empty() => {
                tracing::info!(removed = removed.len(), "sessões antigas apagadas");
            }
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "retenção das sessões não terminou"),
        }
        if let Err(error) = self
            .state
            .packs
            .set_last_test(self.pack_id, Some(last_test_value(outcome)))
        {
            tracing::warn!(%error, "último teste não gravado em packs.json");
        }
        Ok(())
    }
}

/// A abertura planejada.
struct Launch {
    command: warden_launcher::LaunchCommand,
    memory: launch_plan::MemoryChoice,
    signature: ProfileSignature,
    machine: MachineFingerprint,
}

/// Momento dos ganchos antes do jogo.
#[derive(Debug, Clone, Copy)]
enum Moment {
    BeforePrepare,
    BeforeLaunch,
}

/// A linha do Warden com o fim do jogo.
fn exit_line(exit: &GameExit, at_ms: u64) -> ConsoleLine {
    let duration = ("duracaoMs", exit.duration_ms.to_string());
    match exit.outcome {
        Outcome::StoppedByUser => {
            ConsoleLine::warden("console.encerrado", ConsoleLevel::Info, at_ms, [duration])
        }
        Outcome::ClosedNormally => {
            ConsoleLine::warden("console.fechou", ConsoleLevel::Info, at_ms, [duration])
        }
        Outcome::Crashed => ConsoleLine::warden(
            "console.travou",
            ConsoleLevel::Error,
            at_ms,
            [
                duration,
                (
                    "codigo",
                    exit.exit_code
                        .map_or_else(|| "?".to_owned(), |code| code.to_string()),
                ),
            ],
        ),
    }
}

/// Nome da pasta da sessão.
fn session_name(session: &SessionDir) -> String {
    session
        .path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// O que o teste lê do pack.
struct PackInfo {
    requirement: GameRequirement,
    version: Option<String>,
    tree: Option<PackTree>,
    mod_count: usize,
}

fn read_pack_info(root: &Path) -> Result<PackInfo, AppError> {
    let read = warden_packwiz::read_pack(root).map_err(|error| AppError::from_domain(&error))?;
    let manifest = &read.pack.value;
    let requirement = warden_instance::game_requirement(manifest)
        .map_err(|error| AppError::from_domain(&error))?;
    let mod_count = read.index.as_ref().map_or(0, |index| {
        index
            .value
            .files
            .iter()
            .filter(|entry| entry.file.starts_with("mods/"))
            .count()
    });
    let hash = pack_tree_hash(root, &manifest.index.file)
        .inspect_err(|error| tracing::warn!(%error, "hash da árvore do pack não calculado"))
        .ok();
    let tree = hash.map(|hash| {
        let repo = warden_versioning::PackRepo::open(root).ok();
        PackTree {
            hash,
            head: repo.as_ref().and_then(|repo| repo.head_id().ok().flatten()),
            unsaved_files: repo
                .as_ref()
                .and_then(|repo| repo.unsaved_changes().ok())
                .map_or(0, |changes| {
                    u32::try_from(changes.len()).unwrap_or(u32::MAX)
                }),
        }
    });
    let version = Some(manifest.version.trim().to_owned()).filter(|version| !version.is_empty());
    Ok(PackInfo {
        requirement,
        version,
        tree,
        mod_count,
    })
}

/// O jogo do `pack.toml`. Erros: `app.TEST_LOADER_UNSUPPORTED` (`LiteLoader`) e os de versão.
fn game_spec(requirement: &GameRequirement) -> Result<GameSpec, AppError> {
    let loader = match &requirement.loader {
        None => LoaderSpec::Vanilla,
        Some(loader) => {
            let version = loader.version.clone();
            match loader.loader.as_str() {
                "fabric" => LoaderSpec::Fabric { version },
                "forge" => LoaderSpec::Forge { version },
                "neoforge" => LoaderSpec::NeoForge { version },
                "quilt" => LoaderSpec::Quilt { version },
                other => {
                    return Err(AppError::new(AppErrorCode::TestLoaderUnsupported)
                        .with_param("loader", other));
                }
            }
        }
    };
    GameSpec::new(&requirement.minecraft, loader).map_err(|error| AppError::from_domain(&error))
}

/// O loader para a política de Java.
fn loader_kind(loader: &LoaderSpec) -> LoaderKind {
    match loader {
        LoaderSpec::Vanilla => LoaderKind::Vanilla,
        LoaderSpec::Fabric { .. } => LoaderKind::Fabric,
        LoaderSpec::Quilt { .. } => LoaderKind::Quilt,
        LoaderSpec::Forge { .. } => LoaderKind::Forge,
        LoaderSpec::NeoForge { .. } => LoaderKind::NeoForge,
    }
}

/// Rede, CurseForge (com a chave, quando houver) e o cache de downloads.
fn materialize_sources(state: &AppState) -> Result<Sources, AppError> {
    let http = warden_http::HttpClient::new(warden_http::HttpConfig::for_version(env!(
        "CARGO_PKG_VERSION"
    )))
    .map_err(|error| AppError::from_domain(&error))?;
    let key = state.secrets.get(warden_secrets::SecretKind::Curseforge)?;
    let curseforge = match key {
        Some(key) => Some(
            warden_curseforge::CurseforgeClient::new(http.clone(), Some(key))
                .map_err(|error| AppError::from_domain(&error))?,
        ),
        None => None,
    };
    let cache = DownloadCache::open(&state.paths.downloads_cache_dir())
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(Sources {
        http,
        curseforge,
        cache: Arc::new(cache),
    })
}

/// Roda trabalho de disco fora da thread do IPC.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::internal(format!("tarefa do teste interrompida: {error}")))?
}

/// As sessões gravadas do pack, da mais nova para a mais antiga (as que não terminaram de ser
/// gravadas ficam de fora).
pub(crate) fn sessions_of(
    paths: &AppPaths,
    pack_id: PackId,
) -> Result<Vec<TestSessionSummary>, AppError> {
    let dirs = InstanceDirs::for_pack(paths, pack_id);
    let sessions = list_sessions(&dirs.state_dir.join("sessions"))
        .map_err(|error| AppError::from_domain(&error))?;
    Ok(sessions
        .iter()
        .filter_map(|session| {
            let record = session.record.as_ref()?;
            let id = session_name(&session.dir);
            Some(TestSessionSummary::from_record(&id, record))
        })
        .collect())
}

/// A pasta de uma sessão gravada. Erros: `app.TEST_SESSION_NOT_FOUND` (nome estranho ou
/// pasta ausente).
pub(crate) fn session_dir(
    paths: &AppPaths,
    pack_id: PackId,
    session_id: &str,
) -> Result<SessionDir, AppError> {
    let valid = !session_id.is_empty()
        && session_id.len() <= 64
        && session_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'-' | b'_'));
    let not_found =
        || AppError::new(AppErrorCode::TestSessionNotFound).with_param("sessionId", session_id);
    if !valid {
        return Err(not_found());
    }
    let path = InstanceDirs::for_pack(paths, pack_id)
        .state_dir
        .join("sessions")
        .join(session_id);
    if !path.is_dir() {
        return Err(not_found());
    }
    Ok(SessionDir::open(path))
}

#[cfg(test)]
mod tests;
