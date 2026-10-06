/**
 * Menu lateral do pack (SPEC T05; `design/system/components.js` → `sectionMenu`): as 6 seções,
 * com nome em pixel, descrição e contador. Seção sem página registrada aparece desabilitada,
 * com o motivo na dica. Recolhido (`compact`, página de descoberta da P1-09; ESTRUTURA N6):
 * só os ícones, com nome e descrição na dica e no nome acessível.
 */
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import { Tooltip } from '../../../components/ui/tooltip';
import { cn } from '../../../lib/cn';
import type { PackId } from '../../../lib/ipc/bindings';
import {
  PACK_SECTIONS,
  sectionPage,
  type PackSection,
  type SectionCount,
  type SectionPage,
} from '../sections';

export interface SectionMenuProps {
  packId: PackId;
  compact?: boolean;
}

export function SectionMenu({ packId, compact = false }: SectionMenuProps) {
  const { t } = useTranslation('editor');
  return (
    <nav className={cn('secmenu', compact && 'secmenu--compact')} aria-label={t('secoes.rotulo')}>
      <ul>
        {PACK_SECTIONS.map((section) => (
          <li key={section.id}>
            <SectionItem
              packId={packId}
              section={section}
              page={sectionPage(section.id)}
              compact={compact}
            />
          </li>
        ))}
      </ul>
    </nav>
  );
}

interface SectionItemProps {
  packId: PackId;
  section: PackSection;
  page: SectionPage | undefined;
  compact: boolean;
}

function SectionItem({ packId, section, page, compact }: SectionItemProps) {
  const { t } = useTranslation('editor');
  // A lista de seções é fixa: o hook do contador é chamado sempre na mesma ordem.
  const count = (page?.useCount ?? noCount)(packId);
  const name = t(`secoes.${section.id}.nome`);
  const desc = t(`secoes.${section.id}.desc`);
  const className = cn('secmenu__item', section.ai && 'secmenu__item--ai');

  const content = compact ? (
    <>
      <Icon icon={section.icon} />
      {count && count.value > 0 ? (
        <span className={`secmenu__dot secmenu__dot--${count.kind}`} aria-hidden="true" />
      ) : null}
      <span className="sr-only">
        {name}
        {count ? `, ${count.label}` : ''}
      </span>
    </>
  ) : (
    <>
      <Icon icon={section.icon} />
      <span className="secmenu__name">{name}</span>
      {count ? (
        <span
          className={cn('count', count.kind !== 'neutral' && `count--${count.kind}`)}
          aria-label={count.label}
        >
          {count.value}
        </span>
      ) : null}
      <span className="secmenu__desc">{desc}</span>
    </>
  );

  if (!page) {
    return (
      <Tooltip
        content={compact ? `${name}: ${t('secoes.indisponivel')}` : t('secoes.indisponivel')}
      >
        <button type="button" className={cn(className, 'is-unavailable')} aria-disabled="true">
          {content}
        </button>
      </Tooltip>
    );
  }
  const link = (
    <Link
      to={page.to}
      params={{ packId }}
      className={className}
      activeProps={{ 'aria-current': 'page' }}
    >
      {content}
    </Link>
  );
  return compact ? <Tooltip content={`${name}: ${desc}`}>{link}</Tooltip> : link;
}

function noCount(): SectionCount | null {
  return null;
}
