/**
 * "N itens para revisar" em Problemas (SPEC T24: o pack importado abre "com o aviso 'N itens
 * para revisar' em Problemas"), no app inteiro com o backend simulado.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { ImportPending } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { orderedProblemsBlocks } from '../../diagnostics/problems-blocks';
import { makeInventory, makeItem } from '../../pack-editor/editor.fixtures';
import { editorBackend } from '../../pack-editor/testing';

const PENDING: ImportPending = {
  file: 'Create+ 6.0.0.mrpack',
  items: [
    { path: 'mods/status.pw.toml', name: 'Status Effect Bars', reason: 'unknownSide' },
    { path: 'config/cliente.toml', name: 'cliente.toml', reason: 'clientOverride' },
    { path: 'mods/caseiro.jar', name: 'caseiro.jar', reason: 'localFile' },
  ],
};

const inventory = makeInventory([
  makeItem({ path: 'mods/status.pw.toml', name: 'Status Effect Bars' }),
  makeItem({ path: 'mods/caseiro.jar', name: 'caseiro.jar', source: 'local' }),
]);

async function openProblems(url: string) {
  renderApp(`${url}/problemas`);
  return screen.findByRole('heading', { level: 1, name: 'Problemas' }, { timeout: 10_000 });
}

function notice(): HTMLElement {
  const block = screen.getByText('3 itens para revisar').closest<HTMLElement>('.alert');
  if (!block) throw new Error('o aviso não está num .alert');
  return block;
}

describe('itens para revisar do pack importado, em Problemas (T24)', () => {
  it('mostra "3 itens para revisar" com o motivo de cada um e Revisar; passa no axe', async () => {
    const { url } = editorBackend({ inventory, handlers: { import_review: () => PENDING } });
    await openProblems(url);
    expect(await screen.findByText('3 itens para revisar')).toBeDefined();
    const block = notice();
    expect(
      within(block).getByText(
        'A importação de Create+ 6.0.0.mrpack marcou estes itens para você conferir. Quando terminar, clique em Já revisei.',
      ),
    ).toBeDefined();
    expect(
      within(block).getByText('lado desconhecido no modpack: entrou como Cliente e servidor'),
    ).toBeDefined();
    expect(
      within(block).getByText(
        'ficou como arquivo local: confira se a licença permite levá-lo no pack',
      ),
    ).toBeDefined();
    // Mods e jars abrem em Mods; o arquivo de client-overrides/, em Configs.
    expect(
      within(block).getByRole('link', { name: 'Revisar Status Effect Bars' }).getAttribute('href'),
    ).toBe(`${url}/mods?item=mods%2Fstatus.pw.toml`);
    expect(
      within(block).getByRole('link', { name: 'Revisar cliente.toml' }).getAttribute('href'),
    ).toBe(`${url}/configs?arquivo=config%2Fcliente.toml`);
    // É o primeiro bloco da seção.
    expect(orderedProblemsBlocks()[0]?.id).toBe('import-review');
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('um item só: "1 item para revisar"', async () => {
    const { url } = editorBackend({
      inventory,
      handlers: { import_review: () => ({ ...PENDING, items: PENDING.items.slice(0, 1) }) },
    });
    await openProblems(url);
    expect(await screen.findByText('1 item para revisar')).toBeDefined();
  });

  it('Revisar abre o item em Mods', async () => {
    const { url } = editorBackend({ inventory, handlers: { import_review: () => PENDING } });
    await openProblems(url);
    await screen.findByText('3 itens para revisar');
    await userEvent.setup().click(screen.getByRole('link', { name: 'Revisar Status Effect Bars' }));
    // A seção Mods fica aberta (os detalhes do item podem cobrir o resto da página).
    await waitFor(() => {
      const nav = screen.getByRole('navigation', { name: 'Seções do pack', hidden: true });
      const mods = within(nav).getByRole('link', { name: /^Mods/, hidden: true });
      expect(mods.getAttribute('aria-current')).toBe('page');
    });
  });

  it('"Já revisei" grava e tira o aviso (a consulta é relida)', async () => {
    let reviewed = false;
    const { backend, url } = editorBackend({
      inventory,
      handlers: {
        import_review: () => (reviewed ? null : PENDING),
        import_review_done: () => {
          reviewed = true;
          return null;
        },
      },
    });
    await openProblems(url);
    await screen.findByText('3 itens para revisar');
    await userEvent.setup().click(screen.getByRole('button', { name: 'Já revisei' }));
    await waitFor(() => {
      expect(screen.queryByText('3 itens para revisar')).toBeNull();
    });
    expect(backend.callsOf('import_review_done')).toHaveLength(1);
    expect(backend.callsOf('import_review').length).toBeGreaterThanOrEqual(2);
  });

  it('quando um item sai do pack (inventário mudou), a contagem é relida', async () => {
    let items = PENDING.items;
    const { backend, row, url } = editorBackend({
      inventory,
      handlers: { import_review: () => ({ ...PENDING, items }) },
    });
    await openProblems(url);
    await screen.findByText('3 itens para revisar');
    items = PENDING.items.slice(1);
    await backend.emit('pack-changed', { packId: row.id, areas: ['inventory'], external: false });
    expect(await screen.findByText('2 itens para revisar')).toBeDefined();
  });

  it('pack não importado (ou erro ao ler): nada aparece', async () => {
    const { url } = editorBackend({
      inventory,
      handlers: {
        import_review: () => ipcError(makeAppError({ domain: 'import', code: 'INTERNAL' })),
      },
    });
    await openProblems(url);
    expect(screen.queryByText(/para revisar/)).toBeNull();
  });
});
