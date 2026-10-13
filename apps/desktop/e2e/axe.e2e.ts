/**
 * A ajuda do axe dos E2E (`axe.ts`) no WebView real: um aviso que entra no meio da medição não
 * reprova pelo contraste do meio da animação, e o que está errado na tela parada continua
 * reprovando.
 *
 * O aviso de teste fica fixo no canto, com as cores do `.toast` (título `--color-text` sobre
 * `--color-surface-2`). A entrada do título é uma animação de opacidade (Web Animations, que não
 * dispara `animationstart`) posta na tela logo antes de o axe começar e terminada logo depois que
 * ele acaba, como acontece com os avisos no WebKitGTK: o axe mede o título com opacidade 0,3
 * (contraste ~2,5:1) e nada sobra em `document.getAnimations()`.
 */
import { $, browser, expect } from '@wdio/globals';

import { seriousViolationSummaries } from './axe';

/** Cores do aviso parado: as do `.toast` (contraste ~14:1). */
const GOOD = { color: '#e4f3ef', background: '#0e2630' };
/** Texto apagado sobre o mesmo fundo (contraste ~1,5:1): erro de verdade. */
const BAD = { color: '#24414a', background: '#0e2630' };

type Colors = typeof GOOD;

/**
 * Põe na página a função que mostra o aviso de teste (`window.__axeNotice`); ela devolve o título,
 * que é o que entra com a animação.
 */
async function defineNotice(): Promise<void> {
  await browser.execute(() => {
    const page = window as unknown as { __axeNotice?: (palette: Colors) => HTMLElement };
    page.__axeNotice = (palette) => {
      document.querySelector('[data-axe-test]')?.remove();
      const notice = document.createElement('div');
      notice.dataset.axeTest = 'aviso';
      Object.assign(notice.style, {
        position: 'fixed',
        left: '24px',
        top: '24px',
        zIndex: '9999',
        padding: '12px',
        background: palette.background,
      });
      const title = document.createElement('strong');
      title.textContent = 'Concluída: Ler arquivos do computador';
      title.style.color = palette.color;
      notice.append(title);
      document.body.append(notice);
      return title;
    };
  });
}

/** Um aviso parado (sem animação) com as cores dadas. */
async function stillNotice(colors: Colors): Promise<void> {
  await browser.execute((palette: Colors) => {
    (window as unknown as { __axeNotice: (palette: Colors) => HTMLElement }).__axeNotice(palette);
  }, colors);
}

/**
 * Nas próximas `times` medições, logo antes de o axe começar, põe na tela um aviso com as cores
 * dadas, entrando (opacidade 0,3 → 1), ou, com `leaving`, faz o aviso que já está na tela começar
 * a sair como o Radix faz (`data-state="closed"` e opacidade 0,3 → 0). A animação termina logo
 * depois que o axe acaba.
 */
async function noticeDuringRuns(colors: Colors, times: number, leaving = false): Promise<void> {
  await browser.execute(
    (palette: Colors, count: number, leave: boolean) => {
      const state = window as unknown as { __axeHook?: { left: number } };
      state.__axeHook = { left: count };
      interface Axe {
        run: (...args: unknown[]) => Promise<unknown>;
      }
      let current: Axe | undefined;
      const wrapped = new WeakSet<Axe>();
      // A ajuda injeta o axe de novo a cada medição; o `run` de cada cópia só existe depois que
      // ela termina de se montar, então a troca é feita quando a ajuda pega o `axe`.
      const wrap = (axe: Axe) => {
        wrapped.add(axe);
        const run = axe.run.bind(axe);
        axe.run = async (...args: unknown[]) => {
          const hook = state.__axeHook;
          if (!hook || hook.left === 0) return run(...args);
          hook.left -= 1;
          const title = leave
            ? document.querySelector<HTMLElement>('[data-axe-test] strong')
            : (window as unknown as { __axeNotice: (palette: Colors) => HTMLElement }).__axeNotice(
                palette,
              );
          if (!title) throw new Error('o aviso de teste não está na tela');
          if (leave && title.parentElement) title.parentElement.dataset.state = 'closed';
          const moving = title.animate(
            leave ? [{ opacity: 0.3 }, { opacity: 0 }] : [{ opacity: 0.3 }, { opacity: 1 }],
            { duration: 60_000 },
          );
          try {
            return await run(...args);
          } finally {
            moving.finish();
          }
        };
      };
      Object.defineProperty(window, 'axe', {
        configurable: true,
        get: () => {
          if (current && !wrapped.has(current)) wrap(current);
          return current;
        },
        set: (value: Axe) => {
          current = value;
        },
      });
    },
    colors,
    times,
    leaving,
  );
}

describe('axe dos E2E: avisos animados (axe.ts)', () => {
  before(async () => {
    // A tela em que o app abre (Meus packs, ou o que o teste anterior deixou).
    await $('main#conteudo h1').waitForDisplayed({ timeout: 30_000 });
    await defineNotice();
  });

  afterEach(async () => {
    await browser.execute(() => {
      const state = window as unknown as { __axeHook?: { left: number } };
      delete state.__axeHook;
      document.querySelector('[data-axe-test]')?.remove();
    });
  });

  it('a tela inicial, parada, não tem violação', async () => {
    expect(await seriousViolationSummaries()).toEqual([]);
  });

  it('um aviso parado com contraste ruim reprova', async () => {
    await stillNotice(BAD);
    expect(await seriousViolationSummaries()).toEqual([expect.stringMatching(/^color-contrast: /)]);
  });

  it('um aviso que entra durante a medição é medido de novo, parado, e passa', async () => {
    await noticeDuringRuns(GOOD, 1);
    expect(await seriousViolationSummaries()).toEqual([]);
    // O aviso ficou na tela e foi medido parado.
    await expect($('[data-axe-test]')).toBeDisplayed();
  });

  it('um aviso que entra durante a medição e para com contraste ruim reprova', async () => {
    // Na segunda medição o aviso já parou, com as mesmas cores ruins.
    await noticeDuringRuns(BAD, 1);
    expect(await seriousViolationSummaries()).toEqual([expect.stringMatching(/^color-contrast: /)]);
  });

  it('um aviso que começa a sair durante a medição é medido de novo e passa', async () => {
    await stillNotice(GOOD);
    await noticeDuringRuns(GOOD, 1, true);
    expect(await seriousViolationSummaries()).toEqual([]);
  });

  it('se a tela se mexe em todas as medições, falha dizendo isso', async () => {
    await noticeDuringRuns(GOOD, 100);
    expect(await seriousViolationSummaries()).toEqual([expect.stringMatching(/^tela-sem-parar: /)]);
  });
});
