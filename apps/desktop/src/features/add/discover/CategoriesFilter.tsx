/**
 * Categoria da coluna de filtros (SPEC T08, CA-T08-10; esboço `design/bug-01/adicionar.html`):
 * a lista curada em português que junta as categorias do Modrinth e da CurseForge, curta (8 e
 * "Mais N categorias"), sem texto quebrando. Uma por vez ("Todas" limpa); a que só existe numa
 * loja leva a marca discreta dela (e filtra só aquela fonte).
 */
import { Check } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import type { ProjectKind } from '../../../lib/ipc/bindings';
import { SourceDots } from '../common/text';
import { useDiscoverCategories } from './api';

/** Quantas categorias aparecem antes de "Mais N categorias". */
const SHORT_LIST = 8;

export interface CategoriesFilterProps {
  kind: ProjectKind;
  value: string | null;
  onChange: (category: string | null) => void;
}

export function CategoriesFilter({ kind, value, onChange }: CategoriesFilterProps) {
  const { t } = useTranslation('descoberta');
  const { t: ta } = useTranslation('adicionar');
  const categories = useDiscoverCategories(kind);
  const [more, setMore] = useState(false);
  const id = useId();
  if (categories.isError) return <p className="t-xs t-3">{t('categorias.erro')}</p>;
  if (categories.isPending) return <p className="t-xs t-3">{t('categorias.carregando')}</p>;
  if (categories.data.length === 0) return null;
  // A marca de uma loja só faz sentido com as duas ligadas: sem a CurseForge, toda categoria é
  // só do Modrinth e a marca seria ruído.
  const activeSources = new Set(categories.data.flatMap((category) => category.sources)).size;
  const all = categories.data;
  // A escolhida continua à vista mesmo fora das 8 primeiras.
  const shown = more
    ? all
    : all.filter((category, index) => index < SHORT_LIST || category.id === value);
  const hidden = all.length - SHORT_LIST;
  return (
    <section className="fsec" aria-labelledby={`${id}-t`}>
      <h3 className="fsec__t" id={`${id}-t`}>
        {t('categorias.titulo')}
      </h3>
      <ul className="clist">
        <li>
          <button
            type="button"
            aria-pressed={value === null}
            onClick={() => {
              onChange(null);
            }}
          >
            <Icon icon={Check} size="sm" />
            <span>{t('categorias.todas')}</span>
          </button>
        </li>
        {shown.map((category) => {
          const only = category.sources.length < activeSources ? category.sources[0] : undefined;
          return (
            <li key={category.id}>
              <button
                type="button"
                aria-pressed={value === category.id}
                title={
                  only
                    ? `${category.name} · ${ta(only === 'curseforge' ? 'fonte.soCurseforge' : 'fonte.soModrinth')}`
                    : category.name
                }
                onClick={() => {
                  onChange(value === category.id ? null : category.id);
                }}
              >
                <Icon icon={Check} size="sm" />
                <span>{category.name}</span>
                {only ? <SourceDots sources={[{ source: only }]} /> : null}
              </button>
            </li>
          );
        })}
      </ul>
      {hidden > 0 ? (
        <Button
          variant="link"
          className="clist__more"
          aria-expanded={more}
          onClick={() => {
            setMore((now) => !now);
          }}
        >
          {more ? t('categorias.menos') : t('categorias.mais', { count: hidden })}
        </Button>
      ) : null}
    </section>
  );
}
