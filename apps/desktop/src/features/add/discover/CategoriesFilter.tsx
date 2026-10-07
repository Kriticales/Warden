/**
 * Categorias da coluna de filtros (SPEC T08, CA-T08-10): a lista curada em português que junta
 * as categorias do Modrinth e da CurseForge. Uma por vez ("Todas as categorias" limpa); a que
 * só existe numa fonte filtra só aquela fonte e diz qual.
 */
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import type { ProjectKind } from '../../../lib/ipc/bindings';
import { useDiscoverCategories } from './api';

export interface CategoriesFilterProps {
  kind: ProjectKind;
  value: string | null;
  onChange: (category: string | null) => void;
}

export function CategoriesFilter({ kind, value, onChange }: CategoriesFilterProps) {
  const { t } = useTranslation('descoberta');
  const { t: ta } = useTranslation('adicionar');
  const categories = useDiscoverCategories(kind);
  const name = useId();
  if (categories.isError) return <p className="t-xs t-3">{t('categorias.erro')}</p>;
  if (categories.isPending) return <p className="t-xs t-3">{t('categorias.carregando')}</p>;
  if (categories.data.length === 0) return null;
  return (
    <fieldset className="disc__fgroup">
      <legend className="field__label">{t('categorias.titulo')}</legend>
      <div className="stack-2">
        <label className="check">
          <input
            type="radio"
            name={name}
            checked={value === null}
            onChange={() => {
              onChange(null);
            }}
          />
          <span>{t('categorias.todas')}</span>
        </label>
        {categories.data.map((category) => (
          <label key={category.id} className="check">
            <input
              type="radio"
              name={name}
              checked={value === category.id}
              onChange={() => {
                onChange(category.id);
              }}
            />
            <span>
              {category.name}
              {category.sources.length === 1 ? (
                <span className="t-3">
                  {' · '}
                  {t('categorias.soUmaFonte', {
                    fonte: ta(`fonte.${category.sources[0] ?? 'modrinth'}`),
                  })}
                </span>
              ) : null}
            </span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
