//! Moldura da janela principal (UI-01).
//!
//! No Windows a janela não tem a barra de título do sistema (`decorations: false`): a barra é
//! desenhada pela interface (`TitleBar.tsx`) com a identidade do Warden. O comportamento nativo
//! continua:
//!
//! - arrastar, duplo clique para maximizar, arrastar para o topo e o menu da janela (clique
//!   direito) vêm do WebView2: a barra usa o CSS `app-region: drag`, e o `wry` liga o suporte a
//!   regiões não-cliente do WebView2;
//! - redimensionar pelas bordas, a sombra e os cantos arredondados vêm do Tauri para janelas sem
//!   decoração;
//! - o menu de encaixe (Snap Layouts) ao passar o mouse no maximizar só aparece para uma janela
//!   que responde `HTMAXBUTTON` ao `WM_NCHITTEST`. O WebView2 cobre a área cliente e é de outro
//!   processo, então o Warden põe uma janela nativa transparente, filha da principal, exatamente
//!   sobre o botão maximizar da interface ([`windows`]). É a técnica do Windows Terminal e do
//!   VS Code; a do `tauri-plugin-frame` (MIT) serviu de referência (`THIRD_PARTY.md`).
//!
//! No Linux (só CI) a janela fica com a barra do sistema.

#[cfg(windows)]
mod windows;

use serde::Deserialize;
use tauri::WebviewWindow;

use crate::error::AppError;

/// A janela usa a barra de título própria (só no Windows).
pub(crate) const CUSTOM_TITLE_BAR: bool = cfg!(windows);

/// Onde está o botão maximizar, em pixels CSS, medido pela interface. A posição horizontal é a
/// distância até a borda direita da janela, que não muda quando a janela muda de largura.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MaximizeButtonArea {
    /// Distância da borda direita do botão até a borda direita da janela.
    pub(crate) right: f64,
    /// Distância do topo do botão até o topo da janela.
    pub(crate) top: f64,
    /// Largura do botão.
    pub(crate) width: f64,
    /// Altura do botão.
    pub(crate) height: f64,
}

/// Retângulo em pixels físicos dentro da área cliente da janela.
#[cfg_attr(not(windows), allow(dead_code))] // só a janela do menu de encaixe (Windows) usa
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhysicalRect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// Limite de bom senso para qualquer medida (pixels CSS).
const MAX_CSS_PX: f64 = 10_000.0;

impl MaximizeButtonArea {
    /// Medidas finitas, dentro do limite, com largura e altura positivas.
    pub(crate) fn is_valid(&self) -> bool {
        let in_range = |value: f64| value.is_finite() && (0.0..=MAX_CSS_PX).contains(&value);
        in_range(self.right)
            && in_range(self.top)
            && in_range(self.width)
            && in_range(self.height)
            && self.width > 0.0
            && self.height > 0.0
    }

    /// O botão em pixels físicos, numa área cliente de `client_width` pixels físicos e com a
    /// escala `scale` (1,0 = 96 DPI). As bordas são arredondadas uma a uma, como faz o
    /// navegador, para a janela nativa casar com o botão desenhado.
    #[cfg_attr(not(windows), allow(dead_code))] // só a janela do menu de encaixe (Windows) usa
    pub(crate) fn to_physical(self, client_width: i32, scale: f64) -> Option<PhysicalRect> {
        if !self.is_valid() || !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let px = |css: f64| to_px(css * scale);
        let right_edge = client_width - px(self.right);
        let left_edge = client_width - px(self.right + self.width);
        let top_edge = px(self.top);
        let bottom_edge = px(self.top + self.height);
        let rect = PhysicalRect {
            x: left_edge,
            y: top_edge,
            width: right_edge - left_edge,
            height: bottom_edge - top_edge,
        };
        (rect.width > 0 && rect.height > 0).then_some(rect)
    }
}

/// Arredonda pixels físicos. A entrada já foi limitada por [`MaximizeButtonArea::is_valid`]
/// (até 10 000 px CSS) e por uma escala de tela, então cabe num `i32`.
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(not(windows), allow(dead_code))] // só a janela do menu de encaixe (Windows) usa
fn to_px(value: f64) -> i32 {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

/// Liga a moldura à janela principal recém-criada: reposiciona a janela do menu de encaixe
/// quando a janela muda de tamanho ou de escala e a desfaz quando a janela fecha.
pub(crate) fn attach(window: &WebviewWindow) {
    #[cfg(windows)]
    windows::attach(window);
    #[cfg(not(windows))]
    let _ = window;
}

/// Guarda a área do botão maximizar e põe a janela do menu de encaixe sobre ele.
#[cfg_attr(not(windows), allow(clippy::unnecessary_wraps))] // no Linux não faz nada
pub(crate) fn set_maximize_area(
    window: &WebviewWindow,
    area: MaximizeButtonArea,
) -> Result<(), AppError> {
    #[cfg(windows)]
    {
        let target = window.clone();
        window
            .run_on_main_thread(move || windows::set_area(&target, area))
            .map_err(|error| AppError::internal(error.to_string()))
    }
    #[cfg(not(windows))]
    {
        let _ = (window, area);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> MaximizeButtonArea {
        MaximizeButtonArea {
            right: 48.0,
            top: 0.0,
            width: 48.0,
            height: 36.0,
        }
    }

    #[test]
    fn botao_em_escala_100() {
        assert_eq!(
            area().to_physical(1280, 1.0),
            Some(PhysicalRect {
                x: 1184,
                y: 0,
                width: 48,
                height: 36
            })
        );
    }

    #[test]
    fn botao_em_escala_150_arredonda_as_bordas() {
        let area = MaximizeButtonArea {
            right: 47.5,
            top: 0.5,
            width: 47.5,
            height: 35.5,
        };
        // Bordas: direita 1920 − 71 (71,25), esquerda 1920 − 143 (142,5 → 143), topo 1
        // (0,75), base 54 (54).
        assert_eq!(
            area.to_physical(1920, 1.5),
            Some(PhysicalRect {
                x: 1777,
                y: 1,
                width: 72,
                height: 53
            })
        );
    }

    #[test]
    fn area_invalida_nao_vira_retangulo() {
        let invalid = [
            MaximizeButtonArea {
                width: 0.0,
                ..area()
            },
            MaximizeButtonArea {
                right: -1.0,
                ..area()
            },
            MaximizeButtonArea {
                height: f64::NAN,
                ..area()
            },
            MaximizeButtonArea {
                top: f64::INFINITY,
                ..area()
            },
            MaximizeButtonArea {
                width: 20_000.0,
                ..area()
            },
        ];
        for item in invalid {
            assert!(!item.is_valid(), "{item:?}");
            assert_eq!(item.to_physical(1280, 1.0), None);
        }
        assert_eq!(area().to_physical(1280, 0.0), None);
        assert_eq!(area().to_physical(1280, f64::NAN), None);
    }

    #[test]
    fn area_vem_em_camel_case() {
        let parsed: MaximizeButtonArea =
            serde_json::from_str(r#"{"right":48,"top":0,"width":48,"height":36}"#).unwrap();
        assert_eq!(parsed, area());
    }

    #[test]
    fn barra_propria_so_no_windows() {
        assert_eq!(CUSTOM_TITLE_BAR, cfg!(windows));
    }
}
