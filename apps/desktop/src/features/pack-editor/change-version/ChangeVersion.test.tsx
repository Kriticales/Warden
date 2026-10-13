/**
 * "Trocar de versão", a versão do loader e o diálogo de remover com a cadeia e as dependências
 * sem uso (P1-15), no app inteiro com o backend simulado.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type {
  ChangeVersionOptions,
  ChangeVersionPlan,
  InventoryItem,
  LoaderCheck,
  RemovalPlan,
  VersionChoice,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { renderApp } from '../../../test/render';
import { AE2, makeDetails, makeInventory, makeItem, SODIUM } from '../editor.fixtures';
import { editorBackend } from '../testing';

const MODMENU = makeItem({
  key: 'modrinth:mOgUt4GM',
  name: 'Mod Menu',
  path: 'mods/modmenu.pw.toml',
  source: 'modrinth',
  projectId: 'mOgUt4GM',
  sourceVersionId: 'NEW',
});

const LOCAL = makeItem({
  key: 'path:mods/solto.jar',
  name: 'Solto',
  path: 'mods/solto.jar',
  source: 'local',
  projectId: null,
  sourceVersionId: null,
  sideEditable: false,
});

function version(overrides: Partial<VersionChoice>): VersionChoice {
  return {
    id: 'NEW',
    number: '11.0.5',
    channel: 'release',
    published: '2026-09-24T19:00:03Z',
    fileName: 'modmenu-11.0.5.jar',
    installed: false,
    older: false,
    ...overrides,
  };
}

const OPTIONS: ChangeVersionOptions = {
  path: MODMENU.path,
  name: 'Mod Menu',
  source: 'modrinth',
  currentId: 'NEW',
  current: '11.0.5',
  currentIncompatible: false,
  pinned: true,
  versions: [
    version({ installed: true }),
    version({ id: 'OLD', number: '11.0.4', published: '2026-03-25T00:45:03Z', older: true }),
    version({
      id: 'BETA',
      number: '11.0.3-beta',
      channel: 'beta',
      published: '2024-10-14T04:11:09Z',
      older: true,
    }),
  ],
};

function planFor(versionId: string): ChangeVersionPlan {
  const to = OPTIONS.versions.find((v) => v.id === versionId) ?? version({});
  return {
    path: MODMENU.path,
    name: 'Mod Menu',
    current: '11.0.5',
    to,
    same: versionId === 'NEW',
    downgrade: to.older,
    newDependencies:
      versionId === 'OLD'
        ? [
            {
              projectId: 'eXts2L7r',
              name: 'Text Placeholder API',
              source: 'modrinth',
              neededBy: ['Mod Menu'],
            },
          ]
        : [],
    conflicts: versionId === 'OLD' ? [{ item: 'Mod Menu', with: 'Outro Menu' }] : [],
  };
}

async function openDetails(item: InventoryItem, handlers: Parameters<typeof editorBackend>[0]) {
  const setup = editorBackend({
    ...handlers,
    inventory: makeInventory([item, AE2]),
    handlers: { item_details: () => makeDetails(item), ...handlers?.handlers },
  });
  renderApp(setup.url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: item.name }));
  const drawer = await screen.findByRole('dialog', { name: 'Detalhes do mod' });
  return { ...setup, drawer, user };
}

describe('Trocar de versão (T07)', () => {
  it('lista as versões, avisa o rebaixamento, as dependências e a incompatibilidade, e grava só ao confirmar', async () => {
    const { backend, drawer, user } = await openDetails(MODMENU, {
      handlers: {
        item_version_options: () => OPTIONS,
        item_version_plan: (args) => planFor(String(args.versionId)),
        item_version_change: () => ({
          path: MODMENU.path,
          name: 'Mod Menu',
          from: '11.0.5',
          to: '11.0.4',
          toId: 'OLD',
          downgrade: true,
        }),
      },
    });
    const opener = within(drawer).getByRole('button', { name: 'Trocar de versão…' });
    await user.click(opener);
    const dialog = await screen.findByRole('dialog', { name: 'Trocar a versão de Mod Menu' });
    const select = await within(dialog).findByRole<HTMLSelectElement>('combobox', {
      name: 'Versão',
    });
    expect([...select.options].map((option) => option.textContent)).toEqual([
      '11.0.5 · 24/09/2026 · instalada',
      '11.0.4 · 25/03/2026 · mais antiga',
      '11.0.3-beta · 14/10/2024 · mais antiga, beta',
    ]);
    expect(within(dialog).getByText('Instalada agora: 11.0.5')).toBeDefined();
    expect(
      within(dialog).getByText('O item está fixado: continua fixado na versão que você escolher.'),
    ).toBeDefined();
    // A instalada vem escolhida: nada a trocar.
    expect(
      await within(dialog).findByText('É a versão instalada. Escolha outra para trocar.'),
    ).toBeDefined();
    const confirm = within(dialog).getByRole('button', { name: 'Trocar para 11.0.5' });
    expect(confirm.hasAttribute('disabled')).toBe(true);

    await user.selectOptions(select, 'OLD');
    expect(await within(dialog).findByText('Versão mais antiga que a instalada')).toBeDefined();
    expect(
      within(dialog).getByText(
        /Você vai voltar de 11.0.5 para 11.0.4\. Versões antigas podem não ter correções/,
      ),
    ).toBeDefined();
    expect(
      within(dialog).getByText('Esta versão precisa de um mod que não está no pack'),
    ).toBeDefined();
    expect(
      within(dialog).getByText(
        'Text Placeholder API. Adicione pela página Adicionar depois de trocar.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByText('Outro Menu.')).toBeDefined();
    expect(backend.callsOf('item_version_change')).toHaveLength(0);
    expect(await axePage(document.body)).toHaveNoViolations();

    const inventoryBefore = backend.callsOf('inventory_list').length;
    const rowBefore = backend.callsOf('pack_get').length;
    await user.click(within(dialog).getByRole('button', { name: 'Voltar para 11.0.4' }));
    await waitFor(() => {
      expect(backend.callsOf('item_version_change')).toHaveLength(1);
    });
    expect(backend.callsOf('item_version_change')[0]?.args).toMatchObject({
      path: MODMENU.path,
      expectedCurrentId: 'NEW',
      versionId: 'OLD',
    });
    expect(await screen.findByText('Mod Menu agora está na versão 11.0.4.')).toBeDefined();
    // Dados na tela atuais: a lista e a linha do pack (cabeçalho, alterações) são relidas.
    await waitFor(() => {
      expect(backend.callsOf('inventory_list').length).toBeGreaterThan(inventoryBefore);
      expect(backend.callsOf('pack_get').length).toBeGreaterThan(rowBefore);
    });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Trocar a versão de Mod Menu' })).toBeNull();
    });
    // O foco volta para quem abriu o diálogo.
    await waitFor(() => {
      expect(document.activeElement).toBe(
        within(screen.getByRole('dialog', { name: 'Detalhes do mod' })).getByRole('button', {
          name: 'Trocar de versão…',
        }),
      );
    });
  });

  it('versão nova sem avisos troca com "Trocar para"', async () => {
    const options: ChangeVersionOptions = {
      ...OPTIONS,
      pinned: false,
      currentId: 'OLD',
      current: '11.0.4',
      versions: [version({}), version({ id: 'OLD', number: '11.0.4', installed: true })],
    };
    const { backend, drawer, user } = await openDetails(MODMENU, {
      handlers: {
        item_version_options: () => options,
        item_version_plan: (args) => ({
          ...planFor(String(args.versionId)),
          same: args.versionId === 'OLD',
          downgrade: false,
          to: options.versions.find((v) => v.id === args.versionId),
        }),
        item_version_change: () => ({
          path: MODMENU.path,
          name: 'Mod Menu',
          from: '11.0.4',
          to: '11.0.5',
          toId: 'NEW',
          downgrade: false,
        }),
      },
    });
    await user.click(within(drawer).getByRole('button', { name: 'Trocar de versão…' }));
    const dialog = await screen.findByRole('dialog', { name: 'Trocar a versão de Mod Menu' });
    await user.selectOptions(
      await within(dialog).findByRole('combobox', { name: 'Versão' }),
      'NEW',
    );
    const confirm = await within(dialog).findByRole('button', { name: 'Trocar para 11.0.5' });
    await waitFor(() => {
      expect(confirm.hasAttribute('disabled')).toBe(false);
    });
    expect(within(dialog).queryByText('Versão mais antiga que a instalada')).toBeNull();
    await user.click(confirm);
    await waitFor(() => {
      expect(backend.callsOf('item_version_change')[0]?.args).toMatchObject({
        expectedCurrentId: 'OLD',
        versionId: 'NEW',
      });
    });
  });

  it('a versão instalada que não serve mais é avisada; sem versões compatíveis, diz isso', async () => {
    const { drawer, user } = await openDetails(MODMENU, {
      handlers: {
        item_version_options: () => ({ ...OPTIONS, currentIncompatible: true, versions: [] }),
      },
    });
    await user.click(within(drawer).getByRole('button', { name: 'Trocar de versão…' }));
    const dialog = await screen.findByRole('dialog', { name: 'Trocar a versão de Mod Menu' });
    expect(
      await within(dialog).findByText(/A versão instalada não serve para este pack/),
    ).toBeDefined();
    expect(within(dialog).getByText('Nenhuma versão deste item serve para o pack.')).toBeDefined();
  });

  it('arquivo local não oferece trocar de versão', async () => {
    const { drawer } = await openDetails(LOCAL, {});
    expect(within(drawer).queryByRole('button', { name: 'Trocar de versão…' })).toBeNull();
  });
});

const CHECK: LoaderCheck = {
  loader: 'forge',
  from: '47.3.0',
  to: '47.1.3',
  breaks: [{ path: 'mods/novo.pw.toml', name: 'Mod Novo', requires: '[47.2,)' }],
  stillBroken: [],
  fixes: [{ path: 'mods/velho.pw.toml', name: 'Mod Velho', requires: '[47.1,47.2)' }],
  checked: 2,
  unchecked: ['Sem Arquivo'],
};

describe('Trocar a versão do loader (T11)', () => {
  it('mostra antes o que para de funcionar e grava só ao confirmar, com a versão mostrada', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        catalog_loader_versions: () => ({
          loader: 'forge',
          minecraft: '1.20.1',
          versions: [
            { version: '47.3.0', maven: 'f', stable: true, recommended: false },
            { version: '47.2.0', maven: 'f', stable: true, recommended: true },
            { version: '47.1.3', maven: 'f', stable: true, recommended: false },
          ],
          preselected: '47.2.0',
          fetchedAt: 0,
          fromCacheOffline: false,
        }),
        loader_version_check: () => CHECK,
        loader_version_set: () => ({
          changed: { loader: 'forge', from: '47.3.0', to: '47.1.3', changed: true },
          check: CHECK,
        }),
      },
    });
    renderApp(url);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole('button', { name: 'Editar informações' }, { timeout: 10_000 }),
    );
    const info = await screen.findByRole('dialog', { name: 'Informações do pack' });
    await user.click(await within(info).findByRole('button', { name: 'Trocar versão do loader…' }));
    const dialog = await screen.findByRole('dialog', { name: 'Trocar a versão do Forge' });
    const select = await within(dialog).findByRole<HTMLSelectElement>('combobox', {
      name: 'Versão do Forge',
    });
    expect([...select.options].map((option) => option.textContent)).toEqual([
      '47.3.0 (atual)',
      '47.2.0 (recomendada)',
      '47.1.3',
    ]);
    expect(
      within(dialog).getByText('É a versão atual do pack. Escolha outra para trocar.'),
    ).toBeDefined();
    expect(backend.callsOf('loader_version_check')).toHaveLength(0);

    await user.selectOptions(select, '47.1.3');
    expect(
      await within(dialog).findByText('1 mod para de funcionar com a versão 47.1.3'),
    ).toBeDefined();
    expect(within(dialog).getByText('Mod Novo: pede [47.2,)')).toBeDefined();
    expect(within(dialog).getByText('1 mod volta a funcionar')).toBeDefined();
    expect(
      within(dialog).getByText(/1 mod ainda não foi baixado e não pôde ser conferido: Sem Arquivo/),
    ).toBeDefined();
    expect(backend.callsOf('loader_version_check')[0]?.args).toMatchObject({ version: '47.1.3' });
    expect(backend.callsOf('loader_version_set')).toHaveLength(0);
    expect(await axePage(document.body)).toHaveNoViolations();

    const metaBefore = backend.callsOf('pack_meta_get').length;
    const rowBefore = backend.callsOf('pack_get').length;
    await user.click(within(dialog).getByRole('button', { name: 'Trocar para 47.1.3' }));
    await waitFor(() => {
      expect(backend.callsOf('loader_version_set')).toHaveLength(1);
    });
    expect(backend.callsOf('loader_version_set')[0]?.args).toMatchObject({
      expectedFrom: '47.3.0',
      version: '47.1.3',
    });
    expect(
      await screen.findByText('Versão do Forge trocada para 47.1.3. 1 mod não funciona com ela.'),
    ).toBeDefined();
    // Informações do pack e cabeçalho (linha do pack) relidos.
    await waitFor(() => {
      expect(backend.callsOf('pack_meta_get').length).toBeGreaterThan(metaBefore);
      expect(backend.callsOf('pack_get').length).toBeGreaterThan(rowBefore);
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(
        within(screen.getByRole('dialog', { name: 'Informações do pack' })).getByRole('button', {
          name: 'Trocar versão do loader…',
        }),
      );
    });
  });

  it('pack vanilla não oferece trocar a versão do loader', async () => {
    const { url } = editorBackend({ pack: { loader: null, loaderVersion: null } });
    renderApp(url);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole('button', { name: 'Editar informações' }, { timeout: 10_000 }),
    );
    const info = await screen.findByRole('dialog', { name: 'Informações do pack' });
    await within(info).findByText('Versões');
    expect(within(info).queryByRole('button', { name: 'Trocar versão do loader…' })).toBeNull();
  });
});

const CHAIN_PLAN: RemovalPlan = {
  targets: [{ key: SODIUM.key, path: SODIUM.path, name: 'Sodium', files: [SODIUM.path] }],
  dependents: [
    { key: 'modrinth:b', path: 'mods/b.pw.toml', name: 'Indium', needs: ['Sodium'], depth: 1 },
    {
      key: 'modrinth:a',
      path: 'mods/a.pw.toml',
      name: 'Continuity',
      needs: ['Indium'],
      depth: 2,
    },
  ],
  orphans: [
    { key: 'modrinth:k', path: 'mods/kotlin.pw.toml', name: 'Kotlin', optionalUsers: [] },
    {
      key: 'modrinth:c',
      path: 'mods/cloth.pw.toml',
      name: 'Cloth Config',
      optionalUsers: ['Mod Menu'],
    },
  ],
};

describe('Remover com a cadeia e as dependências sem uso (CA-T06-06)', () => {
  async function openRemove(plan: RemovalPlan) {
    const setup = editorBackend({
      handlers: {
        items_remove_plan: () => plan,
        items_remove: (args) => args.paths,
      },
    });
    renderApp(setup.url);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    await screen.findByRole('searchbox', { name: 'Buscar no pack' });
    const user = userEvent.setup();
    await user.click(await screen.findByRole('checkbox', { name: 'Selecionar Sodium' }));
    await user.click(screen.getByRole('button', { name: 'Remover' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Remover Sodium?' });
    return { ...setup, user, dialog };
  }

  it('lista a cadeia inteira e oferece as sem uso desmarcadas; marcada, sai junto', async () => {
    const { backend, user, dialog } = await openRemove(CHAIN_PLAN);
    expect(
      await within(dialog).findByText(
        'Estes mods dependem dele: Indium, Continuity. Sem ele, podem não funcionar.',
      ),
    ).toBeDefined();
    const chain = within(dialog).getByTestId('cadeia');
    expect([...chain.querySelectorAll('li')].map((li) => li.textContent)).toEqual([
      'Indium: precisa de Sodium',
      'Continuity: precisa de Indium (por consequência)',
    ]);
    expect(within(dialog).getByText('2 dependências ficam sem uso')).toBeDefined();
    expect(within(dialog).getByText('Usada só como opcional por Mod Menu')).toBeDefined();
    const kotlin = within(dialog).getByRole<HTMLInputElement>('checkbox', { name: 'Kotlin' });
    expect(kotlin.checked).toBe(false);
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.click(kotlin);
    await user.click(within(dialog).getByRole('button', { name: 'Remover 2 itens' }));
    await waitFor(() => {
      expect(backend.callsOf('items_remove')).toHaveLength(1);
    });
    expect(backend.callsOf('items_remove')[0]?.args).toMatchObject({
      paths: [SODIUM.path, 'mods/kotlin.pw.toml'],
    });
  });

  it('sem marcar nenhuma, remove só o item', async () => {
    const { backend, user, dialog } = await openRemove(CHAIN_PLAN);
    await within(dialog).findByText('2 dependências ficam sem uso');
    await user.click(within(dialog).getByRole('button', { name: 'Remover Sodium' }));
    await waitFor(() => {
      expect(backend.callsOf('items_remove')).toHaveLength(1);
    });
    expect(backend.callsOf('items_remove')[0]?.args).toMatchObject({ paths: [SODIUM.path] });
  });
});
