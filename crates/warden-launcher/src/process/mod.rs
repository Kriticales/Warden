//! O processo do jogo, iniciado e supervisionado pelo Warden (ARCHITECTURE §7.4).
//!
//! - `stdout` e `stderr` por pipes, lidos em tarefas separadas, linha a linha; cada linha é
//!   decodificada ([`crate::decode`]), lida pelo [`LogParser`] e enviada como
//!   [`GameEvent::Line`]; o texto cru vai para o `output.log` da sessão.
//! - Windows: o jogo entra num Job Object com `KILL_ON_JOB_CLOSE` logo depois de criado.
//!   "Parar jogo" encerra o job (o jogo e todos os filhos); fechar o Warden fecha o handle e o
//!   sistema mata o job (CA-T13-02, CA-T13-07).
//! - Linux: grupo de processos próprio; parar manda `SIGTERM` ao grupo e `SIGKILL` depois de
//!   3 s.
//! - Quando o processo principal sai, o que sobrar da árvore é encerrado (para os pipes
//!   fecharem e nenhum `java` ficar órfão), e o resultado é classificado
//!   ([`crate::outcome`]).

#[cfg(windows)]
mod windows;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, watch};

use crate::decode::{decode_line, trim_line_end};
use crate::error::{Error, Result};
use crate::log::{LogLine, LogParser, LogSource};
use crate::outcome::{CrashArtifacts, Outcome, classify, find_crash_artifacts};
use crate::spec::LaunchCommand;

/// Espera entre o `SIGTERM` e o `SIGKILL` no Linux.
pub const TERM_GRACE: Duration = Duration::from_secs(3);

/// `CREATE_NO_WINDOW`: sem janela de console piscando (o `javaw` nem tem console).
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Como abrir.
#[derive(Debug, Clone, Default)]
pub struct SpawnOptions {
    /// Onde gravar tudo o que sair dos pipes (`output.log` da sessão).
    pub output_log: Option<PathBuf>,
    /// Deixar a entrada padrão aberta para comandos (servidor local).
    pub stdin: bool,
}

/// O que o jogo fez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    /// Uma linha (ou evento do log4j) da saída.
    Line(LogLine),
    /// O processo terminou (sempre o último evento).
    Exited(GameExit),
}

/// Como o processo terminou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameExit {
    /// Classificação.
    pub outcome: Outcome,
    /// Código de saída (`None` se morto por sinal no Linux).
    pub exit_code: Option<i32>,
    /// Se "Parar jogo" foi pedido.
    pub stop_requested: bool,
    /// Do início do processo até a saída.
    pub duration_ms: u64,
    /// Crash reports e `hs_err_pid` novos.
    pub artifacts: CrashArtifacts,
}

/// Árvore de processos do jogo.
#[derive(Debug)]
struct Tree {
    #[cfg(windows)]
    job: windows::Job,
    #[cfg(unix)]
    group: Option<rustix::process::Pid>,
}

impl Tree {
    #[cfg_attr(unix, allow(clippy::unnecessary_wraps))] // Só o Windows pode falhar aqui.
    fn new() -> std::io::Result<Self> {
        Ok(Self {
            #[cfg(windows)]
            job: windows::Job::new()?,
            #[cfg(unix)]
            group: None,
        })
    }

    fn configure(command: &mut Command) {
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        #[cfg(unix)]
        command.process_group(0);
        command.kill_on_drop(true);
    }

    #[cfg_attr(unix, allow(clippy::unnecessary_wraps))] // Só o Windows pode falhar aqui.
    fn attach(&mut self, child: &Child) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            let handle = child
                .raw_handle()
                .ok_or_else(|| std::io::Error::other("o processo já terminou"))?;
            self.job.assign(handle)
        }
        #[cfg(unix)]
        {
            self.group = child
                .id()
                .and_then(|id| i32::try_from(id).ok())
                .and_then(rustix::process::Pid::from_raw);
            Ok(())
        }
    }

    /// Pede para fechar: no Windows encerra o job; no Linux manda `SIGTERM` ao grupo.
    fn terminate(&self) {
        #[cfg(windows)]
        self.job.terminate();
        #[cfg(unix)]
        if let Some(group) = self.group {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::TERM);
        }
    }

    /// Mata tudo na hora.
    fn kill(&self) {
        #[cfg(windows)]
        self.job.terminate();
        #[cfg(unix)]
        if let Some(group) = self.group {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
    }

    /// Processos vivos na árvore (só o Windows sabe contar; no Linux, `None`).
    #[allow(clippy::unnecessary_wraps)] // Some no Windows, None no Linux.
    #[cfg_attr(unix, allow(clippy::unused_self))] // Só o Windows usa self.
    fn active_processes(&self) -> Option<u32> {
        #[cfg(windows)]
        {
            Some(self.job.active_processes())
        }
        #[cfg(unix)]
        {
            None
        }
    }
}

/// Estado compartilhado entre o processo, as alças e a tarefa que vigia a saída.
#[derive(Debug)]
struct Control {
    tree: Tree,
    stop_requested: AtomicBool,
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    exited: watch::Receiver<bool>,
}

/// Alça para parar o jogo ou mandar comandos; barata de clonar.
#[derive(Debug, Clone)]
pub struct GameHandle {
    control: Arc<Control>,
}

impl GameHandle {
    /// "Parar jogo": encerra o jogo e todos os filhos. No Windows na hora; no Linux com
    /// `SIGTERM` e, se não sair em [`TERM_GRACE`], `SIGKILL`. Volta quando o processo
    /// principal saiu (ou depois do `SIGKILL`).
    pub async fn stop(&self) {
        self.control.stop_requested.store(true, Ordering::SeqCst);
        let mut exited = self.control.exited.clone();
        if *exited.borrow() {
            self.control.tree.kill();
            return;
        }
        self.control.tree.terminate();
        let finished = tokio::time::timeout(TERM_GRACE, exited.wait_for(|done| *done))
            .await
            .is_ok();
        if !finished {
            self.control.tree.kill();
            let _ = tokio::time::timeout(TERM_GRACE, exited.wait_for(|done| *done)).await;
        }
    }

    /// Manda uma linha para a entrada padrão (comandos do servidor local).
    ///
    /// Erros: [`Error::ProcessControl`] sem entrada aberta ou com o processo já fechado.
    pub async fn send_line(&self, line: &str) -> Result<()> {
        let mut guard = self.control.stdin.lock().await;
        let stdin = guard.as_mut().ok_or_else(|| Error::ProcessControl {
            action: "escrever na entrada do processo",
            source: std::io::Error::other("entrada padrão fechada"),
        })?;
        let write = async {
            stdin.write_all(line.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await
        };
        write.await.map_err(|source| Error::ProcessControl {
            action: "escrever na entrada do processo",
            source,
        })
    }

    /// Processos vivos na árvore do jogo (Windows; `None` no Linux).
    #[must_use]
    pub fn active_processes(&self) -> Option<u32> {
        self.control.tree.active_processes()
    }

    /// Se o processo principal já saiu.
    #[must_use]
    pub fn has_exited(&self) -> bool {
        *self.control.exited.borrow()
    }
}

/// O jogo aberto.
#[derive(Debug)]
pub struct GameProcess {
    pid: u32,
    started_at: SystemTime,
    handle: GameHandle,
    finished: oneshot::Receiver<GameExit>,
}

/// Os eventos do jogo, na ordem; o último é [`GameEvent::Exited`].
pub type GameEvents = mpsc::UnboundedReceiver<GameEvent>;

impl GameProcess {
    /// Abre o jogo com a linha de comando dada (grava antes o `@argfile`, se houver).
    ///
    /// Erros: [`Error::LaunchFailed`] (programa ausente, sem permissão),
    /// [`Error::ProcessControl`] (Job Object) e erros de disco do `@argfile` e do log.
    pub async fn spawn(
        command: &LaunchCommand,
        options: SpawnOptions,
    ) -> Result<(Self, GameEvents)> {
        if let Some(argfile) = &command.argfile {
            write_file(
                &argfile.path,
                &encode_argfile(&argfile.contents),
                "gravar o arquivo de argumentos",
            )
            .await?;
        }
        let log = match &options.output_log {
            Some(path) => Some(open_output_log(path).await?),
            None => None,
        };

        let mut tree = Tree::new().map_err(|source| Error::ProcessControl {
            action: "criar o Job Object",
            source,
        })?;
        let mut process = build_process(command, options.stdin);
        let started_at = SystemTime::now();
        let started = Instant::now();
        let mut child = process.spawn().map_err(|source| Error::LaunchFailed {
            program: command.program.clone(),
            source,
        })?;
        if let Err(source) = tree.attach(&child) {
            let _ = child.start_kill();
            return Err(Error::ProcessControl {
                action: "pôr o jogo no Job Object",
                source,
            });
        }
        let pid = child.id().unwrap_or_default();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdin = child.stdin.take();

        let (exited_tx, exited_rx) = watch::channel(false);
        let control = Arc::new(Control {
            tree,
            stop_requested: AtomicBool::new(false),
            stdin: tokio::sync::Mutex::new(stdin),
            exited: exited_rx,
        });
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (finished_tx, finished_rx) = oneshot::channel();
        let (raw_tx, raw_rx) = mpsc::unbounded_channel::<String>();

        let writer = tokio::spawn(write_output(log, raw_rx));
        let mut readers = Vec::new();
        if let Some(stdout) = stdout {
            readers.push(tokio::spawn(read_pipe(
                stdout,
                LogSource::Stdout,
                started,
                events_tx.clone(),
                raw_tx.clone(),
            )));
        }
        if let Some(stderr) = stderr {
            readers.push(tokio::spawn(read_pipe(
                stderr,
                LogSource::Stderr,
                started,
                events_tx.clone(),
                raw_tx.clone(),
            )));
        }
        drop(raw_tx);

        tokio::spawn(watch_exit(
            child,
            Arc::clone(&control),
            Watch {
                exited: exited_tx,
                events: events_tx,
                finished: finished_tx,
                readers,
                writer,
                game_dir: command.cwd.clone(),
                started,
                started_at,
            },
        ));

        Ok((
            Self {
                pid,
                started_at,
                handle: GameHandle { control },
                finished: finished_rx,
            },
            events_rx,
        ))
    }

    /// O pid do processo principal.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Quando o processo começou.
    #[must_use]
    pub fn started_at(&self) -> SystemTime {
        self.started_at
    }

    /// Alça para parar e mandar comandos.
    #[must_use]
    pub fn handle(&self) -> GameHandle {
        self.handle.clone()
    }

    /// Espera o jogo terminar (e a saída ser toda lida e gravada).
    ///
    /// Erros: [`Error::Internal`] se a tarefa que vigia o processo sumiu.
    pub async fn wait(self) -> Result<GameExit> {
        self.finished
            .await
            .map_err(|_| Error::internal("a tarefa que vigia o jogo terminou sem resultado"))
    }
}

/// O `Command` do jogo, com pipes e a árvore de processos configurada.
fn build_process(command: &LaunchCommand, stdin: bool) -> Command {
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .current_dir(&command.cwd)
        .envs(command.env.iter().map(|(key, value)| (key, value)))
        .stdin(if stdin { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Tree::configure(&mut process);
    process
}

/// Cria o `output.log` (e a pasta da sessão).
async fn open_output_log(path: &Path) -> Result<BufWriter<tokio::fs::File>> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| Error::io("criar a pasta da sessão", parent, error))?;
    }
    let file = tokio::fs::File::create(path)
        .await
        .map_err(|error| Error::io("criar o output.log", path, error))?;
    Ok(BufWriter::new(file))
}

/// O que a tarefa que vigia a saída precisa.
struct Watch {
    exited: watch::Sender<bool>,
    events: mpsc::UnboundedSender<GameEvent>,
    finished: oneshot::Sender<GameExit>,
    readers: Vec<tokio::task::JoinHandle<()>>,
    writer: tokio::task::JoinHandle<()>,
    game_dir: PathBuf,
    started: Instant,
    started_at: SystemTime,
}

/// Espera o processo sair, encerra o que sobrou da árvore, termina a leitura e classifica.
async fn watch_exit(mut child: Child, control: Arc<Control>, watch: Watch) {
    let status = child.wait().await;
    let _ = watch.exited.send(true);
    // O que sobrou da árvore (filhos do jogo) morre junto, senão os pipes não fecham.
    control.tree.kill();
    for reader in watch.readers {
        let _ = reader.await;
    }
    let _ = watch.writer.await;
    *control.stdin.lock().await = None;
    let exit_code = status.ok().and_then(|status| status.code());
    let stop_requested = control.stop_requested.load(Ordering::SeqCst);
    let artifacts = find_crash_artifacts(&watch.game_dir, watch.started_at);
    let exit = GameExit {
        outcome: classify(stop_requested, exit_code, &artifacts),
        exit_code,
        stop_requested,
        duration_ms: elapsed_ms(watch.started),
        artifacts,
    };
    let _ = watch.events.send(GameEvent::Exited(exit.clone()));
    let _ = watch.finished.send(exit);
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Lê um pipe até o fim.
async fn read_pipe(
    pipe: impl AsyncRead + Unpin,
    source: LogSource,
    started: Instant,
    events: mpsc::UnboundedSender<GameEvent>,
    raw: mpsc::UnboundedSender<String>,
) {
    let mut reader = BufReader::new(pipe);
    let mut parser = LogParser::new(source);
    let mut buffer = Vec::with_capacity(512);
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let text = decode_line(trim_line_end(&buffer));
        for line in parser.push(&text, elapsed_ms(started)) {
            // O receptor pode já ter ido embora; a leitura continua para o pipe não encher.
            let _ = events.send(GameEvent::Line(line));
        }
        let _ = raw.send(text);
    }
    for line in parser.finish(elapsed_ms(started)) {
        let _ = events.send(GameEvent::Line(line));
    }
}

/// Grava as linhas cruas no `output.log`, descarregando quando a fila esvazia.
async fn write_output(
    mut log: Option<BufWriter<tokio::fs::File>>,
    mut lines: mpsc::UnboundedReceiver<String>,
) {
    let Some(file) = log.as_mut() else {
        while lines.recv().await.is_some() {}
        return;
    };
    let mut failed = false;
    while let Some(line) = lines.recv().await {
        if failed {
            continue;
        }
        let write = async {
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
            if lines.is_empty() {
                file.flush().await?;
            }
            Ok::<(), std::io::Error>(())
        };
        if let Err(error) = write.await {
            tracing::warn!(%error, "falha ao gravar o output.log; a saída continua no console");
            failed = true;
        }
    }
    let _ = file.flush().await;
}

/// Codificação do `@argfile`: o lançador do Java lê os bytes do arquivo na codificação da
/// plataforma (no Windows, a página de código ANSI, como faz com a própria linha de comando;
/// verificado com o Temurin 17: UTF-8 vira `Ã§`). Caractere fora da página vira `?`, como na
/// linha de comando.
fn encode_argfile(contents: &str) -> Vec<u8> {
    #[cfg(windows)]
    {
        let encoding = encoding_for_code_page(windows::ansi_code_page());
        let (bytes, _, lossy) = encoding.encode(contents);
        if lossy {
            tracing::warn!(
                "o arquivo de argumentos tem caracteres fora da página de código do Windows"
            );
        }
        bytes.into_owned()
    }
    #[cfg(unix)]
    {
        contents.as_bytes().to_vec()
    }
}

/// A codificação do `encoding_rs` para uma página de código do Windows.
#[cfg(any(windows, test))]
fn encoding_for_code_page(code_page: u32) -> &'static encoding_rs::Encoding {
    let label = match code_page {
        65001 => "utf-8",
        932 => "shift_jis",
        936 => "gbk",
        949 => "euc-kr",
        950 => "big5",
        874 | 1250..=1258 => {
            return encoding_rs::Encoding::for_label(format!("windows-{code_page}").as_bytes())
                .unwrap_or(encoding_rs::WINDOWS_1252);
        }
        _ => "windows-1252",
    };
    encoding_rs::Encoding::for_label(label.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252)
}

/// Grava um arquivo criando a pasta.
async fn write_file(path: &Path, contents: &[u8], action: &'static str) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| Error::io(action, parent, error))?;
    }
    tokio::fs::write(path, contents)
        .await
        .map_err(|error| Error::io(action, path, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paginas_de_codigo() {
        assert_eq!(encoding_for_code_page(1252), encoding_rs::WINDOWS_1252);
        assert_eq!(encoding_for_code_page(1251), encoding_rs::WINDOWS_1251);
        assert_eq!(encoding_for_code_page(65001), encoding_rs::UTF_8);
        assert_eq!(encoding_for_code_page(932), encoding_rs::SHIFT_JIS);
        assert_eq!(encoding_for_code_page(437), encoding_rs::WINDOWS_1252);
    }
}
