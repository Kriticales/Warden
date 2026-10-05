//! Janela nativa do menu de encaixe (Snap Layouts) sobre o botão maximizar. Único arquivo da
//! crate com `unsafe` (QUALITY §2.1): cada bloco tem o comentário `SAFETY` com o motivo.
//!
//! A janela é filha da principal, fica acima do WebView2 e logo abaixo da janela de
//! redimensionamento do Tauri (para a borda de cima continuar redimensionando), nunca pinta
//! nada (o botão desenhado pela interface aparece através dela) e responde `HTMAXBUTTON` a
//! qualquer `WM_NCHITTEST`. Como ela recebe o mouse no lugar da interface, devolve o estado
//! pelo evento `title-bar-maximize` e ela mesma maximiza ou restaura no clique. Tudo aqui
//! roda na thread principal (a da janela).

#![allow(unsafe_code)]

use std::sync::{Mutex, OnceLock, PoisonError};

use tauri::{WebviewWindow, WindowEvent};
use tauri_specta::Event as _;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, HBRUSH, NULL_BRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, FindWindowExW, GetClientRect, HTMAXBUTTON, HWND_TOP,
    IDC_ARROW, IsZoomed, LoadCursorW, RegisterClassExW, SW_MAXIMIZE, SW_RESTORE, SWP_HIDEWINDOW,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WM_NCHITTEST,
    WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WNDCLASSEXW, WS_CHILD,
    WS_CLIPSIBLINGS, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use super::MaximizeButtonArea;
use crate::events::TitleBarMaximize;

/// Classe da janela do menu de encaixe.
const CLASS_NAME: PCWSTR = w!("WardenSnapLayoutsButton");
/// Classe da janela que o Tauri usa para redimensionar janelas sem decoração
/// (`tauri-runtime-wry`, `undecorated_resizing.rs`).
const TAURI_RESIZE_CLASS: PCWSTR = w!("TAURI_DRAG_RESIZE_BORDERS");

/// A janela do menu de encaixe e o estado que a interface desenha.
struct Overlay {
    /// Janela principal.
    window: WebviewWindow,
    /// `HWND` da janela principal, guardado como número (`HWND` não é `Send`).
    parent: isize,
    /// `HWND` da janela do menu de encaixe.
    hwnd: isize,
    /// Onde está o botão (`None` até a interface medir).
    area: Option<MaximizeButtonArea>,
    /// Último estado enviado à interface.
    state: TitleBarMaximize,
}

static OVERLAY: Mutex<Option<Overlay>> = Mutex::new(None);

fn overlay_lock() -> std::sync::MutexGuard<'static, Option<Overlay>> {
    OVERLAY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reposiciona quando a janela muda de tamanho ou de escala e solta tudo quando ela fecha.
pub(super) fn attach(window: &WebviewWindow) {
    window.on_window_event(|event| match event {
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => reposition(),
        WindowEvent::Destroyed => {
            overlay_lock().take();
        }
        _ => {}
    });
}

/// Guarda a área do botão e mostra a janela do menu de encaixe sobre ele, criando-a na
/// primeira vez.
pub(super) fn set_area(window: &WebviewWindow, area: MaximizeButtonArea) {
    {
        let mut guard = overlay_lock();
        if guard.is_none() {
            match create(window) {
                Ok(overlay) => *guard = Some(overlay),
                Err(error) => {
                    tracing::warn!(%error, "janela do menu de encaixe não criada");
                    return;
                }
            }
        }
        if let Some(overlay) = guard.as_mut() {
            overlay.area = Some(area);
        }
    }
    reposition();
}

fn create(window: &WebviewWindow) -> windows::core::Result<Overlay> {
    let parent = window
        .hwnd()
        .map_err(|error| windows::core::Error::new(windows::core::HRESULT(-1), error.to_string()))?
        .0 as isize;
    register_class();
    // SAFETY: a classe foi registrada acima com um `WNDPROC` válido; `parent` é a janela
    // principal, viva (esta função roda na thread dela, chamada por ela).
    let overlay = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS_NAME,
            CLASS_NAME,
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            Some(HWND(parent as _)),
            None,
            GetModuleHandleW(None).ok().map(Into::into),
            None,
        )?
    };
    Ok(Overlay {
        window: window.clone(),
        parent,
        hwnd: overlay.0 as isize,
        area: None,
        state: TitleBarMaximize {
            hovered: false,
            pressed: false,
        },
    })
}

fn register_class() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        // SAFETY: `LoadCursorW` com instância nula lê um cursor do sistema; `GetStockObject`
        // devolve um objeto do sistema que não precisa ser liberado; a estrutura está completa
        // e `CLASS_NAME` é uma constante estática.
        unsafe {
            let class = WNDCLASSEXW {
                cbSize: u32::try_from(std::mem::size_of::<WNDCLASSEXW>()).unwrap_or_default(),
                lpfnWndProc: Some(overlay_proc),
                hInstance: GetModuleHandleW(None).map(Into::into).unwrap_or_default(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH(GetStockObject(NULL_BRUSH).0),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            RegisterClassExW(&raw const class);
        }
    });
}

/// Põe a janela do menu de encaixe sobre o botão (ou a esconde, sem área).
fn reposition() {
    let placement = {
        let guard = overlay_lock();
        let Some(overlay) = guard.as_ref() else {
            return;
        };
        let parent = HWND(overlay.parent as _);
        let mut client = RECT::default();
        // A escala vem do Win32, não do Tauri: os getters do Tauri, chamados de dentro de um
        // evento da janela, emprestariam de novo o estado das janelas dele.
        // SAFETY: `parent` é a janela principal, viva enquanto o estado existir (ele é solto
        // no `Destroyed`).
        let scale = f64::from(unsafe { GetDpiForWindow(parent) }) / 96.0;
        // SAFETY: idem.
        let rect = unsafe { GetClientRect(parent, &raw mut client) }
            .ok()
            .and_then(|()| overlay.area?.to_physical(client.right - client.left, scale));
        (overlay.parent, overlay.hwnd, rect)
    };
    let (parent, overlay, rect) = placement;
    let parent = HWND(parent as _);
    // SAFETY: as duas janelas estão vivas (mesmo motivo acima) e esta é a thread delas.
    unsafe {
        // Logo abaixo da janela de redimensionamento do Tauri, que fica no topo; sem ela, no
        // topo.
        let after = FindWindowExW(Some(parent), None, TAURI_RESIZE_CLASS, PCWSTR::null())
            .unwrap_or(HWND_TOP);
        let result = match rect {
            Some(rect) => SetWindowPos(
                HWND(overlay as _),
                Some(after),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            ),
            None => SetWindowPos(
                HWND(overlay as _),
                None,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_HIDEWINDOW,
            ),
        };
        if let Err(error) = result {
            tracing::warn!(%error, "janela do menu de encaixe não reposicionada");
        }
    }
}

/// Muda o estado desenhado e avisa a interface se ele mudou.
fn update_state(change: impl FnOnce(&mut TitleBarMaximize)) -> Option<TitleBarMaximize> {
    let (window, state) = {
        let mut guard = overlay_lock();
        let overlay = guard.as_mut()?;
        let before = overlay.state;
        change(&mut overlay.state);
        if overlay.state == before {
            return Some(before);
        }
        (overlay.window.clone(), overlay.state)
    };
    if let Err(error) = state.emit(&window) {
        tracing::warn!(%error, "evento title-bar-maximize não enviado");
    }
    Some(state)
}

fn parent_hwnd() -> Option<HWND> {
    overlay_lock()
        .as_ref()
        .map(|overlay| HWND(overlay.parent as _))
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // O Windows 11 mostra o menu de encaixe para quem responde "botão maximizar".
        WM_NCHITTEST => LRESULT(isize::try_from(HTMAXBUTTON).unwrap_or_default()),
        WM_NCMOUSEMOVE => {
            let mut track = TRACKMOUSEEVENT {
                cbSize: u32::try_from(std::mem::size_of::<TRACKMOUSEEVENT>()).unwrap_or_default(),
                dwFlags: TME_LEAVE | TME_NONCLIENT,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            // SAFETY: `track` está completa e `hwnd` é esta janela.
            if let Err(error) = unsafe { TrackMouseEvent(&raw mut track) } {
                tracing::debug!(%error, "TrackMouseEvent falhou");
            }
            update_state(|state| state.hovered = true);
            LRESULT(0)
        }
        WM_NCMOUSELEAVE => {
            update_state(|state| {
                state.hovered = false;
                state.pressed = false;
            });
            LRESULT(0)
        }
        // Não passa ao `DefWindowProcW`: ele desenharia o botão clássico do Windows.
        WM_NCLBUTTONDOWN => {
            update_state(|state| state.pressed = true);
            LRESULT(0)
        }
        WM_NCLBUTTONUP => {
            let mut clicked = false;
            update_state(|state| {
                clicked = state.pressed;
                state.pressed = false;
            });
            if clicked {
                toggle_maximize();
            }
            LRESULT(0)
        }
        // SAFETY: repassa a mensagem recebida, com os mesmos argumentos.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn toggle_maximize() {
    let Some(parent) = parent_hwnd() else {
        return;
    };
    // SAFETY: `parent` é a janela principal, viva (o clique veio de uma filha dela), e esta é
    // a thread dela.
    unsafe {
        let command = if IsZoomed(parent).as_bool() {
            SW_RESTORE
        } else {
            SW_MAXIMIZE
        };
        let _ = ShowWindow(parent, command);
    }
}
