/**
 * Coluna de filtros da página Adicionar (protótipo `filters`): a linha travada com o que o pack
 * aceita (versão do Minecraft, loader e tipo, sempre aplicados), Ambiente, Ordenar e, em "Mais
 * filtros", Fonte (só com mais de uma fonte) e "Mostrar também os sem versão compatível".
 * Categorias chegam com a P1-16.
 */
import { Lock } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import type {
  EnvironmentFilter,
  SearchSort,
  SourceFilter,
  SourceId,
} from '../../../lib/ipc/bindings';
import { type PackTarget, useTargetText } from './text';

/** Os filtros que o usuário muda (o resto vem do pack). */
export interface UserFilters {
  sort: SearchSort;
  source: SourceFilter;
  environment: EnvironmentFilter | null;
  includeIncompatible: boolean;
}

export const NO_FILTERS: UserFilters = {
  sort: 'relevance',
  source: 'all',
  environment: null,
  includeIncompatible: false,
};

const SORTS: readonly SearchSort[] = ['relevance', 'downloads', 'updated', 'newest'];
const SOURCE_FILTERS: readonly SourceFilter[] = ['all', 'modrinth', 'curseforge'];
const ENVIRONMENTS: readonly EnvironmentFilter[] = ['client', 'server'];

export interface FiltersColumnProps {
  target: PackTarget;
  filters: UserFilters;
  onChange: (filters: UserFilters) => void;
  /** Fontes conhecidas nesta busca: o filtro Fonte só aparece com mais de uma. */
  sources: readonly SourceId[];
}

export function FiltersColumn({ target, filters, onChange, sources }: FiltersColumnProps) {
  const { t } = useTranslation('adicionar');
  const targetText = useTargetText(target);
  const id = useId();
  return (
    <aside className="disc__filters" aria-label={t('filtros.rotulo')}>
      <p className="filters-lock">
        <Icon icon={Lock} size="sm" />
        <span>
          {t('filtros.trava')} <b>{targetText}</b>.
        </span>
      </p>
      <div className="field">
        <label className="field__label" htmlFor={`${id}-env`}>
          {t('filtros.ambiente')}
        </label>
        <select
          id={`${id}-env`}
          className="select"
          value={filters.environment ?? ''}
          onChange={(event) => {
            const environment = ENVIRONMENTS.find((value) => value === event.target.value) ?? null;
            onChange({ ...filters, environment });
          }}
        >
          <option value="">{t('filtros.ambienteOpcoes.qualquer')}</option>
          {ENVIRONMENTS.map((value) => (
            <option key={value} value={value}>
              {t(`filtros.ambienteOpcoes.${value}`)}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <label className="field__label" htmlFor={`${id}-sort`}>
          {t('filtros.ordenar')}
        </label>
        <select
          id={`${id}-sort`}
          className="select"
          value={filters.sort}
          onChange={(event) => {
            const sort = SORTS.find((value) => value === event.target.value) ?? 'relevance';
            onChange({ ...filters, sort });
          }}
        >
          {SORTS.map((value) => (
            <option key={value} value={value}>
              {t(`filtros.ordem.${value}`)}
            </option>
          ))}
        </select>
      </div>
      <details className="disclosure">
        <summary>{t('filtros.mais')}</summary>
        <div className="stack-2">
          {sources.length > 1 ? (
            <div className="field">
              <label className="field__label" htmlFor={`${id}-src`}>
                {t('filtros.fonte')}
              </label>
              <select
                id={`${id}-src`}
                className="select"
                value={filters.source}
                onChange={(event) => {
                  const source =
                    SOURCE_FILTERS.find((value) => value === event.target.value) ?? 'all';
                  onChange({ ...filters, source });
                }}
              >
                {SOURCE_FILTERS.map((value) => (
                  <option key={value} value={value}>
                    {t(`filtros.fonteOpcoes.${value}`)}
                  </option>
                ))}
              </select>
            </div>
          ) : null}
          <label className="check">
            <input
              type="checkbox"
              checked={filters.includeIncompatible}
              onChange={(event) => {
                onChange({ ...filters, includeIncompatible: event.target.checked });
              }}
            />
            <span>{t('filtros.semVersao')}</span>
          </label>
        </div>
      </details>
    </aside>
  );
}
