/**
 * Coluna de filtros da página Adicionar, à direita (esboço `design/bug-01/adicionar.html`,
 * opção 4; ADR-0068), igual na Lista e nos Cartões: Categoria (lista curta com 8 e "Mais N"),
 * Funciona em, Mostrar também (Bibliotecas, O que já está no pack, Mods sem versão para o pack)
 * e Fonte (só com mais de uma fonte ligada). **Limpar** volta tudo ao padrão.
 *
 * Bibliotecas e "O que já está no pack" mudam só o que aparece (sem nova busca); categoria,
 * lado, fonte e "sem versão" vão para a busca e trocam o início pelos resultados.
 */
import { ListFilter } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { Segmented } from '../../../components/ui/segmented';
import type {
  EnvironmentFilter,
  ProjectKind,
  SearchSort,
  SourceFilter,
  SourceId,
} from '../../../lib/ipc/bindings';
import { CategoriesFilter } from '../discover/CategoriesFilter';

/** Os filtros que o usuário muda (o resto vem do pack). */
export interface UserFilters {
  sort: SearchSort;
  source: SourceFilter;
  environment: EnvironmentFilter | null;
  includeIncompatible: boolean;
  /** `id` da categoria curada (nenhuma: todas). */
  category: string | null;
  /** "Mostrar também: Bibliotecas" (escondidas por padrão). */
  showLibraries: boolean;
  /** "Mostrar também: O que já está no pack" (ligado por padrão, com o selo No pack). */
  showInPack: boolean;
}

export const NO_FILTERS: UserFilters = {
  sort: 'relevance',
  source: 'all',
  environment: null,
  includeIncompatible: false,
  category: null,
  showLibraries: false,
  showInPack: true,
};

/** Quantos filtros mudam a busca (qualquer um deles troca o início pelos resultados). */
export function activeFilterCount(filters: UserFilters): number {
  return [
    filters.sort !== NO_FILTERS.sort,
    filters.source !== NO_FILTERS.source,
    filters.environment !== null,
    filters.includeIncompatible,
    filters.category !== null,
  ].filter(Boolean).length;
}

/** Se há algo para o **Limpar** desfazer (inclusive o que só muda o que aparece). */
export function filtersChanged(filters: UserFilters): boolean {
  return (
    activeFilterCount(filters) > 0 ||
    filters.showLibraries !== NO_FILTERS.showLibraries ||
    filters.showInPack !== NO_FILTERS.showInPack
  );
}

/** Os filtros de busca, sem o que só muda o que aparece. */
export function searchFilters(filters: UserFilters) {
  return {
    sort: filters.sort,
    source: filters.source,
    environment: filters.environment,
    includeIncompatible: filters.includeIncompatible,
    category: filters.category,
  };
}

const SOURCE_FILTERS: readonly SourceFilter[] = ['all', 'modrinth', 'curseforge'];
const ENVIRONMENTS: readonly ('all' | EnvironmentFilter)[] = ['all', 'client', 'server'];

export interface FiltersColumnProps {
  kind: ProjectKind;
  filters: UserFilters;
  onChange: (filters: UserFilters) => void;
  /** Fontes conhecidas: o filtro Fonte só aparece com mais de uma. */
  sources: readonly SourceId[];
}

export function FiltersColumn({ kind, filters, onChange, sources }: FiltersColumnProps) {
  const { t } = useTranslation('adicionar');
  const changed = filtersChanged(filters);
  const id = useId();
  return (
    <aside className="side" aria-label={t('filtros.rotulo')}>
      <div className="sidehead">
        <h2 className="sidehead__t">
          <Icon icon={ListFilter} />
          {t('filtros.rotulo')}
        </h2>
        <Button
          size="sm"
          variant="ghost"
          disabled={!changed}
          onClick={() => {
            onChange({ ...NO_FILTERS, sort: filters.sort });
          }}
        >
          {t('filtros.limpar')}
        </Button>
      </div>
      <div className="sidebody">
        <CategoriesFilter
          kind={kind}
          value={filters.category}
          onChange={(category) => {
            onChange({ ...filters, category });
          }}
        />
        <section className="fsec">
          <h3 className="fsec__t" id={`${id}-lado`}>
            {t('filtros.funcionaEm')}
          </h3>
          <Segmented
            full
            labelledBy={`${id}-lado`}
            value={filters.environment ?? 'all'}
            options={ENVIRONMENTS.map((value) => ({
              value,
              label: t(`filtros.ambiente.${value === 'all' ? 'qualquer' : value}`),
            }))}
            onChange={(value) => {
              onChange({ ...filters, environment: value === 'all' ? null : value });
            }}
          />
        </section>
        <section className="fsec">
          <h3 className="fsec__t">{t('filtros.mostrar')}</h3>
          <div className="fchecks">
            {kind === 'mod' ? (
              <label className="check">
                <input
                  type="checkbox"
                  checked={filters.showLibraries}
                  onChange={(event) => {
                    onChange({ ...filters, showLibraries: event.target.checked });
                  }}
                />
                <span className="check__text">
                  {t('filtros.bibliotecas')}
                  <span className="check__desc">{t('filtros.bibliotecasDica')}</span>
                </span>
              </label>
            ) : null}
            <label className="check">
              <input
                type="checkbox"
                checked={filters.showInPack}
                onChange={(event) => {
                  onChange({ ...filters, showInPack: event.target.checked });
                }}
              />
              <span>{t('filtros.noPack')}</span>
            </label>
            <label className="check">
              <input
                type="checkbox"
                checked={filters.includeIncompatible}
                onChange={(event) => {
                  onChange({ ...filters, includeIncompatible: event.target.checked });
                }}
              />
              <span className="check__text">
                {t('filtros.semVersao')}
                <span className="check__desc">{t('filtros.semVersaoDica')}</span>
              </span>
            </label>
          </div>
        </section>
        {sources.length > 1 ? (
          <section className="fsec">
            <h3 className="fsec__t" id={`${id}-fonte`}>
              {t('filtros.fonte')}
            </h3>
            <Segmented
              full
              labelledBy={`${id}-fonte`}
              value={filters.source}
              options={SOURCE_FILTERS.map((value) => ({
                value,
                label: t(`filtros.fonteOpcoes.${value}`),
              }))}
              onChange={(source) => {
                onChange({ ...filters, source });
              }}
            />
          </section>
        ) : null}
      </div>
    </aside>
  );
}
