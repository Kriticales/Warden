//! O processo do servidor local (ARCHITECTURE §7.6): o mesmo supervisor do jogo
//! (`warden_launcher::process`: Job Object no Windows, grupo de processos no Linux, pipes,
//! decodificação, `output.log`), com a entrada padrão aberta para comandos.
//!
//! - [`ServerCommand`] monta a linha de comando (memória, codificação, log4j seguro,
//!   `-Dfml.queryResult=confirm` no Forge, `nogui`).
//! - [`RunningServer::spawn`] abre e devolve os [`ServerEvent`]: cada linha, o "pronto"
//!   (`Done (…)! For help, type "help"`), cada jogador que entrou e a saída.
//! - [`ServerHandle::stop`] manda `stop` pelo stdin e, se o servidor não fechar em
//!   [`STOP_TIMEOUT`] (60 s), encerra a árvore à força.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use tokio::sync::mpsc;
use warden_launcher::log::LogLine;
use warden_launcher::process::{GameEvent, GameExit, GameHandle, GameProcess, SpawnOptions};
use warden_launcher::{JavaRuntime, LaunchCommand, LoaderSpec};

use crate::error::{Error, Result};
use crate::install::{InstalledServer, ServerLaunch};
use crate::markers::{joined_line, ready_line};

/// Quanto o servidor tem para fechar depois do `stop` antes de ser encerrado à força.
pub const STOP_TIMEOUT: Duration = Duration::from_mins(1);

/// Maior comando aceito pelo stdin.
pub const MAX_COMMAND_LEN: usize = 256;

/// Memória e argumentos do servidor.
#[derive(Debug, Clone)]
pub struct ServerCommand {
    /// O Java (o servidor usa o `java` de console, sem janela).
    pub java: JavaRuntime,
    /// Memória máxima, em MB.
    pub memory_mb: u32,
    /// Argumentos extras da JVM (antes do jar).
    pub extra_jvm_args: Vec<String>,
}

impl ServerCommand {
    /// A linha de comando do servidor instalado.
    #[must_use]
    pub fn build(&self, server: &InstalledServer) -> LaunchCommand {
        let mut args = vec![
            format!("-Xmx{}M", self.memory_mb),
            "-XX:ErrorFile=hs_err_pid%p.log".to_owned(),
        ];
        args.extend(
            warden_launcher::command::encoding_args(self.java.major)
                .iter()
                .map(|&arg| arg.to_owned()),
        );
        args.extend(crate::log4j::log4j_args(
            &server.spec.loader,
            &server.release_time,
        ));
        if matches!(server.spec.loader, LoaderSpec::Forge { .. }) {
            // Forge 1.12.2 pergunta no stdin se o mundo pode abrir sem algum mod; o servidor
            // do teste é descartável.
            args.push("-Dfml.queryResult=confirm".to_owned());
        }
        args.extend(self.extra_jvm_args.iter().cloned());
        match &server.launch {
            ServerLaunch::Jar { jar } => {
                args.extend(["-jar".to_owned(), jar.clone(), "nogui".to_owned()]);
            }
            ServerLaunch::ArgsFile { file } => {
                args.extend([format!("@{file}"), "nogui".to_owned()]);
            }
            ServerLaunch::Class {
                class_dir,
                main_class,
                args: extra,
            } => {
                args.extend([
                    "-cp".to_owned(),
                    class_dir.display().to_string(),
                    main_class.clone(),
                ]);
                args.extend(extra.iter().cloned());
            }
        }
        LaunchCommand {
            program: self.java.java.clone(),
            args,
            cwd: server.dir.clone(),
            env: Vec::new(),
            argfile: None,
        }
    }
}

/// O que o servidor fez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    /// Uma linha da saída.
    Line(LogLine),
    /// Ficou pronto (`Done`), `elapsed_ms` depois de abrir; `reported_ms` é o tempo que o
    /// próprio servidor escreveu.
    Ready {
        /// Desde a abertura do processo.
        elapsed_ms: u64,
        /// O que o servidor escreveu (`Done (6.912s)`).
        reported_ms: Option<u64>,
    },
    /// Um jogador entrou.
    Joined {
        /// O nome do jogador.
        player: String,
        /// Desde a abertura do processo.
        elapsed_ms: u64,
    },
    /// O processo terminou (sempre o último evento).
    Exited(GameExit),
}

/// Os eventos do servidor.
pub type ServerEvents = mpsc::UnboundedReceiver<ServerEvent>;

/// Alça do servidor aberto: comandos, parar e encerrar; barata de clonar.
#[derive(Debug, Clone)]
pub struct ServerHandle {
    game: GameHandle,
    stop_timeout: Duration,
}

/// Como o servidor parou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopOutcome {
    /// O `stop` chegou ao servidor.
    pub sent: bool,
    /// O servidor não fechou a tempo e foi encerrado à força.
    pub forced: bool,
}

impl ServerHandle {
    /// Manda um comando pelo stdin (`reload`, `kubejs reload server_scripts`, `spark …`).
    ///
    /// Erros: [`Error::InvalidCommand`] (vazio, com quebra de linha ou maior que
    /// [`MAX_COMMAND_LEN`]); [`Error::NotRunning`] com o servidor fechado.
    pub async fn send_command(&self, command: &str) -> Result<()> {
        let command = command.trim();
        if command.is_empty()
            || command.len() > MAX_COMMAND_LEN
            || command.chars().any(char::is_control)
        {
            return Err(Error::InvalidCommand);
        }
        if self.game.has_exited() {
            return Err(Error::NotRunning);
        }
        self.game
            .send_line(command)
            .await
            .map_err(|_| Error::NotRunning)
    }

    /// Para o servidor: `stop` pelo stdin e, se não fechar em [`STOP_TIMEOUT`], encerra a
    /// árvore à força. Volta quando o processo saiu.
    pub async fn stop(&self) -> StopOutcome {
        if self.game.has_exited() {
            return StopOutcome {
                sent: false,
                forced: false,
            };
        }
        let sent = self.game.send_line("stop").await.is_ok();
        let deadline = Instant::now() + self.stop_timeout;
        while !self.game.has_exited() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let forced = !self.game.has_exited();
        if forced {
            tracing::warn!("o servidor não fechou depois do stop; encerrando à força");
            self.game.stop().await;
        }
        StopOutcome { sent, forced }
    }

    /// Encerra a árvore do servidor na hora (fechar o Warden).
    pub async fn kill(&self) {
        self.game.stop().await;
    }

    /// Se o processo já saiu.
    #[must_use]
    pub fn has_exited(&self) -> bool {
        self.game.has_exited()
    }

    /// Pico de memória da árvore (Job Object; no Linux, `None`).
    #[must_use]
    pub fn peak_memory_bytes(&self) -> Option<u64> {
        self.game.peak_memory_bytes()
    }
}

/// Um servidor aberto.
#[derive(Debug)]
pub struct RunningServer {
    process: GameProcess,
    handle: ServerHandle,
}

impl RunningServer {
    /// Abre o servidor, com a saída em `output_log`.
    ///
    /// Erros: os da abertura do processo ([`Error::Process`]).
    pub async fn spawn(
        command: &LaunchCommand,
        output_log: Option<PathBuf>,
    ) -> Result<(Self, ServerEvents)> {
        Self::spawn_with_stop_timeout(command, output_log, STOP_TIMEOUT).await
    }

    /// O mesmo, com outro prazo para o `stop` (testes).
    pub async fn spawn_with_stop_timeout(
        command: &LaunchCommand,
        output_log: Option<PathBuf>,
        stop_timeout: Duration,
    ) -> Result<(Self, ServerEvents)> {
        let (process, mut events) = GameProcess::spawn(
            command,
            SpawnOptions {
                output_log,
                stdin: true,
            },
        )
        .await?;
        let handle = ServerHandle {
            game: process.handle(),
            stop_timeout,
        };
        let (sender, receiver) = mpsc::unbounded_channel();
        let started = Instant::now();
        tokio::spawn(async move {
            let mut ready = false;
            while let Some(event) = events.recv().await {
                match event {
                    GameEvent::Line(line) => {
                        let elapsed_ms =
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                        let marker = if ready {
                            joined_line(&line.text)
                                .map(|player| ServerEvent::Joined { player, elapsed_ms })
                        } else {
                            ready_line(&line.text).map(|reported_ms| ServerEvent::Ready {
                                elapsed_ms,
                                reported_ms,
                            })
                        };
                        ready |= matches!(marker, Some(ServerEvent::Ready { .. }));
                        let _ = sender.send(ServerEvent::Line(line));
                        if let Some(marker) = marker {
                            let _ = sender.send(marker);
                        }
                    }
                    GameEvent::Exited(exit) => {
                        let _ = sender.send(ServerEvent::Exited(exit));
                    }
                }
            }
        });
        Ok((Self { process, handle }, receiver))
    }

    /// O pid do processo principal.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.process.pid()
    }

    /// Quando o processo começou.
    #[must_use]
    pub fn started_at(&self) -> SystemTime {
        self.process.started_at()
    }

    /// A alça (comandos e parar).
    #[must_use]
    pub fn handle(&self) -> ServerHandle {
        self.handle.clone()
    }

    /// Espera o processo terminar.
    ///
    /// Erros: [`Error::Process`] se a tarefa que vigia o processo sumiu (bug).
    pub async fn wait(self) -> Result<GameExit> {
        Ok(self.process.wait().await?)
    }
}

/// Espera o "pronto" lendo os eventos e entregando cada um a `on_event` (o console). Devolve o
/// evento [`ServerEvent::Ready`]; se o servidor sair antes, devolve a saída como erro.
///
/// Erros: [`Error::ExitedBeforeReady`] (com a saída em `exit`), [`Error::ReadyTimeout`].
pub async fn wait_ready(
    events: &mut ServerEvents,
    timeout: Duration,
    mut on_event: impl FnMut(&ServerEvent),
) -> std::result::Result<ServerEvent, ReadyFailure> {
    let wait = async {
        while let Some(event) = events.recv().await {
            on_event(&event);
            match event {
                ServerEvent::Ready { .. } => return Ok(event),
                ServerEvent::Exited(exit) => return Err(ReadyFailure::Exited(exit)),
                ServerEvent::Line(_) | ServerEvent::Joined { .. } => {}
            }
        }
        Err(ReadyFailure::Exited(GameExit {
            outcome: warden_launcher::outcome::Outcome::Crashed,
            exit_code: None,
            stop_requested: false,
            duration_ms: 0,
            artifacts: warden_launcher::outcome::CrashArtifacts::default(),
        }))
    };
    tokio::time::timeout(timeout, wait)
        .await
        .unwrap_or(Err(ReadyFailure::Timeout))
}

/// Por que o servidor não ficou pronto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadyFailure {
    /// Fechou antes (travou, ou fechou por causa da EULA).
    Exited(GameExit),
    /// Não ficou pronto a tempo.
    Timeout,
}

impl ReadyFailure {
    /// O erro correspondente.
    #[must_use]
    pub fn error(&self, timeout: Duration) -> Error {
        match self {
            Self::Exited(exit) => Error::ExitedBeforeReady {
                exit_code: exit.exit_code,
            },
            Self::Timeout => Error::ReadyTimeout {
                seconds: timeout.as_secs(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linha_de_comando_por_tipo_de_servidor() {
        let java = JavaRuntime {
            java: PathBuf::from("java"),
            launcher: PathBuf::from("javaw"),
            major: 21,
        };
        let command = ServerCommand {
            java,
            memory_mb: 4096,
            extra_jvm_args: vec!["-XX:+UseG1GC".into()],
        };
        let forge = InstalledServer {
            dir: PathBuf::from("servidor"),
            spec: warden_launcher::GameSpec::new(
                "1.20.1",
                LoaderSpec::Forge {
                    version: "47.3.0".into(),
                },
            )
            .unwrap(),
            release_time: "2023-06-12T13:25:51+00:00".into(),
            launch: ServerLaunch::ArgsFile {
                file: "libraries/net/minecraftforge/forge/1.20.1-47.3.0/unix_args.txt".into(),
            },
        };
        let line = command.build(&forge);
        assert_eq!(
            line.program,
            PathBuf::from("java"),
            "java de console, não javaw"
        );
        assert_eq!(line.cwd, PathBuf::from("servidor"));
        assert_eq!(line.args[0], "-Xmx4096M");
        assert!(line.args.contains(&"-Dstdout.encoding=UTF-8".to_owned()));
        assert!(line.args.contains(&"-Dfml.queryResult=confirm".to_owned()));
        assert!(
            !line
                .args
                .iter()
                .any(|arg| arg.starts_with("-Dlog4j.configurationFile"))
        );
        assert_eq!(
            &line.args[line.args.len() - 3..],
            [
                "-XX:+UseG1GC",
                "@libraries/net/minecraftforge/forge/1.20.1-47.3.0/unix_args.txt",
                "nogui"
            ]
        );
        let old = InstalledServer {
            spec: warden_launcher::GameSpec::new("1.12.2", LoaderSpec::Vanilla).unwrap(),
            release_time: "2017-09-18T08:39:46+00:00".into(),
            launch: ServerLaunch::Jar {
                jar: "server.jar".into(),
            },
            ..forge
        };
        let line = command.build(&old);
        assert!(
            line.args
                .contains(&"-Dlog4j.configurationFile=warden-log4j2.xml".to_owned())
        );
        assert!(!line.args.contains(&"-Dfml.queryResult=confirm".to_owned()));
        assert_eq!(
            &line.args[line.args.len() - 3..],
            ["-jar", "server.jar", "nogui"]
        );
    }
}
