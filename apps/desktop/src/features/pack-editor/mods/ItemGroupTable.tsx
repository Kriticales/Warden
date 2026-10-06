/**
 * Um grupo da lista de Mods (Mods, Resource packs, Shaders): título recolhível com a contagem e
 * a tabela (`design/system/components.js` → `group` de `screens-pack.js`).
 *
 * Virtualizada (SPEC T06: fluida com 500 itens): acima de `VIRTUALIZE_FROM` linhas, só as que
 * estão na tela (mais uma folga) existem no DOM; espaçadores no começo e no fim mantêm a altura
 * e a barra de rolagem. A rolagem é a do `<main>` do pack, a mesma das outras seções.
 */
import { useVirtualizer } from '@tanstack/react-virtual';
import { ArrowUpDown, ChevronDown } from 'lucide-react';
import { useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { MAIN_ID } from '../../../app/layout/focus';
import { Icon } from '../../../components/ui/icon';
import { Tooltip } from '../../../components/ui/tooltip';
import type { InventoryItem, ItemKind } from '../../../lib/ipc/bindings';

/** A partir de quantas linhas o grupo é virtualizado. */
export const VIRTUALIZE_FROM = 80;
/** Altura de uma linha (`.modrow td { height: 52px }` mais a borda). */
export const ROW_HEIGHT = 53;
/** Linhas a mais renderizadas acima e abaixo da área visível. */
const OVERSCAN = 12;

export interface ItemGroupTableProps {
  kind: ItemKind;
  items: readonly InventoryItem[];
  collapsed: boolean;
  onToggleCollapsed: () => void;
  /** Todos os itens do grupo estão selecionados? (`'some'`: só alguns). */
  allSelected: boolean | 'some';
  onSelectAll: (selected: boolean) => void;
  sortDirection: 'asc' | 'desc';
  onSort: () => void;
  renderRow: (item: InventoryItem, rowIndex: number | undefined) => ReactNode;
}

export function ItemGroupTable({
  kind,
  items,
  collapsed,
  onToggleCollapsed,
  allSelected,
  onSelectAll,
  sortDirection,
  onSort,
  renderRow,
}: ItemGroupTableProps) {
  const { t } = useTranslation('editor');
  const title = t(`mods.grupos.${kind}`);
  const headId = `grupo-${kind}`;
  const bodyId = `grupo-${kind}-itens`;
  return (
    <section className="listgroup" aria-labelledby={headId}>
      <div className="listgroup__head">
        <button
          type="button"
          className="listgroup__toggle"
          id={headId}
          aria-expanded={!collapsed}
          aria-controls={bodyId}
          onClick={onToggleCollapsed}
        >
          <Icon icon={ChevronDown} />
          {title}
        </button>
        <span className="t-3 t-sm">{items.length}</span>
      </div>
      <div id={bodyId} hidden={collapsed}>
        {collapsed ? null : (
          <div className="tablewrap">
            <table
              className="table"
              aria-label={t('mods.tabela.legenda', { grupo: title })}
              aria-rowcount={items.length + 1}
            >
              <thead>
                <tr aria-rowindex={1}>
                  <th className="modrow__check">
                    <label className="check">
                      <input
                        type="checkbox"
                        checked={allSelected === true}
                        ref={(input) => {
                          if (input) input.indeterminate = allSelected === 'some';
                        }}
                        aria-label={t('mods.tabela.selecionarTodos', { grupo: title })}
                        onChange={(event) => {
                          onSelectAll(event.target.checked);
                        }}
                      />
                    </label>
                  </th>
                  <th aria-sort={sortDirection === 'asc' ? 'ascending' : 'descending'}>
                    <button
                      type="button"
                      className="sort"
                      aria-label={t('mods.tabela.ordenarNome')}
                      onClick={onSort}
                    >
                      {t('mods.tabela.nome')} <Icon icon={ArrowUpDown} size="sm" />
                    </button>
                  </th>
                  <th>{t('mods.tabela.versao')}</th>
                  <th>{t('mods.tabela.fonte')}</th>
                  <th>
                    <Tooltip content={t('mods.tabela.ladoDica')}>
                      <button type="button" className="modlist__hint">
                        {t('mods.tabela.lado')}
                      </button>
                    </Tooltip>
                  </th>
                </tr>
              </thead>
              {items.length >= VIRTUALIZE_FROM ? (
                <VirtualBody items={items} renderRow={renderRow} />
              ) : (
                <tbody>{items.map((item) => renderRow(item, undefined))}</tbody>
              )}
            </table>
          </div>
        )}
      </div>
    </section>
  );
}

function VirtualBody({
  items,
  renderRow,
}: {
  items: readonly InventoryItem[];
  renderRow: ItemGroupTableProps['renderRow'];
}) {
  const body = useRef<HTMLTableSectionElement>(null);
  const [margin, setMargin] = useState(0);
  const scroller = () => document.getElementById(MAIN_ID);

  // Distância do começo das linhas até o topo do conteúdo rolável (muda com avisos, filtros e
  // grupos acima deste).
  useLayoutEffect(() => {
    const main = scroller();
    if (!body.current || !main) return;
    const top =
      body.current.getBoundingClientRect().top - main.getBoundingClientRect().top + main.scrollTop;
    if (Math.abs(top - margin) > 1) setMargin(top);
  }, [items, margin]);

  // O TanStack Virtual devolve funções que o React Compiler não memoiza; este componente fica
  // fora da compilação, o que é o esperado para a lista virtualizada.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: scroller,
    estimateSize: () => ROW_HEIGHT,
    overscan: OVERSCAN,
    scrollMargin: margin,
    getItemKey: (index) => items[index]?.path ?? index,
    // Sem medidas (testes, primeira pintura): uma tela de 900 px.
    initialRect: { width: 1200, height: 900 },
  });
  const rows = virtualizer.getVirtualItems();
  const first = rows[0];
  const last = rows[rows.length - 1];
  const before = first ? first.start - margin : 0;
  const after = last ? virtualizer.getTotalSize() - (last.end - margin) : 0;
  return (
    <tbody ref={body}>
      {before > 0 ? (
        <tr aria-hidden="true" className="modlist__spacer" style={{ height: before }} />
      ) : null}
      {rows.map((row) => {
        const item = items[row.index];
        return item ? renderRow(item, row.index + 2) : null;
      })}
      {after > 0 ? (
        <tr aria-hidden="true" className="modlist__spacer" style={{ height: after }} />
      ) : null}
    </tbody>
  );
}
