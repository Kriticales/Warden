/**
 * Bloco "Depende de / Usado por / Por que está no pack" nos detalhes do item (T07; CA-T07-03) e
 * o aviso "Se você remover…" (T06; CA-T06-06), com o backend simulado.
 */
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { DependentsReport, InventoryItem, WhyReport } from '../../../../lib/ipc/bindings';
import { axePage } from '../../../../test/axe';
import { ipcError } from '../../../../test/backend';
import { makeAppError } from '../../../../test/factories';
import { renderApp } from '../../../../test/render';
import { makeDetails, makeInventory, makeItem } from '../../editor.fixtures';
import { editorBackend } from '../../testing';
import { describeWhy } from './why';

const node = (name: string) => ({ path: `mods/${name.toLowerCase()}.pw.toml`, name });
const LIB = makeItem({ name: 'Lib' });
const BACKPACKS = makeItem({ name: 'Backpacks' });

const emptyReport: DependentsReport = { targets: [], dependsOn: [], usedBy: [], affected: [] };

async function openDrawer(
  item: InventoryItem,
  dependents: DependentsReport,
  why: WhyReport,
  items: InventoryItem[] = [item],
) {
  const setup = editorBackend({
    inventory: makeInventory(items),
    handlers: {
      item_details: () => makeDetails(item),
      graph_dependents: () => dependents,
      graph_why_in_pack: () => why,
    },
  });
  renderApp(setup.url);
  await screen.findByRole('searchbox', { name: 'Buscar no pack' }, { timeout: 10_000 });
  await userEvent.setup().click(screen.getByRole('button', { name: item.name }));
  const drawer = await screen.findByRole('dialog', { name: 'Detalhes do mod' });
  return { ...setup, drawer };
}

describe('bloco Dependências dos detalhes (D-07)', () => {
  it('CA-T07-03: biblioteca mostra a cadeia até o que o usuário adicionou; axe', async () => {
    const { drawer, backend } = await openDrawer(
      LIB,
      {
        ...emptyReport,
        dependsOn: [
          {
            id: 'forge',
            kind: 'required',
            range: '[47,)',
            state: 'inPack',
            providers: [{ item: node('Forge'), modId: 'forge', embedded: false, alias: false }],
            note: null,
          },
          {
            id: 'jei',
            kind: 'optional',
            range: null,
            state: 'missing',
            providers: [],
            note: null,
          },
          { id: 'sodium', kind: 'breaks', range: null, state: 'inPack', providers: [], note: null },
        ],
        usedBy: [
          {
            item: node('Backpacks'),
            kind: 'required',
            range: null,
            id: 'lib',
            embedded: false,
            note: null,
          },
        ],
        affected: [{ item: node('Backpacks'), depth: 1, needs: [node('Lib')] }],
      },
      {
        item: node('Lib'),
        why: {
          kind: 'requiredBy',
          chains: [
            [
              { item: node('Lib'), cycle: [] },
              { item: node('Backpacks'), cycle: [] },
            ],
          ],
        },
        cycle: [],
      },
      [LIB, BACKPACKS],
    );
    expect(
      await within(drawer).findByText('Exigido por Backpacks, que você adicionou'),
    ).toBeDefined();
    expect(drawer.textContent).toContain('Depende de');
    expect(drawer.textContent).toContain(
      'forge · obrigatória · [47,) · no pack · fornecido por Forge',
    );
    expect(drawer.textContent).toContain('jei · opcional · falta no pack');
    expect(drawer.textContent).toContain('sodium · incompatível · está no pack');
    expect(drawer.textContent).toContain('Backpacks · obrigatória');
    expect(drawer.textContent).toContain('1 mod para de funcionar: Backpacks');
    expect(backend.callsOf('graph_why_in_pack')[0]?.args).toMatchObject({ path: LIB.path });
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T07-03: mod adicionado direto diz "Você adicionou"; sem relações mostra o vazio', async () => {
    const { drawer } = await openDrawer(LIB, emptyReport, {
      item: node('Lib'),
      why: { kind: 'userAdded', at: '2026-09-02T10:00:00Z' },
      cycle: [],
    });
    expect(await within(drawer).findByText(/^Você adicionou em /)).toBeDefined();
    expect(drawer.textContent).toContain('Nenhum outro mod');
    expect(drawer.textContent).toContain('Nenhum mod do pack');
    expect(drawer.textContent).not.toContain('Se você remover');
  });

  it('biblioteca sem uso diz que pode ser removida (CA-T06-06)', async () => {
    const { drawer } = await openDrawer(LIB, emptyReport, {
      item: node('Lib'),
      why: { kind: 'unused', optionalUsers: [node('Extra')] },
      cycle: [node('Outro')],
    });
    expect(
      await within(drawer).findByText(
        'Nenhum mod usa mais; pode ser removido. Só é usado como opcional por Extra',
      ),
    ).toBeDefined();
    expect(drawer.textContent).toContain('Faz parte de um ciclo de dependência com Outro');
  });

  it('falha da consulta mostra o erro com "Tentar de novo", sem derrubar o painel', async () => {
    const setup = editorBackend({
      handlers: {
        item_details: () => makeDetails(LIB),
        graph_dependents: () =>
          ipcError(
            makeAppError(
              { domain: 'project', code: 'ITEM_NOT_FOUND' },
              { params: { path: LIB.path } },
            ),
          ),
      },
      inventory: makeInventory([LIB]),
    });
    renderApp(setup.url);
    await screen.findByRole('searchbox', { name: 'Buscar no pack' }, { timeout: 10_000 });
    await userEvent.setup().click(screen.getByRole('button', { name: LIB.name }));
    const drawer = await screen.findByRole('dialog', { name: 'Detalhes do mod' });
    expect(await within(drawer).findByRole('button', { name: /Tentar de novo/ })).toBeDefined();
    expect(within(drawer).getByRole('button', { name: 'Remover' })).toBeDefined();
  });

  it('resource packs e shaders não consultam o grafo', async () => {
    const shader = makeItem({ name: 'Brilho', kind: 'shader' });
    const setup = editorBackend({
      inventory: makeInventory([shader]),
      handlers: { item_details: () => makeDetails(shader) },
    });
    renderApp(setup.url);
    await screen.findByRole('searchbox', { name: 'Buscar no pack' }, { timeout: 10_000 });
    await userEvent.setup().click(screen.getByRole('button', { name: 'Brilho' }));
    await screen.findByRole('dialog', { name: 'Detalhes do shader' });
    expect(setup.backend.callsOf('graph_dependents')).toHaveLength(0);
  });
});

describe('describeWhy', () => {
  it('cadeia longa lista os nomes depois do item e o ciclo vem por último', () => {
    const lines = describeWhy({
      item: node('A'),
      why: {
        kind: 'requiredBy',
        chains: [
          [
            { item: node('A'), cycle: [] },
            { item: node('B'), cycle: [] },
            { item: node('C'), cycle: [] },
          ],
        ],
      },
      cycle: [node('B')],
    });
    expect(lines.map((line) => line.kind)).toEqual(['chain', 'cycle']);
    expect(lines[0]).toMatchObject({ names: ['B', 'C'] });
  });
});
