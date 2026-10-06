/**
 * Etapa 3, "Loader" (T03): Forge, NeoForge, Fabric ou Nenhum (vanilla). Só aparecem os
 * loaders que existem para a versão escolhida (lista vazia no catálogo = o loader não existe;
 * CA-T03-03). A versão vem pré-selecionada pelo catálogo (Forge "recomendada", NeoForge e
 * Fabric a estável mais nova); "Escolher outra versão" mostra a lista completa. Quilt aparece
 * desabilitado, com o motivo.
 */
import { useQueries } from '@tanstack/react-query';
import { useEffect, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../../components/common/ErrorPanel';
import { Skeleton } from '../../../../components/common/LoadingState';
import { Button } from '../../../../components/ui/button';
import type { Loader, LoaderVersions } from '../../../../lib/ipc/bindings';
import { loaderVersionsQuery } from '../../api';
import { loaderName } from '../../lib/pack-list';

const LOADERS: readonly Loader[] = ['forge', 'neoforge', 'fabric'];

export interface LoaderStepProps {
  minecraft: string;
  loader: Loader | null;
  version: string | null;
  /** O usuário (ou a pré-seleção) já escolheu nesta versão do Minecraft. */
  ready: boolean;
  onChange: (loader: Loader | null, version: string | null) => void;
}

/** A versão pré-selecionada, ou a primeira da lista (a mais nova). */
export function defaultVersion(list: LoaderVersions): string | null {
  return list.preselected ?? list.versions[0]?.version ?? null;
}

export function LoaderStep({ minecraft, loader, version, ready, onChange }: LoaderStepProps) {
  const { t } = useTranslation('packs');
  const name = useId();
  const results = useQueries({
    queries: LOADERS.map((key) => loaderVersionsQuery(key, minecraft)),
  });
  const pending = results.some((result) => result.isPending);
  const lists = new Map<Loader, LoaderVersions>();
  LOADERS.forEach((key, index) => {
    const data = results[index]?.data;
    if (data && data.versions.length > 0) lists.set(key, data);
  });
  const available = LOADERS.filter((key) => lists.has(key));

  // Ao chegar na etapa, marca o primeiro loader que existe (ou vanilla, se nenhum existir).
  const first = available[0];
  const firstList = first ? lists.get(first) : undefined;
  const failed = results.some((result) => result.isError);
  useEffect(() => {
    // Com uma consulta falhando, nada é marcado sozinho: o usuário decide depois de tentar de novo.
    if (ready || pending || failed) return;
    if (first && firstList) onChange(first, defaultVersion(firstList));
    else onChange(null, null);
  }, [ready, pending, failed, first, firstList, onChange]);

  const choose = (key: Loader | null) => {
    const list = key ? lists.get(key) : undefined;
    onChange(key, list ? defaultVersion(list) : null);
  };

  return (
    <div className="wizard__body--wide">
      <p className="t-sm t-2 mb-3">{t('criar.loader.disponiveis', { mc: minecraft })}</p>
      <fieldset className="fieldset-bare" aria-busy={pending || undefined}>
        <legend className="sr-only">{t('criar.loader.legenda')}</legend>
        <div className="choice-list">
          {LOADERS.map((key, index) => {
            const result = results[index];
            if (result?.isPending) {
              return <LoaderSkeleton key={key} />;
            }
            if (result?.isError) {
              return (
                <ErrorPanel
                  key={key}
                  compact
                  title={t('criar.loader.erro', { loader: loaderName(key) })}
                  error={result.error}
                  onRetry={() => {
                    void result.refetch();
                  }}
                />
              );
            }
            const list = lists.get(key);
            if (!list) return null;
            return (
              <LoaderChoice
                key={key}
                name={name}
                loader={key}
                list={list}
                checked={loader === key}
                version={loader === key ? version : null}
                onSelect={() => {
                  choose(key);
                }}
                onVersion={(next) => {
                  onChange(key, next);
                }}
              />
            );
          })}
          <SimpleChoice
            name={name}
            title={t('criar.loader.vanilla')}
            desc={t('criar.loader.vanillaDesc')}
            checked={loader === null && ready}
            onSelect={() => {
              choose(null);
            }}
          />
          <SimpleChoice
            name={name}
            title={t('criar.loader.quilt')}
            desc={t('criar.loader.quiltDesc')}
            checked={false}
            disabled
          />
        </div>
      </fieldset>
      {!pending && available.length === 0 && results.every((result) => result.isSuccess) ? (
        <p className="field__hint mt-2">{t('criar.loader.nenhum', { mc: minecraft })}</p>
      ) : null}
    </div>
  );
}

/** Opção de rádio grande (`.choice`) só com título e explicação. */
function SimpleChoice({
  name,
  title,
  desc,
  checked,
  disabled = false,
  onSelect,
}: {
  name: string;
  title: string;
  desc: string;
  checked: boolean;
  disabled?: boolean;
  onSelect?: () => void;
}) {
  const id = useId();
  return (
    // O texto do rótulo (`title`) fica três níveis abaixo, como na marcação `.choice` do design
    // system; o rádio está ligado pelo `htmlFor`.
    // eslint-disable-next-line jsx-a11y/label-has-associated-control
    <label className="choice" htmlFor={id}>
      <span className="check">
        <input
          type="radio"
          id={id}
          name={name}
          checked={checked}
          disabled={disabled}
          aria-describedby={`${id}-desc`}
          onChange={onSelect}
        />
      </span>
      <span className="grow">
        <span className="choice__title">{title}</span>
        <span className="choice__desc block" id={`${id}-desc`}>
          {desc}
        </span>
      </span>
    </label>
  );
}

function LoaderSkeleton() {
  return (
    <div className="choice" aria-hidden="true">
      <Skeleton width="18px" />
      <div className="grow">
        <Skeleton width="30%" />
        <Skeleton width="70%" />
      </div>
    </div>
  );
}

function LoaderChoice({
  name,
  loader,
  list,
  checked,
  version,
  onSelect,
  onVersion,
}: {
  name: string;
  loader: Loader;
  list: LoaderVersions;
  checked: boolean;
  version: string | null;
  onSelect: () => void;
  onVersion: (version: string) => void;
}) {
  const { t } = useTranslation('packs');
  const [picking, setPicking] = useState(false);
  const selectId = useId();
  const shownVersion = version ?? defaultVersion(list);
  const entry = list.versions.find((item) => item.version === shownVersion);
  return (
    <div className="choice choice--split">
      <span className="check">
        <input
          type="radio"
          name={name}
          id={`${selectId}-radio`}
          checked={checked}
          onChange={onSelect}
        />
      </span>
      <span className="grow">
        <label htmlFor={`${selectId}-radio`} className="block">
          <span className="choice__title">{loaderName(loader)}</span>{' '}
          <span className="t-mono t-sm t-2">{shownVersion}</span>{' '}
          {entry?.recommended ? (
            <span className="tag tag--ok">{t('criar.loader.recomendada')}</span>
          ) : null}
          <span className="choice__desc block">{t(`criar.loader.${loader}`)}</span>
        </label>
        {checked ? (
          picking ? (
            <span className="field mt-2">
              <label className="field__label" htmlFor={selectId}>
                {t('criar.loader.versaoRotulo', { loader: loaderName(loader) })}
              </label>
              <select
                id={selectId}
                className="select select--sm"
                value={shownVersion ?? ''}
                onChange={(event) => {
                  onVersion(event.target.value);
                }}
              >
                {list.versions.map((item) => (
                  <option key={item.version} value={item.version}>
                    {item.version}
                    {item.recommended ? ` · ${t('criar.loader.recomendada')}` : ''}
                    {item.stable ? '' : ` · ${t('criar.loader.instavel')}`}
                  </option>
                ))}
              </select>
            </span>
          ) : (
            <span className="block mt-1">
              <Button
                variant="link"
                size="sm"
                onClick={() => {
                  setPicking(true);
                }}
              >
                {t('criar.loader.outraVersao', { loader: loaderName(loader) })}
              </Button>
            </span>
          )
        ) : null}
      </span>
    </div>
  );
}
