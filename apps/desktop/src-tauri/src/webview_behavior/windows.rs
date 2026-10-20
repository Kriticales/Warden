//! Ajustes do WebView2 da janela principal (ADR-0063): atalhos de navegador, menu de contexto e
//! preenchimento automático. Único lugar desta crate com `unsafe` além de `window_chrome`
//! (QUALITY §2.1): cada bloco tem o comentário `SAFETY` com o motivo.

#![allow(unsafe_code)]

use tauri::WebviewWindow;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND, COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR,
    ICoreWebView2_11, ICoreWebView2ContextMenuRequestedEventArgs, ICoreWebView2Controller,
    ICoreWebView2Settings3, ICoreWebView2Settings4,
};
use webview2_com::{ContextMenuRequestedEventHandler, take_pwstr};
use windows::core::{BOOL, Interface as _, PWSTR};

use super::{MenuEntry, kept_entries};

/// Liga os ajustes no WebView da janela. Uma falha fica no log e deixa o padrão do WebView2.
pub(super) fn attach(window: &WebviewWindow) {
    let result = window.with_webview(|webview| {
        if let Err(error) = configure(&webview.controller()) {
            tracing::warn!(%error, "não foi possível ajustar o WebView2 da janela");
        }
    });
    if let Err(error) = result {
        tracing::warn!(%error, "não foi possível chegar ao WebView2 da janela");
    }
}

fn configure(controller: &ICoreWebView2Controller) -> windows::core::Result<()> {
    // SAFETY: o `with_webview` roda no thread da janela, onde o WebView2 aceita chamadas; o
    // controlador é o da janela, vivo enquanto ela existir.
    let webview = unsafe { controller.CoreWebView2() }?;
    // SAFETY: idem; `Settings` devolve a interface das opções deste WebView.
    let settings = unsafe { webview.Settings() }?;
    let settings3: ICoreWebView2Settings3 = settings.cast()?;
    // SAFETY: idem; só muda uma opção booleana.
    unsafe { settings3.SetAreBrowserAcceleratorKeysEnabled(false) }?;
    if let Ok(settings4) = settings.cast::<ICoreWebView2Settings4>() {
        // SAFETY: idem; só mudam opções booleanas.
        unsafe {
            settings4.SetIsGeneralAutofillEnabled(false)?;
            settings4.SetIsPasswordAutosaveEnabled(false)?;
        }
    }
    let webview11: ICoreWebView2_11 = webview.cast()?;
    let handler = ContextMenuRequestedEventHandler::create(Box::new(|_, args| {
        if let Some(args) = args
            && let Err(error) = filter_menu(&args)
        {
            tracing::warn!(%error, "não foi possível filtrar o menu de contexto");
        }
        Ok(())
    }));
    let mut token = 0_i64;
    // SAFETY: o WebView2 guarda a própria referência do `handler` (COM); o token é um inteiro
    // local que ele preenche. O manipulador vale enquanto o WebView existir.
    unsafe { webview11.add_ContextMenuRequested(&handler, &raw mut token) }?;
    Ok(())
}

/// Tira do menu nativo o que não é edição; sem nada que fique, o menu não aparece.
fn filter_menu(args: &ICoreWebView2ContextMenuRequestedEventArgs) -> windows::core::Result<()> {
    // SAFETY: chamadas no thread do evento do WebView2, sobre as interfaces que o próprio evento
    // entrega; os ponteiros de saída são variáveis locais, e cada `PWSTR` devolvido é liberado
    // por `take_pwstr` (o WebView2 o aloca com `CoTaskMemAlloc`).
    unsafe {
        let target = args.ContextMenuTarget()?;
        let mut editable = BOOL::default();
        target.IsEditable(&raw mut editable)?;
        let mut selection = BOOL::default();
        target.HasSelection(&raw mut selection)?;

        let items = args.MenuItems()?;
        let mut count = 0_u32;
        items.Count(&raw mut count)?;
        let mut names = Vec::new();
        for index in 0..count {
            let item = items.GetValueAtIndex(index)?;
            let mut kind = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND::default();
            item.Kind(&raw mut kind)?;
            if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR {
                names.push(None);
            } else {
                let mut name = PWSTR::null();
                item.Name(&raw mut name)?;
                names.push(Some(take_pwstr(name)));
            }
        }
        let entries: Vec<MenuEntry<'_>> = names
            .iter()
            .map(|name| {
                name.as_deref()
                    .map_or(MenuEntry::Separator, MenuEntry::Command)
            })
            .collect();
        let kept = kept_entries(editable.as_bool(), selection.as_bool(), &entries);
        if kept.is_empty() {
            // Com `Handled`, o WebView2 não mostra o menu dele.
            return args.SetHandled(true);
        }
        for index in (0..count).rev() {
            let keep = usize::try_from(index).is_ok_and(|index| kept.contains(&index));
            if !keep {
                items.RemoveValueAtIndex(index)?;
            }
        }
    }
    Ok(())
}
