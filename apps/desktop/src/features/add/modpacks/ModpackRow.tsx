/**
 * Linha de um modpack nos resultados com Tipo = Modpacks (protótipo `adicionar-modpacks`): sem
 * caixa de seleção (um modpack nunca entra no pack), ícone, nome que abre a lista de mods,
 * autor, resumo, fonte, downloads, atualização e **Ver os mods**.
 */
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { cn } from '../../../lib/cn';
import type { SearchResult } from '../../../lib/ipc/bindings';
import { formatCount } from '../common/model';
import { ProjectIcon } from '../common/ProjectIcon';
import { SourceMark, useAgoText } from '../common/text';

export interface ModpackRowProps {
  result: SearchResult;
  current: boolean;
  onOpen: (result: SearchResult) => void;
}

export const ModpackRow = memo(function ModpackRow({ result, current, onOpen }: ModpackRowProps) {
  const { t } = useTranslation('adicionar');
  const { t: tm } = useTranslation('modpacks');
  const agoText = useAgoText();
  const updated = agoText(result.updated);
  return (
    <li className={cn('drow', 'drow--nobox', current && 'is-current')}>
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
        <Button
          size="sm"
          aria-label={tm('verModsDe', { name: result.title })}
          onClick={() => {
            onOpen(result);
          }}
        >
          {tm('verMods')}
        </Button>
      </span>
    </li>
  );
});
