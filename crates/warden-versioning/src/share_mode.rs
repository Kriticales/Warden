//! Modo de compartilhamento com que a libgit2 abre arquivos no Windows (ARCHITECTURE §15).
//!
//! Por padrão a libgit2 abre com `FILE_SHARE_READ | FILE_SHARE_WRITE`, sem
//! `FILE_SHARE_DELETE` (`git_win32__createfile_sharemode`). Enquanto ela lê um arquivo do pack
//! para calcular o hash (a conta de alterações da linha de Meus packs, o Histórico, o início de
//! um teste), renomear por cima dele dá `ERROR_ACCESS_DENIED` e apagá-lo dá
//! `ERROR_SHARING_VIOLATION`: um "Salvar versão", um "Adicionar" ou uma edição de config
//! falhava de vez em quando, sem motivo aparente. A `std` do Rust abre tudo com
//! `FILE_SHARE_DELETE`; com ele, a renomeação atômica da `warden-core` completa pela semântica
//! POSIX e o leitor continua lendo o conteúdo antigo até fechar.
//!
//! [`configure`] põe a libgit2 no mesmo modo da `std` (`GIT_OPT_SET_WINDOWS_SHAREMODE`), uma
//! vez por processo, antes de qualquer repositório ser aberto ou criado. Fora do Windows a
//! opção não existe e nada é feito.

#![cfg_attr(windows, allow(unsafe_code))]

/// Garante o modo de compartilhamento da libgit2 (idempotente; a primeira chamada configura).
pub(crate) fn configure() {
    #[cfg(windows)]
    windows::configure_once();
}

#[cfg(windows)]
mod windows {
    use std::ffi::{c_int, c_ulong};
    use std::sync::Once;

    /// `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE` (`winnt.h`), o mesmo modo da
    /// `std` do Rust.
    pub(super) const SHARE_MODE: c_ulong = 0x1 | 0x2 | 0x4;

    static CONFIGURED: Once = Once::new();

    pub(super) fn configure_once() {
        CONFIGURED.call_once(|| {
            libgit2_sys::init();
            // A constante é `i32` na MSVC e `u32` na GNU; a função pede `c_int`.
            #[allow(clippy::unnecessary_cast)]
            let option = libgit2_sys::GIT_OPT_SET_WINDOWS_SHAREMODE as c_int;
            // SAFETY: `git_libgit2_opts` é variádica; `GIT_OPT_SET_WINDOWS_SHAREMODE` lê um
            // único `unsigned long` (`c_ulong`), que é o que vai aqui; a libgit2 já foi
            // inicializada na linha acima e a opção só grava uma variável global dela.
            let result = unsafe { libgit2_sys::git_libgit2_opts(option, SHARE_MODE) };
            if result != 0 {
                tracing::warn!(
                    result,
                    "não foi possível configurar o compartilhamento de arquivos da libgit2"
                );
            }
        });
    }

    /// O modo de compartilhamento em vigor na libgit2 (`GIT_OPT_GET_WINDOWS_SHAREMODE`).
    #[cfg(test)]
    pub(super) fn current() -> c_ulong {
        let mut mode: c_ulong = 0;
        #[allow(clippy::unnecessary_cast)]
        let option = libgit2_sys::GIT_OPT_GET_WINDOWS_SHAREMODE as c_int;
        // SAFETY: a opção de leitura recebe um `unsigned long *` válido e grava nele.
        let result = unsafe { libgit2_sys::git_libgit2_opts(option, &raw mut mode) };
        assert_eq!(result, 0);
        mode
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Sem a configuração, a libgit2 abre sem `FILE_SHARE_DELETE` (`0x1 | 0x2`); com ela, com
    /// o mesmo modo da `std`. O teste falha sem o conserto.
    #[test]
    fn libgit2_abre_arquivos_com_file_share_delete() {
        const FILE_SHARE_DELETE: std::ffi::c_ulong = 0x4;
        configure();
        let mode = windows::current();
        assert_eq!(mode, windows::SHARE_MODE, "modo em vigor: {mode:#x}");
        assert_ne!(mode & FILE_SHARE_DELETE, 0);
    }

    /// Abrir um repositório já configura: quem só usa `PackRepo` não precisa saber disso.
    #[test]
    fn abrir_um_repositorio_configura() {
        let dir = tempfile::tempdir().unwrap();
        let _ = crate::PackRepo::open(dir.path());
        assert_eq!(windows::current(), windows::SHARE_MODE);
    }
}
