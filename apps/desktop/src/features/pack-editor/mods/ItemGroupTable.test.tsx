/**
 * Grupo virtualizado da lista de Mods (SPEC T06; A-04): um grupo abaixo da tela não monta
 * linhas antes de saber onde está. Antes, a primeira renderização supunha o grupo no topo da
 * área que rola e montava uma tela inteira de linhas que sumiam logo em seguida, o que pesava
 * na abertura de um pack com 500 itens.
 *
 * O jsdom não mede elementos: a área que rola (`#conteudo`) ganha 900 px de altura e o corpo
 * da tabela, a posição dada a cada teste.
 */
import { screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { renderWithProviders } from '../../../test/render';
import type { InventoryItem } from '../../../lib/ipc/bindings';
import { MAIN_ID } from '../../../app/layout/focus';
import { makeItem } from '../editor.fixtures';
import { ItemGroupTable, ROW_HEIGHT } from './ItemGroupTable';

afterEach(() => {
  vi.restoreAllMocks();
});

/** A área que rola tem 900 px e o corpo da tabela começa em `top` px. */
function measure(top: number) {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.id === MAIN_ID ? 900 : 0;
  });
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    const y = this.tagName === 'TBODY' ? top : 0;
    return { top: y, bottom: y, left: 0, right: 0, width: 0, height: 0, x: 0, y } as DOMRect;
  });
}

/** Nada a fazer: os testes só olham as linhas montadas. */
const ignore = vi.fn();

function renderGroup(items: InventoryItem[]) {
  const renderRow = vi.fn((item: InventoryItem) => (
    <tr key={item.path} className="modrow">
      <td>{item.name}</td>
    </tr>
  ));
  renderWithProviders(
    <main id={MAIN_ID}>
      <ItemGroupTable
        kind="resourcePack"
        items={items}
        collapsed={false}
        onToggleCollapsed={ignore}
        allSelected={false}
        onSelectAll={ignore}
        sortDirection="asc"
        onSort={ignore}
        renderRow={renderRow}
      />
    </main>,
  );
  return renderRow;
}

const ITEMS = Array.from({ length: 300 }, (_, index) =>
  makeItem({ name: `Item ${String(index).padStart(3, '0')}`, kind: 'resourcePack' }),
);

/** Linhas de uma tela de 900 px. */
const SCREEN_ROWS = Math.ceil(900 / ROW_HEIGHT);

describe('ItemGroupTable virtualizado', () => {
  it('um grupo bem abaixo da tela não monta uma tela de linhas antes de medir onde está', () => {
    measure(20_000);
    const renderRow = renderGroup(ITEMS);
    // Só as linhas de folga do fim da área visível, nunca uma tela inteira.
    expect(renderRow.mock.calls.length).toBeLessThan(SCREEN_ROWS);
    // A altura de todas as linhas fica nos espaçadores: a barra de rolagem continua certa.
    const spacers = Array.from(document.querySelectorAll<HTMLElement>('tr.modlist__spacer'));
    const rowsHeight = document.querySelectorAll('tbody tr.modrow').length * ROW_HEIGHT;
    const spacersHeight = spacers.reduce((sum, spacer) => sum + parseFloat(spacer.style.height), 0);
    expect(spacersHeight + rowsHeight).toBe(ITEMS.length * ROW_HEIGHT);
  });

  it('um grupo no topo monta as linhas da tela', () => {
    measure(0);
    renderGroup(ITEMS);
    expect(screen.getByRole('table').querySelectorAll('tbody tr.modrow').length).toBeGreaterThan(
      SCREEN_ROWS,
    );
    expect(screen.getByText('Item 000')).toBeDefined();
  });
});
