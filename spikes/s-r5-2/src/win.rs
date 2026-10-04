//! Acesso ao `hsperfdata` de uma JVM em execução no Windows.
//!
//! A JVM (OpenJDK `src/hotspot/os/windows/perfMemory_windows.cpp`) cria:
//! - a pasta `<GetTempPath()>\hsperfdata_<USERNAME>` e, dentro, o arquivo `<pid>`
//!   (aberto com `FILE_SHARE_READ | FILE_SHARE_DELETE` e `FILE_FLAG_DELETE_ON_CLOSE`);
//! - o *file mapping* nomeado `hsperfdata_<USERNAME>_<pid>` sobre esse arquivo.
//!
//! `<USERNAME>` vem de `getenv("USERNAME")` (texto ANSI) e cai para `GetUserNameA`
//! se a variável estiver vazia. Por isso o Warden não monta o nome: procura a pasta
//! `hsperfdata_*` que tem o arquivo `<pid>` e usa o sufixo dela (como o `jstat` faz
//! em `get_user_name_slow`).

use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ,
    MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS,
};

const FILE_SHARE_READ: u32 = 0x1;
const FILE_SHARE_WRITE: u32 = 0x2;
const FILE_SHARE_DELETE: u32 = 0x4;

#[derive(Debug, Clone)]
pub struct Location {
    /// Sufixo da pasta (o nome de usuário como a JVM o gravou).
    pub user_suffix: String,
    pub file: PathBuf,
    pub mapping_name: String,
}

/// Procura `<temp>\hsperfdata_*\<pid>`. Se houver mais de uma, fica com a mais nova.
pub fn locate(temp: &Path, pid: u32) -> Option<Location> {
    let mut best: Option<(std::time::SystemTime, Location)> = None;
    for entry in std::fs::read_dir(temp).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(suffix) = name.strip_prefix("hsperfdata_") else {
            continue;
        };
        let file = entry.path().join(pid.to_string());
        let Ok(meta) = std::fs::metadata(&file) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let created = meta
            .created()
            .or_else(|_| meta.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        let loc = Location {
            user_suffix: suffix.to_owned(),
            mapping_name: format!("hsperfdata_{suffix}_{pid}"),
            file,
        };
        if best.as_ref().is_none_or(|(t, _)| created > *t) {
            best = Some((created, loc));
        }
    }
    best.map(|(_, l)| l)
}

/// Um *file mapping* aberto só para leitura, mapeado inteiro.
pub struct Mapping {
    handle: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    len: usize,
}

#[derive(Debug)]
pub struct OsError {
    pub call: &'static str,
    pub code: u32,
}

impl std::fmt::Display for OsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} falhou com o erro {} do Windows",
            self.call, self.code
        )
    }
}

impl std::error::Error for OsError {}

impl Mapping {
    pub fn open(name: &str) -> Result<Self, OsError> {
        let wide: Vec<u16> = OsStr::new(name)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: `wide` é um texto UTF-16 terminado em NUL que vive até o fim da chamada.
        let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, wide.as_ptr()) };
        if handle.is_null() {
            // SAFETY: sem pré-condições.
            return Err(OsError {
                call: "OpenFileMappingW",
                code: unsafe { GetLastError() },
            });
        }
        // SAFETY: `handle` é um file mapping válido aberto acima com FILE_MAP_READ.
        let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
        if view.Value.is_null() {
            // SAFETY: GetLastError não tem pré-condições; `handle` é nosso e fechado uma vez.
            let code = unsafe { GetLastError() };
            unsafe { CloseHandle(handle) };
            return Err(OsError {
                call: "MapViewOfFile",
                code,
            });
        }
        // O tamanho da vista é o da região reservada (múltiplo de página).
        // SAFETY: MEMORY_BASIC_INFORMATION é uma struct C só de inteiros e ponteiros; zero é válido.
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: `view.Value` aponta para a vista recém-mapeada; `info` é nosso.
        let got = unsafe {
            VirtualQuery(
                view.Value,
                &mut info,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if got == 0 {
            // SAFETY: GetLastError não tem pré-condições; a vista e o handle são nossos
            // e liberados uma vez só.
            let code = unsafe { GetLastError() };
            unsafe {
                UnmapViewOfFile(view);
                CloseHandle(handle);
            }
            return Err(OsError {
                call: "VirtualQuery",
                code,
            });
        }
        Ok(Self {
            handle,
            view,
            len: info.RegionSize,
        })
    }

    /// Cópia dos bytes da vista. Os contadores são `i64` alinhados, que o x64 grava
    /// de forma atômica; a cópia pode misturar instantes diferentes entre contadores,
    /// mas nunca corta um contador ao meio.
    pub fn snapshot(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.len];
        // SAFETY: a vista tem `len` bytes legíveis enquanto `self` existir.
        unsafe {
            std::ptr::copy_nonoverlapping(self.view.Value as *const u8, out.as_mut_ptr(), self.len)
        };
        out
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: a vista e o handle foram abertos por `open` e só são liberados aqui.
        unsafe {
            UnmapViewOfFile(self.view);
            CloseHandle(self.handle);
        }
    }
}

/// Lê o arquivo de apoio `<pid>` direto do disco (caminho alternativo ao mapeamento).
pub fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut f = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(buf)
}
