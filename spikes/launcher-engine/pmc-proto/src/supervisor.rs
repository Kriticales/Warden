//! Supervisão do processo do jogo, independente do motor: inicia, captura stdout e
//! stderr em threads separadas, grava tudo num arquivo próprio, detecta marcadores
//! de "carregou" e de crash, e encerra o processo de forma controlada.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
}

pub struct LogLine {
    pub stream: Stream,
    pub at: Duration,
    pub text: String,
}

pub struct RunningGame {
    child: Child,
    started: Instant,
    lines: Receiver<LogLine>,
    readers: Vec<JoinHandle<()>>,
}

fn pump(
    source: impl Read + Send + 'static,
    stream: Stream,
    started: Instant,
    tx: mpsc::Sender<LogLine>,
    file: Arc<Mutex<File>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(source);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            // Decodificação tolerante: a saída do Java 8/17 no Windows não é UTF-8.
            let text = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string();
            let at = started.elapsed();
            if let Ok(mut f) = file.lock() {
                let tag = if stream == Stream::Out { "out" } else { "err" };
                let _ = writeln!(f, "[{:>8.3}s {tag}] {text}", at.as_secs_f64());
            }
            if tx.send(LogLine { stream, at, text }).is_err() {
                break;
            }
        }
    })
}

impl RunningGame {
    pub fn spawn(mut cmd: Command, capture_file: &Path) -> std::io::Result<Self> {
        let file = Arc::new(Mutex::new(File::create(capture_file)?));
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let started = Instant::now();
        let mut child = cmd.spawn()?;
        let (tx, rx) = mpsc::channel();
        let out = child.stdout.take().expect("stdout com pipe");
        let err = child.stderr.take().expect("stderr com pipe");
        let readers = vec![
            pump(out, Stream::Out, started, tx.clone(), Arc::clone(&file)),
            pump(err, Stream::Err, started, tx, file),
        ];
        Ok(Self { child, started, lines: rx, readers })
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Próxima linha de log, ou `None` se o tempo esgotou ou o processo fechou os pipes.
    pub fn next_line(&self, timeout: Duration) -> Result<LogLine, RecvTimeoutError> {
        self.lines.recv_timeout(timeout)
    }

    pub fn try_exit_status(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    /// Encerramento controlado: pede para o jogo fechar (SIGTERM, que dispara os
    /// shutdown hooks da JVM) e, se não fechar no prazo, força (SIGKILL).
    /// No Windows o equivalente previsto é Job Object + TerminateJobObject.
    pub fn stop(mut self, grace: Duration) -> StopReport {
        let requested_at = self.started.elapsed();
        let mut forced = false;
        if self.child.try_wait().ok().flatten().is_none() {
            #[cfg(unix)]
            unsafe {
                libc::kill(self.child.id() as i32, libc::SIGTERM);
            }
            #[cfg(not(unix))]
            {
                let _ = self.child.kill();
            }
            let deadline = Instant::now() + grace;
            while Instant::now() < deadline {
                if self.child.try_wait().ok().flatten().is_some() {
                    break;
                }
                // Drena linhas para os pipes não encherem enquanto esperamos.
                while self.lines.try_recv().is_ok() {}
                thread::sleep(Duration::from_millis(100));
            }
            if self.child.try_wait().ok().flatten().is_none() {
                forced = true;
                let _ = self.child.kill();
            }
        }
        let status = self.child.wait().ok();
        let took = self.started.elapsed() - requested_at;
        for r in self.readers.drain(..) {
            let _ = r.join();
        }
        StopReport { status, forced, took }
    }
}

#[derive(Debug)]
pub struct StopReport {
    pub status: Option<ExitStatus>,
    pub forced: bool,
    pub took: Duration,
}
