//! Uma rodada da busca que precisa do servidor local (D-13, CA-T25-06).
//!
//! A D-12 materializa o mesmo subconjunto nos diretórios exclusivos `bisect/client` e
//! `bisect/server`, instala o servidor com [`crate::server_session::install`] e chama
//! [`PreparedRound::prepare`] depois do aceite explícito no diálogo. `run` é o ponto de
//! encaixe do laço da D-12: o cliente recebe `QuickPlay::Multiplayer`, que o launcher
//! traduz para `--server`/`--port` antes da 1.20 ou Quick Play depois. A D-12 fornece
//! o processo do cliente, seu log e o token de cancelamento. O resultado traz os dois
//! logs; [`verdict`] usa a assinatura v6 da D-06 em cada lado.
//!
//! A pasta de servidor é **exclusiva** da busca: nunca se usa `instances/<pack>/server`
//! da L-09. O `world` preexistente é recusado para não apagar dados de outro teste.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use warden_core::{CancellationToken, PackId};
use warden_diagnostics::postcrash::{AnalysisContext, LogSource, SourceKind};
use warden_diagnostics::{CrashSignature, SIGNATURE_VERSION, signature};
use warden_launcher::process::GameExit;
use warden_launcher::{JavaRuntime, QuickPlay};
use warden_server::{InstalledServer, ServerCommand, ServerEvent};

use crate::error::AppError;
use crate::server_session::{READY_TIMEOUT, ServerSessions};

/// A D-12 só deve abrir o servidor após explicar a necessidade e registrar o aceite.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Consent {
    pub(crate) server: bool,
    pub(crate) insufficient_memory: bool,
}

/// Dados exibidos antes de pedir o aceite do servidor.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RoundPlan {
    pub(crate) memory: warden_server::MemoryPlan,
    pub(crate) eula_accepted: bool,
}

pub(crate) fn plan(
    mod_count: usize,
    game_mb: u32,
    free_mb: Option<u64>,
    eula: Option<&str>,
) -> RoundPlan {
    RoundPlan {
        memory: warden_server::memory_plan(
            warden_server::server_memory_mb(mod_count),
            Some(game_mb),
            free_mb,
        ),
        eula_accepted: eula.is_some_and(|date| !date.trim().is_empty()),
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RoundError {
    #[error("o servidor local exige aceite explícito para a busca")]
    ConsentRequired,
    #[error("a memória livre é insuficiente para o servidor e o jogo")]
    InsufficientMemory,
    #[error("a pasta world da busca já existe")]
    WorldExists,
    #[error("o mundo de origem contém um link simbólico")]
    WorldSymlink,
    #[error("a cópia do mundo ficaria dentro do mundo de origem")]
    WorldRecursive,
    #[error("o mundo de origem não é uma pasta")]
    WorldMissing,
    #[error("a rodada foi cancelada")]
    Cancelled,
    #[error("não foi possível copiar o mundo descartável: {0}")]
    WorldIo(#[source] std::io::Error),
    #[error(transparent)]
    Server(#[from] warden_server::Error),
    #[error(transparent)]
    App(#[from] AppError),
    #[error("não foi possível analisar os logs da rodada: {0}")]
    Diagnosis(#[from] warden_diagnostics::DiagnosticsError),
}

/// A cópia existe apenas até a parada do servidor. O `Drop` também cobre erro na preparação.
struct WorldCopy(PathBuf);

impl Drop for WorldCopy {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(kind = ?error.kind(), "não foi possível apagar a cópia do mundo da busca");
        }
    }
}

fn copy_world(source: &Path, destination: &Path) -> Result<WorldCopy, RoundError> {
    if !source.is_dir() {
        return Err(RoundError::WorldMissing);
    }
    if source
        .symlink_metadata()
        .map_err(RoundError::WorldIo)?
        .file_type()
        .is_symlink()
    {
        return Err(RoundError::WorldSymlink);
    }
    let parent = destination.parent().ok_or(RoundError::WorldRecursive)?;
    if parent
        .canonicalize()
        .map_err(RoundError::WorldIo)?
        .starts_with(source.canonicalize().map_err(RoundError::WorldIo)?)
    {
        return Err(RoundError::WorldRecursive);
    }
    if destination.exists() || destination.symlink_metadata().is_ok() {
        return Err(RoundError::WorldExists);
    }
    std::fs::create_dir(destination).map_err(RoundError::WorldIo)?;
    let world = WorldCopy(destination.to_path_buf());
    copy_contents(source, destination)?;
    Ok(world)
}

fn copy_contents(source: &Path, destination: &Path) -> Result<(), RoundError> {
    for entry in std::fs::read_dir(source).map_err(RoundError::WorldIo)? {
        let entry = entry.map_err(RoundError::WorldIo)?;
        let kind = entry.file_type().map_err(RoundError::WorldIo)?;
        if kind.is_symlink() {
            return Err(RoundError::WorldSymlink);
        }
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            std::fs::create_dir(&target).map_err(RoundError::WorldIo)?;
            copy_contents(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target).map_err(RoundError::WorldIo)?;
        }
    }
    Ok(())
}

/// O servidor instalado e a cópia do mundo prontos para uma única rodada.
pub(crate) struct PreparedRound {
    command: warden_launcher::LaunchCommand,
    port: u16,
    server_log: PathBuf,
    _world: WorldCopy,
}

impl PreparedRound {
    /// `installed.dir` deve ser o diretório exclusivo do servidor da busca, já materializado
    /// com `InstallSide::Server`. O aceite da EULA vem exclusivamente das configurações do app.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        installed: &InstalledServer,
        java: &JavaRuntime,
        source_world: &Path,
        server_log: PathBuf,
        motd: &str,
        plan: RoundPlan,
        consent: Consent,
        eula_accepted_at: Option<&str>,
        first_port: u16,
    ) -> Result<Self, RoundError> {
        if !consent.server {
            return Err(RoundError::ConsentRequired);
        }
        if !plan.memory.fits && !consent.insufficient_memory {
            return Err(RoundError::InsufficientMemory);
        }
        if !plan.eula_accepted || eula_accepted_at.is_none_or(|date| date.trim().is_empty()) {
            return Err(RoundError::Server(warden_server::Error::EulaNotAccepted));
        }
        let world = copy_world(source_world, &installed.dir.join("world"))?;
        warden_server::write_eula(&installed.dir, eula_accepted_at)?;
        let port = warden_server::properties::free_port_from(first_port)?;
        warden_server::write_properties(
            &installed.dir,
            &warden_server::ControlledProperties {
                port,
                motd: motd.to_owned(),
            },
        )?;
        // A pasta é exclusiva; um `level-name` vindo do pack não pode apontar para outro mundo.
        let properties = installed
            .dir
            .join(warden_server::properties::PROPERTIES_FILE);
        let existing = std::fs::read_to_string(&properties).map_err(RoundError::WorldIo)?;
        let mut safe = existing
            .lines()
            .filter(|line| {
                line.trim_start().split(['=', ':', ' ', '\t']).next() != Some("level-name")
            })
            .collect::<Vec<_>>()
            .join("\n");
        safe.push_str("\nlevel-name=world\n");
        std::fs::write(&properties, safe).map_err(RoundError::WorldIo)?;
        let command = ServerCommand {
            java: java.clone(),
            memory_mb: plan.memory.server_mb,
            extra_jvm_args: Vec::new(),
        }
        .build(installed);
        Ok(Self {
            command,
            port,
            server_log,
            _world: world,
        })
    }

    /// A D-12 passa uma função que abre o cliente com o `QuickPlay` indicado e devolve
    /// seu `output.log`. Ela deve respeitar o token e encerrar a árvore do jogo ao cancelar.
    /// O servidor usa o registro da L-09 e é parado/aguardado antes de apagar o mundo.
    pub(crate) async fn run<F, Fut>(
        self,
        sessions: &Arc<ServerSessions>,
        pack_id: PackId,
        cancel: &CancellationToken,
        client: F,
    ) -> Result<RoundLogs, RoundError>
    where
        F: FnOnce(QuickPlay, CancellationToken) -> Fut,
        Fut: Future<Output = Result<PathBuf, AppError>>,
    {
        let guard = sessions.begin(pack_id);
        let result = self.run_inner(sessions, cancel, client).await;
        guard.finish().await;
        result
    }

    async fn run_inner<F, Fut>(
        &self,
        sessions: &ServerSessions,
        cancel: &CancellationToken,
        client: F,
    ) -> Result<RoundLogs, RoundError>
    where
        F: FnOnce(QuickPlay, CancellationToken) -> Fut,
        Fut: Future<Output = Result<PathBuf, AppError>>,
    {
        if cancel.is_cancelled() {
            return Err(RoundError::Cancelled);
        }
        let (running, mut events) = sessions
            .spawn(&self.command, self.server_log.clone(), self.port)
            .await?;
        let handle = running.handle();
        let ready = tokio::select! {
            result = warden_server::process::wait_ready(&mut events, READY_TIMEOUT, |_| {}) => result.is_ok(),
            () = cancel.cancelled() => false,
        };
        let mut client_log = None;
        let mut client_error = None;
        let mut cancelled = cancel.is_cancelled();
        if ready && !cancelled {
            let join = QuickPlay::Multiplayer {
                host: warden_server::LOCAL_HOST.to_owned(),
                port: self.port,
            };
            let child_cancel = cancel.child_token();
            let future = client(join, child_cancel.clone());
            tokio::pin!(future);
            loop {
                tokio::select! {
                    result = &mut future => {
                        match result {
                            Ok(path) => client_log = Some(path),
                            Err(error) => client_error = Some(error),
                        }
                        break;
                    }
                    () = cancel.cancelled() => {
                        cancelled = true;
                        handle.kill().await;
                        child_cancel.cancel();
                        let _ = tokio::time::timeout(Duration::from_secs(5), &mut future).await;
                        break;
                    }
                    event = events.recv() => {
                        if matches!(event, Some(ServerEvent::Exited(_)) | None) {
                            child_cancel.cancel();
                            if let Ok(Ok(path)) = tokio::time::timeout(
                                Duration::from_secs(5), &mut future
                            ).await {
                                client_log = Some(path);
                            }
                            break;
                        }
                    }
                }
            }
        }
        cancelled |= cancel.is_cancelled();
        if cancelled {
            handle.kill().await;
        } else {
            handle.stop().await;
        }
        let exit = running.wait().await?;
        if cancelled {
            return Err(RoundError::Cancelled);
        }
        if let Some(error) = client_error {
            return Err(RoundError::App(error));
        }
        Ok(RoundLogs {
            server: self.server_log.clone(),
            client: client_log,
            server_exit: exit,
        })
    }
}

/// Os logs ficam na sessão da busca, mesmo depois de apagar o mundo descartável.
pub(crate) struct RoundLogs {
    pub(crate) server: PathBuf,
    pub(crate) client: Option<PathBuf>,
    pub(crate) server_exit: GameExit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RoundVerdict {
    Same {
        side: &'static str,
        signature: CrashSignature,
    },
    Other {
        side: &'static str,
        signature: CrashSignature,
    },
    NoCrash,
}

/// Compara cada lado com a assinatura de referência; o laço da D-12 aplica também seus
/// marcadores de sucesso, falha sem crash, timeout e dependência inferida.
pub(crate) fn verdict(
    logs: &RoundLogs,
    context: &AnalysisContext,
    expected: &CrashSignature,
) -> Result<RoundVerdict, RoundError> {
    let mut other = None;
    for (side, path) in [
        ("server", Some(&logs.server)),
        ("client", logs.client.as_ref()),
    ] {
        let Some(path) = path else { continue };
        let source = LogSource::read(path, format!("{side}/output.log"), SourceKind::Output)?;
        let sources = [source];
        let analysis = warden_diagnostics::postcrash::analyze(&sources, context, None)?;
        let found = signature(&analysis, &sources);
        if analysis.primary().is_none() && found.exception.is_none() && found.subject.is_empty() {
            continue;
        }
        if expected.version == SIGNATURE_VERSION && found.hash == expected.hash {
            return Ok(RoundVerdict::Same {
                side,
                signature: found,
            });
        }
        other.get_or_insert(RoundVerdict::Other {
            side,
            signature: found,
        });
    }
    Ok(other.unwrap_or(RoundVerdict::NoCrash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead as _, Write as _};
    use std::process::Command;
    use warden_launcher::{GameSpec, LaunchCommand, LoaderSpec};
    use warden_server::{ServerLaunch, properties::port_is_free};

    const SERVER_GOLDEN: &str = include_str!(
        "../../../../crates/warden-launcher/tests/matrix/matriz/forge-1.16.5-servidor.log"
    );
    const CLIENT_GOLDEN: &str = include_str!(
        "../../../../crates/warden-launcher/tests/matrix/matriz/forge-1.16.5-cliente.log"
    );
    const CRASH_GOLDEN: &str = include_str!(
        "../../../../crates/warden-diagnostics/tests/corpus-assinatura/forge-1.16.5-init-essential-portalmod/crash-reports/crash.txt"
    );

    // Estes dois testes também são executados como processos filhos pelo teste de integração.
    #[test]
    fn fake_server_process() {
        if std::env::var_os("WARDEN_BISECT_FAKE_SERVER").is_none() {
            return;
        }
        std::io::stdout()
            .write_all(SERVER_GOLDEN.as_bytes())
            .unwrap();
        std::io::stdout().flush().unwrap();
        for line in std::io::stdin().lock().lines() {
            if line.unwrap() == "stop" {
                std::io::stdout()
                    .write_all(b"[Server thread/INFO]: Stopping the server\n")
                    .unwrap();
                break;
            }
        }
    }

    #[test]
    fn fake_client_process() {
        if std::env::var_os("WARDEN_BISECT_FAKE_CLIENT").is_some() {
            std::io::stdout()
                .write_all(CLIENT_GOLDEN.as_bytes())
                .unwrap();
            std::io::stdout().flush().unwrap();
        }
    }

    fn fixture_world(root: &Path) -> PathBuf {
        let world = root.join("fonte");
        std::fs::create_dir(&world).unwrap();
        std::fs::write(world.join("level.dat"), b"mundo original").unwrap();
        world
    }

    fn fake_round(root: &Path) -> (PreparedRound, u16, PathBuf) {
        let server_dir = root.join("servidor");
        std::fs::create_dir(&server_dir).unwrap();
        let world = fixture_world(root);
        let world_copy = copy_world(&world, &server_dir.join("world")).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let command = LaunchCommand {
            program: std::env::current_exe().unwrap(),
            args: vec![
                "--exact".into(),
                "bisect_server::tests::fake_server_process".into(),
                "--nocapture".into(),
            ],
            cwd: server_dir.clone(),
            env: vec![("WARDEN_BISECT_FAKE_SERVER".into(), "1".into())],
            argfile: None,
        };
        let server_log = root.join("sessao").join("server").join("output.log");
        (
            PreparedRound {
                command,
                port,
                server_log,
                _world: world_copy,
            },
            port,
            world,
        )
    }

    #[test]
    fn plano_exige_eula_e_soma_memoria() {
        let p = plan(120, 6144, Some(8192), None);
        assert!(!p.eula_accepted);
        assert_eq!(p.memory.server_mb, 4096);
        assert!(!p.memory.fits);
    }

    #[test]
    fn assinatura_v6_compara_crashes_dos_dois_lados() {
        let temp = tempfile::tempdir().unwrap();
        let server = temp.path().join("server.log");
        let client = temp.path().join("client.log");
        std::fs::write(&server, SERVER_GOLDEN).unwrap();
        std::fs::write(&client, CRASH_GOLDEN).unwrap();
        let context = AnalysisContext::default();
        let source = LogSource::from_text("client/output.log", SourceKind::Output, CRASH_GOLDEN);
        let sources = [source];
        let analysis = warden_diagnostics::postcrash::analyze(&sources, &context, None).unwrap();
        let expected = signature(&analysis, &sources);
        assert_eq!(expected.version, 6);
        let logs = RoundLogs {
            server,
            client: Some(client),
            server_exit: GameExit {
                outcome: warden_launcher::outcome::Outcome::ClosedNormally,
                exit_code: Some(0),
                stop_requested: true,
                duration_ms: 1,
                artifacts: warden_launcher::outcome::CrashArtifacts::default(),
            },
        };
        assert!(matches!(
            verdict(&logs, &context, &expected).unwrap(),
            RoundVerdict::Same { side: "client", .. }
        ));
    }

    #[test]
    fn mundo_e_copiado_e_apagado_sem_tocar_na_origem() {
        let temp = tempfile::tempdir().unwrap();
        let source = fixture_world(temp.path());
        let target = temp.path().join("world");
        let copy = copy_world(&source, &target).unwrap();
        assert_eq!(
            std::fs::read(target.join("level.dat")).unwrap(),
            b"mundo original"
        );
        assert!(matches!(
            copy_world(&source, &target),
            Err(RoundError::WorldExists)
        ));
        drop(copy);
        assert!(!target.exists());
        assert_eq!(
            std::fs::read(source.join("level.dat")).unwrap(),
            b"mundo original"
        );
    }

    #[test]
    fn sem_aceite_nao_grava_eula_nem_copia_mundo() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("servidor");
        std::fs::create_dir(&dir).unwrap();
        let source = fixture_world(temp.path());
        let installed = InstalledServer {
            dir: dir.clone(),
            spec: GameSpec::new("1.16.5", LoaderSpec::Vanilla).unwrap(),
            release_time: String::new(),
            launch: ServerLaunch::Jar {
                jar: "server.jar".into(),
            },
        };
        let java = JavaRuntime {
            java: "java".into(),
            launcher: "java".into(),
            major: 8,
        };
        let p = plan(10, 4096, Some(8192), None);
        let result = PreparedRound::prepare(
            &installed,
            &java,
            &source,
            temp.path().join("output.log"),
            "teste",
            p,
            Consent {
                server: true,
                insufficient_memory: false,
            },
            None,
            25565,
        );
        assert!(matches!(
            result,
            Err(RoundError::Server(warden_server::Error::EulaNotAccepted))
        ));
        assert!(!dir.join("eula.txt").exists());
        assert!(!dir.join("world").exists());
    }

    #[test]
    fn aceite_prepara_servidor_local_com_mundo_controlado() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("servidor");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(
            dir.join("server.properties"),
            "level-name=../outro\ndifficulty=hard\n",
        )
        .unwrap();
        let source = fixture_world(temp.path());
        let installed = InstalledServer {
            dir: dir.clone(),
            spec: GameSpec::new("1.16.5", LoaderSpec::Vanilla).unwrap(),
            release_time: String::new(),
            launch: ServerLaunch::Jar {
                jar: "server.jar".into(),
            },
        };
        let java = JavaRuntime {
            java: "java".into(),
            launcher: "java".into(),
            major: 8,
        };
        let accepted_at = "2026-10-17T00:00:00Z";
        let p = plan(120, 6144, Some(8192), Some(accepted_at));
        let round = PreparedRound::prepare(
            &installed,
            &java,
            &source,
            temp.path().join("output.log"),
            "busca",
            p,
            Consent {
                server: true,
                insufficient_memory: true,
            },
            Some(accepted_at),
            25565,
        )
        .unwrap();
        let properties = std::fs::read_to_string(dir.join("server.properties")).unwrap();
        assert!(properties.contains("server-ip=127.0.0.1"));
        assert!(properties.contains("level-name=world"));
        assert!(properties.contains("difficulty=hard"));
        assert!(!properties.contains("../outro"));
        assert!(
            std::fs::read_to_string(dir.join("eula.txt"))
                .unwrap()
                .contains("eula=true")
        );
        assert!(dir.join("world").join("level.dat").is_file());
        drop(round);
        assert!(!dir.join("world").exists());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn processos_simulados_gravam_dois_logs_e_param_servidor() {
        let temp = tempfile::tempdir().unwrap();
        let (round, port, world) = fake_round(temp.path());
        let sessions = Arc::new(ServerSessions::default());
        let client_log = temp.path().join("sessao").join("client").join("output.log");
        let logs = round
            .run(
                &sessions,
                PackId::new(),
                &CancellationToken::new(),
                |quick_play, _| async move {
                    assert_eq!(
                        quick_play,
                        QuickPlay::Multiplayer {
                            host: "127.0.0.1".into(),
                            port,
                        }
                    );
                    let output = Command::new(std::env::current_exe().unwrap())
                        .args([
                            "--exact",
                            "bisect_server::tests::fake_client_process",
                            "--nocapture",
                        ])
                        .env("WARDEN_BISECT_FAKE_CLIENT", "1")
                        .output()
                        .unwrap();
                    assert!(output.status.success());
                    std::fs::create_dir_all(client_log.parent().unwrap()).unwrap();
                    std::fs::write(&client_log, output.stdout).unwrap();
                    Ok(client_log)
                },
            )
            .await
            .unwrap();
        let server_text = std::fs::read_to_string(&logs.server).unwrap();
        let client_text = std::fs::read_to_string(logs.client.unwrap()).unwrap();
        assert!(server_text.contains("Done (5.173s)!"));
        assert!(server_text.contains("Stopping the server"));
        assert!(client_text.contains("Forge") || client_text.contains("forge"));
        assert!(port_is_free(port));
        assert!(sessions.current().is_none());
        assert!(!temp.path().join("servidor").join("world").exists());
        assert_eq!(
            std::fs::read(world.join("level.dat")).unwrap(),
            b"mundo original"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn erro_do_cliente_ainda_para_servidor_e_apaga_mundo() {
        let temp = tempfile::tempdir().unwrap();
        let (round, port, _) = fake_round(temp.path());
        let log = round.server_log.clone();
        let sessions = Arc::new(ServerSessions::default());
        let result = round
            .run(
                &sessions,
                PackId::new(),
                &CancellationToken::new(),
                |_, _| async { Err(AppError::internal("cliente simulado falhou")) },
            )
            .await;
        assert!(matches!(result, Err(RoundError::App(_))));
        assert!(
            std::fs::read_to_string(log)
                .unwrap()
                .contains("Stopping the server")
        );
        assert!(port_is_free(port));
        assert!(!temp.path().join("servidor").join("world").exists());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelar_a_rodada_encerra_servidor_e_apaga_mundo() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let temp = tempfile::tempdir().unwrap();
        let (round, port, _) = fake_round(temp.path());
        let sessions = Arc::new(ServerSessions::default());
        let cancel = CancellationToken::new();
        let to_cancel = cancel.clone();
        let client_finished = Arc::new(AtomicBool::new(false));
        let child_finished = Arc::clone(&client_finished);
        let result = round
            .run(
                &sessions,
                PackId::new(),
                &cancel,
                move |_, client_cancel| async move {
                    to_cancel.cancel();
                    client_cancel.cancelled().await;
                    child_finished.store(true, Ordering::SeqCst);
                    Err(AppError::cancelled())
                },
            )
            .await;
        assert!(matches!(
            result,
            Err(RoundError::Cancelled | RoundError::App(_))
        ));
        assert!(port_is_free(port));
        assert!(client_finished.load(Ordering::SeqCst));
        assert!(!temp.path().join("servidor").join("world").exists());
    }

    /// Executar sob demanda com `WARDEN_BISECT_FORGE_DIR`, `WARDEN_BISECT_WORLD` e
    /// `WARDEN_TEST_JAVA`: usa o Forge 1.16.5 já instalado no diretório indicado.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "servidor"]
    async fn server_forge_1_16_5_real() {
        let dir = PathBuf::from(std::env::var_os("WARDEN_BISECT_FORGE_DIR").unwrap());
        let world = PathBuf::from(std::env::var_os("WARDEN_BISECT_WORLD").unwrap());
        let java = PathBuf::from(std::env::var_os("WARDEN_TEST_JAVA").unwrap());
        let jar = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .find(|name| {
                name.starts_with("forge-1.16.5")
                    && !name.contains("installer")
                    && Path::new(name)
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
            })
            .unwrap();
        let installed = InstalledServer {
            dir: dir.clone(),
            spec: GameSpec::new(
                "1.16.5",
                LoaderSpec::Forge {
                    version: "36.2.39".into(),
                },
            )
            .unwrap(),
            release_time: "2021-01-15T16:05:09+00:00".into(),
            launch: ServerLaunch::Jar { jar },
        };
        let java = JavaRuntime {
            java: java.clone(),
            launcher: java,
            major: 8,
        };
        let temp = tempfile::tempdir().unwrap();
        let p = plan(120, 4096, None, Some("2026-10-17T00:00:00Z"));
        let round = PreparedRound::prepare(
            &installed,
            &java,
            &world,
            temp.path().join("server.log"),
            "busca",
            p,
            Consent {
                server: true,
                insufficient_memory: false,
            },
            Some("2026-10-17T00:00:00Z"),
            25565,
        )
        .unwrap();
        let sessions = Arc::new(ServerSessions::default());
        let logs = round
            .run(
                &sessions,
                PackId::new(),
                &CancellationToken::new(),
                |_, _| async { Ok(temp.path().join("client.log")) },
            )
            .await
            .unwrap();
        assert!(
            std::fs::read_to_string(logs.server)
                .unwrap()
                .contains("Done (")
        );
        assert!(!dir.join("world").exists());
    }
}
