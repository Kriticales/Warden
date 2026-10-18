/**
 * Ajuda dos E2E de foco: um registro do que acontece com o foco, o teclado e as camadas (menu,
 * dica, diálogo, aviso) entre dois pontos do teste. Só aparece na mensagem de erro quando a
 * verificação falha: no WebView2 do Windows não há como abrir o DevTools no meio do Noturno, e
 * sem o registro a falha diz só "o diálogo continuou aberto".
 */
import { browser } from '@wdio/globals';

declare global {
  interface Window {
    __wardenFocusTrace?: string[];
  }
}

/** Começa (ou recomeça) o registro na página. */
export async function startFocusTrace(): Promise<void> {
  await browser.execute(() => {
    const describe = (target: EventTarget | null): string => {
      if (!(target instanceof Element)) return target === document ? 'document' : 'nada';
      const name =
        target.getAttribute('aria-label') ??
        target.textContent.trim().replace(/\s+/g, ' ').slice(0, 30);
      const role = target.getAttribute('role');
      return `${target.tagName.toLowerCase()}${role ? `[${role}]` : ''}.${target.getAttribute('class') ?? ''} "${name}"`;
    };
    const trace: string[] = [];
    const t0 = performance.now();
    const log = (line: string) => {
      trace.push(`${(performance.now() - t0).toFixed(0)}ms ${line}`);
    };
    window.__wardenFocusTrace = trace;
    log(
      `início: foco em ${describe(document.activeElement)}; hasFocus=${String(document.hasFocus())}`,
    );
    for (const type of ['focusin', 'focusout'] as const) {
      document.addEventListener(
        type,
        (event) => {
          if (window.__wardenFocusTrace !== trace) return;
          log(`${type} ${describe(event.target)} (relacionado: ${describe(event.relatedTarget)})`);
        },
        true,
      );
    }
    document.addEventListener(
      'keydown',
      (event) => {
        if (window.__wardenFocusTrace !== trace) return;
        log(`keydown ${event.key} em ${describe(event.target)}`);
      },
      true,
    );
    // Na volta (fase de bolha, na janela): alguém tratou a tecla? O Radix chama
    // `preventDefault` quando uma camada se fecha com o Esc.
    window.addEventListener('keydown', (event) => {
      if (window.__wardenFocusTrace !== trace) return;
      log(`keydown ${event.key} terminou; defaultPrevented=${String(event.defaultPrevented)}`);
    });
    const layers =
      '[role="menu"], [role="dialog"], [role="alertdialog"], [role="tooltip"], .tooltip, .toast';
    new MutationObserver((mutations) => {
      if (window.__wardenFocusTrace !== trace) return;
      for (const mutation of mutations) {
        for (const [verb, nodes] of [
          ['entrou', mutation.addedNodes],
          ['saiu', mutation.removedNodes],
        ] as const) {
          for (const node of nodes) {
            if (!(node instanceof Element)) continue;
            const found = node.matches(layers) ? [node] : Array.from(node.querySelectorAll(layers));
            for (const layer of found) log(`${verb}: ${describe(layer)}`);
          }
        }
      }
    }).observe(document.body, { childList: true, subtree: true });
  });
}

/** O registro até agora, com o estado das camadas e do foco neste instante. */
export async function focusTrace(): Promise<string> {
  return browser.execute(() => {
    const trace = window.__wardenFocusTrace ?? ['(registro não iniciado)'];
    const active = document.activeElement;
    const count = (selector: string) => String(document.querySelectorAll(selector).length);
    const now = [
      `agora: foco em ${active ? `${active.tagName.toLowerCase()}.${active.getAttribute('class') ?? ''} "${active.getAttribute('aria-label') ?? active.textContent.trim().slice(0, 30)}"` : 'nada'}`,
      `hasFocus=${String(document.hasFocus())}`,
      `menus=${count('[role="menu"]')} diálogos=${count('[role="dialog"], [role="alertdialog"]')} dicas=${count('[role="tooltip"], .tooltip')} avisos=${count('.toast')}`,
      `body.pointer-events="${document.body.style.pointerEvents}"`,
    ].join('; ');
    return [...trace, now].join('\n');
  });
}
