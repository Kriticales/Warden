/**
 * Barra de título da janela (UI-01), só no Windows: a janela não tem a barra do sistema e esta
 * ocupa o lugar dela com a identidade do Warden (marca e nome na fonte pixel, botões no estilo
 * dos menus, fechar vermelho).
 *
 * O comportamento continua o do Windows (`src-tauri/src/window_chrome`): a barra é região de
 * arraste nativa (`app-region: drag`: arrastar, duplo clique, arrastar para o topo, menu da
 * janela) e, sobre o botão maximizar, fica uma janela nativa transparente que faz aparecer o
 * menu de encaixe (Snap Layouts). Por isso a barra informa ao Rust onde o maximizar está, e o
 * Rust devolve o hover e o clique dele pelo evento `title-bar-maximize`. Pelo teclado os três
 * botões funcionam normalmente.
 *
 * Janela sem foco: a barra fica apagada, como as do sistema.
 */
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react';
import { useTranslation } from 'react-i18next';

import { BrandMark } from '../../components/common/PixelArt';
import { cn } from '../../lib/cn';
import { commands, events, type TitleBarMaximize } from '../../lib/ipc/bindings';
import { subscribe, useTauriEvent } from '../../lib/ipc/events';
import { log } from '../../lib/log';

/** O app desenha a própria barra de título (no Windows; no Linux fica a do sistema). */
export function usesCustomTitleBar(userAgent: string = navigator.userAgent): boolean {
  return /\bWindows\b/.test(userAgent);
}

/** A barra de título, quando o sistema usa a do Warden. */
export function WindowTitleBar() {
  return usesCustomTitleBar() ? <TitleBar /> : null;
}

/** Desenhos dos botões em pixels, 10 × 10 ("X" = pixel aceso), na cor do texto. */
const GLYPHS = {
  minimize: [
    '..........',
    '..........',
    '..........',
    '..........',
    '..........',
    '..........',
    '..........',
    '.XXXXXXXX.',
    '..........',
    '..........',
  ],
  maximize: [
    '..........',
    '.XXXXXXXX.',
    '.XXXXXXXX.',
    '.X......X.',
    '.X......X.',
    '.X......X.',
    '.X......X.',
    '.X......X.',
    '.XXXXXXXX.',
    '..........',
  ],
  restore: [
    '..........',
    '...XXXXXX.',
    '...XXXXXX.',
    '...X....X.',
    '.XXXXXX.X.',
    '.XXXXXX.X.',
    '.X....XXX.',
    '.X....X...',
    '.XXXXXX...',
    '..........',
  ],
  close: [
    '..........',
    '.XX....XX.',
    '.XXX..XXX.',
    '..XXXXXX..',
    '...XXXX...',
    '...XXXX...',
    '..XXXXXX..',
    '.XXX..XXX.',
    '.XX....XX.',
    '..........',
  ],
} as const;

/** Um retângulo por pixel aceso. */
function pixelsOf(rows: readonly string[]): { x: number; y: number }[] {
  const pixels: { x: number; y: number }[] = [];
  rows.forEach((row, y) => {
    for (let x = 0; x < row.length; x += 1) {
      if (row.charAt(x) === 'X') pixels.push({ x, y });
    }
  });
  return pixels;
}

function Glyph({ kind }: { kind: keyof typeof GLYPHS }) {
  return (
    <svg
      className="titlebar__glyph"
      viewBox="0 0 10 10"
      shapeRendering="crispEdges"
      aria-hidden="true"
      focusable="false"
    >
      {pixelsOf(GLYPHS[kind]).map(({ x, y }) => (
        <rect key={`${String(x)}-${String(y)}`} x={x} y={y} width={1} height={1} />
      ))}
    </svg>
  );
}

/** Erros dos comandos da janela só vão para os registros: a barra continua usável. */
function report(action: string) {
  return (error: unknown) => {
    log.warn(`janela: ${action} falhou`, error);
  };
}

export function TitleBar() {
  const { t } = useTranslation('navegacao');
  const active = useWindowActive();
  const maximized = useWindowMaximized();
  const snap = useSnapButtonState();
  const maximizeRef = useRef<HTMLButtonElement>(null);
  useReportMaximizeArea(maximizeRef);

  const appWindow = getCurrentWindow();
  return (
    <div className={cn('titlebar', !active && 'titlebar--inactive')} data-testid="titlebar">
      <span className="titlebar__brand">
        <BrandMark className="titlebar__mark" />
        <span className="titlebar__name">{t('marca')}</span>
      </span>
      <div className="titlebar__controls" role="group" aria-label={t('janela.grupo')}>
        <button
          type="button"
          className="titlebar__btn"
          aria-label={t('janela.minimizar')}
          onClick={() => {
            appWindow.minimize().catch(report('minimizar'));
          }}
        >
          <Glyph kind="minimize" />
        </button>
        <button
          ref={maximizeRef}
          type="button"
          className={cn('titlebar__btn', snap.hovered && 'is-hover', snap.pressed && 'is-press')}
          aria-label={maximized ? t('janela.restaurar') : t('janela.maximizar')}
          onClick={() => {
            appWindow.toggleMaximize().catch(report('maximizar'));
          }}
        >
          <Glyph kind={maximized ? 'restore' : 'maximize'} />
        </button>
        <button
          type="button"
          className="titlebar__btn titlebar__btn--close"
          aria-label={t('janela.fechar')}
          onClick={() => {
            appWindow.close().catch(report('fechar'));
          }}
        >
          <Glyph kind="close" />
        </button>
      </div>
    </div>
  );
}

/**
 * A janela está em primeiro plano. Vem dos eventos `focus`/`blur` da página: o
 * `onFocusChanged` do Tauri não chega quando o foco está dentro do WebView2 (visto na janela
 * real).
 */
function useWindowActive(): boolean {
  const [active, setActive] = useState(() => document.hasFocus());
  useEffect(() => {
    const onFocus = () => {
      setActive(true);
    };
    const onBlur = () => {
      setActive(false);
    };
    window.addEventListener('focus', onFocus);
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('focus', onFocus);
      window.removeEventListener('blur', onBlur);
    };
  }, []);
  return active;
}

/** A janela está maximizada (o botão vira "Restaurar"). */
function useWindowMaximized(): boolean {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    let current = true;
    const refresh = () => {
      getCurrentWindow()
        .isMaximized()
        .then((value) => {
          if (current) setMaximized(value);
        })
        .catch(report('ler se está maximizada'));
    };
    refresh();
    const stop = subscribe(
      { listen: (callback) => getCurrentWindow().onResized(callback) },
      refresh,
    );
    return () => {
      current = false;
      stop();
    };
  }, []);
  return maximized;
}

/** Hover e clique do maximizar, que chegam do Rust (a janela nativa recebe o mouse ali). */
function useSnapButtonState(): TitleBarMaximize {
  const [state, setState] = useState<TitleBarMaximize>({ hovered: false, pressed: false });
  useTauriEvent(events.titleBarMaximize, setState);
  return state;
}

/**
 * Informa ao Rust onde está o maximizar (pixels CSS, a partir da borda direita), na montagem e
 * sempre que a janela ou o botão mudam de tamanho.
 */
function useReportMaximizeArea(ref: RefObject<HTMLButtonElement | null>) {
  useLayoutEffect(() => {
    const button = ref.current;
    if (!button) return undefined;
    let last = '';
    const send = () => {
      const rect = button.getBoundingClientRect();
      const area = {
        right: document.documentElement.clientWidth - rect.right,
        top: rect.top,
        width: rect.width,
        height: rect.height,
      };
      const key = JSON.stringify(area);
      if (key === last || area.width <= 0 || area.height <= 0) return;
      last = key;
      const request = commands.windowSetMaximizeArea(area);
      request
        .then((result) => {
          if (result.status === 'error') {
            report('posicionar o menu de encaixe')(result.error);
          }
        })
        .catch(report('posicionar o menu de encaixe'));
    };
    send();
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(send);
    observer?.observe(button);
    window.addEventListener('resize', send);
    return () => {
      observer?.disconnect();
      window.removeEventListener('resize', send);
    };
  }, [ref]);
}
