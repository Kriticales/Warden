/**
 * Barra de progresso em blocos (`BlockProgress`; shadcn `Progress` sobre o Radix; HANDOFF §3).
 * `value` de 0 a 100, ou `null` para indeterminado (um trecho andando em degraus). Estados:
 * em andamento, esperando você, erro e concluída. `valueText` vai para `aria-valuetext`
 * (por exemplo, o tempo restante).
 */
import * as ProgressPrimitive from '@radix-ui/react-progress';
import { useId, type CSSProperties, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { cn } from '../../lib/cn';

export type ProgressState = 'running' | 'waiting' | 'error' | 'done';

export interface ProgressBarProps {
  /** 0 a 100; `null` = indeterminado. */
  value: number | null;
  /** O que está acontecendo (rótulo visível e nome acessível). */
  label: ReactNode;
  /** Linha de baixo: quantidades, tempo restante. */
  meta?: ReactNode;
  state?: ProgressState;
  size?: 'md' | 'sm';
  /** Texto para o leitor de tela no lugar da porcentagem. */
  valueText?: string;
  /** Mostra a porcentagem ao lado do rótulo. Padrão: sim, quando determinado. */
  showValue?: boolean;
  className?: string | undefined;
}

function clamp(value: number): number {
  return Math.min(100, Math.max(0, value));
}

export function ProgressBar({
  value,
  label,
  meta,
  state = 'running',
  size = 'md',
  valueText,
  showValue = true,
  className,
}: ProgressBarProps) {
  const { t } = useTranslation();
  const labelId = useId();
  const indeterminate = value === null;
  const percent = indeterminate ? 0 : Math.round(clamp(value));
  return (
    <div
      className={cn(
        'progress',
        indeterminate && 'progress--indeterminate',
        state !== 'running' && `progress--${state}`,
        size === 'sm' && 'progress--sm',
        className,
      )}
    >
      <div className="progress__top">
        <span className="progress__label" id={labelId}>
          {label}
        </span>
        {!indeterminate && showValue ? (
          <span className="progress__value" aria-hidden="true">
            {t('progresso.concluido', { valor: percent })}
          </span>
        ) : null}
      </div>
      <ProgressPrimitive.Root
        className="progress__track"
        value={indeterminate ? null : percent}
        max={100}
        aria-labelledby={labelId}
        {...(valueText ? { getValueLabel: () => valueText } : {})}
      >
        <ProgressPrimitive.Indicator
          className="progress__fill"
          style={{ '--p': `${String(percent)}%` } as CSSProperties}
        />
      </ProgressPrimitive.Root>
      {meta ? <div className="progress__meta">{meta}</div> : null}
    </div>
  );
}
