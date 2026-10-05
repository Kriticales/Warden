//! Árvore de processos do packwiz: encerrar o processo **e os filhos** no cancelamento, no
//! tempo-limite e quando o Warden fecha (ARCHITECTURE §6.3).
//!
//! - Windows: *Job Object* com `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; o processo é posto no job
//!   logo depois de criado (os filhos que ele criar herdam o job), e encerrar o job mata todos.
//!   Fechar o handle (inclusive quando o Warden morre) também mata. `CREATE_NO_WINDOW` evita a
//!   janela de console piscando.
//! - Linux: grupo de processos próprio (`process_group(0)`) e `SIGKILL` no grupo.

use std::io;

use tokio::process::{Child, Command};

#[cfg(windows)]
mod windows;

/// `CREATE_NO_WINDOW`: processo de console sem janela.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// O processo do packwiz e tudo o que ele criar.
#[derive(Debug)]
pub(crate) struct ProcessTree {
    #[cfg(windows)]
    job: windows::Job,
    #[cfg(unix)]
    group: Option<i32>,
}

impl ProcessTree {
    /// Prepara a árvore antes de criar o processo (no Windows, cria o job).
    #[cfg_attr(unix, allow(clippy::unnecessary_wraps))] // Só o Windows pode falhar aqui.
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Self {
            #[cfg(windows)]
            job: windows::Job::new()?,
            #[cfg(unix)]
            group: None,
        })
    }

    /// Ajusta o comando: sem janela no Windows; grupo de processos próprio no Linux.
    pub(crate) fn configure(command: &mut Command) {
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        #[cfg(unix)]
        command.process_group(0);
        command.kill_on_drop(true);
    }

    /// Liga o processo recém-criado à árvore.
    #[cfg_attr(unix, allow(clippy::unnecessary_wraps))] // Só o Windows pode falhar aqui.
    pub(crate) fn attach(&mut self, child: &Child) -> io::Result<()> {
        #[cfg(windows)]
        {
            let handle = child
                .raw_handle()
                .ok_or_else(|| io::Error::other("o processo já terminou"))?;
            self.job.assign(handle)
        }
        #[cfg(unix)]
        {
            self.group = child.id().and_then(|id| i32::try_from(id).ok());
            Ok(())
        }
    }

    /// Mata todos os processos da árvore. Melhor esforço: a árvore pode já ter acabado.
    pub(crate) fn kill(&self) {
        #[cfg(windows)]
        self.job.terminate();
        #[cfg(unix)]
        if let Some(pid) = self.group.and_then(rustix::process::Pid::from_raw) {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
    }
}
