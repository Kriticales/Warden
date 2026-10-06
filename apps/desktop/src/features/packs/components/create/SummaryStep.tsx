/**
 * Etapa final, "Resumo" (T03): o pack como vai ficar e os arquivos que o Warden cria (sem
 * `packwiz init`; SPEC T03 "O que o Warden cria"). A P1-18 acrescenta os mods iniciais.
 */
import { useQuery } from '@tanstack/react-query';
import { Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { NameTile } from '../../../../components/common/PixelArt';
import { Icon } from '../../../../components/ui/icon';
import { createCheckQuery, useCreateDefaults } from '../../api';
import { loaderName } from '../../lib/pack-list';
import type { Draft } from './draft';

const FILES = ['pack', 'gitattributes', 'ignore', 'changelog', 'historico'] as const;

/** Versão de todo pack novo (`pack.toml`). */
const INITIAL_VERSION = '0.1.0';

export function SummaryStep({ draft }: { draft: Draft }) {
  const { t } = useTranslation('packs');
  const defaults = useCreateDefaults();
  const path = useQuery(createCheckQuery(draft.name, draft.destination));
  const name = draft.name.trim();
  const typedAuthor = draft.author.trim();
  const author = typedAuthor !== '' ? typedAuthor : (defaults.data?.author ?? '');
  const loader =
    draft.loader === null
      ? t('lista.vanilla')
      : t('lista.loader', { loader: loaderName(draft.loader), version: draft.loaderVersion ?? '' });
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
        </ul>
      </section>
    </div>
  );
}
