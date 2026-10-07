/**
 * "Kits de desempenho" (SPEC T08; ADR-0033): a lista mostra os mods de cada kit com caixas, e
 * "Adicionar kit" passa sempre pelo diálogo de dependências, com só os itens marcados.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { AddResult, Kit } from '../../../lib/ipc/bindings';
import { axeComponent } from '../../../test/axe';
import { mockBackend } from '../../../test/backend';
import { nextPackId } from '../../../test/factories';
import { renderWithProviders } from '../../../test/render';
import { FABRIC_KIT } from '../../packs/create/initial-mods/initial-mods.fixtures';
import { makeNode, makePlan } from '../add.fixtures';
import { KitsList } from './KitsList';

const ADDED: AddResult = {
  added: [
    { key: 'modrinth:AANobbMI', title: 'Sodium', path: 'mods/sodium.pw.toml' },
    { key: 'modrinth:gvQqBUqZ', title: 'Lithium', path: 'mods/lithium.pw.toml' },
  ],
  skipped: [],
  removed: [],
};

function setup(kits: Kit[] = [FABRIC_KIT]) {
  const backend = mockBackend({
    kits_list: () => kits,
    add_plan: () =>
      makePlan({
        nodes: [
          makeNode({ projectId: 'AANobbMI', slug: 'sodium', title: 'Sodium', versionId: 'V-1' }),
          makeNode({ projectId: 'gvQqBUqZ', slug: 'lithium', title: 'Lithium', versionId: 'V-2' }),
        ],
      }),
    add_apply: () => ADDED,
  });
  const onAdded = vi.fn();
  renderWithProviders(
    <KitsList packId={nextPackId()} minecraft="1.21.1" loader="fabric" onAdded={onAdded} />,
  );
  return { backend, onAdded };
}

describe('Kits de desempenho', () => {
  it('lista o kit da faixa, esconde os mods até pedir e passa no axe', async () => {
    const { backend } = setup();
    expect(await screen.findByRole('heading', { name: 'Kits de desempenho' })).toBeDefined();
    expect(backend.callsOf('kits_list')[0]?.args).toEqual({
      minecraft: '1.21.1',
      loader: 'fabric',
    });
    expect(await screen.findByText('Desempenho para Fabric 1.21 em diante')).toBeDefined();
    expect(screen.getByText('4 mods marcados')).toBeDefined();
    expect(screen.queryByRole('checkbox', { name: 'Incluir Sodium' })).toBeNull();

    await userEvent.click(screen.getByRole('button', { name: 'Ver o que vem no kit' }));
    const list = screen.getByRole('list', {
      name: 'Mods do kit Desempenho para Fabric 1.21 em diante',
    });
    expect(within(list).getAllByRole('checkbox')).toHaveLength(5);
    expect(within(list).getByText('Muda a geração do mundo. Vem desmarcado.')).toBeDefined();
    expect(await axeComponent(document.body)).toHaveNoViolations();
  });

  it('Adicionar kit abre o diálogo de dependências só com os mods marcados e nada entra antes de confirmar', async () => {
    const user = userEvent.setup();
    const { backend, onAdded } = setup();
    await user.click(await screen.findByRole('button', { name: 'Ver o que vem no kit' }));
    await user.click(screen.getByRole('checkbox', { name: 'Incluir FerriteCore' }));
    await user.click(screen.getByRole('checkbox', { name: 'Incluir ImmediatelyFast' }));
    expect(screen.getByText('2 mods marcados')).toBeDefined();
    expect(backend.callsOf('add_apply')).toHaveLength(0);

    await user.click(screen.getByRole('button', { name: 'Adicionar kit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Adicionar 2 itens' });
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: [
        { source: 'modrinth', projectId: 'AANobbMI' },
        { source: 'modrinth', projectId: 'gvQqBUqZ' },
      ],
    });
    // Ainda não gravou: o diálogo mostra o plano e espera a confirmação.
    expect(backend.callsOf('add_apply')).toHaveLength(0);
    await user.click(await within(dialog).findByRole('button', { name: /^Adicionar 2 itens/ }));
    await waitFor(() => {
      expect(onAdded).toHaveBeenCalledWith(ADDED);
    });
    expect(backend.callsOf('add_apply')).toHaveLength(1);
  });

  it('sem nenhum mod marcado o botão fica desabilitado', async () => {
    const user = userEvent.setup();
    setup();
    await user.click(await screen.findByRole('button', { name: 'Ver o que vem no kit' }));
    for (const name of ['Sodium', 'Lithium', 'FerriteCore', 'ImmediatelyFast']) {
      await user.click(screen.getByRole('checkbox', { name: `Incluir ${name}` }));
    }
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Adicionar kit' }).disabled).toBe(
      true,
    );
  });

  it('faixa sem kit explica em vez de ficar vazia', async () => {
    setup([]);
    expect(
      await screen.findByText('Não há kit de desempenho para Minecraft 1.21.1 com Fabric.'),
    ).toBeDefined();
  });
});
