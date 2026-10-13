//! Print da janela do Warden para a área de transferência (Relatar problema, UI-03).
//!
//! A interface esconde o diálogo e chama `problem_report_capture`; aqui o Warden tira o print
//! **só da própria janela** (nunca da tela inteira) e põe a imagem na área de transferência,
//! para a pessoa colar na issue com Ctrl+V. A captura fica atrás de [`WindowCapture`] e a
//! área de transferência atrás de [`ImageClipboard`], para os testes simularem as duas.
//!
//! - Windows: `PrintWindow` com o conteúdo do `WebView2` (`windows.rs`, o único trecho com
//!   `unsafe` do relato).
//! - Linux (X11): `GetImage` na janela com o `_NET_WM_PID` do Warden (`linux.rs`, pela
//!   `x11rb`). Sem X11 (Wayland puro), a captura falha e a issue abre sem o print.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use serde::Serialize;

/// Maior lado aceito, em pixels (uma janela maior que isto é um erro de leitura).
const MAX_SIDE: u32 = 16_384;

/// Imagem RGBA, linha por linha, de cima para baixo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Screenshot {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Screenshot {
    /// Confere o tamanho: nada vazio, nada gigante, 4 bytes por pixel.
    pub(crate) fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, CaptureError> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(CaptureError::Size { width, height });
        }
        let expected = pixel_count(width, height).saturating_mul(4);
        if rgba.len() != expected {
            return Err(CaptureError::Platform(format!(
                "imagem com {} bytes, esperados {expected}",
                rgba.len()
            )));
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    /// Largura em pixels.
    pub(crate) fn width(&self) -> u32 {
        self.width
    }

    /// Altura em pixels.
    pub(crate) fn height(&self) -> u32 {
        self.height
    }

    /// Os pixels RGBA.
    pub(crate) fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// A imagem como PAM (`P7`, `RGB_ALPHA`): o E2E confere o print por este arquivo.
    #[cfg_attr(not(debug_assertions), allow(dead_code))] // só o build de debug grava
    pub(crate) fn to_pam(&self) -> Vec<u8> {
        let header = format!(
            "P7\nWIDTH {}\nHEIGHT {}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
            self.width, self.height
        );
        let mut bytes = Vec::with_capacity(header.len() + self.rgba.len());
        bytes.extend_from_slice(header.as_bytes());
        bytes.extend_from_slice(&self.rgba);
        bytes
    }
}

fn pixel_count(width: u32, height: u32) -> usize {
    usize::try_from(u64::from(width) * u64::from(height)).unwrap_or(usize::MAX)
}

/// Por que o print não saiu. A interface mostra só "o print não foi copiado"; o detalhe vai
/// para os registros.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub(crate) enum CaptureError {
    /// A janela do Warden não foi achada (Wayland sem X11; no Windows, janela já fechada e
    /// sem HWND).
    #[error("a janela do Warden não foi encontrada")]
    WindowNotFound,
    /// Janela vazia (minimizada) ou grande demais.
    #[error("janela com tamanho inválido ({width}×{height})")]
    Size {
        /// Largura lida.
        width: u32,
        /// Altura lida.
        height: u32,
    },
    /// Formato de pixel que o Warden não lê. Só no X11: no Windows o `GetDIBits` já entrega
    /// 32 bits por pixel.
    #[cfg(target_os = "linux")]
    #[error("formato de pixel sem suporte: {0}")]
    PixelFormat(String),
    /// Falha do sistema ao capturar.
    #[error("captura: {0}")]
    Platform(String),
    /// A área de transferência recusou a imagem.
    #[error("área de transferência: {0}")]
    Clipboard(String),
    /// Sistema sem captura de janela.
    #[cfg_attr(any(windows, target_os = "linux"), allow(dead_code))] // só nos outros sistemas
    #[error("captura de janela sem suporte neste sistema")]
    Unsupported,
}

/// Tira o print da janela do Warden.
pub(crate) trait WindowCapture {
    /// O print da janela, sem o que estiver na frente dela.
    fn capture(&self) -> Result<Screenshot, CaptureError>;
}

/// Área de transferência que recebe uma imagem.
pub(crate) trait ImageClipboard {
    /// Põe a imagem na área de transferência (no lugar do que estava lá).
    fn write_image(&self, shot: &Screenshot) -> Result<(), CaptureError>;
}

/// Tamanho do print copiado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturedImage {
    /// Largura em pixels.
    pub(crate) width: u32,
    /// Altura em pixels.
    pub(crate) height: u32,
}

/// Captura e copia. Se a captura falha, a área de transferência fica como estava.
pub(crate) fn capture_to_clipboard(
    capture: &dyn WindowCapture,
    clipboard: &dyn ImageClipboard,
) -> Result<(CapturedImage, Screenshot), CaptureError> {
    let shot = capture.capture()?;
    clipboard.write_image(&shot)?;
    Ok((
        CapturedImage {
            width: shot.width,
            height: shot.height,
        },
        shot,
    ))
}

/// Captura da janela principal do app, no sistema atual.
pub(crate) struct NativeCapture {
    #[cfg_attr(not(windows), allow(dead_code))] // o Linux acha a janela pelo processo
    window: tauri::WebviewWindow,
}

impl NativeCapture {
    pub(crate) fn new(window: tauri::WebviewWindow) -> Self {
        Self { window }
    }
}

impl WindowCapture for NativeCapture {
    #[cfg(windows)]
    fn capture(&self) -> Result<Screenshot, CaptureError> {
        let hwnd = self.window.hwnd().map_err(|error| {
            tracing::warn!(%error, "print da janela: janela sem HWND");
            CaptureError::WindowNotFound
        })?;
        let (width, height, bgra) = windows::print_client_area(hwnd.0 as isize)?;
        Screenshot::new(width, height, opaque_rgba_from_bgra(&bgra))
    }

    #[cfg(target_os = "linux")]
    fn capture(&self) -> Result<Screenshot, CaptureError> {
        linux::capture_window_of_process(std::process::id())
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    fn capture(&self) -> Result<Screenshot, CaptureError> {
        Err(CaptureError::Unsupported)
    }
}

/// A área de transferência do sistema, pelo plugin oficial do Tauri.
pub(crate) struct TauriClipboard<'a> {
    app: &'a tauri::AppHandle,
}

impl<'a> TauriClipboard<'a> {
    pub(crate) fn new(app: &'a tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl ImageClipboard for TauriClipboard<'_> {
    fn write_image(&self, shot: &Screenshot) -> Result<(), CaptureError> {
        use tauri_plugin_clipboard_manager::ClipboardExt as _;
        let image = tauri::image::Image::new(shot.rgba(), shot.width(), shot.height());
        self.app
            .clipboard()
            .write_image(&image)
            .map_err(|error| CaptureError::Clipboard(error.to_string()))
    }
}

/// BGRA (ou BGRX) do sistema → RGBA opaco. O canal alfa do print não vale nada: a janela é
/// opaca.
pub(crate) fn opaque_rgba_from_bgra(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(bgra.len());
    for [blue, green, red, _] in bgra.as_chunks::<4>().0 {
        rgba.extend_from_slice(&[*red, *green, *blue, u8::MAX]);
    }
    rgba
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Captura simulada: devolve sempre o mesmo resultado.
    struct FakeCapture(Result<Screenshot, CaptureError>);

    impl WindowCapture for FakeCapture {
        fn capture(&self) -> Result<Screenshot, CaptureError> {
            self.0.clone()
        }
    }

    /// Área de transferência simulada: guarda o que recebeu ou recusa.
    #[derive(Default)]
    struct FakeClipboard {
        refuse: bool,
        written: RefCell<Vec<Screenshot>>,
    }

    impl ImageClipboard for FakeClipboard {
        fn write_image(&self, shot: &Screenshot) -> Result<(), CaptureError> {
            if self.refuse {
                return Err(CaptureError::Clipboard("ocupada".into()));
            }
            self.written.borrow_mut().push(shot.clone());
            Ok(())
        }
    }

    fn two_by_one() -> Screenshot {
        Screenshot::new(2, 1, vec![1, 2, 3, 255, 4, 5, 6, 255]).unwrap()
    }

    #[test]
    fn copia_o_print_para_a_area_de_transferencia() {
        let clipboard = FakeClipboard::default();
        let (size, shot) =
            capture_to_clipboard(&FakeCapture(Ok(two_by_one())), &clipboard).unwrap();
        assert_eq!(size, CapturedImage { width: 2, height: 1 });
        assert_eq!(shot, two_by_one());
        assert_eq!(*clipboard.written.borrow(), [two_by_one()]);
    }

    #[test]
    fn captura_que_falha_nao_mexe_na_area_de_transferencia() {
        let clipboard = FakeClipboard::default();
        let error =
            capture_to_clipboard(&FakeCapture(Err(CaptureError::WindowNotFound)), &clipboard)
                .unwrap_err();
        assert_eq!(error, CaptureError::WindowNotFound);
        assert!(clipboard.written.borrow().is_empty());
    }

    #[test]
    fn area_de_transferencia_que_recusa_vira_erro() {
        let clipboard = FakeClipboard {
            refuse: true,
            ..FakeClipboard::default()
        };
        let error = capture_to_clipboard(&FakeCapture(Ok(two_by_one())), &clipboard).unwrap_err();
        assert!(matches!(error, CaptureError::Clipboard(_)));
    }

    #[test]
    fn tamanho_invalido_e_recusado() {
        assert_eq!(
            Screenshot::new(0, 10, Vec::new()),
            Err(CaptureError::Size {
                width: 0,
                height: 10
            })
        );
        assert!(matches!(
            Screenshot::new(MAX_SIDE + 1, 1, Vec::new()),
            Err(CaptureError::Size { .. })
        ));
        assert!(matches!(
            Screenshot::new(2, 2, vec![0; 15]),
            Err(CaptureError::Platform(_))
        ));
    }

    #[test]
    fn bgra_vira_rgba_opaco() {
        assert_eq!(
            opaque_rgba_from_bgra(&[10, 20, 30, 0, 1, 2, 3, 7]),
            [30, 20, 10, 255, 3, 2, 1, 255]
        );
        // Bytes que sobram (pixel incompleto) são ignorados.
        assert_eq!(opaque_rgba_from_bgra(&[1, 2, 3]), Vec::<u8>::new());
    }

    #[test]
    fn pam_tem_cabecalho_e_pixels() {
        let pam = two_by_one().to_pam();
        let header = "P7\nWIDTH 2\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n";
        assert!(pam.starts_with(header.as_bytes()));
        assert_eq!(&pam[header.len()..], [1, 2, 3, 255, 4, 5, 6, 255]);
    }
}
