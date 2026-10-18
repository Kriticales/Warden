/**
 * A nota de saúde (`health()` do design system): no topo de Problemas, o número grande, a faixa
 * com ícone e palavra, os 10 blocos e o aviso fixo; na coluna de Meus packs, número, ícone e
 * palavra numa linha. A faixa nunca é só cor: o ícone e a palavra dizem o mesmo. Sem nota
 * ("sem dados"), o número vira "—".
 */
import { CircleCheck, CircleHelp, CircleX, TriangleAlert, type LucideIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import type { HealthBand } from '../../../lib/ipc/bindings';

/** A nota a mostrar (`null`: sem dados). */
export interface HealthValue {
  score: number;
  band: HealthBand;
}

const BANDS: Record<HealthBand | 'none', { css: string; icon: LucideIcon }> = {
  great: { css: 'great', icon: CircleCheck },
  good: { css: 'good', icon: CircleCheck },
  attention: { css: 'warn', icon: TriangleAlert },
  critical: { css: 'bad', icon: CircleX },
  none: { css: 'none', icon: CircleHelp },
};

/** A palavra da faixa ("Crítico", "Sem dados"). */
export function useBandLabel(value: HealthValue | null): string {
  const { t } = useTranslation('saude');
  return value ? t(`faixa.${value.band}`) : t('faixa.semDados');
}

export function HealthMeter({
  value,
  size,
  note,
}: {
  value: HealthValue | null;
  size: 'lg' | 'sm';
  /** Texto abaixo da faixa (lg); padrão: o aviso fixo. */
  note?: string;
}) {
  const { t } = useTranslation('saude');
  const band = BANDS[value?.band ?? 'none'];
  const label = useBandLabel(value);
  const number = value ? String(value.score) : t('semNota');

  if (size === 'sm') {
    return (
      <span className={`health health--sm health--${band.css}`}>
        <span className="health__num">{number}</span>
        <Icon icon={band.icon} size="sm" />
        <span className="health__lbl">{label}</span>
      </span>
    );
  }
  const on = value ? Math.round(value.score / 10) : 0;
  return (
    <div
      className={`health health--lg health--${band.css}`}
      role="group"
      aria-label={value ? t('rotulo', { nota: value.score, faixa: label }) : t('rotuloSemDados')}
    >
      <div className="health__num" aria-hidden="true">
        {number}
        <small>{t('de100')}</small>
      </div>
      <div className="health__side">
        <div className="health__band">
          <Icon icon={band.icon} />
          <span>{label}</span>
        </div>
        <div className="health__blocks" aria-hidden="true">
          {Array.from({ length: 10 }, (_, index) => (
            <i key={index} className={index < on ? 'on' : undefined} />
          ))}
        </div>
        <p className="health__note">{note ?? t('aviso')}</p>
      </div>
    </div>
  );
}
