/**
 * Navegar por modpacks (SPEC T08, CA-T08-14, lado da interface) no app inteiro, com o backend
 * simulado: Tipo = Modpacks busca modpacks (sem caixa de seleção e sem o início), abrir um deles
 * mostra os mods com os quatro estados, o filtro "Só os que não estão no seu pack", o seletor
 * **Versão**, "Adicionar 5 selecionados" abre um único diálogo com as versões escolhidas, e
 * depois de gravar a lista e as contagens são relidas.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AddResult, Inventory, ModpackContents, ModpackMod } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { type Handler, ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { makeInventory, makeItem } from '../../pack-editor/editor.fixtures';
import { sendOnChannel } from '../../test/test.fixtures';
import { editorBackend } from '../../pack-editor/testing';
import { makeNode, makePage, makePlan, makeResult, ref } from '../add.fixtures';
import { choicesFor, countByState, modState, visibleMods } from './model';

const PACK = { minecraft: '1.21.1', loader: 'neoforge', loaderVersion: '21.1.77' };

const CREATE_PLUS = makeResult({
  title: 'Create+',
  author: 'Sshmoob',
  summary: 'Um pack Vanilla+ com o Create.',
  downloads: 1_419_817,
  sources: [ref('t1tOiUHZ', { slug: 'create_plus' })],
});
const COZY = makeResult({
  title: 'Cozy Create',
  author: 'harborlight',
  sources: [ref('900100', { source: 'curseforge', slug: 'cozy-create' })],
});

function mod(title: string, state: ModpackMod['state'], overrides: Partial<ModpackMod> = {}) {
  const projectId = title.replace(/[^A-Za-z]/g, '').slice(0, 8);
  const external = state === 'external';
  return {
    key: external ? `arquivo:${title}.jar` : `modrinth:${projectId}`,
    source: external ? null : ('modrinth' as const),
    projectId: external ? null : projectId,
    versionId: external ? null : `V-${projectId}`,
    title,
    summary: `${title}: resumo.`,
    iconUrl: null,
    fileName: external ? `${title}.jar` : null,
    state,
    packPath: state === 'inPack' ? `mods/${projectId}.pw.toml` : null,
    ...overrides,
  } satisfies ModpackMod;
}

const COMPATIBLE = ['Aileron', 'AppleSkin', 'Architectury API', 'Comforts', 'Handcrafted', 'Jade'];

function makeContents(overrides: Partial<ModpackContents> = {}): ModpackContents {
  return {
    source: 'modrinth',
    projectId: 't1tOiUHZ',
    title: 'Create+',
    versionNumber: '6.0.0-alpha-f',
    gameVersions: ['1.21.1'],
    loaders: ['neoforge'],
    sameTarget: true,
    license: { name: 'LGPL-3.0-only', free: true },
    pageUrl: 'https://modrinth.com/modpack/create_plus',
    mods: [
      ...COMPATIBLE.map((title) => mod(title, 'compatible')),
      mod('Create', 'inPack'),
      mod('Smart Block Placement', 'noVersion'),
      mod('enchancement-1.21-r14', 'external'),
    ],
    wholeDownload: false,
    ...overrides,
  };
}

function setup(handlers: Record<string, Handler> = {}, inventory?: () => Inventory) {
  return editorBackend({
    pack: PACK,
    inventory: makeInventory([]),
    handlers: {
      search_projects: () => makePage([CREATE_PLUS, COZY]),
      discover_categories: () => [],
      modpack_contents: () => makeContents(),
      ...(inventory ? { inventory_list: inventory } : {}),
      ...handlers,
    },
  });
}

async function openModpacks(url: string) {
  renderApp(`${url}/adicionar?tipo=modpack`);
  await screen.findByRole('heading', { level: 1, name: 'Adicionar ao pack' }, { timeout: 10_000 });
  return screen.findByRole('list', { name: 'Resultados' });
}

async function openCreatePlus(url: string) {
  const list = await openModpacks(url);
  await userEvent.click(within(list).getByRole('button', { name: 'Ver os mods de Create+' }));
  return screen.findByRole('complementary', { name: 'Mods do modpack Create+' });
}

function rowOf(panel: HTMLElement, name: string): HTMLElement {
  const list = within(panel).getByRole('list', { name: 'Mods do modpack' });
  const row = within(list)
    .getAllByRole('listitem')
    .find((item) => item.querySelector('.modpack__name')?.textContent === name);
  if (!row) throw new Error(`linha ${name} não encontrada`);
  return row;
}

describe('Tipo = Modpacks', () => {
  it('busca modpacks sem o início e sem caixa de seleção; os filtros continuam; axe', async () => {
    const { url, backend } = setup();
    const list = await openModpacks(url);
    expect(screen.getByRole<HTMLSelectElement>('combobox', { name: 'Tipo' }).value).toBe('modpack');
    expect(backend.callsOf('discover_home')).toHaveLength(0);
    expect(backend.callsOf('search_projects')[0]?.args.request).toMatchObject({
      kind: 'modpack',
      query: '',
    });
    expect(within(list).queryAllByRole('checkbox')).toHaveLength(0);
    expect(within(list).getByRole('button', { name: 'Ver os mods de Cozy Create' })).toBeDefined();
    expect(screen.getByRole('complementary', { name: 'Filtros' })).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T08-14: abrir mostra o cabeçalho, a licença e os quatro estados; axe', async () => {
    const { url, backend } = setup();
    const panel = await openCreatePlus(url);
    expect(await within(panel).findByText('Mods deste modpack (9)')).toBeDefined();
    expect(backend.callsOf('modpack_contents')[0]?.args).toMatchObject({
      source: 'modrinth',
      projectId: 't1tOiUHZ',
    });
    expect(
      within(panel).getByText(
        'por Sshmoob · versão 6.0.0-alpha-f · 1,4 mi downloads · NeoForge 1.21.1',
      ),
    ).toBeDefined();
    expect(within(panel).getByText('Licença do modpack: LGPL-3.0-only.')).toBeDefined();
    expect(
      within(panel).getByText(
        '1 já no seu pack · 6 compatíveis · 1 sem versão para o seu pack · 1 fora das lojas',
      ),
    ).toBeDefined();
    // Já no seu pack: sem caixa.
    const create = rowOf(panel, 'Create');
    expect(within(create).queryByRole('checkbox')).toBeNull();
    expect(within(create).getByText('Já no pack')).toBeDefined();
    // Compatível: com caixa.
    expect(within(rowOf(panel, 'Jade')).getByRole<HTMLInputElement>('checkbox').disabled).toBe(
      false,
    );
    // Sem versão: desabilitado, com o motivo.
    const smart = rowOf(panel, 'Smart Block Placement');
    expect(within(smart).getByRole<HTMLInputElement>('checkbox').disabled).toBe(true);
    expect(within(smart).getByText('Sem versão para Minecraft 1.21.1 com NeoForge')).toBeDefined();
    // Fora das lojas: desabilitado, com o motivo.
    const external = rowOf(panel, 'enchancement-1.21-r14');
    expect(within(external).getByRole<HTMLInputElement>('checkbox').disabled).toBe(true);
    expect(
      within(external).getByText(
        'Arquivo fora das lojas: o Warden não copia jars de pacotes de terceiros',
      ),
    ).toBeDefined();
    // Com o modpack aberto, a lista de mods ocupa o lugar dos filtros.
    expect(screen.queryByRole('complementary', { name: 'Filtros' })).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T08-14: "Adicionar 5 selecionados" abre um único diálogo com as versões do modpack', async () => {
    const { url, backend } = setup({ add_plan: () => makePlan({ nodes: [makeNode()] }) });
    const panel = await openCreatePlus(url);
    await within(panel).findByText('Mods deste modpack (9)');
    const versao = within(panel).getByRole<HTMLSelectElement>('combobox', {
      name: 'Versão dos mods',
    });
    expect(versao.value).toBe('same');
    for (const name of COMPATIBLE.slice(0, 5)) {
      await userEvent.click(
        within(rowOf(panel, name)).getByRole('checkbox', { name: `Selecionar ${name}` }),
      );
    }
    const bar = within(panel).getByRole('region', { name: 'Mods selecionados' });
    expect(within(bar).getByText('5 selecionados')).toBeDefined();
    await userEvent.click(within(bar).getByRole('button', { name: 'Adicionar 5 selecionados' }));
    expect(await screen.findByRole('dialog', { name: 'Adicionar 5 itens' })).toBeDefined();
    expect(screen.getAllByRole('dialog')).toHaveLength(1);
    await waitFor(() => {
      expect(backend.callsOf('add_plan')).toHaveLength(1);
    });
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: COMPATIBLE.slice(0, 5).map((title) => {
        const id = title.replace(/[^A-Za-z]/g, '').slice(0, 8);
        return { source: 'modrinth', projectId: id, versionId: `V-${id}` };
      }),
    });
  });

  it('"a mais nova compatível" manda os itens sem versão; o filtro esconde os do pack', async () => {
    const { url, backend } = setup({ add_plan: () => makePlan({ nodes: [makeNode()] }) });
    const panel = await openCreatePlus(url);
    await within(panel).findByText('Mods deste modpack (9)');
    await userEvent.selectOptions(
      within(panel).getByRole('combobox', { name: 'Versão dos mods' }),
      'newest',
    );
    await userEvent.click(
      within(panel).getByRole('checkbox', { name: 'Só os que não estão no seu pack' }),
    );
    expect(within(panel).queryByText('Já no pack')).toBeNull();
    await userEvent.click(within(panel).getByRole('checkbox', { name: 'Selecionar Jade' }));
    await userEvent.click(within(panel).getByRole('button', { name: 'Adicionar 1 selecionado' }));
    await waitFor(() => {
      expect(backend.callsOf('add_plan')).toHaveLength(1);
    });
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: [{ source: 'modrinth', projectId: 'Jade', versionId: null }],
    });
  });

  it('modpack de outro loader: "a mesma do modpack" fica desabilitada, com o motivo', async () => {
    const { url } = setup({
      modpack_contents: () => makeContents({ sameTarget: false, loaders: ['fabric'] }),
    });
    const panel = await openCreatePlus(url);
    await within(panel).findByText('Mods deste modpack (9)');
    const versao = within(panel).getByRole<HTMLSelectElement>('combobox', {
      name: 'Versão dos mods',
    });
    expect(versao.value).toBe('newest');
    expect(
      within(versao).getByRole<HTMLOptionElement>('option', {
        name: 'Versão: a mesma do modpack (testada junto)',
      }).disabled,
    ).toBe(true);
    expect(
      within(panel).getByText(
        'O modpack é de Fabric 1.21.1: a versão de cada mod será a mais nova compatível com o seu pack.',
      ),
    ).toBeDefined();
  });

  it('licença não informada e download inteiro avisam', async () => {
    const { url } = setup({
      modpack_contents: () =>
        makeContents({ license: { name: null, free: false }, wholeDownload: true }),
    });
    const panel = await openCreatePlus(url);
    expect(
      await within(panel).findByText('A fonte não informa a licença deste modpack.'),
    ).toBeDefined();
    expect(
      within(panel).getByText(
        'O servidor do arquivo não deixou ler só a lista; o modpack foi baixado inteiro para a memória. Nada foi gravado no computador.',
      ),
    ).toBeDefined();
  });

  it('modpack bloqueado a terceiros: a mensagem e "Abrir a página do modpack"', async () => {
    const { url } = setup({
      modpack_contents: () =>
        ipcError(
          makeAppError(
            { domain: 'discovery', code: 'MODPACK_BLOCKED' },
            { params: { page: 'https://www.curseforge.com/minecraft/modpacks/cozy-create' } },
          ),
        ),
    });
    const list = await openModpacks(url);
    await userEvent.click(within(list).getByRole('button', { name: 'Ver os mods de Cozy Create' }));
    const panel = await screen.findByRole('complementary', { name: 'Mods do modpack Cozy Create' });
    expect(
      await within(panel).findByText(
        'Este modpack não pode ser aberto por apps de terceiros. Abra a página na CurseForge.',
      ),
    ).toBeDefined();
    expect(within(panel).getByRole('button', { name: 'Abrir a página do modpack' })).toBeDefined();
    expect(within(panel).queryByRole('button', { name: 'Tentar de novo' })).toBeNull();
  });

  it('depois de adicionar, a lista do modpack, o inventário e o pack são relidos', async () => {
    let items = makeInventory([]);
    const added: AddResult = {
      added: [{ key: 'modrinth:Jade', title: 'Jade', path: 'mods/jade.pw.toml' }],
      skipped: [],
      removed: [],
    };
    const { url, backend } = setup(
      {
        add_plan: () =>
          makePlan({
            nodes: [makeNode({ key: 'modrinth:Jade', projectId: 'Jade', title: 'Jade' })],
          }),
        add_apply: () => {
          items = makeInventory([
            makeItem({ key: 'modrinth:Jade', name: 'Jade', path: 'mods/jade.pw.toml' }),
          ]);
          return added;
        },
      },
      () => items,
    );
    const panel = await openCreatePlus(url);
    await within(panel).findByText('Mods deste modpack (9)');
    const contentsBefore = backend.callsOf('modpack_contents').length;
    const packBefore = backend.callsOf('pack_get').length;
    await userEvent.click(within(panel).getByRole('checkbox', { name: 'Selecionar Jade' }));
    await userEvent.click(within(panel).getByRole('button', { name: 'Adicionar 1 selecionado' }));
    const dialog = await screen.findByRole('dialog');
    await userEvent.click(await within(dialog).findByRole('button', { name: /^Adicionar/ }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    // A lista do modpack e a linha do pack foram relidas, e o Jade já aparece no pack.
    await waitFor(() => {
      expect(backend.callsOf('modpack_contents').length).toBeGreaterThan(contentsBefore);
    });
    expect(backend.callsOf('pack_get').length).toBeGreaterThan(packBefore);
    await waitFor(() => {
      expect(within(rowOf(panel, 'Jade')).getByText('Já no pack')).toBeDefined();
    });
    expect(
      within(panel).getByText(
        '2 já no seu pack · 5 compatíveis · 1 sem versão para o seu pack · 1 fora das lojas',
      ),
    ).toBeDefined();
    expect(within(panel).queryByRole('region', { name: 'Mods selecionados' })).toBeNull();
  });

  it('trocar de modpack ou fechar o painel cancela a leitura no backend', async () => {
    const channels: unknown[] = [];
    const { url, backend } = setup({
      // A leitura nunca termina sozinha: só o cancelamento a para.
      modpack_contents: (args) => {
        channels.push(args.onEvent);
        return new Promise(() => undefined);
      },
      operation_cancel: () => null,
    });
    const cancelled = () =>
      backend.callsOf('operation_cancel').map((call) => call.args.operationId);
    const list = await openModpacks(url);
    await userEvent.click(within(list).getByRole('button', { name: 'Ver os mods de Create+' }));
    const first = await screen.findByRole('complementary', { name: 'Mods do modpack Create+' });
    expect(within(first).getByText('Lendo a lista de mods do modpack…')).toBeDefined();
    await waitFor(() => {
      expect(channels).toHaveLength(1);
    });
    act(() => {
      sendOnChannel(channels[0], [
        { type: 'started', operationId: '01J9ZQ00000000000000000001', kind: 'add.modpackContents' },
      ]);
    });
    expect(cancelled()).toEqual([]);

    // Trocar de modpack cancela a leitura do anterior.
    await userEvent.click(within(list).getByRole('button', { name: 'Ver os mods de Cozy Create' }));
    const second = await screen.findByRole('complementary', {
      name: 'Mods do modpack Cozy Create',
    });
    await waitFor(() => {
      expect(cancelled()).toEqual(['01J9ZQ00000000000000000001']);
    });
    await waitFor(() => {
      expect(channels).toHaveLength(2);
    });

    // Fechar antes de o ID chegar: cancela assim que ele chega.
    await userEvent.click(within(second).getByRole('button', { name: 'Fechar o modpack' }));
    expect(cancelled()).toHaveLength(1);
    act(() => {
      sendOnChannel(channels[1], [
        { type: 'started', operationId: '01J9ZQ00000000000000000002', kind: 'add.modpackContents' },
      ]);
    });
    await waitFor(() => {
      expect(cancelled()).toEqual(['01J9ZQ00000000000000000001', '01J9ZQ00000000000000000002']);
    });
  });

  it('"Fechar o modpack" volta os filtros e devolve o foco a quem abriu', async () => {
    const { url } = setup();
    const list = await openModpacks(url);
    const opener = within(list).getByRole('button', { name: 'Ver os mods de Create+' });
    await userEvent.click(opener);
    const panel = await screen.findByRole('complementary', { name: 'Mods do modpack Create+' });
    expect(document.activeElement).toBe(within(panel).getByRole('heading', { name: 'Create+' }));
    await userEvent.click(within(panel).getByRole('button', { name: 'Fechar o modpack' }));
    expect(screen.queryByRole('complementary', { name: 'Mods do modpack Create+' })).toBeNull();
    expect(screen.getByRole('complementary', { name: 'Filtros' })).toBeDefined();
    expect(document.activeElement).toBe(opener);
  });
});

describe('regras da lista de mods', () => {
  const mods = makeContents().mods;

  it('o que acabou de entrar no pack conta como "no pack" na hora', () => {
    const jade = mods.find((item) => item.title === 'Jade');
    if (!jade) throw new Error('sem Jade');
    expect(modState(jade, new Set())).toBe('compatible');
    expect(modState(jade, new Set([jade.key]))).toBe('inPack');
    expect(countByState(mods, new Set([jade.key]))).toEqual({
      compatible: 5,
      inPack: 2,
      noVersion: 1,
      external: 1,
    });
    expect(visibleMods(mods, true, new Set([jade.key])).map((item) => item.title)).not.toContain(
      'Jade',
    );
  });

  it('itens do plano: versão do modpack ou nenhuma; arquivos fora das lojas nunca entram', () => {
    expect(choicesFor(mods.slice(0, 1), 'same')).toEqual([
      { source: 'modrinth', projectId: 'Aileron', versionId: 'V-Aileron' },
    ]);
    expect(choicesFor(mods.slice(0, 1), 'newest')[0]?.versionId).toBeNull();
    expect(
      choicesFor(
        mods.filter((item) => item.state === 'external'),
        'same',
      ),
    ).toEqual([]);
  });
});
