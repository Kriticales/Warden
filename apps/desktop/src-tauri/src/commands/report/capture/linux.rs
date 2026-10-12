//! Print da janela do Warden no X11 (UI-03), pela `x11rb` (protocolo em Rust, sem biblioteca
//! C nem `unsafe`).
//!
//! A janela é a de nível de cima que tem o `_NET_WM_PID` do processo (o GTK grava esse
//! atributo), está visível e é a maior delas (o GTK também marca janelas auxiliares, que
//! nunca ficam visíveis). O `GetImage` lê o conteúdo dela, não o da tela.

use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, ImageFormat, ImageOrder, MapState, Window};

use super::{CaptureError, Screenshot};

/// Janelas visitadas no máximo (proteção contra uma árvore enorme).
const MAX_WINDOWS: usize = 20_000;

fn platform(error: impl std::fmt::Display) -> CaptureError {
    CaptureError::Platform(format!("X11: {error}"))
}

/// O print da maior janela visível do processo `pid`.
pub(super) fn capture_window_of_process(pid: u32) -> Result<Screenshot, CaptureError> {
    let (connection, screen) = x11rb::connect(None).map_err(|_| CaptureError::WindowNotFound)?;
    let setup = connection.setup();
    let root = setup.roots.get(screen).ok_or(CaptureError::WindowNotFound)?.root;
    let pid_atom = connection
        .intern_atom(true, b"_NET_WM_PID")
        .map_err(platform)?
        .reply()
        .map_err(platform)?
        .atom;
    if pid_atom == u32::from(AtomEnum::NONE) {
        return Err(CaptureError::WindowNotFound);
    }

    let mut best: Option<(Window, u16, u16)> = None;
    let mut pending = vec![root];
    let mut visited = 0;
    while let Some(parent) = pending.pop() {
        let Ok(tree) = connection.query_tree(parent).map_err(platform)?.reply() else {
            continue; // a janela sumiu no meio do caminho
        };
        for window in tree.children {
            visited += 1;
            if visited > MAX_WINDOWS {
                break;
            }
            pending.push(window);
            let Ok(property) = connection
                .get_property(false, window, pid_atom, AtomEnum::CARDINAL, 0, 1)
                .map_err(platform)?
                .reply()
            else {
                continue;
            };
            if property.value32().and_then(|mut values| values.next()) != Some(pid) {
                continue;
            }
            let Ok(attributes) = connection
                .get_window_attributes(window)
                .map_err(platform)?
                .reply()
            else {
                continue;
            };
            if attributes.map_state != MapState::VIEWABLE {
                continue;
            }
            let Ok(geometry) = connection.get_geometry(window).map_err(platform)?.reply() else {
                continue;
            };
            let area = u32::from(geometry.width) * u32::from(geometry.height);
            if best.is_none_or(|(_, w, h)| area > u32::from(w) * u32::from(h)) {
                best = Some((window, geometry.width, geometry.height));
            }
        }
    }
    let (window, width, height) = best.ok_or(CaptureError::WindowNotFound)?;

    let image = connection
        .get_image(ImageFormat::Z_PIXMAP, window, 0, 0, width, height, u32::MAX)
        .map_err(platform)?
        .reply()
        .map_err(platform)?;
    let format = setup
        .pixmap_formats
        .iter()
        .find(|format| format.depth == image.depth)
        .ok_or_else(|| CaptureError::PixelFormat(format!("profundidade {}", image.depth)))?;
    let layout = Layout {
        width: u32::from(width),
        height: u32::from(height),
        depth: image.depth,
        bits_per_pixel: format.bits_per_pixel,
        scanline_pad: format.scanline_pad,
        lsb_first: setup.image_byte_order == ImageOrder::LSB_FIRST,
    };
    let rgba = rgba_from_zpixmap(&image.data, &layout)?;
    Screenshot::new(layout.width, layout.height, rgba)
}

/// Como os pixels vêm no `ZPixmap`.
#[derive(Debug, Clone, Copy)]
struct Layout {
    width: u32,
    height: u32,
    depth: u8,
    bits_per_pixel: u8,
    scanline_pad: u8,
    lsb_first: bool,
}

/// `ZPixmap` de 24 ou 32 bits por pixel em 32 bits (o que o X11 usa em todo monitor de
/// hoje) → RGBA opaco. Outros formatos são recusados.
fn rgba_from_zpixmap(data: &[u8], layout: &Layout) -> Result<Vec<u8>, CaptureError> {
    if !matches!(layout.depth, 24 | 32) || layout.bits_per_pixel != 32 || !layout.lsb_first {
        return Err(CaptureError::PixelFormat(format!(
            "profundidade {}, {} bits por pixel, {}",
            layout.depth,
            layout.bits_per_pixel,
            if layout.lsb_first { "LSB" } else { "MSB" }
        )));
    }
    let width = usize::try_from(layout.width).map_err(platform)?;
    let height = usize::try_from(layout.height).map_err(platform)?;
    let pad_bits = usize::from(layout.scanline_pad.max(8));
    let stride = (width * 32).div_ceil(pad_bits) * pad_bits / 8;
    if data.len() < stride * height {
        return Err(CaptureError::Platform(format!(
            "X11: imagem com {} bytes, esperados {}",
            data.len(),
            stride * height
        )));
    }
    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in data.chunks_exact(stride).take(height) {
        rgba.extend(super::opaque_rgba_from_bgra(&row[..width * 4]));
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(width: u32, height: u32) -> Layout {
        Layout {
            width,
            height,
            depth: 24,
            bits_per_pixel: 32,
            scanline_pad: 32,
            lsb_first: true,
        }
    }

    #[test]
    fn zpixmap_bgrx_vira_rgba() {
        // 2×2: azul, verde / vermelho, branco (B, G, R, X).
        let data = [255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 255, 0, 255, 255, 255, 0];
        assert_eq!(
            rgba_from_zpixmap(&data, &layout(2, 2)).unwrap(),
            [0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255]
        );
    }

    #[test]
    fn linha_com_preenchimento_maior_e_respeitada() {
        // Pad de 64 bits: 1 pixel por linha ocupa 8 bytes.
        let mut wide = layout(1, 2);
        wide.scanline_pad = 64;
        let data = [1, 2, 3, 0, 9, 9, 9, 9, 4, 5, 6, 0, 9, 9, 9, 9];
        assert_eq!(
            rgba_from_zpixmap(&data, &wide).unwrap(),
            [3, 2, 1, 255, 6, 5, 4, 255]
        );
    }

    #[test]
    fn formatos_sem_suporte_e_dados_curtos_sao_recusados() {
        let mut sixteen = layout(1, 1);
        sixteen.depth = 16;
        sixteen.bits_per_pixel = 16;
        assert!(matches!(
            rgba_from_zpixmap(&[0, 0], &sixteen),
            Err(CaptureError::PixelFormat(_))
        ));
        let mut msb = layout(1, 1);
        msb.lsb_first = false;
        assert!(matches!(
            rgba_from_zpixmap(&[0; 4], &msb),
            Err(CaptureError::PixelFormat(_))
        ));
        assert!(matches!(
            rgba_from_zpixmap(&[0; 4], &layout(2, 1)),
            Err(CaptureError::Platform(_))
        ));
    }

    #[test]
    fn sem_x11_a_janela_nao_e_encontrada() {
        // Um processo que não existe nunca tem janela (com ou sem servidor X na máquina).
        assert!(capture_window_of_process(u32::MAX).is_err());
    }
}
