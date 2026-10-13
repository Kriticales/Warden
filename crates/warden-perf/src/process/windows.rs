//! Windows: `GetProcessMemoryInfo` e a lista de processos do Toolhelp. Único arquivo da crate
//! com `unsafe` (QUALITY §2.1): cada bloco tem o comentário `SAFETY` com o motivo.

#![allow(unsafe_code)]

use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    PROCESS_VM_READ, WaitForSingleObject,
};

use super::ProcessMemory;

/// Um handle de processo aberto pelo Warden, fechado no `Drop`. Guardado como número:
/// `HANDLE` tem um ponteiro e não é `Send`, mas um handle de processo vale em qualquer thread.
#[derive(Debug)]
struct Process {
    handle: isize,
}

impl Process {
    /// Abre o processo para consultar a memória e saber se ele saiu.
    fn open(pid: u32) -> Option<Self> {
        // SAFETY: a função só recebe valores (direitos, herança e pid) e devolve um handle
        // novo, que passa a pertencer a `Process` e é fechado uma única vez no `Drop`.
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        }
        .ok()?;
        Some(Self {
            handle: handle.0 as isize,
        })
    }

    fn raw(&self) -> HANDLE {
        HANDLE(self.handle as *mut c_void)
    }

    /// Se o processo já saiu (o objeto do processo fica sinalizado quando ele termina).
    fn has_exited(&self) -> bool {
        // SAFETY: o handle está aberto até o `Drop` e tem `SYNCHRONIZE`; espera zero não
        // bloqueia.
        unsafe { WaitForSingleObject(self.raw(), 0) == WAIT_OBJECT_0 }
    }

    /// Quando o processo foi criado (intervalos de 100 ns desde 1601).
    fn created(&self) -> Option<u64> {
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: as quatro `FILETIME` são válidas, graváveis e vivas durante a chamada; o
        // handle está aberto até o `Drop` e tem `PROCESS_QUERY_LIMITED_INFORMATION`.
        unsafe {
            GetProcessTimes(
                self.raw(),
                &raw mut created,
                &raw mut exited,
                &raw mut kernel,
                &raw mut user,
            )
        }
        .ok()?;
        Some((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }

    /// Os contadores de memória do processo.
    fn memory(&self) -> Option<ProcessMemory> {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
        let size = u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>()).ok()?;
        counters.cb = size;
        // SAFETY: `counters` é uma `PROCESS_MEMORY_COUNTERS_EX` válida e gravável, viva
        // durante a chamada; `size` é o tamanho exato dela e `cb` diz à função qual das duas
        // estruturas recebeu (a estendida começa com a básica, por isso o ponteiro pode ser
        // convertido). O handle está aberto até o `Drop`.
        unsafe {
            GetProcessMemoryInfo(
                self.raw(),
                (&raw mut counters).cast::<PROCESS_MEMORY_COUNTERS>(),
                size,
            )
        }
        .ok()?;
        Some(ProcessMemory {
            rss: counters.WorkingSetSize as u64,
            private: Some(counters.PrivateUsage as u64),
            peak: Some(counters.PeakWorkingSetSize as u64),
        })
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: o handle pertence a este `Process` e só é fechado aqui.
        let _ = unsafe { CloseHandle(self.raw()) };
    }
}

/// O processo principal, aberto do começo ao fim da medição.
#[derive(Debug)]
pub(super) struct Root {
    process: Process,
    created: u64,
}

impl Root {
    pub(super) fn open(pid: u32) -> Option<Self> {
        let process = Process::open(pid)?;
        let created = process.created()?;
        Some(Self { process, created })
    }

    /// `None` quando o processo saiu.
    pub(super) fn read_root(&self) -> Option<ProcessMemory> {
        if self.process.has_exited() {
            return None;
        }
        self.process.memory()
    }

    /// A memória de um filho (aberto e fechado na hora); `None` se ele saiu ou foi criado
    /// antes do processo principal.
    pub(super) fn read_child(&self, pid: u32) -> Option<ProcessMemory> {
        let process = Process::open(pid)?;
        if process.has_exited() || process.created()? < self.created {
            return None;
        }
        process.memory()
    }
}

/// Os pares `(pid, ppid)` de todos os processos, pela foto do Toolhelp.
pub(super) fn process_list() -> Option<Vec<(u32, u32)>> {
    // SAFETY: a função só recebe valores e devolve um handle novo, que este bloco fecha
    // abaixo com o `Snapshot`.
    let snapshot = Snapshot(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?);
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).ok()?,
        ..PROCESSENTRY32W::default()
    };
    let mut pairs = Vec::new();
    // SAFETY: a foto está aberta e `entry` é uma `PROCESSENTRY32W` válida e gravável, com
    // `dwSize` igual ao tamanho da estrutura, como a função exige.
    let mut more = unsafe { Process32FirstW(snapshot.0, &raw mut entry) }.is_ok();
    while more {
        pairs.push((entry.th32ProcessID, entry.th32ParentProcessID));
        // SAFETY: as mesmas condições do `Process32FirstW` acima.
        more = unsafe { Process32NextW(snapshot.0, &raw mut entry) }.is_ok();
    }
    Some(pairs)
}

/// A foto do Toolhelp, fechada no `Drop`.
struct Snapshot(HANDLE);

impl Drop for Snapshot {
    fn drop(&mut self) {
        // SAFETY: o handle veio do `CreateToolhelp32Snapshot` e só é fechado aqui.
        let _ = unsafe { CloseHandle(self.0) };
    }
}
