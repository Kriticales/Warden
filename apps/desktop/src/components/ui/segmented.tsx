/**
 * Controle segmentado (`.segmented` do design system): um grupo de rádio com um só ponto de
 * parada do Tab e as setas trocando a opção, como um grupo de rádio nativo. Para escolhas
 * curtas e exclusivas ("Lista | Cartões", "Qualquer | Cliente | Servidor").
 */
import type { LucideIcon } from 'lucide-react';
import { type KeyboardEvent, type ReactNode, useRef } from 'react';

import { cn } from '../../lib/cn';
import { Icon } from './icon';

export interface SegmentedOption<T extends string> {
  value: T;
  label: ReactNode;
  icon?: LucideIcon;
  /** Dica (atributo `title`). */
  hint?: string;
}

export interface SegmentedProps<T extends string> {
  options: readonly SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  /** Nome do grupo (ou `labelledBy`). */
  label?: string;
  labelledBy?: string;
  /** Ocupa a largura toda, com as opções do mesmo tamanho. */
  full?: boolean;
  className?: string;
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  label,
  labelledBy,
  full = false,
  className,
}: SegmentedProps<T>) {
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const now = options.findIndex((option) => option.value === value);
    const index = (now + step + options.length) % options.length;
    const next = options[index];
    if (!next) return;
    onChange(next.value);
    buttons.current[index]?.focus();
  };
  return (
    // As setas trocam a opção, como num grupo de rádio nativo.
    // eslint-disable-next-line jsx-a11y/interactive-supports-focus
    <div
      className={cn('segmented', full && 'segmented--full', className)}
      role="radiogroup"
      aria-label={label}
      aria-labelledby={labelledBy}
      onKeyDown={onKeyDown}
    >
      {options.map((option, index) => {
        const on = option.value === value;
        return (
          <button
            key={option.value}
            ref={(element) => {
              buttons.current[index] = element;
            }}
            type="button"
            role="radio"
            aria-checked={on}
            tabIndex={on ? 0 : -1}
            title={option.hint}
            onClick={() => {
              onChange(option.value);
            }}
          >
            {option.icon ? <Icon icon={option.icon} size="sm" /> : null}
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
