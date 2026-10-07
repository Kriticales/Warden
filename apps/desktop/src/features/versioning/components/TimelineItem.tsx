/**
 * Um item da linha do tempo do Histórico (`.timeline` do design system; `W.tlItem` do
 * protótipo): nó, número da versão, estado, data, resumo, ações e detalhe.
 */
import { Flag, Globe } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import { cn } from '../../../lib/cn';
import { whenParts } from '../model';

export type TimelineKind = 'unsaved' | 'saved' | 'final' | 'published';

/** O estado escrito ao lado do número, com ícone para não depender só da cor. */
export function StateTag({ kind }: { kind: TimelineKind }) {
  const { t } = useTranslation('versoes');
  switch (kind) {
    case 'unsaved':
      return <span className="tag tag--warn">{t('estado.naoSalvas')}</span>;
    case 'saved':
      return <span className="tag tag--plain">{t('estado.salva')}</span>;
    case 'final':
      return (
        <span className="tag tag--primary">
          <Icon icon={Flag} size="sm" />
          {t('estado.final')}
        </span>
      );
    case 'published':
      return (
        <span className="tag tag--ok">
          <Icon icon={Globe} size="sm" />
          {t('estado.publicada')}
        </span>
      );
  }
}

/** "hoje, 15:02", "ontem, 15:02" ou "28/09/2026, 15:02". */
export function WhenText({ iso }: { iso: string }) {
  const { t } = useTranslation('versoes');
  const when = whenParts(iso);
  if (when === null) {
    return null;
  }
  const text =
    when.relation === 'today'
      ? t('data.hoje', { hora: when.time })
      : when.relation === 'yesterday'
        ? t('data.ontem', { hora: when.time })
        : t('data.outro', { data: when.date, hora: when.time });
  return (
    <time className="tl-date" dateTime={iso}>
      {text}
    </time>
  );
}

export interface TimelineItemProps {
  kind: TimelineKind;
  /** Texto do número ("1.4.2" ou "Agora"). */
  version: ReactNode;
  /** Data e hora (RFC 3339); sem ela, o item não mostra data. */
  date?: string;
  summary?: ReactNode;
  actions?: ReactNode;
  detail?: ReactNode;
  open?: boolean;
  /** Rótulo da região, para o leitor de tela. */
  id?: string;
}

export function TimelineItem({
  kind,
  version,
  date,
  summary,
  actions,
  detail,
  open = false,
  id,
}: TimelineItemProps) {
  return (
    <li id={id} className={cn('tl-item', `tl-item--${kind}`, open && 'tl-item--open')}>
      <span className="tl-node" aria-hidden="true" />
      <div className="tl-body">
        <div className="tl-head">
          <span className="tl-version">{version}</span>
          <StateTag kind={kind} />
          {date ? <WhenText iso={date} /> : null}
          {actions ? <span className="tl-actions">{actions}</span> : null}
        </div>
        {summary ? <p className="tl-summary">{summary}</p> : null}
        {detail ? <div className="tl-detail">{detail}</div> : null}
      </div>
    </li>
  );
}
