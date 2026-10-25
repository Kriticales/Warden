/**
 * Início da descoberta (SPEC T08, CA-T08-10; esboço `design/bug-01/adicionar.html`): com o campo
 * vazio, os grupos "Populares" e "Atualizados recentemente", já filtrados para o pack, na Lista
 * ou nos Cartões, com a contagem "17 de 2.481" e **Ver todos** (a busca completa já ordenada).
 * O que já está no pack aparece com o selo No pack; as bibliotecas ficam escondidas numa linha
 * (ADR-0068). Para mods, os "Kits de desempenho" da P1-18 vêm depois dos grupos (cada kit abre
 * o diálogo Adicionar itens).
 */
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../../../components/common/EmptyState';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { Skeleton } from '../../../components/common/LoadingState';
import type { AddResult, Loader, PackId, ProjectKind, SearchSort } from '../../../lib/ipc/bindings';
import { type PackTarget, useTargetText } from '../common/text';
import { WarningAlert } from '../common/WarningAlert';
import { KitsList } from '../kits/KitsList';
import { type ResultGroup, ResultsView, type ResultsViewProps } from '../results/ResultsView';
import { useDiscoverHome } from './api';

/** Os grupos do início (chaves, não textos da interface). */
const POPULAR: ResultGroup['key'] = 'populares';
const UPDATED: ResultGroup['key'] = 'atualizados';

/** Os loaders que têm kits (o pack guarda o loader como texto). */
const KIT_LOADERS: readonly Loader[] = ['forge', 'neoforge', 'fabric'];

export interface DiscoverHomeProps {
  packId: PackId;
  kind: ProjectKind;
  target: PackTarget;
  /** Como mostrar os grupos (modo, seleção, filtros do que aparece). */
  results: Omit<ResultsViewProps, 'groups'>;
  /** "Ver todos": abre a busca completa já ordenada. */
  onSeeMore: (sort: SearchSort) => void;
  /** Depois de gravar um kit pelo diálogo Adicionar itens. */
  onAdded: (result: AddResult) => void;
}

export function DiscoverHome({
  packId,
  kind,
  target,
  results,
  onSeeMore,
  onAdded,
}: DiscoverHomeProps) {
  const { t } = useTranslation('descoberta');
  const alvo = useTargetText(target);
  const home = useDiscoverHome(packId, kind, true);

  // Os kits são listas de mods prontas (dados embutidos, sem rede).
  const kits =
    kind === 'mod' && target.minecraft !== '' ? (
      <KitsList
        packId={packId}
        minecraft={target.minecraft}
        loader={KIT_LOADERS.find((loader) => loader === target.loader) ?? null}
        onAdded={onAdded}
      />
    ) : null;

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
  const { popular, updated, popularTotal, updatedTotal, warnings } = home.data;
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
        {kits}
      </div>
    );
  }
  return (
    <div className="stack" role="region" aria-label={t('inicio.rotulo')}>
      {warningAlerts}
      <ResultsView
        {...results}
        groups={[
          ...(popular.length > 0
            ? [
                {
                  key: POPULAR,
                  title: t('inicio.populares'),
                  items: popular,
                  total: popularTotal,
                  seeAllLabel: t('inicio.verTodosPopulares'),
                  onSeeAll: () => {
                    onSeeMore('downloads');
                  },
                },
              ]
            : []),
          ...(updated.length > 0
            ? [
                {
                  key: UPDATED,
                  title: t('inicio.atualizados'),
                  items: updated,
                  total: updatedTotal,
                  seeAllLabel: t('inicio.verTodosAtualizados'),
                  onSeeAll: () => {
                    onSeeMore('updated');
                  },
                },
              ]
            : []),
        ]}
      />
      {kits}
    </div>
  );
}

function HomeSkeleton() {
  return (
    <div className="tbl" aria-busy="true">
      {Array.from({ length: 8 }, (_, index) => (
        <div key={index} className="trow" aria-hidden="true">
          <Skeleton shape="tile" width="18px" />
          <Skeleton width="70%" />
          <Skeleton width="85%" />
          <Skeleton width="80%" />
          <Skeleton width="80%" />
          <span />
          <span />
        </div>
      ))}
    </div>
  );
}
