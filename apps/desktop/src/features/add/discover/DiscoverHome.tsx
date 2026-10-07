/**
 * Início da descoberta (SPEC T08, CA-T08-10; protótipo `adicionar` → `startContent`): com o
 * campo vazio, as listas "Populares para <loader> <versão>" e "Atualizados recentemente", já
 * filtradas para o pack. Listas (nunca grade de cartões), com a mesma linha da busca: caixa de
 * seleção, abrir a pré-visualização e "Já no pack". O que já está no pack não ocupa lugar nas
 * listas; os nomes dos mais baixados que já entraram aparecem numa nota.
 */
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Skeleton } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import type { PackId, ProjectKind, SearchResult, SearchSort } from '../../../lib/ipc/bindings';
import { isInPack } from '../common/model';
import { ResultRow } from '../common/ResultRow';
import { type PackTarget, useTargetText } from '../common/text';
import { WarningAlert } from '../common/WarningAlert';
import { useDiscoverHome } from './api';

export interface DiscoverHomeProps {
  packId: PackId;
  kind: ProjectKind;
  target: PackTarget;
  selected: ReadonlyMap<string, SearchResult>;
  currentKey: string | null;
  inventoryKeys: ReadonlySet<string>;
  onToggle: (result: SearchResult) => void;
  onOpen: (result: SearchResult) => void;
  /** "Ver mais": abre a busca completa já ordenada. */
  onSeeMore: (sort: SearchSort) => void;
}

export function DiscoverHome({
  packId,
  kind,
  target,
  selected,
  currentKey,
  inventoryKeys,
  onToggle,
  onOpen,
  onSeeMore,
}: DiscoverHomeProps) {
  const { t } = useTranslation('descoberta');
  const alvo = useTargetText(target);
  const home = useDiscoverHome(packId, kind, true);
  const names = useMemo(
    () =>
      new Intl.ListFormat('pt-BR', { style: 'long', type: 'conjunction' }).format(
        home.data?.popularInPack ?? [],
      ),
    [home.data],
  );

  if (home.isPending) return <HomeSkeleton />;
  if (home.isError) {
    return (
      <ErrorPanel
        title={t('inicio.erro')}
        error={home.error}
        onRetry={() => {
          void home.refetch();
        }}
      />
    );
  }
  const { popular, updated, popularInPack, warnings } = home.data;
  const warningAlerts = warnings.map((warning) => (
    <WarningAlert
      key={`${warning.source}:${warning.reason}`}
      warning={warning}
      onRetry={() => {
        void home.refetch();
      }}
    />
  ));
  if (popular.length === 0 && updated.length === 0) {
    return (
      <div className="stack">
        {warningAlerts}
        <EmptyState
          compact
          glyph="search"
          title={t('inicio.vazio')}
          text={t('inicio.vazioTexto', { alvo })}
        />
      </div>
    );
  }
  const list = (items: readonly SearchResult[], label: string) => (
    <ul className="dlist" aria-label={label}>
      {items.map((result) => (
        <ResultRow
          key={result.key}
          result={result}
          inPack={isInPack(result, inventoryKeys)}
          selected={selected.has(result.key)}
          current={currentKey === result.key}
          onToggle={onToggle}
          onOpen={onOpen}
        />
      ))}
    </ul>
  );
  const popularTitle = t('inicio.populares', { alvo });
  const updatedTitle = t('inicio.atualizados');
  return (
    <div className="stack" role="region" aria-label={t('inicio.rotulo')}>
      {warningAlerts}
      {popular.length > 0 ? (
        <section aria-labelledby="disc-popular">
          <div className="disc__h">
            <h2 className="group-title" id="disc-popular">
              {popularTitle}
            </h2>
            <Button
              variant="link"
              aria-label={t('inicio.verMaisPopulares')}
              onClick={() => {
                onSeeMore('downloads');
              }}
            >
              {t('inicio.verMais')}
            </Button>
          </div>
          {list(popular, popularTitle)}
          {popularInPack.length > 0 ? (
            <p className="t-xs t-3 disc__note">
              {t('inicio.jaNoPack', { count: popularInPack.length, names })}
            </p>
          ) : null}
        </section>
      ) : null}
      {updated.length > 0 ? (
        <section aria-labelledby="disc-updated">
          <div className="disc__h">
            <h2 className="group-title" id="disc-updated">
              {updatedTitle}
            </h2>
            <Button
              variant="link"
              aria-label={t('inicio.verMaisAtualizados')}
              onClick={() => {
                onSeeMore('updated');
              }}
            >
              {t('inicio.verMais')}
            </Button>
          </div>
          {list(updated, updatedTitle)}
        </section>
      ) : null}
    </div>
  );
}

function HomeSkeleton() {
  return (
    <div className="stack" aria-busy="true">
      {[0, 1].map((group) => (
        <ul key={group} className="dlist">
          {Array.from({ length: 4 }, (_, index) => (
            <li key={index} className="drow" aria-hidden="true">
              <Skeleton shape="tile" width="18px" />
              <Skeleton shape="tile" width="40px" />
              <span className="stack-2">
                <Skeleton width="45%" />
                <Skeleton width="80%" />
              </span>
              <span />
            </li>
          ))}
        </ul>
      ))}
    </div>
  );
}
