import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { installAppBehavior, isEditableTarget } from './app-behavior';

/** Dispara o evento e diz se a página impediu a ação padrão (menu, voltar, avançar). */
function blocked(target: Element, event: Event): boolean {
  target.dispatchEvent(event);
  return event.defaultPrevented;
}

/** O elemento de `id` (falha o teste se não existir). */
function byId(id: string): HTMLElement {
  const element = document.getElementById(id);
  if (!element) {
    throw new Error(`sem #${id}`);
  }
  return element;
}

function contextMenu(): MouseEvent {
  return new MouseEvent('contextmenu', { bubbles: true, cancelable: true, button: 2 });
}

describe('o Warden se comporta como app (ADR-0063)', () => {
  let uninstall: () => void;

  beforeEach(() => {
    document.body.innerHTML = `
      <p id="texto">Texto da interface</p>
      <input id="campo" type="text" />
      <input id="caixa" type="checkbox" />
      <textarea id="area"></textarea>
      <div class="cm-editor"><div class="cm-content" contenteditable="true"><span id="cm">a=1</span></div></div>
      <a id="link" href="#x">link</a>`;
    uninstall = installAppBehavior(window);
  });

  afterEach(() => {
    uninstall();
    window.getSelection()?.removeAllRanges();
    document.body.innerHTML = '';
  });

  it('clique direito em área vazia ou em texto da interface não abre menu', () => {
    expect(blocked(document.body, contextMenu())).toBe(true);
    expect(blocked(byId('texto'), contextMenu())).toBe(true);
    expect(blocked(byId('link'), contextMenu())).toBe(true);
    expect(blocked(byId('caixa'), contextMenu())).toBe(true);
  });

  it('em campo editável o menu abre (recortar, copiar, colar)', () => {
    expect(blocked(byId('campo'), contextMenu())).toBe(false);
    expect(blocked(byId('area'), contextMenu())).toBe(false);
    expect(blocked(byId('cm'), contextMenu())).toBe(false);
  });

  it('sobre texto selecionado o menu abre (copiar)', () => {
    const texto = byId('texto');
    const range = document.createRange();
    range.selectNodeContents(texto);
    window.getSelection()?.addRange(range);
    expect(blocked(texto, contextMenu())).toBe(false);
  });

  it('os botões voltar e avançar do mouse não navegam; os outros seguem', () => {
    for (const type of ['mousedown', 'mouseup', 'auxclick']) {
      for (const button of [3, 4]) {
        const event = new MouseEvent(type, { bubbles: true, cancelable: true, button });
        expect(blocked(document.body, event)).toBe(true);
      }
      const middle = new MouseEvent(type, { bubbles: true, cancelable: true, button: 1 });
      expect(blocked(document.body, middle)).toBe(false);
    }
  });

  it('Alt+← e Alt+→ não voltam nem avançam fora dos campos', () => {
    const key = (init: KeyboardEventInit) =>
      new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
    expect(blocked(document.body, key({ key: 'ArrowLeft', altKey: true }))).toBe(true);
    expect(blocked(document.body, key({ key: 'ArrowRight', altKey: true }))).toBe(true);
    expect(blocked(document.body, key({ key: 'BrowserBack' }))).toBe(true);
    expect(blocked(document.body, key({ key: 'ArrowLeft' }))).toBe(false);
    expect(blocked(byId('campo'), key({ key: 'ArrowLeft', altKey: true }))).toBe(false);
  });

  it('desligar remove as regras', () => {
    uninstall();
    expect(blocked(document.body, contextMenu())).toBe(false);
    uninstall = () => undefined;
  });

  it('campo só de leitura ou desabilitado não conta como editável', () => {
    const campo = byId('campo') as HTMLInputElement;
    expect(isEditableTarget(campo)).toBe(true);
    campo.readOnly = true;
    expect(isEditableTarget(campo)).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });
});
