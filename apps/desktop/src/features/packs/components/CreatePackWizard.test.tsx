import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { CreatePack, Loader } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError, mockBackend, type Handler } from '../../../test/backend';
import {
  makeAppError,
  makeLoaderVersions,
  makeMinecraftVersions,
  makePackRow,
} from '../../../test/factories';
import { renderApp } from '../../../test/render';
import {
  FABRIC_KIT,
  FABRIC_OFFER,
  makeInitialResult,
} from '../create/initial-mods/initial-mods.fixtures';

const PACKS_DIR = 'C:/Users/jogador/Documents/Warden';

/** Loaders que existem para cada versão do Minecraft (como o catálogo real). */
const LOADERS: Record<string, Partial<Record<Loader, string[]>>> = {
  '26.3': { neoforge: ['26.3.4'], fabric: ['0.18.1'] },
  '1.21.1': { forge: ['52.0.1'], neoforge: ['21.1.252', '21.1.200'], fabric: ['0.16.5'] },
  '1.20.1': { forge: ['47.3.0', '47.2.0'], neoforge: ['47.1.106'], fabric: ['0.16.5'] },
  '1.19.2': { forge: ['43.4.0'], fabric: ['0.16.5'] },
  '1.12.2': { forge: ['14.23.5.2860'] },
  '1.7.10': { forge: ['10.13.4.1614'] },
};

function slug(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}

function wizardBackend(extra: Record<string, Handler> = {}) {
  return mockBackend({
    pack_create_defaults: () => ({ author: 'Jogador', packsDir: PACKS_DIR }),
    pack_create_check: ({ name, destination }) => {
      if (typeof destination === 'string') return destination;
      return `${PACKS_DIR}/${slug(name as string)}`;
    },
    catalog_minecraft_versions: () => makeMinecraftVersions(),
    catalog_loader_versions: ({ loader, minecraft }) =>
      makeLoaderVersions(
        loader as Loader,
        minecraft as string,
        LOADERS[minecraft as string]?.[loader as Loader] ?? [],
      ),
    pack_create: ({ request }) => ({
      id: '01JA0000000000000000000099',
      path: (request as CreatePack).destination ?? `${PACKS_DIR}/vale-sereno`,
    }),
    pack_get: () => makePackRow({ id: '01JA0000000000000000000099', name: 'Vale Sereno' }),
    pack_hygiene_scan: () => [],
    initial_mods_offer: () => FABRIC_OFFER,
    initial_mods_apply: () => makeInitialResult(),
    kits_list: () => [FABRIC_KIT],
    ...extra,
  });
}

async function fillName(user: ReturnType<typeof userEvent.setup>, name = 'Vale Sereno') {
  await user.type(await screen.findByRole('textbox', { name: 'Nome do pack' }), name);
}

async function next(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Próximo' }));
}

describe('Criar pack (T03)', () => {
  it('etapa 1: pasta acompanha o nome, autor padrão no exemplo; passa no axe', async () => {
    const user = userEvent.setup();
    wizardBackend();
    const { container } = renderApp('/packs/novo');
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Criar pack' }, { timeout: 5000 }),
    ).toBeDefined();
    const steps = screen.getByRole('list', { name: 'Etapas de criar pack' });
    expect(
      within(steps)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual([
      '1Nome e pasta (agora)',
      '2Versão do Minecraft',
      '3Loader',
      '4Mods iniciais',
      '5Resumo',
    ]);
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Próximo' }).disabled).toBe(true);
    expect(await screen.findByText('Em branco, fica “Jogador”, o nome do jogador configurado.'));
    await fillName(user);
    const folder = screen.getByRole('textbox', { name: 'Pasta' });
    await waitFor(() => {
      expect((folder as HTMLInputElement).value).toBe(`${PACKS_DIR}/vale-sereno`);
    });
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('CA-T03-04 (interface): pasta com arquivos é recusada na etapa 1, com a explicação', async () => {
    const user = userEvent.setup();
    const backend = wizardBackend({
      pack_create_check: ({ destination }) =>
        ipcError(
          makeAppError(
            { domain: 'project', code: 'DESTINATION_NOT_EMPTY' },
            { params: { path: (destination as string | null) ?? `${PACKS_DIR}/vale-sereno` } },
          ),
        ),
    });
    renderApp('/packs/novo');
    await fillName(user);
    expect(
      await screen.findByText(
        'Esta pasta já tem arquivos. Escolha uma pasta vazia ou mude o nome do pack.',
      ),
    ).toBeDefined();
    const folder = screen.getByRole('textbox', { name: 'Pasta' });
    expect(folder.getAttribute('aria-invalid')).toBe('true');
    expect((folder as HTMLInputElement).value).toBe(`${PACKS_DIR}/vale-sereno`);
    await next(user);
    // Continua na etapa 1 e nada foi criado.
    expect(screen.getByRole('textbox', { name: 'Nome do pack' })).toBeDefined();
    expect(backend.callsOf('pack_create')).toHaveLength(0);
  });

  it('cria um pack Forge 1.20.1 com as versões do catálogo e abre o pack', async () => {
    const user = userEvent.setup();
    const backend = wizardBackend();
    renderApp('/packs/novo');
    await fillName(user);
    await user.type(screen.getByRole('textbox', { name: 'Descrição (opcional)' }), 'Calmo');
    await next(user);

    // Etapa 2: só releases, na ordem oficial; a mais nova vem marcada.
    const list = await screen.findByRole('group', { name: 'Versão do Minecraft' });
    const radios = within(list).getAllByRole('radio');
    expect(radios.map((radio) => (radio as HTMLInputElement).value)).toEqual([
      '26.3',
      '1.21.1',
      '1.20.1',
      '1.19.2',
      '1.12.2',
      '1.7.10',
      '1.6.4',
    ]);
    expect((radios[0] as HTMLInputElement).checked).toBe(true);
    expect(within(list).getByText('Melhor esforço: versões anteriores a 1.7.10 podem não abrir.'));
    await user.type(screen.getByRole('searchbox', { name: 'Buscar versão (ex.: 1.20)' }), '1.20');
    expect(within(list).getAllByRole('radio')).toHaveLength(1);
    await user.click(within(list).getByRole('radio', { name: '1.20.1' }));
    await next(user);

    // Etapa 3: Forge pré-selecionado na recomendada.
    const forge = await screen.findByRole('radio', { name: /Forge 47\.3\.0 Recomendada/ });
    await waitFor(() => {
      expect((forge as HTMLInputElement).checked).toBe(true);
    });
    expect(screen.getByRole('radio', { name: /NeoForge 47\.1\.106/ })).toBeDefined();
    expect(screen.getByRole('radio', { name: /Fabric 0\.16\.5/ })).toBeDefined();
    expect(screen.getByRole<HTMLInputElement>('radio', { name: /Quilt/ }).disabled).toBe(true);
    await user.click(screen.getByRole('button', { name: 'Escolher outra versão do Forge' }));
    await user.selectOptions(screen.getByRole('combobox', { name: 'Versão do Forge' }), '47.2.0');
    await next(user);

    // Etapa 4: spark e Crash Assistant vêm marcados (D16); o kit, não.
    const spark = await screen.findByRole<HTMLInputElement>('checkbox', {
      name: 'Adicionar spark',
    });
    await waitFor(() => {
      expect(spark.checked).toBe(true);
    });
    expect(
      screen.getByRole<HTMLInputElement>('checkbox', { name: 'Adicionar Crash Assistant' }).checked,
    ).toBe(true);
    await next(user);

    // Resumo.
    expect(await screen.findByText('Minecraft 1.20.1 · Forge 47.2.0 · por Jogador')).toBeDefined();
    expect(screen.getByText(`${PACKS_DIR}/vale-sereno`)).toBeDefined();
    expect(screen.getByText('0.1.0')).toBeDefined();
    expect(
      screen.getByText(
        'Mods iniciais: spark, Crash Assistant, com a config do Crash Assistant sem envio de dados ao autor',
      ),
    ).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Criar pack' }));

    expect(await screen.findByText('“Vale Sereno” criado.')).toBeDefined();
    // Depois do "Pack criado", num ponto único, os mods iniciais entram pelo mesmo caminho de Adicionar.
    expect(await screen.findByText('3 mods iniciais adicionados')).toBeDefined();
    expect(backend.callsOf('initial_mods_apply')).toEqual([
      {
        command: 'initial_mods_apply',
        args: {
          packId: '01JA0000000000000000000099',
          request: { tools: ['spark', 'crash-assistant'], kit: null },
        },
      },
    ]);
    expect(await screen.findByRole('heading', { level: 1, name: 'Vale Sereno' })).toBeDefined();
    expect(backend.callsOf('pack_create')).toEqual([
      {
        command: 'pack_create',
        args: {
          request: {
            name: 'Vale Sereno',
            author: '',
            description: 'Calmo',
            destination: null,
            minecraft: '1.20.1',
            loader: 'forge',
            loaderVersion: '47.2.0',
          },
        },
      },
    ]);
  });

  /** Percorre o assistente até a etapa "Mods iniciais" de um pack Fabric 1.21.1. */
  async function toModsStep(user: ReturnType<typeof userEvent.setup>) {
    await fillName(user);
    await next(user);
    await user.click(await screen.findByRole('radio', { name: '1.21.1' }));
    await next(user);
    await user.click(await screen.findByRole('radio', { name: /^Fabric/ }));
    await next(user);
    await screen.findByRole('checkbox', { name: 'Adicionar spark' });
  }

  it('CA-T03-07 (interface): desmarcar os dois cria o pack sem gravar mods iniciais', async () => {
    const user = userEvent.setup();
    const backend = wizardBackend();
    renderApp('/packs/novo');
    await toModsStep(user);
    await waitFor(() => {
      expect(
        screen.getByRole<HTMLInputElement>('checkbox', { name: 'Adicionar spark' }).checked,
      ).toBe(true);
    });
    await user.click(screen.getByRole('checkbox', { name: 'Adicionar spark' }));
    await user.click(screen.getByRole('checkbox', { name: 'Adicionar Crash Assistant' }));
    await next(user);
    expect(await screen.findByText('Nenhum mod inicial')).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Criar pack' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Vale Sereno' })).toBeDefined();
    expect(backend.callsOf('pack_create')).toHaveLength(1);
    expect(backend.callsOf('initial_mods_apply')).toHaveLength(0);
  });

  it('o kit escolhido vai junto no pedido e aparece no resumo', async () => {
    const user = userEvent.setup();
    const backend = wizardBackend();
    renderApp('/packs/novo');
    await toModsStep(user);
    await user.click(
      screen.getByRole('checkbox', {
        name: 'Adicionar o kit Desempenho para Fabric 1.21 em diante',
      }),
    );
    await next(user);
    expect(
      await screen.findByText(
        /Mods iniciais: spark, Crash Assistant, kit Desempenho para Fabric 1\.21 em diante \(4 mods\)/,
      ),
    ).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Criar pack' }));
    await waitFor(() => {
      expect(backend.callsOf('initial_mods_apply')).toHaveLength(1);
    });
    expect(backend.callsOf('initial_mods_apply')[0]?.args.request).toEqual({
      tools: ['spark', 'crash-assistant'],
      kit: { id: 'fabric-moderno', projects: ['AANobbMI', 'gvQqBUqZ', 'uXXizFIs', '5ZwdcRci'] },
    });
  });

  it('se os mods iniciais não entrarem, o pack já criado abre com o aviso', async () => {
    const user = userEvent.setup();
    const backend = wizardBackend({
      initial_mods_apply: () =>
        ipcError(
          makeAppError(
            { domain: 'project', code: 'SEARCH_SOURCE_UNAVAILABLE' },
            { params: { source: 'Modrinth' }, retryable: true },
          ),
        ),
    });
    renderApp('/packs/novo');
    await toModsStep(user);
    await next(user);
    await user.click(await screen.findByRole('button', { name: 'Criar pack' }));
    expect(
      await screen.findByText(
        'O pack foi criado, mas os mods iniciais não entraram. Adicione-os pela página Mods.',
      ),
    ).toBeDefined();
    expect(await screen.findByRole('heading', { level: 1, name: 'Vale Sereno' })).toBeDefined();
    expect(backend.callsOf('pack_create')).toHaveLength(1);
  });

  it('itens que ficaram de fora são nomeados no aviso', async () => {
    const user = userEvent.setup();
    wizardBackend({
      initial_mods_apply: () =>
        makeInitialResult({ added: [], leftOut: ['spark'], playerTools: [] }),
    });
    renderApp('/packs/novo');
    await toModsStep(user);
    await next(user);
    await user.click(await screen.findByRole('button', { name: 'Criar pack' }));
    expect(
      await screen.findByText('Ficaram de fora: spark. Dá para adicionar depois pela página Mods.'),
    ).toBeDefined();
  });

  it('CA-T03-03: NeoForge não aparece para 1.19.2; Fabric não aparece para 1.12.2', async () => {
    const user = userEvent.setup();
    wizardBackend();
    renderApp('/packs/novo');
    await fillName(user);
    await next(user);
    await user.click(await screen.findByRole('radio', { name: '1.19.2' }));
    await next(user);
    expect(await screen.findByRole('radio', { name: /^Forge/ })).toBeDefined();
    expect(screen.getByRole('radio', { name: /^Fabric/ })).toBeDefined();
    expect(screen.queryByRole('radio', { name: /^NeoForge/ })).toBeNull();
    expect(screen.getByText('Só aparecem os loaders que existem para o Minecraft 1.19.2.'));

    await user.click(screen.getByRole('button', { name: 'Voltar' }));
    await user.click(await screen.findByRole('radio', { name: '1.12.2' }));
    await next(user);
    expect(await screen.findByRole('radio', { name: /^Forge 14\.23\.5\.2860/ })).toBeDefined();
    expect(screen.queryByRole('radio', { name: /^Fabric/ })).toBeNull();
    expect(screen.queryByRole('radio', { name: /^NeoForge/ })).toBeNull();
  });

  it('cria um pack vanilla sem loader', async () => {
    const user = userEvent.setup();
    const backend = wizardBackend();
    renderApp('/packs/novo');
    await fillName(user, 'Puro');
    await next(user);
    await user.click(await screen.findByRole('radio', { name: '1.21.1' }));
    await next(user);
    await user.click(await screen.findByRole('radio', { name: /Nenhum \(vanilla\)/ }));
    await next(user);
    // Sem loader não há mods: a etapa explica e não consulta nada.
    expect(await screen.findByText('Este pack não tem loader')).toBeDefined();
    await next(user);
    expect(await screen.findByText('Minecraft 1.21.1 · vanilla · por Jogador')).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Criar pack' }));
    await waitFor(() => {
      expect(backend.callsOf('pack_create')[0]?.args.request).toMatchObject({
        minecraft: '1.21.1',
        loader: null,
        loaderVersion: null,
      });
    });
    expect(await screen.findByRole('heading', { level: 1, name: 'Vale Sereno' })).toBeDefined();
    expect(backend.callsOf('initial_mods_offer')).toHaveLength(0);
    expect(backend.callsOf('initial_mods_apply')).toHaveLength(0);
  });

  it('sem internet e sem lista guardada: o erro com "Tentar de novo", que recarrega', async () => {
    const user = userEvent.setup();
    let fail = true;
    wizardBackend({
      catalog_minecraft_versions: () =>
        fail
          ? ipcError(
              makeAppError(
                { domain: 'core', code: 'NETWORK_UNAVAILABLE' },
                { params: { source: 'Mojang' }, retryable: true },
              ),
            )
          : makeMinecraftVersions(),
    });
    renderApp('/packs/novo');
    await fillName(user);
    await next(user);
    expect(
      await screen.findByRole('heading', {
        name: 'Não foi possível carregar as versões do Minecraft',
      }),
    ).toBeDefined();
    expect(
      screen.getByText(
        'Sem internet e sem uma cópia guardada da lista. Verifique a conexão e tente de novo.',
      ),
    ).toBeDefined();
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Próximo' }).disabled).toBe(true);
    fail = false;
    await user.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    expect(await screen.findByRole('radio', { name: '26.3' })).toBeDefined();
  });

  it('lista vinda do cache mostra "Lista de versões de <data>"', async () => {
    const user = userEvent.setup();
    wizardBackend({
      catalog_minecraft_versions: () =>
        makeMinecraftVersions({
          freshness: { fetchedAtMs: new Date(2026, 8, 30).getTime(), offline: true },
        }),
    });
    renderApp('/packs/novo');
    await fillName(user);
    await next(user);
    expect(await screen.findByText('Lista de versões de 30/09/2026')).toBeDefined();
  });

  it('pasta que ganhou arquivos depois da etapa 1 volta para ela com o motivo', async () => {
    const user = userEvent.setup();
    wizardBackend({
      pack_create: () =>
        ipcError(
          makeAppError(
            { domain: 'project', code: 'DESTINATION_NOT_EMPTY' },
            { params: { path: `${PACKS_DIR}/vale-sereno` } },
          ),
        ),
    });
    renderApp('/packs/novo');
    await fillName(user);
    await next(user);
    await screen.findByRole('radio', { name: '26.3' });
    await next(user);
    await waitFor(async () => {
      expect(
        (await screen.findByRole<HTMLInputElement>('radio', { name: /NeoForge/ })).checked,
      ).toBe(true);
    });
    await next(user);
    await screen.findByRole('checkbox', { name: 'Adicionar spark' });
    await next(user);
    await user.click(await screen.findByRole('button', { name: 'Criar pack' }));
    expect(
      await screen.findByText(
        'Esta pasta já tem arquivos. Escolha uma pasta vazia ou mude o nome do pack.',
      ),
    ).toBeDefined();
    expect(screen.getByRole('textbox', { name: 'Nome do pack' })).toBeDefined();
  });

  it('"Escolher…" usa o diálogo nativo; "Usar a pasta padrão" volta para a automática', async () => {
    const user = userEvent.setup();
    const backend = wizardBackend({ pack_choose_folder: () => 'D:/Packs/novo' });
    renderApp('/packs/novo');
    await fillName(user);
    await user.click(screen.getByRole('button', { name: 'Escolher…' }));
    const folder = screen.getByRole('textbox', { name: 'Pasta' });
    await waitFor(() => {
      expect((folder as HTMLInputElement).value).toBe('D:/Packs/novo');
    });
    expect(backend.callsOf('pack_choose_folder')[0]?.args).toEqual({
      purpose: 'createDestination',
    });
    await user.click(screen.getByRole('button', { name: 'Usar a pasta padrão' }));
    await waitFor(() => {
      expect((folder as HTMLInputElement).value).toBe(`${PACKS_DIR}/vale-sereno`);
    });
  });

  it('Cancelar volta para Meus packs sem criar nada', async () => {
    const user = userEvent.setup();
    const backend = wizardBackend({ packs_list: () => [] });
    renderApp('/packs/novo');
    await user.click(await screen.findByRole('button', { name: 'Cancelar' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Meus packs' })).toBeDefined();
    expect(backend.callsOf('pack_create')).toHaveLength(0);
  });
});
