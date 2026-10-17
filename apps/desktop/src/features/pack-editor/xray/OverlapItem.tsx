/**
 * Uma sobreposição no raio-x (protótipo `W.mixinRow`): a parte do jogo, o risco com selo e
 * palavra, quem altera e como, o motivo em linguagem simples, o que subiu o risco e o que o
 * baixou ("compatibilidade intencional", "pode ser desligado pelo próprio mod", "par conhecido
 * como compatível").
 */
import { Info } from 'lucide-react';
import { Fragment } from 'react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import type { Demoter, Elevator, Overlap, Risk } from '../../../lib/ipc/bindings';
import { annotation, fullClass, partLabel } from './text';

const RISK_CLASS: Record<Risk, string> = { high: 'high', medium: 'medium', low: 'low' };
const BADGE: Record<Risk, string> = { high: 'danger', medium: 'warn', low: 'neutral' };

export function RiskBadge({ risk }: { risk: Risk }) {
  const { t } = useTranslation('raioX');
  return <span className={`badge badge--${BADGE[risk]}`}>{t(`risco.${risk}`)}</span>;
}

export interface OverlapItemProps {
  overlap: Overlap;
  /** Nome de cada item do pack pelo caminho. */
  names: Partial<Record<string, string>>;
}

export function OverlapItem({ overlap, names }: OverlapItemProps) {
  const { t } = useTranslation('raioX');
  const name = (path: string) => names[path] ?? path.split('/').pop() ?? path;
  const demoter = (d: Demoter): string => {
    switch (d.kind) {
      case 'allPlugin':
      case 'knownCompatible':
        return t(`rebaixador.${d.kind}`);
      case 'compatPackage':
        return t('rebaixador.compatPackage', { nome: name(d.item) });
      case 'referencesOther':
      case 'declaresDependency':
      case 'pluginCitesOther':
        return t(`rebaixador.${d.kind}`, { nome: name(d.item), outro: name(d.other) });
    }
  };
  const elevator = (e: Elevator): string => {
    switch (e.kind) {
      case 'knownConflict':
        return t('elevador.knownConflict');
      case 'crash':
        return t('elevador.crash', {
          config: e.config,
          arquivo: e.file,
          linha: e.line.toLocaleString('pt-BR'),
        });
      case 'require':
        return t('elevador.require', { config: e.config });
    }
  };
  return (
    <li className={`mixin mixin--${RISK_CLASS[overlap.risk]}`}>
      <div className="mixin__head">
        <span className="mixin__target">{partLabel(overlap)}</span>
        <RiskBadge risk={overlap.risk} />
      </div>
      <div className="path">{fullClass(overlap)}</div>
      <div className="mixin__who">
        {overlap.participants.map((participant, index) => (
          <Fragment key={`${participant.item}:${participant.mixinClass}:${participant.kind}`}>
            {index > 0 ? (
              <span className="t-3" aria-hidden="true">
                +
              </span>
            ) : null}
            <span className="mixin__mod">
              <b>{name(participant.item)}</b>{' '}
              <span className="t-mono t-3">{annotation(participant.kind)}</span>{' '}
              <span className="t-3">{t(`tipo.${participant.kind}`)}</span>
            </span>
          </Fragment>
        ))}
      </div>
      <p className="mixin__why">{t(`regra.${overlap.rule}`)}</p>
      {overlap.elevators.map((e) => (
        <p key={JSON.stringify(e)} className="mixin__why">
          {elevator(e)}
        </p>
      ))}
      {overlap.demoters.map((d) => (
        <p key={JSON.stringify(d)} className="mixin__lower">
          <Icon icon={Info} size="sm" />
          {demoter(d)}
        </p>
      ))}
    </li>
  );
}
