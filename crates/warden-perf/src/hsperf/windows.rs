//! Windows: o *file mapping* nomeado do `hsperfdata` (S-R5-2 §2 e §3). Porte de
//! `spikes/s-r5-2/src/win.rs`; único arquivo de `hsperf` com `unsafe` (QUALITY §2.1): cada bloco
//! tem o comentário `SAFETY` com o motivo.
//!
//! A JVM cria o mapeamento `hsperfdata_<usuário>_<pid>` sobre o arquivo `<pid>` (aberto com
//! `FILE_SHARE_READ | FILE_SHARE_DELETE` e `FILE_FLAG_DELETE_ON_CLOSE`). O Warden abre o
//! mapeamento só para leitura (`OpenFileMappingW(FILE_MAP_READ)` + `MapViewOfFile`), mede a
//! vista com `VirtualQuery` e copia a vista inteira a cada amostra (64 KB, menos de 1 ms).
//!
//! **Fechar ao sair o jogo:** enquanto o Warden segura o mapeamento, o nome continua ocupado; se
//! um jogo novo pegar o mesmo pid, a JVM dele recebe `ERROR_ALREADY_EXISTS` e cai para memória
//! privada (sem leitura). O `Drop` fecha a vista e o handle.

#![allow(unsafe_code)]

use std::ffi::{OsStr, c_void};
use std::io::Read as _;
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::fs::OpenOptionsExt as _;
use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    FILE_MAP_READ, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    OpenFileMappingW, UnmapViewOfFile, VirtualQuery,
};
use windows::core::PCWSTR;

const FILE_SHARE_READ: u32 = 0x1;
const FILE_SHARE_WRITE: u32 = 0x2;
const FILE_SHARE_DELETE: u32 = 0x4;

/// Um *file mapping* aberto só para leitura, com a vista mapeada inteira. Os ponteiros ficam
/// guardados como números: `HANDLE` e a vista não são `Send`, mas valem em qualquer thread do
/// processo (a amostra roda em `spawn_blocking`).
#[derive(Debug)]
pub(super) struct Mapping {
    handle: isize,
    view: usize,
    len: usize,
}

impl Mapping {
    /// Abre o mapeamento pelo nome. `None` se ele não existe (a JVM não publica, ou já saiu).
    pub(super) fn open(name: &OsStr) -> Option<Self> {
        let wide: Vec<u16> = name.encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` é um texto UTF-16 terminado em NUL, vivo até o fim da chamada; a função
        // só lê o nome e devolve um handle novo, que passa a ser deste `Mapping`.
        let handle =
            unsafe { OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR(wide.as_ptr())) }.ok()?;
        // SAFETY: `handle` é um file mapping válido, aberto acima com `FILE_MAP_READ`; zero bytes
        // e deslocamento zero mapeiam a seção inteira.
        let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
        if view.Value.is_null() {
            // SAFETY: o handle é nosso e é fechado uma única vez, aqui.
            let _ = unsafe { CloseHandle(handle) };
            return None;
        }
        let mut info = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: `view.Value` aponta para a vista recém-mapeada; `info` é uma estrutura válida e
        // gravável, viva durante a chamada, e o tamanho passado é o dela.
        let got = unsafe {
            VirtualQuery(
                Some(view.Value.cast_const()),
                &raw mut info,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        let mapping = Self {
            handle: handle.0 as isize,
            view: view.Value as usize,
            len: info.RegionSize,
        };
        // Sem tamanho, o `Drop` do `mapping` libera a vista e o handle.
        (got != 0 && mapping.len > 0).then_some(mapping)
    }

    /// Cópia dos bytes da vista. Os contadores são `i64` alinhados, que o x64 grava de uma vez:
    /// a cópia pode misturar instantes diferentes entre contadores, mas nunca corta um ao meio.
    pub(super) fn snapshot(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.len];
        // SAFETY: a vista tem `len` bytes legíveis (tamanho dado pelo `VirtualQuery`) enquanto
        // este `Mapping` existir; `out` tem o mesmo tamanho e não se sobrepõe à vista.
        unsafe {
            std::ptr::copy_nonoverlapping(self.view as *const u8, out.as_mut_ptr(), self.len);
        }
        out
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: a vista e o handle foram abertos por `open` e só são liberados aqui, uma vez.
        unsafe {
            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view as *mut c_void,
            });
            let _ = CloseHandle(HANDLE(self.handle as *mut c_void));
        }
    }
}

/// Lê o arquivo `<pid>` direto do disco (o caminho alternativo ao mapeamento). A JVM o mantém
/// aberto com escrita: o compartilhamento tem de incluir `WRITE` e `DELETE`.
pub(super) fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}
