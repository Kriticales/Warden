/**
 * Diálogo de dependências (SPEC T09): um diálogo para vários itens com a biblioteca comum uma
 * vez (CA-T09-05), obrigatória desmarcada com aviso, incompatibilidades (com o pack e entre os
 * itens), duplicado entre fontes com "Substituir" (CA-T09-04) e a gravação de uma vez.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { AddPlan, AddResult } from '../../../lib/ipc/bindings';
import { axeComponent } from '../../../test/axe';
import { ipcError, mockBackend } from '../../../test/backend';
import { makeAppError, nextPackId } from '../../../test/factories';
import { renderWithProviders } from '../../../test/render';
import { makeNode, makePlan } from '../add.fixtures';
import { DependenciesDialog, type DependenciesRequest } from './DependenciesDialog';

const TWO: DependenciesRequest = {
  choices: [
    { source: 'modrinth', projectId: 'mOgUt4GM', versionId: null },
    { source: 'modrinth', projectId: 'EsAfCjCV', versionId: null },
  ],
  titles: ['Mod Menu', 'AppleSkin'],
};

const ADDED: AddResult = {
  added: [
    { key: 'modrinth:mOgUt4GM', title: 'Mod Menu', path: 'mods/modmenu.pw.toml' },
    { key: 'modrinth:EsAfCjCV', title: 'AppleSkin', path: 'mods/appleskin.pw.toml' },
    {
      key: 'modrinth:eXts2L7r',
      title: 'Text Placeholder API',
      path: 'mods/placeholder-api.pw.toml',
    },
  ],
  skipped: [],
  removed: [],
};

function setup(plan: AddPlan, request: DependenciesRequest = TWO, apply: unknown = ADDED) {
  const backend = mockBackend({
    add_plan: () => plan,
    add_apply: () => (typeof apply === 'function' ? (apply as () => unknown)() : apply),
  });
  const onAdded = vi.fn();
  const onClose = vi.fn();
  renderWithProviders(
    <DependenciesDialog
      packId={nextPackId()}
      request={request}
      target={{ minecraft: '1.21.1', loader: 'fabric' }}
      onClose={onClose}
      onAdded={onAdded}
    />,
  );
  return { backend, onAdded, onClose };
}

describe('diálogo de dependências', () => {
  it('CA-T08-11/CA-T09-05: um diálogo para os itens, a biblioteca comum uma vez; desmarcar avisa; grava de uma vez; axe', async () => {
    const { backend, onAdded } = setup(makePlan());
    const dialog = await screen.findByRole('dialog', {
      name: 'Adicionar 2 escolhidos + 2 dependências',
    });
    expect(
      within(dialog).getByText('Nada foi gravado ainda. Confira o que vai entrar no pack.'),
    ).toBeDefined();
    const required = await within(dialog).findByRole('region', { name: 'Obrigatórias' });
    expect(within(required).getAllByText('Fabric API')).toHaveLength(1);
    expect(within(required).getByText('Pedida por Mod Menu e AppleSkin.')).toBeDefined();
    expect(within(dialog).getByText('Nenhuma incompatibilidade declarada')).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Adicionar 4 itens' })).toBeDefined();
    expect(await axeComponent(dialog)).toHaveNoViolations();

    await userEvent.click(within(required).getByRole('checkbox', { name: /Fabric API/ }));
    expect(
      within(dialog).getByText('Sem esta dependência o jogo provavelmente não abre.'),
    ).toBeDefined();
    expect(
      screen.getByRole('dialog', { name: 'Adicionar 2 escolhidos + 1 dependência' }),
    ).toBeDefined();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Adicionar 3 itens' }));

    await waitFor(() => {
      expect(onAdded).toHaveBeenCalledWith(ADDED);
    });
    const calls = backend.callsOf('add_apply');
    expect(calls).toHaveLength(1);
    expect(calls[0]?.args.request).toEqual({
      items: [
        {
          source: 'modrinth',
          projectId: 'mOgUt4GM',
          versionId: 'V-MODMENU',
          side: 'client',
          replaces: null,
        },
        {
          source: 'modrinth',
          projectId: 'EsAfCjCV',
          versionId: 'V-APPLESKIN',
          side: 'client',
          replaces: null,
        },
        {
          source: 'modrinth',
          projectId: 'eXts2L7r',
          versionId: 'V-PLACEHOLDER',
          side: 'client',
          replaces: null,
        },
      ],
    });
    expect(await screen.findByText('3 itens adicionados ao pack')).toBeDefined();
    expect(screen.getByText('Mod Menu, AppleSkin e Text Placeholder API.')).toBeDefined();
  });

  it('CA-T09-02/CA-T09-04: incompatibilidades, já no pack por outra fonte, sem versão e substituir', async () => {
    const plan = makePlan({
      nodes: [
        makeNode({ projectId: 'sk9rgfiA', slug: 'embeddium', title: 'Embeddium' }),
        makeNode({ projectId: 'mOgUt4GM', title: 'Mod Menu', versionId: 'V-MODMENU' }),
      ],
      installed: [
        {
          key: 'modrinth:P7dR8mSH',
          title: 'Fabric API',
          path: 'mods/fabric-api.pw.toml',
          packSource: 'curseforge',
          chosen: false,
          requiredBy: ['modrinth:mOgUt4GM'],
        },
      ],
      missing: [
        {
          key: 'modrinth:eXts2L7r',
          title: 'Text Placeholder API',
          role: 'required',
          requiredBy: ['modrinth:mOgUt4GM'],
        },
      ],
      conflicts: [
        {
          item: { key: 'modrinth:sk9rgfiA', title: 'Embeddium', path: null },
          other: { key: 'modrinth:AANobbMI', title: 'Sodium', path: 'mods/sodium.pw.toml' },
          withPack: true,
          declaredBy: 'modrinth:AANobbMI',
          reason: null,
        },
        {
          item: { key: 'modrinth:sk9rgfiA', title: 'Embeddium', path: null },
          other: { key: 'modrinth:mOgUt4GM', title: 'Mod Menu', path: null },
          withPack: false,
          declaredBy: 'modrinth:sk9rgfiA',
          reason: 'Os dois mexem no mesmo menu.',
        },
      ],
      duplicates: [
        {
          key: 'modrinth:mOgUt4GM',
          existingPath: 'mods/modmenu.pw.toml',
          existingTitle: 'Mod Menu (CurseForge)',
          existingSource: 'curseforge',
          matchedBy: 'hash',
        },
      ],
    });
    const { backend } = setup(plan, {
      choices: [
        { source: 'modrinth', projectId: 'sk9rgfiA', versionId: null },
        { source: 'modrinth', projectId: 'mOgUt4GM', versionId: null },
      ],
      titles: ['Embeddium', 'Mod Menu'],
    });
    // O Mod Menu já está no pack pela CurseForge e não entra por padrão: só o Embeddium.
    const dialog = await screen.findByRole('dialog', { name: 'Adicionar Embeddium' });
    expect(
      await within(dialog).findByText('Embeddium é incompatível com Sodium, que já está no pack.'),
    ).toBeDefined();
    expect(within(dialog).getByText('Declarado por Sodium.')).toBeDefined();
    expect(
      within(dialog).getByText('Embeddium e Mod Menu são incompatíveis entre si.'),
    ).toBeDefined();
    expect(within(dialog).getByText('Motivo: Os dois mexem no mesmo menu.')).toBeDefined();
    const installed = within(dialog).getByRole('region', { name: 'Já no pack' });
    expect(within(installed).getByRole('listitem').textContent).toBe('Fabric API pela CurseForge');
    expect(
      within(dialog).getByText(
        'Nenhuma versão de Text Placeholder API para Minecraft 1.21.1 com Fabric.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByText('Este mod já está no pack pela CurseForge.')).toBeDefined();
    // Por padrão o duplicado não entra (manter os dois travaria o jogo).
    expect(within(dialog).getByRole('button', { name: 'Adicionar 1 item' })).toBeDefined();
    await userEvent.click(
      within(dialog).getByRole('radio', { name: 'Substituir pela versão do Modrinth' }),
    );
    await userEvent.click(within(dialog).getByRole('button', { name: 'Adicionar 2 itens' }));
    await waitFor(() => {
      expect(backend.callsOf('add_apply')).toHaveLength(1);
    });
    expect(backend.callsOf('add_apply')[0]?.args.request).toEqual({
      items: [
        {
          source: 'modrinth',
          projectId: 'sk9rgfiA',
          versionId: 'V-MODMENU',
          side: 'client',
          replaces: null,
        },
        {
          source: 'modrinth',
          projectId: 'mOgUt4GM',
          versionId: 'V-MODMENU',
          side: 'client',
          replaces: 'mods/modmenu.pw.toml',
        },
      ],
    });
  });

  it('B1-B: o título e o botão fazem a mesma conta, e "Pedida por" só cita quem entra', async () => {
    const plan = makePlan({
      nodes: [
        ...makePlan().nodes,
        makeNode({
          projectId: 'nU0bVIaL',
          slug: 'badpackets',
          title: 'Bad Packets',
          role: 'optional',
          versionId: 'V-BADPACKETS',
          optionalFor: ['modrinth:EsAfCjCV'],
        }),
      ],
    });
    setup(plan);
    const dialog = await screen.findByRole('dialog', {
      name: 'Adicionar 2 escolhidos + 2 dependências',
    });
    expect(within(dialog).getByRole('button', { name: 'Adicionar 4 itens' })).toBeDefined();
    const optional = within(dialog).getByRole('region', { name: 'Opcionais' });
    await userEvent.click(within(optional).getByRole('checkbox', { name: /Bad Packets/ }));
    expect(
      screen.getByRole('dialog', { name: 'Adicionar 2 escolhidos + 3 dependências' }),
    ).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Adicionar 5 itens' })).toBeDefined();
    await userEvent.click(within(optional).getByRole('checkbox', { name: /Bad Packets/ }));

    // Desmarcar o AppleSkin: a Fabric API só é pedida pelo Mod Menu, e a opcional dele some.
    const chosen = within(dialog).getByRole('region', { name: 'O que você escolheu' });
    await userEvent.click(within(chosen).getByRole('checkbox', { name: /AppleSkin/ }));
    const required = within(dialog).getByRole('region', { name: 'Obrigatórias' });
    expect(within(required).getAllByText('Pedida por Mod Menu.')).toHaveLength(2);
    expect(within(required).queryByText(/AppleSkin/)).toBeNull();
    expect(within(optional).queryByText(/Opcional para/)).toBeNull();
    expect(
      screen.getByRole('dialog', { name: 'Adicionar Mod Menu + 2 dependências' }),
    ).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Adicionar 3 itens' })).toBeDefined();

    // Sem nenhum escolhido, nenhuma obrigatória diz quem a pede.
    await userEvent.click(within(chosen).getByRole('checkbox', { name: /Mod Menu/ }));
    expect(within(required).queryByText(/Pedida por/)).toBeNull();
  });

  it('falha ao gravar: o erro aparece no diálogo e nada é dado como adicionado', async () => {
    const { onAdded } = setup(makePlan(), TWO, () =>
      ipcError(
        makeAppError(
          { domain: 'project', code: 'SEARCH_SOURCE_UNAVAILABLE' },
          {
            params: { source: 'Modrinth' },
          },
        ),
      ),
    );
    const dialog = await screen.findByRole('dialog', {
      name: 'Adicionar 2 escolhidos + 2 dependências',
    });
    await userEvent.click(await within(dialog).findByRole('button', { name: 'Adicionar 4 itens' }));
    expect(
      await within(dialog).findByText('Não foi possível adicionar. Nada foi gravado.'),
    ).toBeDefined();
    expect(
      within(dialog).getByText(
        'Não foi possível falar com o Modrinth agora. Confira a internet e tente de novo.',
      ),
    ).toBeDefined();
    expect(onAdded).not.toHaveBeenCalled();
  });
});
