//! Print da área da janela do Warden no Windows (UI-03): `PrintWindow` com
//! `PW_RENDERFULLCONTENT`, que pede ao DWM o conteúdo da janela, inclusive o do `WebView2`, sem
//! o que estiver na frente dela. `PW_CLIENTONLY` deixa de fora a borda do sistema (a barra de
//! título do Warden é desenhada pela interface, dentro da área).
//!
//! Segundo arquivo da crate com `unsafe` (QUALITY §2.1; o primeiro é o
//! `window_chrome/windows.rs`): cada bloco tem o comentário `SAFETY` com o motivo, e cada
//! recurso do GDI é solto por um guarda (`Drop`), mesmo no caminho de erro.

#![allow(unsafe_code)]

use std::ffi::c_void;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HBITMAP, HDC, HGDIOBJ, ReleaseDC,
    SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PW_CLIENTONLY, PrintWindow};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

use super::{CaptureError, MAX_SIDE};

/// `PW_RENDERFULLCONTENT` (Windows 8.1+): sem ele, o conteúdo do `WebView2` sai preto.
const PW_RENDERFULLCONTENT: u32 = 0x0000_0002;

fn platform(what: &str, error: impl std::fmt::Display) -> CaptureError {
    CaptureError::Platform(format!("{what}: {error}"))
}

/// DC da janela, devolvido ao sistema no fim.
struct WindowDc {
    hwnd: HWND,
    hdc: HDC,
}

impl Drop for WindowDc {
    fn drop(&mut self) {
        // SAFETY: `hdc` veio de `GetDC(hwnd)` e é devolvido uma vez só.
        unsafe {
            ReleaseDC(Some(self.hwnd), self.hdc);
        }
    }
}

/// DC em memória, apagado no fim.
struct MemoryDc(HDC);

impl Drop for MemoryDc {
    fn drop(&mut self) {
        // SAFETY: o DC veio de `CreateCompatibleDC` e é apagado uma vez só, depois que o
        // bitmap saiu dele (o guarda do bitmap é solto antes; veja `print_client_area`).
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

/// Bitmap do GDI, apagado no fim.
struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: o bitmap veio de `CreateCompatibleBitmap`, não está selecionado em nenhum
        // DC quando o guarda é solto e é apagado uma vez só.
        unsafe {
            let _ = DeleteObject(HGDIOBJ::from(self.0));
        }
    }
}

/// O print da área da janela `hwnd`: largura, altura e os pixels BGRA de cima para baixo.
pub(super) fn print_client_area(hwnd: isize) -> Result<(u32, u32, Vec<u8>), CaptureError> {
    let hwnd = HWND(hwnd as *mut c_void);
    let mut rect = RECT::default();
    // SAFETY: `GetClientRect` só escreve em `rect`, que vive até o fim da chamada; um `hwnd`
    // inválido devolve erro.
    unsafe { GetClientRect(hwnd, &raw mut rect) }.map_err(|error| platform("GetClientRect", error))?;
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let size_error = || CaptureError::Size {
        width: u32::try_from(width).unwrap_or(0),
        height: u32::try_from(height).unwrap_or(0),
    };
    let (Ok(width_px), Ok(height_px)) = (u32::try_from(width), u32::try_from(height)) else {
        return Err(size_error());
    };
    if width_px == 0 || height_px == 0 || width_px > MAX_SIDE || height_px > MAX_SIDE {
        return Err(size_error());
    }

    // SAFETY: `GetDC` aceita qualquer `hwnd`; um DC nulo é tratado logo abaixo.
    let window_dc = WindowDc {
        hwnd,
        hdc: unsafe { GetDC(Some(hwnd)) },
    };
    if window_dc.hdc.is_invalid() {
        return Err(platform("GetDC", "DC nulo"));
    }
    // SAFETY: `window_dc.hdc` é um DC válido (conferido acima).
    let memory_dc = MemoryDc(unsafe { CreateCompatibleDC(Some(window_dc.hdc)) });
    if memory_dc.0.is_invalid() {
        return Err(platform("CreateCompatibleDC", "DC nulo"));
    }
    // SAFETY: DC válido e tamanho positivo, conferido acima.
    let bitmap = Bitmap(unsafe { CreateCompatibleBitmap(window_dc.hdc, width, height) });
    if bitmap.0.is_invalid() {
        return Err(platform("CreateCompatibleBitmap", "bitmap nulo"));
    }

    // SAFETY: DC e bitmap válidos; o objeto anterior volta para o DC logo depois do print,
    // antes de qualquer saída da função.
    let printed = unsafe {
        let previous = SelectObject(memory_dc.0, HGDIOBJ::from(bitmap.0));
        let printed = PrintWindow(
            hwnd,
            memory_dc.0,
            PRINT_WINDOW_FLAGS(PW_CLIENTONLY.0 | PW_RENDERFULLCONTENT),
        );
        SelectObject(memory_dc.0, previous);
        printed
    };
    if !printed.as_bool() {
        return Err(platform("PrintWindow", "a janela não foi impressa"));
    }

    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).unwrap_or(40),
            biWidth: width,
            // Altura negativa: linhas de cima para baixo.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..BITMAPINFOHEADER::default()
        },
        ..BITMAPINFO::default()
    };
    let pixels = usize::try_from(u64::from(width_px) * u64::from(height_px))
        .map_err(|error| platform("tamanho", error))?;
    let mut bgra = vec![0_u8; pixels * 4];
    // SAFETY: o bitmap não está selecionado em nenhum DC (voltou o anterior acima); `bgra` tem
    // exatamente `largura × altura × 4` bytes, o que o cabeçalho de 32 bits pede, e vive até o
    // fim da chamada.
    let lines = unsafe {
        GetDIBits(
            window_dc.hdc,
            bitmap.0,
            0,
            height_px,
            Some(bgra.as_mut_ptr().cast::<c_void>()),
            &raw mut info,
            DIB_RGB_COLORS,
        )
    };
    if lines != height {
        return Err(platform("GetDIBits", format!("{lines} de {height} linhas")));
    }
    drop(bitmap);
    drop(memory_dc);
    drop(window_dc);
    Ok((width_px, height_px, bgra))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn janela_que_nao_existe_e_erro_e_nao_trava() {
        assert!(matches!(print_client_area(0), Err(CaptureError::Platform(_))));
    }
}
