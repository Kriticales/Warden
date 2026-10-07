/**
 * Página Adicionar (SPEC T08) no app inteiro, com o backend simulado: tela cheia com o menu
 * recolhido, busca com "Já no pack" e sem versão compatível, rolagem com a próxima página,
 * seleção múltipla que abre **um** diálogo de dependências (CA-T08-11, lado da interface),
 * aviso de fonte indisponível com "Tentar de novo" e erro da busca.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AddResult, SearchPage } from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { ipcError, type Handler } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { makeInventory, SODIUM as SODIUM_ITEM } from '../pack-editor/editor.fixtures';
import { editorBackend } from '../pack-editor/testing';
import {
  APPLESKIN,
  CATEGORIES,
  LITHIUM,
  makeHome,
  makeNode,
  makePage,
  makePlan,
  makePreview,
  makeResult,
  makeVersions,
  MODMENU,
  OLD_MOD,
  ref,
  SODIUM,
} from './add.fixtures';

const IRIS = makeResult({
  title: 'Iris',
  author: 'coderbot',
  summary: 'Shaders modernos.',
  sources: [ref('YL57xq9U', { slug: 'iris' })],
});

const PACK = { minecraft: '1.21.1', loader: 'fabric', loaderVersion: '0.16.5' };

function setup(handlers: Record<string, Handler> = {}) {
  return editorBackend({
    pack: PACK,
    inventory: makeInventory([SODIUM_ITEM]),
    handlers: {
      search_projects: () => makePage([SODIUM, MODMENU, APPLESKIN, IRIS, LITHIUM, OLD_MOD]),
      project_details: () => makePreview({ title: 'Mod Menu', projectId: 'mOgUt4GM' }),
      project_versions: () => makeVersions(),
      discover_home: () => makeHome(),
      discover_categories: () => CATEGORIES,
      ...handlers,
    },
  });
}

/** Abre a página; com `query`, digita no campo (sem texto o início da descoberta aparece). */
async function openAdd(url: string, query?: string) {
  renderApp(`${url}/adicionar`);
  const heading = await screen.findByRole(
    'heading',
    { level: 1, name: 'Adicionar ao pack' },
    { timeout: 10_000 },
  );
  if (query) await userEvent.type(screen.getByRole('searchbox', { name: 'Buscar' }), query);
  return heading;
}

function results() {
  return screen.findByRole('list', { name: 'Resultados' });
}

describe('página Adicionar', () => {
  it('abre pelo botão Adicionar de Mods, em tela cheia com o menu recolhido; axe', async () => {
    const { url } = setup();
    renderApp(`${url}/mods`);
    // A lista pronta (o cabeçalho da seção é remontado quando o inventário chega).
    await screen.findByRole('searchbox', { name: 'Buscar no pack' }, { timeout: 10_000 });
    await userEvent.click(screen.getByRole('link', { name: 'Adicionar' }));
    expect(
      await screen.findByRole(
        'heading',
        { level: 1, name: 'Adicionar ao pack' },
        { timeout: 10_000 },
      ),
    ).toBeDefined();
    expect(document.querySelector('.secmenu--compact')).not.toBeNull();
    expect(screen.getByRole('link', { name: 'Voltar para Mods' })).toBeDefined();
    expect(screen.getByText('Minecraft 1.21.1 com Fabric')).toBeDefined();
    expect(screen.getByRole('combobox', { name: 'Tipo' })).toBeDefined();
    // Escolher arquivo chega com a P1-11: visível e indisponível.
    expect(
      screen
        .getByRole('button', { name: 'Escolher arquivo do computador…' })
        .getAttribute('aria-disabled'),
    ).toBe('true');
    // Campo vazio e sem filtros: o início da descoberta, não a busca.
    expect(
      await screen.findByRole('heading', { name: 'Populares para Minecraft 1.21.1 com Fabric' }),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('busca: Já no pack sem caixa, sem versão desabilitado; próxima página pelo cursor', async () => {
    const next = { offsets: [{ source: 'modrinth' as const, offset: 20, done: false }] };
    const second = makePage([makeResult({ title: 'Continuum', sources: [ref('CONT1234')] })]);
    const { url, backend } = setup({
      search_projects: (args) => {
        const request = args.request as { cursor?: unknown };
        return request.cursor
          ? second
          : makePage([SODIUM, MODMENU, LITHIUM, OLD_MOD], { next, total: 21 });
      },
    });
    await openAdd(url);
    await userEvent.type(screen.getByRole('searchbox', { name: 'Buscar' }), 'sodium');
    expect(await screen.findByText('21 resultados para “sodium”, do Modrinth')).toBeDefined();
    const list = await results();
    const last = backend.callsOf('search_projects').at(-1);
    expect(last?.args.request).toEqual({
      query: 'sodium',
      kind: 'mod',
      sort: 'relevance',
      source: 'all',
      environment: null,
      includeIncompatible: false,
      category: null,
      cursor: null,
    });
    // Sodium está no inventário; Lithium vem marcado pela busca.
    const rowOf = (name: string) => {
      const row = within(list).getByRole('button', { name }).closest('li');
      if (!row) throw new Error(`sem linha para ${name}`);
      return row;
    };
    expect(within(rowOf('Sodium')).queryByRole('checkbox')).toBeNull();
    expect(within(rowOf('Sodium')).getByText('Já no pack')).toBeDefined();
    expect(within(rowOf('Lithium')).queryByRole('checkbox')).toBeNull();
    expect(within(rowOf('Mod Antigo')).getByRole<HTMLInputElement>('checkbox').disabled).toBe(true);
    expect(within(rowOf('Mod Antigo')).getByText('Sem versão para o seu pack')).toBeDefined();
    expect(within(rowOf('Mod Menu')).getByText('por Prospector')).toBeDefined();

    await userEvent.click(screen.getByRole('button', { name: 'Carregar mais' }));
    expect(await within(list).findByRole('button', { name: 'Continuum' })).toBeDefined();
    expect(backend.callsOf('search_projects').at(-1)?.args.request).toMatchObject({ cursor: next });
  });

  it('CA-T08-11: marcar 3 e "Adicionar 3 ao pack" abre um único diálogo com os 3 e as dependências', async () => {
    const plan = makePlan({
      nodes: [
        ...makePlan().nodes.slice(0, 2),
        makeNode({ projectId: 'YL57xq9U', slug: 'iris', title: 'Iris', versionId: 'V-IRIS' }),
        ...makePlan().nodes.slice(2),
      ],
    });
    const added: AddResult = {
      added: plan.nodes.map((node) => ({
        key: node.key,
        title: node.title,
        path: `mods/${node.slug}.pw.toml`,
      })),
      skipped: [],
      removed: [],
    };
    const { url, backend } = setup({ add_plan: () => plan, add_apply: () => added });
    await openAdd(url, 'mod');
    const list = await results();
    for (const name of ['Mod Menu', 'AppleSkin', 'Iris']) {
      await userEvent.click(within(list).getByRole('checkbox', { name: `Selecionar ${name}` }));
    }
    const bar = screen.getByRole('region', { name: 'Itens selecionados' });
    expect(within(bar).getByText('3 selecionados')).toBeDefined();
    await userEvent.click(within(bar).getByRole('button', { name: 'Adicionar 3 ao pack' }));

    const dialog = await screen.findByRole('dialog', { name: 'Adicionar 3 itens' });
    expect(screen.getAllByRole('dialog')).toHaveLength(1);
    const required = await within(dialog).findByRole('region', { name: 'Obrigatórias' });
    expect(within(required).getByText('Fabric API')).toBeDefined();
    expect(backend.callsOf('add_plan')).toHaveLength(1);
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: [
        { source: 'modrinth', projectId: 'mOgUt4GM', versionId: null },
        { source: 'modrinth', projectId: 'EsAfCjCV', versionId: null },
        { source: 'modrinth', projectId: 'YL57xq9U', versionId: null },
      ],
    });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Adicionar 5 itens' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(backend.callsOf('add_apply')).toHaveLength(1);
    expect(
      await screen.findByRole('link', { name: 'Voltar para Mods · 5 adicionados' }),
    ).toBeDefined();
    expect(screen.queryByRole('region', { name: 'Itens selecionados' })).toBeNull();
  });

  it('pré-visualização: clicar no nome mostra a descrição; Adicionar ao pack abre o diálogo do item', async () => {
    const { url, backend } = setup({
      add_plan: () => makePlan({ nodes: [makeNode()] }),
    });
    await openAdd(url, 'mod');
    const list = await results();
    await userEvent.click(within(list).getByRole('button', { name: 'Mod Menu' }));
    const preview = await screen.findByRole('complementary', {
      name: 'Pré-visualização: Mod Menu',
    });
    expect(await within(preview).findByText('mais leve')).toBeDefined();
    await userEvent.click(within(preview).getByRole('button', { name: 'Adicionar ao pack' }));
    expect(await screen.findByRole('dialog', { name: 'Adicionar Mod Menu' })).toBeDefined();
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: [{ source: 'modrinth', projectId: 'mOgUt4GM', versionId: 'SMxNOGZ6' }],
    });
  });

  it('uma fonte fora do ar: aviso com "Tentar de novo", que refaz a busca', async () => {
    const page: SearchPage = makePage([MODMENU], {
      warnings: [{ source: 'curseforge', reason: 'unavailable', detail: 'erro 500' }],
    });
    const { url, backend } = setup({ search_projects: () => page });
    await openAdd(url, 'mod');
    await results();
    expect(screen.getByText('Não foi possível buscar no CurseForge agora.')).toBeDefined();
    const before = backend.callsOf('search_projects').length;
    await userEvent.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    await waitFor(() => {
      expect(backend.callsOf('search_projects').length).toBeGreaterThan(before);
    });
  });

  it('busca com erro: painel com o motivo e "Tentar de novo"', async () => {
    const { url } = setup({
      search_projects: () =>
        ipcError(
          makeAppError(
            { domain: 'project', code: 'SEARCH_SOURCE_UNAVAILABLE' },
            { params: { source: 'Modrinth' }, retryable: true },
          ),
        ),
    });
    await openAdd(url, 'mod');
    expect(
      await screen.findByText(
        'Não foi possível falar com o Modrinth agora. Confira a internet e tente de novo.',
        {},
        { timeout: 5000 },
      ),
    ).toBeDefined();
    expect(screen.getByText('Não foi possível buscar agora.')).toBeDefined();
  });

  it('nada encontrado: oferece mostrar também os sem versão compatível', async () => {
    const { url, backend } = setup({ search_projects: () => makePage([]) });
    await openAdd(url, 'xyz');
    expect(await screen.findByText('Nada encontrado para “xyz”')).toBeDefined();
    await userEvent.click(
      screen.getByRole('button', { name: 'Mostrar também os sem versão compatível' }),
    );
    await waitFor(() => {
      expect(backend.callsOf('search_projects').at(-1)?.args.request).toMatchObject({
        includeIncompatible: true,
      });
    });
  });
});
