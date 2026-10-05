//! Comandos da janela: a barra de título própria do Windows (UI-01).

use tauri::WebviewWindow;

use crate::error::AppError;
use crate::window_chrome::{self, MaximizeButtonArea};

/// Informa onde está o botão maximizar da barra de título, para a janela nativa do menu de
/// encaixe (Snap Layouts) ficar exatamente em cima dele. No Linux a barra é a do sistema e o
/// comando não faz nada.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // contrato do Tauri
pub(crate) fn window_set_maximize_area(
    window: WebviewWindow,
    area: MaximizeButtonArea,
) -> Result<(), AppError> {
    if !area.is_valid() {
        return Err(AppError::internal(format!(
            "área do botão maximizar inválida: {area:?}"
        )));
    }
    window_chrome::set_maximize_area(&window, area)
}
