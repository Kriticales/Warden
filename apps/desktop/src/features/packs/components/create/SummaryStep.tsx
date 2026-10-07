/**
 * Etapa final, "Resumo" (T03): o pack como vai ficar e os arquivos que o Warden cria (sem
 * `packwiz init`; SPEC T03 "O que o Warden cria") e os mods iniciais escolhidos (P1-18).
 */
import { useQuery } from '@tanstack/react-query';
import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { NameTile } from '../../../../components/common/PixelArt';
import { Icon } from '../../../../components/ui/icon';
import { useKits } from '../../../add/kits/api';
import { createCheckQuery, useCreateDefaults } from '../../api';
import { loaderName } from '../../lib/pack-list';
import { useInitialOffer } from '../../create/initial-mods/api';
import { toolNames } from '../../create/initial-mods/model';
import type { Draft } from './draft';

const FILES = ['pack', 'gitattributes', 'ignore', 'changelog', 'historico'] as const;

/** Versão de todo pack novo (`pack.toml`). */
const INITIAL_VERSION = '0.1.0';

export function SummaryStep({ draft }: { draft: Draft }) {
  const { t } = useTranslation('packs');
  const { t: tMods } = useTranslation('modsIniciais');
  const defaults = useCreateDefaults();
  const path = useQuery(createCheckQuery(draft.name, draft.destination));
  const name = draft.name.trim();
  const typedAuthor = draft.author.trim();
  const author = typedAuthor !== '' ? typedAuthor : (defaults.data?.author ?? '');
  // A oferta já está no cache (a etapa anterior a buscou): o resumo só lê.
  const offer = useInitialOffer(draft.minecraft, draft.loader);
  const tools = draft.initial === null ? [] : toolNames(offer.data, draft.initial);
  const kit = draft.initial?.kit ?? null;
  const kits = useKits(draft.minecraft, draft.loader);
  const kitName = kit === null ? undefined : kits.data?.find((known) => known.id === kit.id)?.name;
  const loader =
    draft.loader === null
      ? t('lista.vanilla')
      : t('lista.loader', { loader: loaderName(draft.loader), version: draft.loaderVersion ?? '' });
  const parts = [
    ...tools,
    ...(kit !== null && kitName !== undefined
      ? [tMods('resumo.kit', { name: kitName, count: kit.projects.length })]
      : []),
  ];
  const withCrashAssistant = draft.initial?.tools.includes('crash-assistant') ?? false;
  const initialLine =
    parts.length === 0
      ? draft.loader === null
        ? null
        : tMods('resumo.nenhum')
      : `${tMods('resumo.lista', { nomes: parts.join(', ') })}${
          withCrashAssistant ? `, com a ${tMods('resumo.config')}` : ''
        }`;
  return (
    <div className="wizard__body--wide stack">
      <div className="packcard">
        <NameTile seed={name} size="xl" />
        <div className="packcard__name">{name}</div>
        <div className="packcard__meta">
          {t('criar.resumo.meta', { mc: draft.minecraft ?? '', loader, autor: author })}
        </div>
        <dl className="packcard__facts">
          <div>
            <dt>{t('criar.resumo.versaoInicial')}</dt>
            <dd className="t-mono">{INITIAL_VERSION}</dd>
          </div>
          <div>
            <dt>{t('criar.resumo.pasta')}</dt>
            <dd className="path">{path.data ?? draft.destination ?? ''}</dd>
          </div>
        </dl>
      </div>
      <section className="panel" aria-labelledby="criar-arquivos">
        <h2 className="panel__title panel__title--sans" id="criar-arquivos">
          {t('criar.resumo.arquivos')}
        </h2>
        <ul className="checklist mt-2">
          {FILES.map((file) => (
            <li key={file} className="is-ok">
              <Icon icon={Check} />
              <span>{t(`criar.resumo.itens.${file}`)}</span>
            </li>
          ))}
          {initialLine === null ? null : (
            <li className="is-ok">
              <Icon icon={Check} />
              <span>{initialLine}</span>
            </li>
          )}
        </ul>
      </section>
    </div>
  );
}
