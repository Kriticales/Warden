/**
 * Indicador de etapas de um assistente (DESIGN-SYSTEM §6; `steps()` em
 * `design/system/components.js`): número de cada etapa, ✓ nas concluídas e a atual marcada
 * com `aria-current="step"`. O leitor de tela ouve "(concluída)" e "(agora)".
 */
import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { cn } from '../../lib/cn';
import { Icon } from '../ui/icon';

export interface StepsProps {
  /** Nome de cada etapa, na ordem. */
  items: readonly string[];
  /** Índice da etapa atual (a partir de 0). */
  current: number;
  /** Nome acessível da lista ("Etapas de criar pack"). */
  label: string;
}

export function Steps({ items, current, label }: StepsProps) {
  const { t } = useTranslation();
  return (
    <ol className="steps" aria-label={label}>
      {items.map((name, index) => {
        const done = index < current;
        const now = index === current;
        return (
          <li
            key={name}
            className={cn('steps__item', done && 'steps__item--done')}
            aria-current={now ? 'step' : undefined}
          >
            <span className="steps__mark" aria-hidden="true">
              {done ? <Icon icon={Check} /> : index + 1}
            </span>
            <span className="steps__label">
              {name}
              {done || now ? (
                <span className="sr-only">{done ? t('etapas.concluida') : t('etapas.agora')}</span>
              ) : null}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
