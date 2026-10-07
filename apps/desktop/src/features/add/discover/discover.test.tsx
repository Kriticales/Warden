/**
 * Início da descoberta e categorias (SPEC T08, CA-T08-10, lado da interface) no app inteiro, com
 * o backend simulado: campo vazio mostra "Populares para <loader> <versão>" e "Atualizados
 * recentemente" (sem chamar a busca), a nota dos que já estão no pack, "Ver mais", as categorias
 * em português que filtram a busca, o aviso de fonte fora do ar e a coluna de filtros que recolhe.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { axePage } from '../../../test/axe';
import { type Handler, ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { makeInventory, SODIUM as SODIUM_ITEM } from '../../pack-editor/editor.fixtures';
import { editorBackend } from '../../pack-editor/testing';
import {
  APPLESKIN,
  CATEGORIES,
  LITHIUM,
  makeHome,
  makePage,
  makePreview,
  makeVersions,
  MODMENU,
  SODIUM,
} from '../add.fixtures';

const PACK = { minecraft: '1.20.1', loader: 'forge', loaderVersion: '47.3.0' };

function setup(handlers: Record<string, Handler> = {}) {
  return editorBackend({
    pack: PACK,
    inventory: makeInventory([SODIUM_ITEM]),
    handlers: {
      discover_home: () => makeHome(),
      discover_categories: () => CATEGORIES,
      search_projects: () => makePage([MODMENU, APPLESKIN]),
      project_details: () => makePreview(),
      project_versions: () => makeVersions(),
      project_gallery: () => [],
      version_dependencies: () => ({ items: [] }),
      ...handlers,
    },
  });
}

async function openAdd(url: string) {
  renderApp(`${url}/adicionar`);
  await screen.findByRole('heading', { level: 1, name: 'Adicionar ao pack' }, { timeout: 10_000 });
}

describe('início da descoberta', () => {
  it('CA-T08-10: campo vazio mostra populares, atualizados e categorias, sem chamar a busca; axe', async () => {
    const { url, backend } = setup();
    await openAdd(url);
    const popular = await screen.findByRole('list', {
      name: 'Populares para Minecraft 1.20.1 com Forge',
    });
    expect(
      within(popular)
        .getAllByRole('button')
        .map((button) => button.textContent),
    ).toEqual(['Sodium', 'Mod Menu']);
    const updated = screen.getByRole('list', { name: 'Atualizados recentemente' });
    expect(within(updated).getByRole('button', { name: 'AppleSkin' })).toBeDefined();
    // Categorias na coluna de filtros, com nome em português.
    const filters = screen.getByRole('complementary', { name: 'Filtros' });
    expect(within(filters).getByRole('radio', { name: 'Todas as categorias' })).toBeDefined();
    expect(within(filters).getByRole('radio', { name: 'Tecnologia' })).toBeDefined();
    expect(backend.callsOf('discover_home')).toHaveLength(1);
    expect(backend.callsOf('discover_home')[0]?.args).toMatchObject({ kind: 'mod' });
    expect(backend.callsOf('search_projects')).toHaveLength(0);
    expect(
      screen.getByText('Mostrando o que funciona no pack. Digite para buscar pelo nome.'),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('o que já está no pack não ocupa lugar e os nomes dos populares aparecem numa nota', async () => {
    const { url } = setup({
      discover_home: () => makeHome({ popularInPack: ['Create', 'Jade', 'Waystones'] }),
    });
    await openAdd(url);
    expect(
      await screen.findByText(
        'Create, Jade e Waystones também estão entre os mais baixados, mas já estão no seu pack.',
      ),
    ).toBeDefined();
  });

  it('um nome só usa o singular da nota', async () => {
    const { url } = setup({ discover_home: () => makeHome({ popularInPack: ['Create'] }) });
    await openAdd(url);
    expect(
      await screen.findByText(
        'Create também está entre os mais baixados, mas já está no seu pack.',
      ),
    ).toBeDefined();
  });

  it('abrir um item do início mostra a pré-visualização e a seleção abre um diálogo só', async () => {
    const { url } = setup();
    await openAdd(url);
    const popular = await screen.findByRole('list', {
      name: 'Populares para Minecraft 1.20.1 com Forge',
    });
    await userEvent.click(within(popular).getByRole('checkbox', { name: 'Selecionar Mod Menu' }));
    expect(screen.getByRole('region', { name: 'Itens selecionados' })).toBeDefined();
    await userEvent.click(within(popular).getByRole('button', { name: 'Mod Menu' }));
    expect(
      await screen.findByRole('complementary', { name: 'Pré-visualização: Mod Menu' }),
    ).toBeDefined();
  });

  it('"Ver mais" dos populares abre a busca ordenada por downloads', async () => {
    const { url, backend } = setup();
    await openAdd(url);
    await userEvent.click(await screen.findByRole('button', { name: 'Ver mais dos populares' }));
    await waitFor(() => {
      expect(backend.callsOf('search_projects').at(-1)?.args.request).toMatchObject({
        query: '',
        sort: 'downloads',
        category: null,
      });
    });
    expect(await screen.findByRole('list', { name: 'Resultados' })).toBeDefined();
    expect(screen.queryByRole('list', { name: 'Atualizados recentemente' })).toBeNull();
  });

  it('escolher a categoria Tecnologia filtra a busca; "Todas as categorias" volta ao início', async () => {
    const { url, backend } = setup();
    await openAdd(url);
    await userEvent.click(await screen.findByRole('radio', { name: 'Tecnologia' }));
    await waitFor(() => {
      expect(backend.callsOf('search_projects').at(-1)?.args.request).toMatchObject({
        kind: 'mod',
        category: 'tecnologia',
      });
    });
    expect(await screen.findByRole('list', { name: 'Resultados' })).toBeDefined();
    expect(screen.getByRole<HTMLInputElement>('radio', { name: 'Tecnologia' }).checked).toBe(true);
    // O botão de filtros conta a categoria.
    expect(screen.getByRole('button', { name: 'Filtros · 1 ativo' })).toBeDefined();
    await userEvent.click(screen.getByRole('radio', { name: 'Todas as categorias' }));
    expect(
      await screen.findByRole('list', { name: 'Populares para Minecraft 1.20.1 com Forge' }),
    ).toBeDefined();
  });

  it('categoria só do Modrinth diz de qual fonte é', async () => {
    const { url } = setup();
    await openAdd(url);
    const radio = await screen.findByRole('radio', { name: /Desempenho/ });
    expect(radio.closest('label')?.textContent).toContain('Só no Modrinth');
  });

  it('trocar o tipo volta os filtros (as categorias são de cada tipo)', async () => {
    const { url } = setup({
      discover_categories: (args) =>
        args.kind === 'shader'
          ? [{ id: 'sh-realista', name: 'Realista', sources: ['modrinth'] }]
          : CATEGORIES,
    });
    await openAdd(url);
    await userEvent.click(await screen.findByRole('radio', { name: 'Magia' }));
    await screen.findByRole('list', { name: 'Resultados' });
    await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Tipo' }), 'shader');
  });

  it('fonte fora do ar no início: aviso com "Tentar de novo" que refaz o início', async () => {
    const { url, backend } = setup({
      discover_home: () =>
        makeHome({
          warnings: [{ source: 'curseforge', reason: 'unavailable', detail: 'erro 500' }],
        }),
    });
    await openAdd(url);
    expect(await screen.findByText('Não foi possível buscar no CurseForge agora.')).toBeDefined();
    const before = backend.callsOf('discover_home').length;
    await userEvent.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    await waitFor(() => {
      expect(backend.callsOf('discover_home').length).toBeGreaterThan(before);
    });
  });

  it('sem chave da CurseForge: o início mostra o aviso e abre Configurações', async () => {
    const { url } = setup({
      discover_home: () =>
        makeHome({ warnings: [{ source: 'curseforge', reason: 'keyMissing', detail: null }] }),
    });
    await openAdd(url);
    expect(await screen.findByText('Mostrando só o Modrinth.')).toBeDefined();
    expect(screen.getByRole('link', { name: 'Abrir Configurações' })).toBeDefined();
  });

  it('início sem nada compatível e início com erro', async () => {
    const empty = setup({ discover_home: () => makeHome({ popular: [], updated: [] }) });
    await openAdd(empty.url);
    expect(await screen.findByText('Nada para mostrar aqui ainda.')).toBeDefined();
    expect(
      screen.getByText(
        'Nenhum item popular funciona em Minecraft 1.20.1 com Forge. Digite um nome para buscar.',
      ),
    ).toBeDefined();
  });

  it('início com erro mostra o motivo', async () => {
    const { url } = setup({
      discover_home: () =>
        ipcError(
          makeAppError(
            { domain: 'project', code: 'SEARCH_SOURCE_UNAVAILABLE' },
            { params: { source: 'Modrinth' }, retryable: true },
          ),
        ),
    });
    await openAdd(url);
    expect(await screen.findByText('Não foi possível montar o início agora.')).toBeDefined();
  });

  it('Já no pack pelo inventário também some da seleção do início', async () => {
    const { url } = setup({
      discover_home: () => makeHome({ popular: [LITHIUM, SODIUM], updated: [] }),
    });
    await openAdd(url);
    const popular = await screen.findByRole('list', {
      name: 'Populares para Minecraft 1.20.1 com Forge',
    });
    const lithium = within(popular).getByRole('button', { name: 'Lithium' }).closest('li');
    expect(lithium && within(lithium).queryByRole('checkbox')).toBeNull();
  });
});

describe('coluna de filtros que recolhe', () => {
  it('o botão Filtros abre e fecha a coluna (a CSS decide quando ele aparece)', async () => {
    const { url } = setup();
    await openAdd(url);
    const button = await screen.findByRole('button', { name: 'Filtros' });
    expect(button.getAttribute('aria-expanded')).toBe('false');
    const body = document.getElementById(button.getAttribute('aria-controls') ?? '');
    expect(body).not.toBeNull();
    expect(body?.closest('.disc__filters')?.classList.contains('is-open')).toBe(false);
    await userEvent.click(button);
    expect(button.getAttribute('aria-expanded')).toBe('true');
    expect(body?.closest('.disc__filters')?.classList.contains('is-open')).toBe(true);
    await userEvent.click(button);
    expect(button.getAttribute('aria-expanded')).toBe('false');
  });
});
