/**
 * Os arquivos que pedem decisão ou explicação (SPEC T19): troca CurseForge → Modrinth por hash,
 * mods que o formato não pode levar (bloqueio explicado) e arquivos de terceiros que iriam
 * dentro, com a confirmação de licença.
 */
import { ExternalLink } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import type { FormatAnalysis, FormatItem, LauncherFormat } from '../../../lib/ipc/bindings';
import { openExternal, safeExternalUrl } from '../../../lib/external';
import { Checkbox } from '../../settings/components/fields';
import { embeddedCount, isSwapRequired, isSwapped, type FormatDecisions } from './model';

function useOriginText(): (item: FormatItem) => string {
  const { t } = useTranslation('exportarFormatos');
  return (item) => {
    switch (item.origin) {
      case 'modrinth':
        return t('decisoes.modrinth');
      case 'curseforge':
        return t('decisoes.curseforge');
      case 'local':
        return t('decisoes.local');
      case 'link':
        return item.host ? t('decisoes.link', { host: item.host }) : t('decisoes.linkSemHost');
    }
  };
}

export function DecisionsPanel({
  analysis,
  decisions,
  format,
  onChange,
}: {
  analysis: FormatAnalysis;
  decisions: FormatDecisions;
  format: LauncherFormat;
  onChange: (patch: Partial<FormatDecisions>) => void;
}) {
  const { t } = useTranslation('exportarFormatos');
  const origin = useOriginText();
  const embedded = embeddedCount(analysis, decisions);
  const toggleKeep = (path: string, keep: boolean) => {
    const next = new Set(decisions.keep);
    if (keep) next.add(path);
    else next.delete(path);
    onChange({ keep: next });
  };
  return (
    <section className="panel" aria-labelledby="exf-decisoes">
      <h2 className="panel__title panel__title--sans" id="exf-decisoes">
        {t('decisoes.titulo', { count: analysis.items.length })}
      </h2>
      <ul className="stack mt-3">
        {analysis.items.map((item) => (
          <li key={item.path}>
            <div className="t-sm">
              <b>{item.name}</b> <span className="t-3">{item.filename}</span>{' '}
              <span className="t-3">· {origin(item)}</span>
            </div>
            <ItemBody
              item={item}
              swapped={isSwapped(item, decisions)}
              onSwap={(swap) => {
                toggleKeep(item.path, !swap);
              }}
            />
          </li>
        ))}
      </ul>
      {embedded > 0 ? (
        <Alert kind="warn" title={t('confirmar.titulo')} className="mt-4">
          <p>{format === 'mrpack' ? t('confirmar.mrpack') : t('confirmar.curseforge')}</p>
          <div className="mt-2">
            <Checkbox
              label={t('confirmar.rotulo')}
              checked={decisions.confirmEmbed}
              onChange={(confirmEmbed) => {
                onChange({ confirmEmbed });
              }}
            />
          </div>
        </Alert>
      ) : null}
    </section>
  );
}

function ItemBody({
  item,
  swapped,
  onSwap,
}: {
  item: FormatItem;
  swapped: boolean;
  onSwap: (swap: boolean) => void;
}) {
  const { t } = useTranslation('exportarFormatos');
  const outcome = item.outcome;
  switch (outcome.kind) {
    case 'swap':
      return (
        <div className="mt-1">
          <p className="t-sm t-2">
            {isSwapRequired(item) ? t('troca.obrigatoria') : t('troca.opcional')}
          </p>
          <Checkbox
            label={t('troca.rotulo')}
            desc={
              swapped
                ? t('troca.destino', { projeto: outcome.project_id, arquivo: outcome.filename })
                : t('troca.semTroca')
            }
            checked={swapped}
            disabled={isSwapRequired(item)}
            onChange={onSwap}
          />
        </div>
      );
    case 'blocked': {
      const page = safeExternalUrl(outcome.page_url);
      return (
        <Alert kind="danger" compact title={t('bloqueado.titulo')} className="mt-1">
          {t('bloqueado.texto')}
          {page ? (
            <div className="mt-2">
              <Button
                size="sm"
                icon={ExternalLink}
                onClick={() => {
                  void openExternal(page.href);
                }}
              >
                {t('bloqueado.pagina')}
              </Button>
            </div>
          ) : null}
        </Alert>
      );
    }
    case 'unavailable':
      return (
        <Alert kind="danger" compact title={t('indisponivel.titulo')} className="mt-1">
          {t('indisponivel.texto')}
        </Alert>
      );
    case 'embed':
      return <p className="t-sm t-2 mt-1">{t('embutido.texto')}</p>;
  }
}
