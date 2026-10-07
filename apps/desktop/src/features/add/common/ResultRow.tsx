/**
 * Linha de resultado da busca (`design/system/components.js` → `discoverRow`): caixa de
 * seleção (sem caixa quando já está no pack; desabilitada sem versão para o pack), ícone, nome
 * que abre a pré-visualização, autor, resumo, fonte, downloads, atualização e as marcas.
 */
import { CircleMinus, Download } from 'lucide-react';
import { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { NameTile } from '../../../components/common/PixelArt';
import { Icon } from '../../../components/ui/icon';
import { cn } from '../../../lib/cn';
import type { SearchResult } from '../../../lib/ipc/bindings';
import { formatCount } from './model';
import { InPackTag, SourceMark, useAgoText } from './text';

export interface ResultRowProps {
  result: SearchResult;
  inPack: boolean;
  selected: boolean;
  current: boolean;
  onToggle: (result: SearchResult) => void;
  onOpen: (result: SearchResult) => void;
}

/** Ícone remoto do projeto; sem ícone (ou se não carregar), o bloco em pixel do nome. */
export function ProjectIcon({
  url,
  name,
  className,
  size,
}: {
  url: string | null;
  name: string;
  className: string;
  size: 'lg' | 'xl';
}) {
  const [failed, setFailed] = useState(false);
  if (!url || failed) return <NameTile seed={name} size={size} />;
  return (
    <img
      className={cn('tile', className)}
      src={url}
      alt=""
      loading="lazy"
      onError={() => {
        setFailed(true);
      }}
    />
  );
}

export const ResultRow = memo(function ResultRow({
  result,
  inPack,
  selected,
  current,
  onToggle,
  onOpen,
}: ResultRowProps) {
  const { t } = useTranslation('adicionar');
  const agoText = useAgoText();
  const state = inPack ? 'inpack' : result.compatible ? 'normal' : 'noversion';
  const updated = agoText(result.updated);
  return (
    <li className={cn('drow', `drow--${state}`, current && 'is-current')}>
      <span className="drow__box">
        {inPack ? null : (
          <label className="check">
            <input
              type="checkbox"
              checked={selected}
              disabled={!result.compatible}
              aria-label={t('resultado.selecionar', { name: result.title })}
              onChange={() => {
                onToggle(result);
              }}
            />
          </label>
        )}
      </span>
      <ProjectIcon url={result.iconUrl} name={result.title} className="drow__icon" size="lg" />
      <span className="drow__main">
        <span className="drow__line">
          <button
            type="button"
            className="drow__name"
            aria-current={current ? 'true' : undefined}
            onClick={() => {
              onOpen(result);
            }}
          >
            {result.title}
          </button>
          {result.author ? (
            <>
              {' '}
              <span className="t-xs t-3">{t('resultado.por', { author: result.author })}</span>
            </>
          ) : null}
        </span>
        {result.summary ? <span className="drow__desc">{result.summary}</span> : null}
        <span className="drow__meta">
          <SourceMark sources={result.sources} />
          <span>{t('resultado.downloads', { value: formatCount(result.downloads) })}</span>
          {updated ? <span>{t('resultado.atualizado', { when: updated })}</span> : null}
        </span>
      </span>
      <span className="drow__end">
        {inPack ? (
          <InPackTag />
        ) : !result.compatible ? (
          <span className="drow__why">
            <Icon icon={CircleMinus} size="sm" />
            {t('resultado.semVersao')}
          </span>
        ) : result.manualDownload ? (
          <span className="tag tag--warn" title={t('resultado.downloadManualDica')}>
            <Icon icon={Download} />
            {t('resultado.downloadManual')}
          </span>
        ) : null}
      </span>
    </li>
  );
});
