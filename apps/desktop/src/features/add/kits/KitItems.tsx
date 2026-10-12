/**
 * Os mods de um kit de desempenho, cada um com a sua caixa (ADR-0033): nada entra sem estar
 * marcado e visível. Protótipo: `design/prototipo-final/screens-discover.js` (`kit`).
 */
import { TriangleAlert } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import type { Kit, KitItem } from '../../../lib/ipc/bindings';
import { SourceMark } from '../common/text';

export interface KitItemsProps {
  kit: Kit;
  selection: ReadonlySet<string>;
  onToggle: (projectId: string) => void;
  /** Só mostra os mods (o kit ainda não foi escolhido). */
  disabled?: boolean;
}

/** Texto do lado fixado pelo kit; sem lado, vale o que a fonte informa. */
function sideKey(item: KitItem): 'both' | 'client' | 'server' | null {
  return item.side;
}

function noteText(note: string): 'worldgen' | 'shaders' | null {
  return note === 'worldgen' || note === 'shaders' ? note : null;
}

export function KitItems({ kit, selection, onToggle, disabled = false }: KitItemsProps) {
  const { t } = useTranslation('modsIniciais');
  return (
    <ul className="dlist" aria-label={t('kit.itens', { name: kit.name })}>
      {kit.items.map((item) => {
        const side = sideKey(item);
        const note = item.note === null ? null : noteText(item.note);
        return (
          <li key={item.projectId} className="drow drow--compact">
            <span className="drow__box">
              <label className="check">
                <input
                  type="checkbox"
                  checked={selection.has(item.projectId)}
                  disabled={disabled}
                  aria-label={t('kit.marcarItem', { name: item.name })}
                  onChange={() => {
                    onToggle(item.projectId);
                  }}
                />
              </label>
            </span>
            <span className="drow__main">
              <span className="drow__line">
                <b>{item.name}</b>
              </span>
              <span className="drow__meta">
                <SourceMark sources={[{ source: item.source }]} />
                {side ? <span>{t(`kit.lado.${side}`)}</span> : null}
              </span>
              {note ? (
                <span className="drow__desc">
                  <Icon icon={TriangleAlert} size="sm" /> {t(`kit.notas.${note}`)}
                </span>
              ) : null}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
