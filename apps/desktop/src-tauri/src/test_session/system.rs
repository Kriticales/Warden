//! O computador do teste: memória total (limite da memória automática, SPEC T13) e a
//! impressão do computador gravada na sessão (gancho 1.1 da ADR-0039: comparar testes só do
//! mesmo computador).

use serde::Serialize;

/// A impressão do computador. Nada que identifique a pessoa: sistema, processador e memória.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MachineFingerprint {
    /// `windows`, `linux`…
    pub(crate) os: &'static str,
    /// `x86_64`, `aarch64`…
    pub(crate) arch: &'static str,
    /// Núcleos lógicos.
    pub(crate) cpus: usize,
    /// Memória total, em MB (`None` se não deu para ler).
    pub(crate) memory_mb: Option<u64>,
}

impl MachineFingerprint {
    /// Lê a impressão deste computador.
    pub(crate) fn current() -> Self {
        Self {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            cpus: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
            memory_mb: total_memory_mb(),
        }
    }
}

/// Memória física total, em MB.
pub(crate) fn total_memory_mb() -> Option<u64> {
    #[cfg(windows)]
    {
        windows::total_memory_bytes().map(|bytes| bytes / (1024 * 1024))
    }
    #[cfg(not(windows))]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        parse_meminfo_total_kb(&text).map(|kb| kb / 1024)
    }
}

/// `MemTotal:       16314240 kB` do `/proc/meminfo`.
#[cfg_attr(windows, allow(dead_code))]
fn parse_meminfo_total_kb(text: &str) -> Option<u64> {
    text.lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse().ok())
}

#[cfg(windows)]
mod windows {
    //! `GlobalMemoryStatusEx`. Único trecho com `unsafe` deste módulo (QUALITY §2.1).
    #![allow(unsafe_code)]

    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    /// Memória física total, em bytes.
    pub(super) fn total_memory_bytes() -> Option<u64> {
        let mut status = MEMORYSTATUSEX {
            dwLength: u32::try_from(size_of::<MEMORYSTATUSEX>()).ok()?,
            ..MEMORYSTATUSEX::default()
        };
        // SAFETY: `status` é uma `MEMORYSTATUSEX` válida e viva durante a chamada, com
        // `dwLength` igual ao tamanho da estrutura, como a função exige; ela só escreve nela.
        unsafe { GlobalMemoryStatusEx(&raw mut status) }.ok()?;
        Some(status.ullTotalPhys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_meminfo() {
        let text = "MemFree:  100 kB\nMemTotal:       16314240 kB\nBuffers: 1 kB\n";
        assert_eq!(parse_meminfo_total_kb(text), Some(16_314_240));
        assert_eq!(parse_meminfo_total_kb("nada"), None);
    }

    #[test]
    fn este_computador_tem_memoria() {
        let machine = MachineFingerprint::current();
        assert!(machine.cpus >= 1);
        let memory = machine.memory_mb.expect("memória do computador");
        assert!(memory > 256, "{memory} MB");
    }
}
