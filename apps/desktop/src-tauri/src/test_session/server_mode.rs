//! Testar como servidor (SPEC T27; ARCHITECTURE §7.6; ADR-0032; ROADMAP L-09): o modo
//! [`TestMode::Server`] do Testar, com as peças do servidor local em
//! [`crate::server_session`].
//!
//! Etapas, na operação do teste:
//! 1. ganchos `before_prepare` ("Verificar o pack");
//! 2. a EULA: sem o aceite, o teste para com `server.EULA_NOT_ACCEPTED` antes de qualquer
//!    download, e nenhum `eula=true` é gravado (CA-T27-02);
//! 3. **Preparar o Minecraft** e **Copiar o pack para o teste** (o jogo; em "Só o servidor",
//!    só o Java);
//! 4. **Preparar o servidor** ([`STAGE_SERVER`]): instala o servidor do loader na primeira vez
//!    e copia o pack para `instances/<pack>/server/` só com os itens de servidor (os "Só
//!    cliente" ficam de fora; CA-T27-04);
//! 5. ganchos `before_launch` do jogo (não em "Só o servidor");
//! 6. **Abrir o servidor e o jogo**: `eula.txt` e `server.properties` controlado (porta livre),
//!    o servidor abre com o console próprio (linhas com a origem `server`; o log fica em
//!    `<sessão>/server/output.log`, ou no `output.log` em "Só o servidor") e, no "Done", o jogo
//!    abre e entra nele pelo Quick Play (`--quickPlayMultiplayer`, ou `--server`/`--port`
//!    antes da 1.20);
//! 7. o jogo fechou (ou "Parar jogo e servidor"): o servidor recebe `stop` (à força depois de
//!    60 s), e o `session.json` ganha o campo [`SESSION_FIELD`] com o lado do servidor
//!    ([`ServerSide`]: como fechou, quando ficou pronto, se o jogo entrou, "Por que travou").
//!
//! Se o servidor travar antes do "Done", o jogo não abre e o resultado é "O servidor travou",
//! com o mod apontado e o trecho do log do servidor.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use warden_core::ProgressSink as _;
use warden_instance::{
    InstallSide, InstanceDirs, MaterializeOptions, OnModified, OptionalChoices, OptionalSelection,
};
use warden_launcher::outcome::Outcome;
use warden_launcher::process::GameExit;
use warden_launcher::session::SessionRecord;
use warden_launcher::{GameSpec, JavaRuntime, LaunchOptions, QuickPlay};
use warden_server::{ServerCommand, ServerEvent, ServerEvents, ServerHandle};

use super::console::{ConsoleLevel, ConsoleLine, ConsoleOrigin};
use super::launch_plan::MemoryChoice;
use super::records::{ProfileSignature, SessionExtras};
use super::system::MachineFingerprint;
use super::{
    CONSOLE_BATCH, Moment, OpenedSession, Runner, STAGE_LAUNCH, STAGE_LAUNCH_LABEL,
    TestSessionSummary, TestSessions, blocking, game_spec, now_ms, read_pack_info,
};
use crate::commands::server::ServerPhase;
use crate::error::AppError;
use crate::events::{GamePhase, OperationEvent};
use crate::server_session::{
    READY_TIMEOUT, STAGE_SERVER, STAGE_SERVER_LABEL, ServerCrash, diagnose_crash, eula_accepted_at,
    server_dirs,
};
use crate::test_hooks::TestHooks;

/// Campo do `session.json` com o lado do servidor.
pub(crate) const SESSION_FIELD: &str = "server";

/// Linhas do fim do log do servidor guardadas para o "Por que travou".
const TAIL_LINES: usize = 400;

/// O lado do servidor numa sessão "Testar como servidor".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerSide {
    /// "Só o servidor" (sem o jogo).
    pub(crate) only: bool,
    /// Como o servidor fechou.
    pub(crate) outcome: Outcome,
    /// Código de saída.
    pub(crate) exit_code: Option<i32>,
    /// Do início do processo à saída.
    #[specta(type = specta_typescript::Number)]
    pub(crate) duration_ms: u64,
    /// Do início do processo ao "Done" (`None`: não ficou pronto).
    #[specta(type = Option<specta_typescript::Number>)]
    pub(crate) ready_ms: Option<u64>,
    /// A porta em `127.0.0.1`.
    pub(crate) port: u16,
    /// Memória do servidor, em MB.
    pub(crate) memory_mb: u32,
    /// O jogo do lado do servidor: não abriu, abriu ou entrou no servidor.
    pub(crate) game: ServerGame,
    /// Não fechou com o `stop` em 60 s e foi encerrado à força.
    pub(crate) stop_forced: bool,
    /// "Por que travou", quando travou.
    pub(crate) crash: Option<ServerCrash>,
}

/// O jogo, visto do servidor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ServerGame {
    /// O jogo não abriu ("Só o servidor", ou o servidor travou antes do "Done").
    NotOpened,
    /// O jogo abriu, mas o servidor não viu o jogador entrar.
    Opened,
    /// O servidor viu o jogador entrar.
    Joined,
}

/// O lado do servidor de um registro gravado (`None` fora do "Testar como servidor").
pub(crate) fn server_side(record: &SessionRecord) -> Option<ServerSide> {
    serde_json::from_value(record.extra.get(SESSION_FIELD)?.clone()).ok()
}

/// Onde fica o log do servidor numa sessão com o jogo.
pub(crate) fn server_log_path(session_dir: &Path) -> PathBuf {
    session_dir.join("server").join("output.log")
}

/// O jogo entra no servidor local pelo Quick Play (L-08: `--quickPlayMultiplayer` na 1.20+;
/// `--server`/`--port` antes).
pub(crate) fn join_server(options: &mut LaunchOptions, port: u16) {
    options.quick_play = Some(QuickPlay::Multiplayer {
        host: warden_server::LOCAL_HOST.to_owned(),
        port,
    });
}

/// O pior dos dois resultados: um lado que travou faz o teste ter travado.
pub(crate) fn worst(game: Outcome, server: Outcome) -> Outcome {
    if server == Outcome::Crashed {
        Outcome::Crashed
    } else {
        game
    }
}

/// O que a leitura do servidor viu até agora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Readiness {
    Waiting,
    Ready { elapsed_ms: u64 },
    Exited,
}

/// O que a leitura do servidor guardou até ele sair.
#[derive(Debug, Default)]
struct PumpReport {
    joined: bool,
    tail: Vec<String>,
}

/// O servidor aberto durante o teste.
pub(crate) struct ServerLink {
    port: u16,
    memory_mb: u32,
    only: bool,
    handle: ServerHandle,
    running: warden_server::RunningServer,
    pump: tokio::task::JoinHandle<PumpReport>,
    /// O "pronto" e a saída, vistos pela leitura.
    readiness: watch::Receiver<Readiness>,
    ready_ms: Option<u64>,
    game_opened: bool,
    /// A pasta do `output.log` do servidor.
    log_dir: PathBuf,
    server_dir: PathBuf,
    pack_root: PathBuf,
}

/// Lê os eventos do servidor até ele sair: as linhas vão para o console do teste (origem
/// `server`) em lotes de até 50 ms, o fim do log fica guardado e o "pronto" é avisado.
async fn pump(
    mut events: ServerEvents,
    tests: Arc<TestSessions>,
    sink: Arc<dyn Fn(OperationEvent) + Send + Sync>,
    started_ms: u64,
    ready: watch::Sender<Readiness>,
) -> PumpReport {
    let mut report = PumpReport::default();
    let mut pending: Vec<ConsoleLine> = Vec::new();
    let mut ticker = tokio::time::interval(CONSOLE_BATCH);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let flush = |pending: &mut Vec<ConsoleLine>| {
        if !pending.is_empty() {
            let numbered = tests.push_console(std::mem::take(pending));
            sink(OperationEvent::Console { lines: numbered });
        }
    };
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(ServerEvent::Line(line)) => {
                    report.tail.push(line.text.clone());
                    if report.tail.len() > TAIL_LINES {
                        report.tail.remove(0);
                    }
                    let mut console = ConsoleLine::from_game(line, Some(started_ms));
                    console.origin = ConsoleOrigin::Server;
                    pending.push(console);
                }
                Some(ServerEvent::Ready { elapsed_ms, .. }) => {
                    flush(&mut pending);
                    ready.send_replace(Readiness::Ready { elapsed_ms });
                }
                Some(ServerEvent::Joined { .. }) => report.joined = true,
                Some(ServerEvent::Exited(_)) | None => {
                    flush(&mut pending);
                    ready.send_replace(Readiness::Exited);
                    return report;
                }
            },
            _ = ticker.tick() => flush(&mut pending),
        }
    }
}

impl ServerLink {
    /// A porta do servidor.
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// Para o servidor (se ainda estiver aberto), espera a saída e acrescenta o lado do
    /// servidor ao registro. Devolve como o servidor fechou.
    pub(crate) async fn finish(self, runner: &Runner<'_>, record: &mut SessionRecord) -> Outcome {
        let stop = if self.handle.has_exited() {
            warden_server::StopOutcome {
                sent: false,
                forced: false,
            }
        } else {
            runner
                .state
                .servers
                .update(|state| state.phase = ServerPhase::Stopping);
            runner.console(vec![ConsoleLine::warden(
                "servidor:console.parando",
                ConsoleLevel::Info,
                now_ms(),
                [],
            )]);
            self.handle.stop().await
        };
        let report = self.pump.await.unwrap_or_default();
        let exit = match self.running.wait().await {
            Ok(exit) => exit,
            Err(error) => {
                tracing::warn!(%error, "saída do servidor local não lida");
                GameExit {
                    outcome: Outcome::Crashed,
                    exit_code: None,
                    stop_requested: false,
                    duration_ms: 0,
                    artifacts: warden_launcher::outcome::CrashArtifacts::default(),
                }
            }
        };
        // O servidor que fechou porque o teste acabou "fechou normalmente", mesmo forçado.
        let outcome = match exit.outcome {
            Outcome::StoppedByUser if stop.forced => Outcome::ClosedNormally,
            outcome => outcome,
        };
        let crash = if outcome == Outcome::Crashed {
            Some(
                diagnose_crash(
                    runner.state,
                    runner.pack_id,
                    &self.pack_root,
                    &self.log_dir,
                    &self.server_dir,
                    &exit,
                    &report.tail,
                )
                .await,
            )
        } else {
            None
        };
        runner.console(vec![server_exit_line(outcome, &exit, now_ms())]);
        let side = ServerSide {
            only: self.only,
            outcome,
            exit_code: exit.exit_code,
            duration_ms: exit.duration_ms,
            ready_ms: self.ready_ms,
            port: self.port,
            memory_mb: self.memory_mb,
            game: match (self.game_opened, report.joined) {
                (_, true) => ServerGame::Joined,
                (true, false) => ServerGame::Opened,
                (false, false) => ServerGame::NotOpened,
            },
            stop_forced: stop.forced,
            crash,
        };
        tracing::info!(?outcome, game = ?side.game, forced = side.stop_forced, "servidor local fechou");
        if let Ok(value) = serde_json::to_value(&side) {
            record.extra.insert(SESSION_FIELD.to_owned(), value);
        }
        outcome
    }
}

/// A linha do Warden com o fim do servidor.
fn server_exit_line(outcome: Outcome, exit: &GameExit, at_ms: u64) -> ConsoleLine {
    let duration = ("duracaoMs", exit.duration_ms.to_string());
    match outcome {
        Outcome::Crashed => ConsoleLine::warden(
            "servidor:console.travou",
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
        Outcome::ClosedNormally | Outcome::StoppedByUser => ConsoleLine::warden(
            "servidor:console.fechou",
            ConsoleLevel::Info,
            at_ms,
            [duration],
        ),
    }
}

/// O que a sessão "só com o servidor" guarda do pack e do Java.
struct SessionFacts {
    java_id: Option<String>,
    pack_version: Option<String>,
    pack_tree: Option<super::records::PackTree>,
    mod_count: usize,
}

/// O que a preparação do servidor deixou pronto para abrir.
struct PreparedServer {
    installed: warden_server::InstalledServer,
    dirs: InstanceDirs,
    memory_mb: u32,
}

impl Runner<'_> {
    /// O teste com o servidor local (o modo [`TestMode::Server`](super::TestMode::Server)).
    pub(super) async fn run_server(
        &self,
        hooks: &[Arc<dyn TestHooks>],
        pack_root: PathBuf,
        dirs: InstanceDirs,
    ) -> Result<TestSessionSummary, AppError> {
        // CA-T27-02: sem o aceite, nada é baixado nem aberto, e nenhum eula=true é gravado.
        if eula_accepted_at(self.state).is_none() {
            let server = server_dirs(&self.state.paths, self.pack_id);
            if server.game_dir.is_dir() {
                let _ = warden_server::write_eula(&server.game_dir, None);
            }
            return Err(AppError::from_domain(
                &warden_server::Error::EulaNotAccepted,
            ));
        }
        let _server = self.state.servers.begin(self.pack_id);
        if self.request.server_only {
            return self.run_server_only(pack_root).await;
        }
        let prepared = self.prepare(pack_root, dirs).await?;
        let minecraft = prepared.spec.minecraft.as_str().to_owned();
        if warden_launcher::quick_play::needs_accessibility_onboarding_off(&minecraft) {
            // A tela de boas-vindas de acessibilidade da 1.21+ segura o Quick Play também no
            // multiplayer (S-R5-3 §4.1); antes da linha de base da captura (ganchos abaixo).
            let game_dir = prepared.dirs.game_dir.clone();
            blocking(move || {
                warden_launcher::quick_play::disable_accessibility_onboarding(&game_dir)
                    .map(|_| ())
                    .map_err(|error| {
                        AppError::internal(format!("gravar o options.txt da instância: {error}"))
                    })
            })
            .await?;
        }
        let server = self
            .prepare_server(
                &prepared.pack_root,
                &prepared.spec,
                prepared.mod_count,
                &prepared.java,
            )
            .await?;
        self.run_hooks(
            hooks,
            &prepared.pack_root,
            &prepared.dirs,
            Moment::BeforeLaunch,
        )
        .await?;
        self.handle.ensure_not_cancelled()?;
        self.handle.stage(STAGE_LAUNCH, STAGE_LAUNCH_LABEL);
        let opened = OpenedSession::create(prepared.sessions_dir.clone())?;
        let log_dir = opened.session.path().join("server");
        let mut link = self
            .open_server(
                server,
                &prepared.pack_root,
                &prepared.spec,
                &prepared.java,
                &opened,
                log_dir,
                false,
            )
            .await?;
        if link.ready_ms.is_none() {
            // O servidor travou antes do "Done": o jogo não abre.
            return self
                .finish_without_game(link, &prepared.spec, &prepared.java, &prepared, opened)
                .await;
        }
        let launch = match self.plan_launch(&prepared, Some(link.port())) {
            Ok(launch) => launch,
            Err(error) => {
                link.handle.kill().await;
                return Err(error);
            }
        };
        link.game_opened = true;
        self.play_in(hooks, prepared, launch, opened, Some(link))
            .await
    }

    /// Passo 4: instala o servidor e copia o pack só com os itens de servidor.
    async fn prepare_server(
        &self,
        pack_root: &Path,
        spec: &GameSpec,
        mod_count: usize,
        java: &JavaRuntime,
    ) -> Result<PreparedServer, AppError> {
        self.handle.ensure_not_cancelled()?;
        self.handle.stage(STAGE_SERVER, STAGE_SERVER_LABEL);
        let dirs = server_dirs(&self.state.paths, self.pack_id);
        let installed =
            crate::server_session::install(self.state, pack_root, spec, java, &dirs, self.handle)
                .await?;
        let options = MaterializeOptions {
            side: InstallSide::Server,
            // A pasta do servidor é descartável: o que o servidor mudou volta à versão do pack.
            on_modified: OnModified::Overwrite,
            optionals: self
                .profile
                .fields
                .optionals
                .clone()
                .map_or(OptionalSelection::Saved, |choices| {
                    OptionalSelection::Explicit(OptionalChoices { choices })
                }),
            ..MaterializeOptions::default()
        };
        self.sync_with(pack_root, &dirs, &options).await?;
        Ok(PreparedServer {
            installed,
            dirs,
            memory_mb: warden_server::server_memory_mb(mod_count),
        })
    }

    /// Passo 6: `eula.txt`, `server.properties`, o processo e a espera do "Done". Com o
    /// servidor travado antes, devolve o vínculo sem `ready_ms`.
    #[allow(clippy::too_many_arguments)] // as peças do teste que o servidor usa
    async fn open_server(
        &self,
        server: PreparedServer,
        pack_root: &Path,
        spec: &GameSpec,
        java: &JavaRuntime,
        opened: &OpenedSession,
        log_dir: PathBuf,
        only: bool,
    ) -> Result<ServerLink, AppError> {
        let record = self
            .state
            .packs
            .get(self.pack_id)
            .map_err(|error| AppError::from_domain(&error))?;
        let server_dir = server.dirs.game_dir.clone();
        // Dois arquivos pequenos e uma porta testada: rápido o bastante para não sair da tarefa.
        let port = crate::server_session::configure(self.state, &server_dir, &record.name)?;
        let command = ServerCommand {
            java: java.clone(),
            memory_mb: server.memory_mb,
            extra_jvm_args: Vec::new(),
        }
        .build(&server.installed);
        let output_log = log_dir.join("output.log");
        let (running, events) = self.state.servers.spawn(&command, output_log, port).await?;
        let handle = running.handle();
        let started_ms = running.started_at().duration_since(UNIX_EPOCH).map_or_else(
            |_| now_ms(),
            |elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        );
        self.state.tests.update(|active| {
            active.state.session_id = Some(opened.session_id.clone());
            if only {
                active.state.state = GamePhase::Running;
            }
        });
        self.console(vec![ConsoleLine::warden(
            "servidor:console.abrindo",
            ConsoleLevel::Info,
            started_ms,
            [
                ("jogo", spec.to_string()),
                ("java", java.major.to_string()),
                ("memoriaMb", server.memory_mb.to_string()),
                ("porta", port.to_string()),
            ],
        )]);
        let (ready_tx, ready_rx) = watch::channel(Readiness::Waiting);
        let pump = tokio::spawn(pump(
            events,
            Arc::clone(&self.state.tests),
            Arc::clone(&self.sink.events),
            started_ms,
            ready_tx,
        ));
        let link = ServerLink {
            port,
            memory_mb: server.memory_mb,
            only,
            handle,
            running,
            pump,
            readiness: ready_rx,
            ready_ms: None,
            game_opened: false,
            log_dir,
            server_dir,
            pack_root: pack_root.to_path_buf(),
        };
        self.wait_server_ready(link, opened).await
    }

    /// Espera o "Done" (ou a saída, o tempo esgotado, o cancelamento).
    async fn wait_server_ready(
        &self,
        mut link: ServerLink,
        opened: &OpenedSession,
    ) -> Result<ServerLink, AppError> {
        let port = link.port;
        let token = self.handle.token().clone();
        let mut ready_rx = link.readiness.clone();
        // O valor sai copiado: a guarda do `watch` não atravessa os `await` abaixo.
        let wait = async move {
            match tokio::time::timeout(
                READY_TIMEOUT,
                ready_rx.wait_for(|readiness| *readiness != Readiness::Waiting),
            )
            .await
            {
                Ok(Ok(readiness)) => Some(*readiness),
                Ok(Err(_)) => Some(Readiness::Exited),
                Err(_) => None,
            }
        };
        let readiness = tokio::select! {
            readiness = wait => readiness,
            () = token.cancelled() => {
                link.handle.kill().await;
                let _ = link.pump.await;
                let _ = std::fs::remove_dir_all(opened.session.path());
                return Err(AppError::from_domain(&warden_core::CoreError::Cancelled));
            }
        };
        match readiness {
            Some(Readiness::Ready { elapsed_ms }) => {
                link.ready_ms = Some(elapsed_ms);
                self.state.servers.update(|state| {
                    state.phase = ServerPhase::Ready;
                    state.ready_ms = Some(elapsed_ms);
                });
                self.console(vec![ConsoleLine::warden(
                    "servidor:console.pronto",
                    ConsoleLevel::Info,
                    now_ms(),
                    [
                        ("duracaoMs", elapsed_ms.to_string()),
                        ("porta", port.to_string()),
                    ],
                )]);
                Ok(link)
            }
            Some(Readiness::Exited | Readiness::Waiting) => Ok(link),
            None => {
                link.handle.kill().await;
                let _ = link.pump.await;
                Err(AppError::from_domain(&warden_server::Error::ReadyTimeout {
                    seconds: READY_TIMEOUT.as_secs(),
                }))
            }
        }
    }

    /// "Só o servidor": prepara, abre e acompanha até "Parar" (ou o servidor fechar).
    async fn run_server_only(&self, pack_root: PathBuf) -> Result<TestSessionSummary, AppError> {
        let root = pack_root.clone();
        let info = blocking(move || read_pack_info(&root)).await?;
        let spec = game_spec(&info.requirement)?;
        let (java, java_id, _lease) = self.choose_java(&info.requirement, &spec).await?;
        let server = self
            .prepare_server(&pack_root, &spec, info.mod_count, &java)
            .await?;
        self.handle.ensure_not_cancelled()?;
        self.handle.stage(STAGE_LAUNCH, STAGE_LAUNCH_LABEL);
        let client_dirs = InstanceDirs::for_pack(&self.state.paths, self.pack_id);
        let opened = OpenedSession::create(client_dirs.state_dir.join("sessions"))?;
        let log_dir = opened.session.path().to_path_buf();
        let link = self
            .open_server(server, &pack_root, &spec, &java, &opened, log_dir, true)
            .await?;
        if link.ready_ms.is_some() {
            // "Parar" (ou cancelar no painel Tarefas) com o servidor aberto: `stop`.
            let token = self.handle.token().clone();
            let mut exited = link.readiness.clone();
            let closed = async move {
                let _ = exited
                    .wait_for(|readiness| *readiness == Readiness::Exited)
                    .await;
            };
            tokio::select! {
                () = token.cancelled() => {}
                () = closed => {}
            }
        }
        let user_stopped = self.handle.token().is_cancelled();
        let facts = SessionFacts {
            java_id,
            pack_version: info.version.clone(),
            pack_tree: info.tree.clone(),
            mod_count: info.mod_count,
        };
        self.record_server_only(link, &spec, &java, facts, opened, user_stopped)
            .await
    }

    /// O servidor travou antes do "Done" num teste com o jogo: a sessão fica com o servidor.
    async fn finish_without_game(
        &self,
        link: ServerLink,
        spec: &GameSpec,
        java: &JavaRuntime,
        prepared: &super::Prepared,
        opened: OpenedSession,
    ) -> Result<TestSessionSummary, AppError> {
        let facts = SessionFacts {
            java_id: prepared.java_id.clone(),
            pack_version: prepared.pack_version.clone(),
            pack_tree: prepared.pack_tree.clone(),
            mod_count: prepared.mod_count,
        };
        self.record_server_only(link, spec, java, facts, opened, false)
            .await
    }

    /// Grava a sessão em que só o servidor abriu.
    async fn record_server_only(
        &self,
        link: ServerLink,
        spec: &GameSpec,
        java: &JavaRuntime,
        facts: SessionFacts,
        opened: OpenedSession,
        user_stopped: bool,
    ) -> Result<TestSessionSummary, AppError> {
        let memory = MemoryChoice {
            mb: link.memory_mb,
            auto: true,
        };
        let started_at = link.running.started_at();
        let started_ms = started_at.duration_since(UNIX_EPOCH).map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        });
        let server_dir = link.server_dir.clone();
        let mut record = SessionRecord::from_exit(
            started_at,
            &GameExit {
                outcome: Outcome::ClosedNormally,
                exit_code: None,
                stop_requested: false,
                duration_ms: 0,
                artifacts: warden_launcher::outcome::CrashArtifacts::default(),
            },
            spec,
            java.major,
            facts.pack_version,
            &server_dir,
        );
        let mut outcome = link.finish(self, &mut record).await;
        let side = server_side(&record);
        if let Some(side) = &side {
            record.duration_ms = side.duration_ms;
            record.exit_code = side.exit_code;
        }
        if user_stopped && outcome != Outcome::Crashed {
            outcome = Outcome::StoppedByUser;
        }
        record.outcome = outcome;
        let extras = SessionExtras {
            session_id: opened.session_id.clone(),
            pack_id: self.pack_id,
            started_at_ms: started_ms,
            mode: self.request.mode,
            profile: self.profile.id.clone(),
            pack_tree: facts.pack_tree,
            memory,
            java_runtime: facts.java_id,
            mod_count: facts.mod_count,
            profile_signature: ProfileSignature::new(
                memory,
                "auto".to_owned(),
                java.major,
                Vec::new(),
                None,
                "server",
            ),
            machine: MachineFingerprint::current(),
            first_launch: opened.first_launch,
        };
        extras.apply(&mut record);
        let summary = TestSessionSummary::from_record(&opened.session_id, &record);
        self.finish_session(&opened.session, &record, &opened.sessions_dir, outcome)?;
        Ok(summary)
    }
}

#[cfg(test)]
#[path = "server_mode_tests.rs"]
mod tests;
