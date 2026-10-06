/**
 * Selos da lista de Mods e dos detalhes (`design/system/components.js` → `source`, `badge`):
 * fonte com a marca de cada origem, e marcas de estado com ícone e texto (nunca só a cor).
 */
import { CircleX, Lock, TriangleAlert, type LucideIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import type { InventoryItem, ItemSource } from '../../../lib/ipc/bindings';

export function SourceTag({ source }: { source: ItemSource }) {
  const { t } = useTranslation('editor');
  return (
    <span className={`tag tag--${source}`}>
      <span className="tag__mark" aria-hidden="true" />
      {t(`mods.fonte.${source}`)}
    </span>
  );
}

type BadgeKind = 'danger' | 'warn';

function Badge({ kind, icon, children }: { kind: BadgeKind; icon?: LucideIcon; children: string }) {
  return (
    <span className={`badge badge--${kind}`}>
      {icon ? <Icon icon={icon} /> : null}
      {children}
    </span>
  );
}

const DANGER: BadgeKind = 'danger';
const WARN: BadgeKind = 'warn';

/** As marcas de um item ao lado do nome: inválido, fora do índice, fixado, opcional. */
export function ItemMarks({ item }: { item: InventoryItem }) {
  const { t } = useTranslation('editor');
  return (
    <>
      {item.state === 'invalid' ? (
        <Badge kind={DANGER} icon={CircleX}>
          {t('mods.marcas.invalido')}
        </Badge>
      ) : null}
      {item.state === 'outsideIndex' ? (
        <Badge kind={WARN} icon={TriangleAlert}>
          {t('mods.marcas.fora')}
        </Badge>
      ) : null}
      {item.pinned ? (
        <span className="tag tag--plain">
          <Icon icon={Lock} />
          {t('mods.marcas.fixado')}
        </span>
      ) : null}
      {item.optional ? <span className="tag">{t('mods.marcas.opcional')}</span> : null}
    </>
  );
}
