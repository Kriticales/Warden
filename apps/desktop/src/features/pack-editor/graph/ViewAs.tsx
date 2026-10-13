/**
 * "Ver como: Lista · Grafo" (SPEC T06; protótipo `viewAs`): o modo de exibição da seção Mods,
 * não uma aba. Grupo de rádio com um só ponto de parada do Tab e as setas trocando o modo.
 * Trocar o modo remonta a barra de ferramentas; o foco volta para a opção escolhida.
 */
import { useEffect, useRef, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

/** Modo da seção Mods (`?ver=grafo` na rota; ARCHITECTURE §18). */
export type ModsView = 'lista' | 'grafo';

const VIEWS: readonly ModsView[] = ['lista', 'grafo'];

/** Modo escolhido por clique ou seta, cujo botão recebe o foco quando montar. */
let pendingFocus: ModsView | null = null;

function requestFocus(view: ModsView) {
  pendingFocus = view;
}

/** `true` (uma vez) se o modo montado é o que pediu o foco. */
function takeFocus(view: ModsView): boolean {
  if (pendingFocus !== view) return false;
  pendingFocus = null;
  return true;
}

export function ViewAs({ view, onChange }: { view: ModsView; onChange: (view: ModsView) => void }) {
  const { t } = useTranslation('editor');
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  useEffect(() => {
    if (!takeFocus(view)) return;
    buttons.current[VIEWS.indexOf(view)]?.focus();
  }, [view]);
  const choose = (next: ModsView) => {
    if (next === view) return;
    requestFocus(next);
    onChange(next);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLSpanElement>) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const index = (VIEWS.indexOf(view) + step + VIEWS.length) % VIEWS.length;
    choose(VIEWS[index] ?? view);
  };
  return (
    <span className="row">
      <span className="t-xs t-3" id="ver-como">
        {t('mods.filtros.verComo')}
      </span>
      {/* As setas trocam o modo, como num grupo de rádio nativo. */}
      {/* eslint-disable-next-line jsx-a11y/interactive-supports-focus */}
      <span
        className="segmented"
        role="radiogroup"
        aria-labelledby="ver-como"
        onKeyDown={onKeyDown}
      >
        {VIEWS.map((option, index) => (
          <button
            key={option}
            ref={(element) => {
              buttons.current[index] = element;
            }}
            type="button"
            role="radio"
            aria-checked={option === view}
            tabIndex={option === view ? 0 : -1}
            onClick={() => {
              choose(option);
            }}
          >
            {option === 'lista' ? t('mods.filtros.lista') : t('mods.filtros.grafo')}
          </button>
        ))}
      </span>
    </span>
  );
}
