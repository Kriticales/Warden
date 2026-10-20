/**
 * O Warden se comporta como app, não como página (B1-C, ADR-0063), no WebView de verdade:
 *
 * - F5 e Ctrl+R não recarregam a página (o estado da interface continua). No Windows quem
 *   desliga é o WebView2 (`AreBrowserAcceleratorKeysEnabled`); o WebKitGTK do Linux não tem
 *   esses atalhos, então ali o teste só confirma que nada mudou;
 * - o clique direito em área vazia não abre menu (a página cancela o `contextmenu`), e num
 *   campo de texto abre (recortar, copiar, colar).
 */
import { $, browser, expect } from '@wdio/globals';
import { Key } from 'webdriverio';

/** Grava, numa propriedade da janela, se o último `contextmenu` foi cancelado pela página. */
async function watchContextMenu(): Promise<void> {
  await browser.execute(() => {
    const view = window as unknown as {
      wardenMenuBlocked?: boolean | null;
      wardenMenuWatcher?: (event: MouseEvent) => void;
    };
    view.wardenMenuBlocked = null;
    view.wardenMenuWatcher = (event) => {
      view.wardenMenuBlocked = event.defaultPrevented;
      // Não deixa o menu nativo (se houver) ficar aberto entre os passos.
      event.preventDefault();
    };
    // Registrado depois do app no mesmo alvo: roda depois do listener do app.
    window.addEventListener('contextmenu', view.wardenMenuWatcher);
  });
}

async function lastMenuBlocked(): Promise<boolean | null> {
  return browser.execute(
    () => (window as unknown as { wardenMenuBlocked?: boolean | null }).wardenMenuBlocked ?? null,
  );
}

describe('app, não página', () => {
  it('F5 e Ctrl+R não recarregam a interface', async () => {
    // A tela inicial depende da pasta de dados (primeira execução ou Meus packs).
    await $('main#conteudo h1').waitForDisplayed();
    const title = await $('main#conteudo h1').getText();
    await browser.execute(() => {
      (window as unknown as { wardenMarker?: number }).wardenMarker = 42;
    });
    await $('main#conteudo').click();
    await browser.keys(Key.F5);
    await browser.keys([Key.Ctrl, 'r']);
    // Um recarregamento levaria a página de volta ao início e apagaria a marca.
    await browser.pause(1500);
    const marker = await browser.execute(
      () => (window as unknown as { wardenMarker?: number }).wardenMarker ?? null,
    );
    expect(marker).toBe(42);
    await expect($('main#conteudo h1')).toHaveText(title);
  });

  it('clique direito em área vazia não abre menu; num campo de texto abre', async () => {
    await watchContextMenu();
    await $('main#conteudo').click({ button: 'right' });
    await browser.waitUntil(async () => (await lastMenuBlocked()) !== null);
    expect(await lastMenuBlocked()).toBe(true);

    await browser.execute(() => {
      const field = document.createElement('input');
      field.type = 'text';
      field.id = 'campo-e2e';
      document.body.append(field);
      (window as unknown as { wardenMenuBlocked?: boolean | null }).wardenMenuBlocked = null;
    });
    await $('#campo-e2e').click({ button: 'right' });
    await browser.waitUntil(async () => (await lastMenuBlocked()) !== null);
    expect(await lastMenuBlocked()).toBe(false);
    await browser.execute(() => {
      document.getElementById('campo-e2e')?.remove();
      const view = window as unknown as { wardenMenuWatcher?: (event: MouseEvent) => void };
      if (view.wardenMenuWatcher) {
        window.removeEventListener('contextmenu', view.wardenMenuWatcher);
      }
    });
  });
});
