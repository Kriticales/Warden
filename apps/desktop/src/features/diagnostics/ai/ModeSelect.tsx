/**
 * "Procurar rápido | Procurar a fundo" (ADR-0064; protótipo `design/bug-01/sensor-sculk.html`):
 * um grupo de rádio no formato `.segmented` do design system, com as setas trocando a escolha
 * como num rádio nativo. Só aparece no Claude e na OpenAI: no Gemini não há escolha.
 */
import { useRef, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Tooltip } from '../../../components/ui/tooltip';
import type { SearchMode } from '../../../lib/ipc/bindings';
import { cn } from '../../../lib/cn';

export const MODES: readonly SearchMode[] = ['quick', 'deep'];

export function ModeSelect({
  value,
  onChange,
  labelledBy,
  disabled = false,
  className,
}: {
  value: SearchMode;
  onChange: (mode: SearchMode) => void;
  /** Id do rótulo visível; sem ele, o grupo leva "Como o Sculk procura" como nome. */
  labelledBy?: string;
  disabled?: boolean;
  className?: string;
}) {
  const { t } = useTranslation('ia');
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const choose = (mode: SearchMode) => {
    onChange(mode);
    buttons.current[MODES.indexOf(mode)]?.focus();
  };
  const onKeyDown = (event: KeyboardEvent<HTMLSpanElement>) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (step === undefined || disabled) return;
    event.preventDefault();
    const index = (MODES.indexOf(value) + step + MODES.length) % MODES.length;
    choose(MODES[index] ?? value);
  };
  return (
    // As setas trocam o modo, como num grupo de rádio nativo.
    // eslint-disable-next-line jsx-a11y/interactive-supports-focus
    <span
      className={cn('segmented', className)}
      role="radiogroup"
      aria-labelledby={labelledBy}
      aria-label={labelledBy ? undefined : t('modo.grupo')}
      onKeyDown={onKeyDown}
    >
      {MODES.map((mode, index) => (
        <Tooltip key={mode} content={t(`modo.dica.${mode}`)}>
          <button
            ref={(element) => {
              buttons.current[index] = element;
            }}
            type="button"
            role="radio"
            aria-checked={mode === value}
            tabIndex={mode === value ? 0 : -1}
            disabled={disabled}
            onClick={() => {
              choose(mode);
            }}
          >
            {t(`modo.${mode}`)}
          </button>
        </Tooltip>
      ))}
    </span>
  );
}
