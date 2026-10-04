//! Supervisão de um processo Java (cliente ou servidor), independente do motor.
//!
//! Evolução do `supervisor.rs` do S1: além de capturar stdout/stderr em threads e
//! gravar tudo num arquivo, aqui o processo entra num *Job Object* do Windows com
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, para que encerrar mate também os
//! subprocessos (o Fabric Loader abre a janela de erro num segundo `java`), e a
//! entrada padrão pode ficar aberta para mandar `stop` ao servidor dedicado.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
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
    #[allow(dead_code)]
    pub stream: Stream,
    pub at: Duration,
    pub text: String,
}

pub struct RunningProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    started: Instant,
    lines: Receiver<LogLine>,
    readers: Vec<JoinHandle<()>>,
    #[cfg(windows)]
    job: job::Job,
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
            // O receptor pode já ter ido embora; continuamos lendo para o pipe não encher.
            let _ = tx.send(LogLine { stream, at, text });
        }
    })
}

impl RunningProcess {
    pub fn spawn(mut cmd: Command, capture_file: &Path, keep_stdin: bool) -> std::io::Result<Self> {
        let file = Arc::new(Mutex::new(File::create(capture_file)?));
        cmd.stdin(if keep_stdin { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        let job = job::Job::new()?;
        let started = Instant::now();
        let mut child = cmd.spawn()?;
        #[cfg(windows)]
        job.assign(&child)?;
        let (tx, rx) = mpsc::channel();
        let out = child.stdout.take().expect("stdout com pipe");
        let err = child.stderr.take().expect("stderr com pipe");
        let stdin = child.stdin.take();
        let readers = vec![
            pump(out, Stream::Out, started, tx.clone(), Arc::clone(&file)),
            pump(err, Stream::Err, started, tx, file),
        ];
        Ok(Self {
            child,
            stdin,
            started,
            lines: rx,
            readers,
            #[cfg(windows)]
            job,
        })
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn started(&self) -> Instant {
        self.started
    }

    pub fn next_line(&self, timeout: Duration) -> Result<LogLine, RecvTimeoutError> {
        self.lines.recv_timeout(timeout)
    }

    pub fn try_next_line(&self) -> Option<LogLine> {
        self.lines.try_recv().ok()
    }

    pub fn try_exit_status(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    /// Quantos processos estão no Job Object (o jogo e os filhos que ele abriu).
    #[cfg(windows)]
    pub fn processes_in_job(&self) -> u32 {
        self.job.active_processes()
    }

    /// Manda uma linha para a entrada padrão (comando do servidor dedicado).
    pub fn send_line(&mut self, line: &str) -> std::io::Result<()> {
        match self.stdin.as_mut() {
            Some(s) => {
                s.write_all(line.as_bytes())?;
                s.write_all(b"\n")?;
                s.flush()
            }
            None => Err(std::io::Error::other("stdin não está aberto")),
        }
    }

    /// Encerra: se `graceful_cmd` vier (ex.: `stop` do servidor), manda pela entrada
    /// padrão e espera até `grace`; depois mata o Job Object inteiro.
    pub fn stop(mut self, graceful_cmd: Option<&str>, grace: Duration, mut on_line: impl FnMut(&LogLine)) -> StopReport {
        let requested_at = self.started.elapsed();
        let mut forced = false;
        if self.child.try_wait().ok().flatten().is_none() {
            if let Some(c) = graceful_cmd {
                let _ = self.send_line(c);
                let deadline = Instant::now() + grace;
                while Instant::now() < deadline && self.child.try_wait().ok().flatten().is_none() {
                    while let Ok(l) = self.lines.try_recv() {
                        on_line(&l);
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
            if self.child.try_wait().ok().flatten().is_none() {
                forced = graceful_cmd.is_some();
                self.kill_all();
            }
        }
        #[cfg(windows)]
        let children_left = {
            // Mesmo que o processo principal tenha saído sozinho, filhos podem ter ficado.
            let n = self.job.active_processes();
            if n > 0 {
                self.job.terminate();
            }
            n
        };
        #[cfg(not(windows))]
        let children_left = 0;
        let status = self.child.wait().ok();
        let took = self.started.elapsed() - requested_at;
        drop(self.stdin.take());
        for r in self.readers.drain(..) {
            let _ = r.join();
        }
        while let Ok(l) = self.lines.try_recv() {
            on_line(&l);
        }
        StopReport { status, forced, took, children_left }
    }

    fn kill_all(&mut self) {
        #[cfg(windows)]
        self.job.terminate();
        #[cfg(unix)]
        unsafe {
            libc::kill(self.child.id() as i32, libc::SIGKILL);
        }
    }
}

#[derive(Debug)]
pub struct StopReport {
    pub status: Option<ExitStatus>,
    /// O comando gracioso não bastou e foi preciso matar.
    pub forced: bool,
    pub took: Duration,
    /// Processos que ainda estavam no Job Object quando o principal já tinha saído.
    #[allow(dead_code)]
    pub children_left: u32,
}

#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation, QueryInformationJobObject,
        SetInformationJobObject, TerminateJobObject,
    };

    pub struct Job(HANDLE);

    // O handle do Job Object pode ser usado de qualquer thread.
    unsafe impl Send for Job {}

    impl Job {
        pub fn new() -> std::io::Result<Self> {
            unsafe {
                let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if h.is_null() {
                    return Err(std::io::Error::last_os_error());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    let e = std::io::Error::last_os_error();
                    CloseHandle(h);
                    return Err(e);
                }
                Ok(Self(h))
            }
        }

        pub fn assign(&self, child: &Child) -> std::io::Result<()> {
            unsafe {
                if AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) == 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        }

        pub fn active_processes(&self) -> u32 {
            unsafe {
                let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = std::mem::zeroed();
                let ok = QueryInformationJobObject(
                    self.0,
                    JobObjectBasicAccountingInformation,
                    &mut info as *mut _ as *mut _,
                    std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                );
                if ok == 0 { 0 } else { info.ActiveProcesses }
            }
        }

        pub fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
