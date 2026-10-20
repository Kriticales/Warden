//! O Warden se comporta como app, não como página (ADR-0063).
//!
//! No Windows, logo depois de criar a janela, o WebView2 é ajustado ([`windows`]):
//!
//! - os atalhos de navegador ficam desligados (`AreBrowserAcceleratorKeysEnabled`): F5 e Ctrl+R
//!   não recarregam a página (perdendo o estado), e Ctrl+P, Ctrl+F, F3, Ctrl+S, Alt+← e Alt+→
//!   não abrem nada. Os atalhos de edição (Ctrl+C, Ctrl+V, Ctrl+Z…) continuam, e a página
//!   continua recebendo as teclas: o Ctrl+S do editor de configs e os atalhos do app funcionam;
//! - o menu de contexto do Edge só aparece num campo editável ou sobre texto selecionado, e só
//!   com os comandos de edição ([`kept_entries`]);
//! - o preenchimento automático e a oferta de salvar senhas ficam desligados.
//!
//! O resto (seleção de texto, arrastar imagem ou link, botões voltar e avançar do mouse e a
//! reserva do menu de contexto no Linux) fica na interface (`src/app/app-behavior.ts`).

#[cfg(windows)]
mod windows;

use tauri::WebviewWindow;

/// Uma entrada do menu de contexto nativo, pelo nome estável do WebView2 (`Name` do
/// `ICoreWebView2ContextMenuItem`: `"copy"`, `"paste"`, `"reload"`…).
#[cfg_attr(not(windows), allow(dead_code))] // só o menu do WebView2 (Windows) usa
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuEntry<'a> {
    /// Um comando.
    Command(&'a str),
    /// Um separador.
    Separator,
}

/// Os comandos de edição que ficam no menu: desfazer, refazer, recortar, copiar, colar e
/// selecionar tudo. O WebView2 já só oferece os que valem ali (colar só num campo editável).
#[cfg_attr(not(windows), allow(dead_code))] // só o menu do WebView2 (Windows) usa
pub(crate) const EDIT_COMMANDS: &[&str] = &["undo", "redo", "cut", "copy", "paste", "selectAll"];

/// Índices das entradas do menu nativo que ficam, em ordem. Fora de campo editável e sem texto
/// selecionado, nenhuma (o menu não aparece). Senão, só os comandos de [`EDIT_COMMANDS`], com
/// os separadores apenas entre dois grupos que ficaram. Vazio: o menu não aparece.
#[cfg_attr(not(windows), allow(dead_code))] // só o menu do WebView2 (Windows) usa
pub(crate) fn kept_entries(
    editable: bool,
    has_selection: bool,
    entries: &[MenuEntry<'_>],
) -> Vec<usize> {
    if !editable && !has_selection {
        return Vec::new();
    }
    let mut kept = Vec::new();
    let mut pending_separator = None;
    for (index, entry) in entries.iter().enumerate() {
        match entry {
            MenuEntry::Command(name) if EDIT_COMMANDS.contains(name) => {
                if let Some(separator) = pending_separator.take() {
                    kept.push(separator);
                }
                kept.push(index);
            }
            MenuEntry::Command(_) => {}
            MenuEntry::Separator => {
                if !kept.is_empty() && pending_separator.is_none() {
                    pending_separator = Some(index);
                }
            }
        }
    }
    kept
}

/// Ajusta o WebView da janela principal (só no Windows; no Linux não faz nada).
pub(crate) fn attach(window: &WebviewWindow) {
    #[cfg(windows)]
    windows::attach(window);
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(test)]
mod tests {
    use super::*;
    use MenuEntry::{Command, Separator};

    /// O menu do WebView2 num campo de texto com seleção (nomes da documentação do WebView2).
    const FIELD_MENU: &[MenuEntry<'static>] = &[
        Command("emoji"),
        Separator,
        Command("undo"),
        Command("redo"),
        Separator,
        Command("cut"),
        Command("copy"),
        Command("paste"),
        Command("pasteAndMatchStyle"),
        Command("selectAll"),
        Separator,
        Command("spellCheck"),
        Separator,
        Command("inspectElement"),
    ];

    #[test]
    fn fora_de_campo_editavel_e_sem_selecao_nao_ha_menu() {
        let page = [
            Command("back"),
            Command("forward"),
            Command("reload"),
            Separator,
            Command("saveAs"),
            Command("print"),
            Separator,
            Command("inspectElement"),
        ];
        assert!(kept_entries(false, false, &page).is_empty());
        assert!(kept_entries(false, false, FIELD_MENU).is_empty());
    }

    #[test]
    fn campo_editavel_fica_so_com_a_edicao_e_separadores_entre_grupos() {
        let kept = kept_entries(true, true, FIELD_MENU);
        let names: Vec<_> = kept.iter().map(|&index| FIELD_MENU[index]).collect();
        assert_eq!(
            names,
            [
                Command("undo"),
                Command("redo"),
                Separator,
                Command("cut"),
                Command("copy"),
                Command("paste"),
                Command("selectAll"),
            ]
        );
    }

    #[test]
    fn texto_selecionado_fora_de_campo_so_copia() {
        let selection = [
            Command("copy"),
            Command("copyLinkToHighlight"),
            Separator,
            Command("print"),
            Command("webSelect"),
            Separator,
            Command("inspectElement"),
        ];
        assert_eq!(kept_entries(false, true, &selection), [0]);
    }

    #[test]
    fn sem_nenhum_comando_de_edicao_nao_ha_menu() {
        let link = [
            Command("openLinkInNewWindow"),
            Separator,
            Command("copyLinkLocation"),
        ];
        assert!(kept_entries(true, true, &link).is_empty());
    }
}
