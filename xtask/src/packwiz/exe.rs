//! Leitura dos cabeçalhos de executáveis PE (Windows) e ELF (Linux), para conferir o sidecar
//! sem depender do comando `file`, que não existe no Windows.
//!
//! Só lê o necessário para provar o formato: no PE, a máquina, o tipo do cabeçalho opcional
//! (PE32+) e o subsistema; no ELF, a classe, a ordem dos bytes, o tipo, a máquina e se há
//! interpretador (`PT_INTERP`, ausente num binário estático). Entradas truncadas ou malformadas
//! devolvem `None`, nunca entram em pânico.

use anyhow::{Result, ensure};

/// `IMAGE_FILE_MACHINE_AMD64`.
pub const PE_MACHINE_AMD64: u16 = 0x8664;
/// Valor mágico do cabeçalho opcional PE32+ (64 bits).
pub const PE32_PLUS_MAGIC: u16 = 0x20B;
/// `IMAGE_SUBSYSTEM_WINDOWS_CUI` (programa de console).
pub const PE_SUBSYSTEM_CONSOLE: u16 = 3;
/// `EM_X86_64`.
pub const ELF_MACHINE_X86_64: u16 = 0x3E;
/// `ET_EXEC`.
pub const ELF_TYPE_EXEC: u16 = 2;
/// `ET_DYN` (executável independente de posição).
pub const ELF_TYPE_DYN: u16 = 3;
const PT_INTERP: u32 = 3;

/// Formato de executável esperado para um alvo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeFormat {
    /// Windows.
    Pe,
    /// Linux.
    Elf,
}

/// O que foi lido do cabeçalho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeInfo {
    /// Executável PE.
    Pe {
        /// Máquina do cabeçalho COFF.
        machine: u16,
        /// Valor mágico do cabeçalho opcional.
        optional_magic: u16,
        /// Subsistema do Windows.
        subsystem: u16,
    },
    /// Executável ELF.
    Elf {
        /// `EI_CLASS == ELFCLASS64`.
        class64: bool,
        /// `EI_DATA == ELFDATA2LSB`.
        little_endian: bool,
        /// `e_type` (só lido em ELF de 64 bits little-endian).
        kind: u16,
        /// `e_machine` (idem).
        machine: u16,
        /// Há um cabeçalho de programa `PT_INTERP` (idem).
        has_interp: bool,
    },
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

fn u64_at(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset.checked_add(8)?)?;
    Some(u64::from_le_bytes(slice.try_into().ok()?))
}

/// Lê o cabeçalho de um executável; `None` se não for PE nem ELF ou se estiver truncado.
pub fn inspect(bytes: &[u8]) -> Option<ExeInfo> {
    if bytes.starts_with(b"MZ") {
        return inspect_pe(bytes);
    }
    if bytes.starts_with(b"\x7fELF") {
        return inspect_elf(bytes);
    }
    None
}

fn inspect_pe(bytes: &[u8]) -> Option<ExeInfo> {
    let pe = usize::try_from(u32_at(bytes, 0x3C)?).ok()?;
    if bytes.get(pe..pe.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let coff = pe + 4;
    let machine = u16_at(bytes, coff)?;
    let optional = coff + 20;
    let optional_magic = u16_at(bytes, optional)?;
    // O subsistema fica no mesmo deslocamento (68) no PE32 e no PE32+.
    let subsystem = u16_at(bytes, optional + 68)?;
    Some(ExeInfo::Pe {
        machine,
        optional_magic,
        subsystem,
    })
}

fn inspect_elf(bytes: &[u8]) -> Option<ExeInfo> {
    let class64 = *bytes.get(4)? == 2;
    let little_endian = *bytes.get(5)? == 1;
    if !(class64 && little_endian) {
        return Some(ExeInfo::Elf {
            class64,
            little_endian,
            kind: 0,
            machine: 0,
            has_interp: false,
        });
    }
    let kind = u16_at(bytes, 16)?;
    let machine = u16_at(bytes, 18)?;
    let phoff = usize::try_from(u64_at(bytes, 32)?).ok()?;
    let phentsize = usize::from(u16_at(bytes, 54)?);
    let phnum = usize::from(u16_at(bytes, 56)?);
    let mut has_interp = false;
    for index in 0..phnum {
        let entry = phoff.checked_add(index.checked_mul(phentsize)?)?;
        if u32_at(bytes, entry)? == PT_INTERP {
            has_interp = true;
        }
    }
    Some(ExeInfo::Elf {
        class64,
        little_endian,
        kind,
        machine,
        has_interp,
    })
}

/// Confere que `bytes` é um executável x86-64 do formato esperado: PE32+ de console, ou ELF de
/// 64 bits little-endian sem interpretador (estático, sem depender da libc).
pub fn check(format: ExeFormat, bytes: &[u8]) -> Result<ExeInfo> {
    let info = inspect(bytes);
    match (format, info) {
        (
            ExeFormat::Pe,
            Some(
                found @ ExeInfo::Pe {
                    machine,
                    optional_magic,
                    subsystem,
                },
            ),
        ) => {
            ensure!(
                machine == PE_MACHINE_AMD64,
                "PE com máquina {machine:#06x} em vez de x86-64"
            );
            ensure!(
                optional_magic == PE32_PLUS_MAGIC,
                "PE com cabeçalho opcional {optional_magic:#x} em vez de PE32+"
            );
            ensure!(
                subsystem == PE_SUBSYSTEM_CONSOLE,
                "PE com subsistema {subsystem} em vez de console"
            );
            Ok(found)
        }
        (
            ExeFormat::Elf,
            Some(
                found @ ExeInfo::Elf {
                    class64,
                    little_endian,
                    kind,
                    machine,
                    has_interp,
                },
            ),
        ) => {
            ensure!(class64, "ELF de 32 bits em vez de 64");
            ensure!(little_endian, "ELF big-endian em vez de little-endian");
            ensure!(
                machine == ELF_MACHINE_X86_64,
                "ELF com máquina {machine:#x} em vez de x86-64"
            );
            ensure!(
                kind == ELF_TYPE_EXEC || kind == ELF_TYPE_DYN,
                "ELF do tipo {kind} não é executável"
            );
            ensure!(!has_interp, "ELF com interpretador: não é estático");
            Ok(found)
        }
        (expected, found) => {
            anyhow::bail!("esperado executável {expected:?}, encontrado {found:?}")
        }
    }
}

#[cfg(test)]
pub mod tests {
    use proptest::prelude::*;

    use super::*;

    /// PE mínimo: cabeçalho MZ, assinatura em 0x80, COFF e cabeçalho opcional.
    pub fn synthetic_pe(machine: u16, magic: u16, subsystem: u16) -> Vec<u8> {
        let mut bytes = vec![0_u8; 0x80 + 4 + 20 + 240];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3C..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        let optional = 0x84 + 20;
        bytes[optional..optional + 2].copy_from_slice(&magic.to_le_bytes());
        bytes[optional + 68..optional + 70].copy_from_slice(&subsystem.to_le_bytes());
        bytes
    }

    /// ELF de 64 bits little-endian mínimo, com os tipos de cabeçalho de programa dados.
    pub fn synthetic_elf(kind: u16, machine: u16, program_headers: &[u32]) -> Vec<u8> {
        let phoff = 64_usize;
        let phentsize = 56_usize;
        let mut bytes = vec![0_u8; phoff + phentsize * program_headers.len()];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[16..18].copy_from_slice(&kind.to_le_bytes());
        bytes[18..20].copy_from_slice(&machine.to_le_bytes());
        bytes[32..40].copy_from_slice(&(phoff as u64).to_le_bytes());
        bytes[54..56].copy_from_slice(&u16::try_from(phentsize).unwrap().to_le_bytes());
        bytes[56..58].copy_from_slice(&u16::try_from(program_headers.len()).unwrap().to_le_bytes());
        for (index, kind) in program_headers.iter().enumerate() {
            let entry = phoff + index * phentsize;
            bytes[entry..entry + 4].copy_from_slice(&kind.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn pe32_plus_de_console_passa() {
        let pe = synthetic_pe(PE_MACHINE_AMD64, PE32_PLUS_MAGIC, PE_SUBSYSTEM_CONSOLE);
        assert!(check(ExeFormat::Pe, &pe).is_ok());
    }

    #[test]
    fn pe_errado_e_recusado_com_motivo() {
        let pe32 = synthetic_pe(PE_MACHINE_AMD64, 0x10B, PE_SUBSYSTEM_CONSOLE);
        assert!(format!("{}", check(ExeFormat::Pe, &pe32).unwrap_err()).contains("PE32+"));
        let arm = synthetic_pe(0xAA64, PE32_PLUS_MAGIC, PE_SUBSYSTEM_CONSOLE);
        assert!(format!("{}", check(ExeFormat::Pe, &arm).unwrap_err()).contains("x86-64"));
        let gui = synthetic_pe(PE_MACHINE_AMD64, PE32_PLUS_MAGIC, 2);
        assert!(format!("{}", check(ExeFormat::Pe, &gui).unwrap_err()).contains("console"));
    }

    #[test]
    fn elf_estatico_passa_e_dinamico_nao() {
        let exec = synthetic_elf(ELF_TYPE_EXEC, ELF_MACHINE_X86_64, &[6, 1, 1]);
        assert!(check(ExeFormat::Elf, &exec).is_ok());
        let pie = synthetic_elf(ELF_TYPE_DYN, ELF_MACHINE_X86_64, &[1]);
        assert!(check(ExeFormat::Elf, &pie).is_ok());
        let dynamic = synthetic_elf(ELF_TYPE_EXEC, ELF_MACHINE_X86_64, &[6, PT_INTERP, 1]);
        let error = check(ExeFormat::Elf, &dynamic).unwrap_err();
        assert!(format!("{error}").contains("interpretador"));
        let object = synthetic_elf(1, ELF_MACHINE_X86_64, &[]);
        assert!(check(ExeFormat::Elf, &object).is_err());
        let arm = synthetic_elf(ELF_TYPE_EXEC, 0xB7, &[]);
        assert!(check(ExeFormat::Elf, &arm).is_err());
    }

    #[test]
    fn elf_de_32_bits_ou_big_endian_e_recusado() {
        let mut elf32 = synthetic_elf(ELF_TYPE_EXEC, ELF_MACHINE_X86_64, &[]);
        elf32[4] = 1;
        assert!(format!("{}", check(ExeFormat::Elf, &elf32).unwrap_err()).contains("32 bits"));
        let mut big = synthetic_elf(ELF_TYPE_EXEC, ELF_MACHINE_X86_64, &[]);
        big[5] = 2;
        assert!(format!("{}", check(ExeFormat::Elf, &big).unwrap_err()).contains("big-endian"));
    }

    #[test]
    fn formato_trocado_ou_desconhecido_e_recusado() {
        let pe = synthetic_pe(PE_MACHINE_AMD64, PE32_PLUS_MAGIC, PE_SUBSYSTEM_CONSOLE);
        let elf = synthetic_elf(ELF_TYPE_EXEC, ELF_MACHINE_X86_64, &[]);
        assert!(check(ExeFormat::Elf, &pe).is_err());
        assert!(check(ExeFormat::Pe, &elf).is_err());
        assert!(check(ExeFormat::Pe, b"#!/bin/sh\n").is_err());
        assert!(check(ExeFormat::Pe, b"").is_err());
    }

    #[test]
    fn cabecalhos_truncados_viram_none() {
        let pe = synthetic_pe(PE_MACHINE_AMD64, PE32_PLUS_MAGIC, PE_SUBSYSTEM_CONSOLE);
        // Último byte lido: o subsistema, em 0x84 + 20 + 68 (+ 2).
        let pe_needed = 0x84 + 20 + 70;
        for cut in 0..pe_needed {
            assert_eq!(inspect(&pe[..cut]), None, "PE cortado em {cut}");
        }
        assert!(inspect(&pe[..pe_needed]).is_some());
        let elf = synthetic_elf(ELF_TYPE_EXEC, ELF_MACHINE_X86_64, &[1, 1]);
        // Último byte lido: o `p_type` do segundo cabeçalho de programa.
        let elf_needed = 64 + 56 + 4;
        for cut in 0..elf_needed {
            assert_eq!(inspect(&elf[..cut]), None, "ELF cortado em {cut}");
        }
        assert!(inspect(&elf[..elf_needed]).is_some());
    }

    #[test]
    fn deslocamentos_absurdos_nao_estouram() {
        let mut pe = synthetic_pe(PE_MACHINE_AMD64, PE32_PLUS_MAGIC, PE_SUBSYSTEM_CONSOLE);
        pe[0x3C..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(inspect(&pe), None);
        let mut elf = synthetic_elf(ELF_TYPE_EXEC, ELF_MACHINE_X86_64, &[1]);
        elf[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(inspect(&elf), None);
        let mut elf = synthetic_elf(ELF_TYPE_EXEC, ELF_MACHINE_X86_64, &[1]);
        elf[54..56].copy_from_slice(&u16::MAX.to_le_bytes());
        elf[56..58].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(inspect(&elf), None);
    }

    proptest! {
        #[test]
        fn bytes_quaisquer_nunca_entram_em_panico(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = inspect(&bytes);
            let _ = check(ExeFormat::Pe, &bytes);
            let _ = check(ExeFormat::Elf, &bytes);
        }

        #[test]
        fn cabecalhos_validos_com_lixo_nunca_entram_em_panico(
            prefix in prop_oneof![Just(b"MZ".to_vec()), Just(b"\x7fELF".to_vec())],
            rest in proptest::collection::vec(any::<u8>(), 0..1024),
        ) {
            let mut bytes = prefix;
            bytes.extend(rest);
            let _ = inspect(&bytes);
        }

        #[test]
        fn pe_sintetico_e_lido_de_volta(machine: u16, magic: u16, subsystem: u16) {
            let pe = synthetic_pe(machine, magic, subsystem);
            prop_assert_eq!(
                inspect(&pe),
                Some(ExeInfo::Pe { machine, optional_magic: magic, subsystem })
            );
        }

        #[test]
        fn elf_sintetico_e_lido_de_volta(
            kind: u16,
            machine: u16,
            headers in proptest::collection::vec(0_u32..8, 0..16),
        ) {
            let elf = synthetic_elf(kind, machine, &headers);
            prop_assert_eq!(
                inspect(&elf),
                Some(ExeInfo::Elf {
                    class64: true,
                    little_endian: true,
                    kind,
                    machine,
                    has_interp: headers.contains(&PT_INTERP),
                })
            );
        }
    }
}
