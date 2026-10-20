/**
 * O Warden se comporta como app, não como página (ADR-0063). No Windows o WebView2 já desliga
 * os atalhos de navegador e filtra o menu de contexto do Edge (`src-tauri/src/webview_behavior`);
 * aqui fica o que vale em qualquer WebView:
 *
 * - o clique direito fora de um campo editável e sem texto selecionado não abre menu (no Linux
 *   é a única proteção; no Windows, a reserva);
 * - os botões voltar e avançar do mouse (botões 3 e 4) não navegam, nem Alt+← e Alt+→ (e as
 *   teclas "voltar" e "avançar" de alguns teclados) fora de um campo editável.
 *
 * A seleção de texto e o arrastar de imagens e links ficam no CSS (`styles/warden.css`).
 */

/** Tipos de `<input>` em que se digita texto (os demais não têm menu de edição). */
const TEXT_INPUTS = new Set(['', 'text', 'search', 'url', 'email', 'password', 'number', 'tel']);

/** Botões "voltar" e "avançar" do mouse (`MouseEvent.button`). */
const HISTORY_BUTTONS = new Set([3, 4]);

/** Teclas que navegam no histórico do WebView. */
function isHistoryKey(event: KeyboardEvent): boolean {
  if (event.key === 'BrowserBack' || event.key === 'BrowserForward') {
    return true;
  }
  return (
    event.altKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    (event.key === 'ArrowLeft' || event.key === 'ArrowRight')
  );
}

/** Se o elemento (ou um ancestral) é um campo onde se digita: input de texto, textarea, conteúdo
 * editável ou o editor de configs. */
export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }
  if (target instanceof HTMLInputElement) {
    return TEXT_INPUTS.has(target.type) && !target.readOnly && !target.disabled;
  }
  if (target instanceof HTMLTextAreaElement) {
    return !target.readOnly && !target.disabled;
  }
  if (target instanceof HTMLElement && target.isContentEditable) {
    return true;
  }
  return target.closest('[contenteditable]:not([contenteditable="false"]), .cm-editor') !== null;
}

/** Se há texto selecionado na página. */
function hasSelection(view: Window): boolean {
  const selection = view.getSelection();
  return selection !== null && !selection.isCollapsed && selection.toString().trim() !== '';
}

/** O menu de contexto só abre num campo editável ou sobre texto selecionado. */
export function shouldBlockContextMenu(event: MouseEvent, view: Window = window): boolean {
  return !isEditableTarget(event.target) && !hasSelection(view);
}

/** Liga as regras na janela; devolve a função que as desliga (testes). */
export function installAppBehavior(view: Window = window): () => void {
  const onContextMenu = (event: MouseEvent) => {
    if (shouldBlockContextMenu(event, view)) {
      event.preventDefault();
    }
  };
  const onHistoryButton = (event: MouseEvent) => {
    if (HISTORY_BUTTONS.has(event.button)) {
      event.preventDefault();
    }
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (isHistoryKey(event) && !isEditableTarget(event.target)) {
      event.preventDefault();
    }
  };
  view.addEventListener('contextmenu', onContextMenu);
  view.addEventListener('keydown', onKeyDown);
  for (const type of ['mousedown', 'mouseup', 'auxclick'] as const) {
    view.addEventListener(type, onHistoryButton);
  }
  return () => {
    view.removeEventListener('contextmenu', onContextMenu);
    view.removeEventListener('keydown', onKeyDown);
    for (const type of ['mousedown', 'mouseup', 'auxclick'] as const) {
      view.removeEventListener(type, onHistoryButton);
    }
  };
}
