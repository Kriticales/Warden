//! Job Object do Windows. Único arquivo da crate com `unsafe` (QUALITY §2.1): cada bloco tem o
//! comentário `SAFETY` com o motivo.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::os::windows::io::RawHandle;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Globalization::GetACP;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
    QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
};
use windows::core::PCWSTR;

/// Código de saída dado aos processos encerrados pelo Warden.
const KILLED_EXIT_CODE: u32 = 1;

/// Um Job Object sem nome, com `KILL_ON_JOB_CLOSE`: encerrar o job, ou fechar o último handle
/// (inclusive quando o Warden morre), mata o jogo e todos os processos que ele criou.
#[derive(Debug)]
pub(super) struct Job {
    /// O handle guardado como número: `HANDLE` tem um ponteiro e não é `Send`, mas o handle
    /// de um job pode ser usado de qualquer thread.
    handle: isize,
}

impl Job {
    /// Cria o job e liga `KILL_ON_JOB_CLOSE`.
    pub(super) fn new() -> io::Result<Self> {
        // SAFETY: sem atributos de segurança (`None`) e sem nome (`PCWSTR::null()`), a função
        // só cria um objeto novo; o handle devolvido passa a pertencer a `Job` e é fechado
        // uma única vez no `Drop`.
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(to_io)?;
        let job = Self {
            handle: handle.0 as isize,
        };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
            .map_err(|_| io::Error::other("estrutura do job grande demais"))?;
        // SAFETY: `info` é uma `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` válida, viva durante a
        // chamada, e `size` é o tamanho exato dela, como a classe
        // `JobObjectExtendedLimitInformation` exige; o handle é o job criado acima.
        unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&raw const info).cast::<c_void>(),
                size,
            )
        }
        .map_err(to_io)?;
        Ok(job)
    }

    /// Põe o processo no job (os filhos que ele criar depois herdam o job).
    pub(super) fn assign(&self, process: RawHandle) -> io::Result<()> {
        // SAFETY: `process` é o handle do processo filho que o `tokio::process::Child`
        // mantém aberto enquanto existe (o chamador o tem emprestado), e o job está aberto.
        unsafe { AssignProcessToJobObject(self.raw(), HANDLE(process)) }.map_err(to_io)
    }

    /// Quantos processos do job ainda estão vivos.
    pub(super) fn active_processes(&self) -> u32 {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let Ok(size) = u32::try_from(size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>()) else {
            return 0;
        };
        // SAFETY: `info` é uma `JOBOBJECT_BASIC_ACCOUNTING_INFORMATION` válida e gravável,
        // viva durante a chamada, `size` é o tamanho exato dela, como a classe
        // `JobObjectBasicAccountingInformation` exige; o tamanho devolvido não é pedido
        // (`None`), e o handle do job está aberto até o `Drop`.
        let result = unsafe {
            QueryInformationJobObject(
                Some(self.raw()),
                JobObjectBasicAccountingInformation,
                (&raw mut info).cast::<c_void>(),
                size,
                None,
            )
        };
        if result.is_ok() {
            info.ActiveProcesses
        } else {
            0
        }
    }

    /// Encerra todos os processos do job. Falha (job já vazio) é ignorada.
    pub(super) fn terminate(&self) {
        // SAFETY: o handle do job está aberto até o `Drop`.
        let _ = unsafe { TerminateJobObject(self.raw(), KILLED_EXIT_CODE) };
    }

    fn raw(&self) -> HANDLE {
        HANDLE(self.handle as *mut c_void)
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: o handle pertence a este `Job` e só é fechado aqui. Com
        // `KILL_ON_JOB_CLOSE`, fechar o último handle mata o que restar no job.
        let _ = unsafe { CloseHandle(self.raw()) };
    }
}

/// A página de código ANSI do Windows (1252 num Windows em português; 65001 com a opção
/// "UTF-8 para todo o sistema").
pub(super) fn ansi_code_page() -> u32 {
    // SAFETY: `GetACP` não recebe argumentos nem toca em memória do chamador.
    unsafe { GetACP() }
}

fn to_io(error: windows::core::Error) -> io::Error {
    io::Error::other(error)
}
