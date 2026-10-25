/**
 * Início da descoberta, modos de exibição e filtros (SPEC T08, CA-T08-10; ADR-0068; esboço
 * `design/bug-01/adicionar.html`, opção 4) no app inteiro, com o backend simulado: campo vazio
 * mostra "Populares" e "Atualizados recentemente" (sem chamar a busca), o que já está no pack com
 * o selo No pack, as bibliotecas escondidas numa linha, **Lista | Cartões** (a seleção continua
 * e o app lembra o modo), as categorias curtas que filtram a busca, a ordem pelas colunas e o
 * detalhe no lugar dos filtros.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';

import { axePage } from '../../../test/axe';
import { type Handler, ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { makeInventory, SODIUM as SODIUM_ITEM } from '../../pack-editor/editor.fixtures';
import { editorBackend } from '../../pack-editor/testing';
import { FABRIC_KIT } from '../../packs/create/initial-mods/initial-mods.fixtures';
import {
  APPLESKIN,
  CATEGORIES,
  FABRIC_API,
  LITHIUM,
  makeHome,
  makeNode,
  makePage,
  makePlan,
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
      kits_list: () => [],
      ...handlers,
    },
  });
}

/** O grupo da lista ("Populares", "Atualizados recentemente", "Resultados"…). */
function group(name: string | RegExp) {
  return screen.findByRole('rowgroup', { name }, { timeout: 10_000 });
}

/** Os nomes na ordem em que aparecem num grupo da lista ou dos cartões. */
function namesIn(element: HTMLElement): string[] {
  return Array.from(
    element.querySelectorAll('.tname__n, .mcard__n'),
    (button) => button.textContent,
  );
}

beforeEach(() => {
  localStorage.clear();
});

async function openAdd(url: string) {
  renderApp(`${url}/adicionar`);
  await screen.findByRole('heading', { level: 1, name: 'Adicionar ao pack' }, { timeout: 10_000 });
}

describe('início da descoberta', () => {
  it('CA-T08-10: campo vazio mostra populares, atualizados e categorias, sem chamar a busca; axe', async () => {
    const { url, backend } = setup();
    await openAdd(url);
    const popular = await group('Populares');
    // Sodium está no inventário do pack: aparece com o selo No pack, sem caixa (ADR-0068).
    expect(namesIn(popular)).toEqual(['Sodium', 'Mod Menu']);
    expect(within(popular).getByText('2 de 2.481')).toBeDefined();
    const updated = await group('Atualizados recentemente');
    expect(namesIn(updated)).toEqual(['AppleSkin']);
    // Categorias na coluna de filtros, à direita, com nome em português.
    const filters = screen.getByRole('complementary', { name: 'Filtros' });
    expect(
      within(filters).getByRole('button', { name: 'Todas' }).getAttribute('aria-pressed'),
    ).toBe('true');
    expect(within(filters).getByRole('button', { name: 'Tecnologia' })).toBeDefined();
    expect(backend.callsOf('discover_home')).toHaveLength(1);
    expect(backend.callsOf('discover_home')[0]?.args).toMatchObject({ kind: 'mod' });
    expect(backend.callsOf('search_projects')).toHaveLength(0);
    // A trava de versão vira uma linha discreta.
    expect(
      screen.getByText(
        (_, element) =>
          element?.classList.contains('lockline') === true &&
          element.textContent.includes('Só versões para Minecraft 1.20.1 com Forge'),
      ),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('o que já está no pack aparece com o selo No pack; "O que já está no pack" desligado esconde', async () => {
    const { url } = setup({
      discover_home: () => makeHome({ popular: [LITHIUM, MODMENU], updated: [] }),
    });
    await openAdd(url);
    const popular = await group('Populares');
    const lithium = within(popular)
      .getByRole('button', { name: 'Lithium' })
      .closest('[role="row"]');
    expect(lithium && within(lithium as HTMLElement).queryByRole('checkbox')).toBeNull();
    expect(lithium && within(lithium as HTMLElement).getByText('No pack')).toBeDefined();
    await userEvent.click(screen.getByRole('checkbox', { name: 'O que já está no pack' }));
    expect(namesIn(await group('Populares'))).toEqual(['Mod Menu']);
  });

  it('bibliotecas ficam escondidas numa linha, e Mostrar as traz de volta', async () => {
    const { url } = setup({
      discover_home: () => makeHome({ popular: [MODMENU, FABRIC_API], updated: [] }),
    });
    await openAdd(url);
    const popular = await group('Populares');
    expect(namesIn(popular)).toEqual(['Mod Menu']);
    expect(within(popular).getByText('1 biblioteca escondida')).toBeDefined();
    expect(within(popular).getByText(/Fabric API/)).toBeDefined();
    await userEvent.click(within(popular).getByRole('button', { name: 'Mostrar' }));
    expect(namesIn(await group('Populares'))).toEqual(['Mod Menu', 'Fabric API']);
    expect(screen.getByRole<HTMLInputElement>('checkbox', { name: /^Bibliotecas/ }).checked).toBe(
      true,
    );
    expect(screen.getByText('Biblioteca')).toBeDefined();
  });

  it('Lista | Cartões: a seleção continua ao trocar, as bibliotecas viram um grupo fechado e o app lembra o modo', async () => {
    const { url } = setup({
      discover_home: () => makeHome({ popular: [MODMENU, FABRIC_API], updated: [APPLESKIN] }),
    });
    await openAdd(url);
    await group('Populares');
    await userEvent.click(screen.getByRole('checkbox', { name: 'Marcar Mod Menu para adicionar' }));
    await userEvent.click(screen.getByRole('radio', { name: 'Cartões' }));
    const cards = await screen.findByRole('list', { name: 'Populares' });
    const mark = within(cards).getByRole('button', { name: 'Marcar Mod Menu para adicionar' });
    expect(mark.getAttribute('aria-pressed')).toBe('true');
    expect(mark.textContent).toBe('Marcado');
    expect(screen.getByRole('region', { name: 'Itens marcados' }).textContent).toContain(
      '1 marcado',
    );
    // As bibliotecas: um grupo fechado.
    const libraries = screen.getByRole('button', { name: /Bibliotecas populares \(1\)/ });
    expect(libraries.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('button', { name: 'Fabric API' })).toBeNull();
    await userEvent.click(libraries);
    expect(screen.getByRole('button', { name: 'Fabric API' })).toBeDefined();
    // Marcar pelo cartão também conta.
    await userEvent.click(
      within(await screen.findByRole('list', { name: 'Atualizados recentemente' })).getByRole(
        'button',
        { name: 'Marcar AppleSkin para adicionar' },
      ),
    );
    expect(screen.getByRole('region', { name: 'Itens marcados' }).textContent).toContain(
      '2 marcados',
    );
    expect(localStorage.getItem('warden.adicionar.modo')).toBe('cartoes');
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('o app abre no último modo usado', async () => {
    localStorage.setItem('warden.adicionar.modo', 'cartoes');
    const { url } = setup();
    await openAdd(url);
    expect(await screen.findByRole('list', { name: 'Populares' })).toBeDefined();
    expect(screen.getByRole('radio', { name: 'Cartões' }).getAttribute('aria-checked')).toBe(
      'true',
    );
    expect(screen.queryByRole('table')).toBeNull();
  });

  it('a barra dos marcados diz os nomes e as obrigatórias que já dá para saber', async () => {
    const { url, backend } = setup({
      add_plan: () =>
        makePlan({
          nodes: [
            makeNode(),
            makeNode({
              projectId: 'P7dR8mSH',
              title: 'Fabric API',
              role: 'required',
              requiredBy: ['modrinth:mOgUt4GM'],
            }),
          ],
        }),
    });
    await openAdd(url);
    await group('Populares');
    await userEvent.click(screen.getByRole('checkbox', { name: 'Marcar Mod Menu para adicionar' }));
    const bar = screen.getByRole('region', { name: 'Itens marcados' });
    expect(bar.textContent).toContain('1 marcado · Mod Menu');
    expect(bar.textContent).toContain('No próximo passo você confere as dependências');
    await waitFor(
      () => {
        expect(bar.textContent).toContain('Já sabemos de 1 obrigatória: Fabric API.');
      },
      { timeout: 5_000 },
    );
    expect(backend.callsOf('add_plan')[0]?.args.request).toEqual({
      items: [{ source: 'modrinth', projectId: 'mOgUt4GM', versionId: null }],
    });
    expect(within(bar).getByRole('button', { name: 'Adicionar 1 ao pack' })).toBeDefined();
  });

  it('clicar no nome mostra o detalhe no lugar dos filtros; "Filtros" volta', async () => {
    const { url } = setup();
    await openAdd(url);
    const popular = await group('Populares');
    await userEvent.click(within(popular).getByRole('button', { name: 'Mod Menu' }));
    expect(
      await screen.findByRole('complementary', { name: 'Pré-visualização: Mod Menu' }),
    ).toBeDefined();
    expect(screen.queryByRole('complementary', { name: 'Filtros' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Filtros' }));
    expect(screen.getByRole('complementary', { name: 'Filtros' })).toBeDefined();
  });

  it('"Kits de desempenho" (P1-18) no início de mods: "Adicionar kit" abre o diálogo de dependências', async () => {
    const { url, backend } = setup({
      kits_list: () => [FABRIC_KIT],
      add_plan: () => makePlan({ nodes: [makeNode({ title: 'Sodium' })] }),
    });
    await openAdd(url);
    expect(await screen.findByRole('heading', { name: 'Kits de desempenho' })).toBeDefined();
    // O kit é o da faixa do pack (loader e versão do pack aberto).
    expect(backend.callsOf('kits_list')[0]?.args).toEqual({
      minecraft: '1.20.1',
      loader: 'forge',
    });
    await userEvent.click(await screen.findByRole('button', { name: 'Adicionar kit' }));
    expect(await screen.findByRole('dialog')).toBeDefined();
    await waitFor(() => {
      expect(backend.callsOf('add_plan')).toHaveLength(1);
    });
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('o início de shaders não mostra kits (são listas de mods)', async () => {
    const { url, backend } = setup({ kits_list: () => [FABRIC_KIT] });
    await openAdd(url);
    await screen.findByRole('heading', { name: 'Kits de desempenho' });
    await userEvent.click(screen.getByRole('radio', { name: 'Shaders' }));
    await waitFor(() => {
      expect(screen.queryByRole('heading', { name: 'Kits de desempenho' })).toBeNull();
    });
    expect(backend.callsOf('kits_list')).toHaveLength(1);
  });

  it('"Ver todos" dos populares abre a busca ordenada por downloads', async () => {
    const { url, backend } = setup();
    await openAdd(url);
    await userEvent.click(await screen.findByRole('button', { name: 'Ver todos os populares' }));
    await waitFor(() => {
      expect(backend.callsOf('search_projects').at(-1)?.args.request).toMatchObject({
        query: '',
        sort: 'downloads',
        category: null,
      });
    });
    expect(await group('Resultados')).toBeDefined();
    expect(screen.queryByRole('rowgroup', { name: 'Atualizados recentemente' })).toBeNull();
    // A coluna Downloads mostra a ordem.
    expect(screen.getByRole('columnheader', { name: 'Downloads' }).getAttribute('aria-sort')).toBe(
      'descending',
    );
  });

  it('as colunas ordenam: Atualizado pede a busca ordenada, Nome ordena o que já veio', async () => {
    const { url, backend } = setup({
      search_projects: () => makePage([MODMENU, APPLESKIN, SODIUM]),
    });
    await openAdd(url);
    await group('Populares');
    await userEvent.click(screen.getByRole('button', { name: 'Atualizado' }));
    await waitFor(() => {
      expect(backend.callsOf('search_projects').at(-1)?.args.request).toMatchObject({
        sort: 'updated',
      });
    });
    const results = await group('Resultados');
    expect(namesIn(results)).toEqual(['Mod Menu', 'AppleSkin', 'Sodium']);
    await userEvent.click(screen.getByRole('button', { name: 'Nome' }));
    expect(namesIn(await group('Resultados'))).toEqual(['AppleSkin', 'Mod Menu', 'Sodium']);
    expect(screen.getByRole('columnheader', { name: 'Nome' }).getAttribute('aria-sort')).toBe(
      'ascending',
    );
    await userEvent.click(screen.getByRole('button', { name: 'Nome' }));
    expect(namesIn(await group('Resultados'))).toEqual(['Sodium', 'Mod Menu', 'AppleSkin']);
  });

  it('escolher a categoria Tecnologia filtra a busca; "Todas" volta ao início', async () => {
    const { url, backend } = setup();
    await openAdd(url);
    await userEvent.click(await screen.findByRole('button', { name: 'Tecnologia' }));
    await waitFor(() => {
      expect(backend.callsOf('search_projects').at(-1)?.args.request).toMatchObject({
        kind: 'mod',
        category: 'tecnologia',
      });
    });
    // O grupo leva o nome da categoria.
    expect(await group('Tecnologia')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Tecnologia' }).getAttribute('aria-pressed')).toBe(
      'true',
    );
    await userEvent.click(screen.getByRole('button', { name: 'Todas' }));
    expect(await group('Populares')).toBeDefined();
  });

  it('categoria só do Modrinth leva a marca da loja, sem texto quebrando', async () => {
    const { url } = setup({
      discover_categories: () => [
        ...CATEGORIES,
        { id: 'redstone', name: 'Redstone', sources: ['curseforge'] },
      ],
    });
    await openAdd(url);
    const only = await screen.findByRole('button', { name: /Desempenho/ });
    expect(only.getAttribute('title')).toBe('Desempenho · Só no Modrinth');
    expect(only.querySelector('.srcdots__mark--modrinth')).not.toBeNull();
    expect(only.textContent).not.toContain('Só no');
    expect(screen.getByRole('button', { name: /Redstone/ }).getAttribute('title')).toBe(
      'Redstone · Só na CurseForge',
    );
  });

  it('sem a CurseForge ligada, nenhuma categoria leva a marca de uma loja só', async () => {
    const { url } = setup({
      discover_categories: () =>
        CATEGORIES.map((category) => ({ ...category, sources: ['modrinth' as const] })),
    });
    await openAdd(url);
    const filters = await screen.findByRole('complementary', { name: 'Filtros' });
    const tech = await within(filters).findByRole('button', { name: 'Tecnologia' });
    expect(tech.getAttribute('title')).toBe('Tecnologia');
    expect(filters.querySelector('.clist .srcdots')).toBeNull();
  });

  it('a lista de categorias é curta: 8 e "Mais N categorias"', async () => {
    const many = Array.from({ length: 11 }, (_, index) => ({
      id: `c${String(index)}`,
      name: `Categoria ${String(index + 1)}`,
      sources: ['modrinth' as const, 'curseforge' as const],
    }));
    const { url } = setup({ discover_categories: () => many });
    await openAdd(url);
    const filters = await screen.findByRole('complementary', { name: 'Filtros' });
    await within(filters).findByRole('button', { name: 'Categoria 8' });
    expect(within(filters).queryByRole('button', { name: 'Categoria 9' })).toBeNull();
    await userEvent.click(within(filters).getByRole('button', { name: 'Mais 3 categorias' }));
    expect(within(filters).getByRole('button', { name: 'Categoria 11' })).toBeDefined();
    expect(within(filters).getByRole('button', { name: 'Mostrar menos' })).toBeDefined();
  });

  it('trocar o tipo volta os filtros (as categorias são de cada tipo)', async () => {
    const { url } = setup({
      discover_categories: (args) =>
        args.kind === 'shader'
          ? [{ id: 'sh-realista', name: 'Realista', sources: ['modrinth'] }]
          : CATEGORIES,
    });
    await openAdd(url);
    await userEvent.click(await screen.findByRole('button', { name: 'Magia' }));
    await group('Magia');
    await userEvent.click(screen.getByRole('radio', { name: 'Shaders' }));
    expect(await screen.findByRole('button', { name: 'Realista' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Todas' }).getAttribute('aria-pressed')).toBe('true');
  });

  it('Limpar volta os filtros ao padrão', async () => {
    const { url, backend } = setup();
    await openAdd(url);
    const filters = await screen.findByRole('complementary', { name: 'Filtros' });
    const clear = within(filters).getByRole<HTMLButtonElement>('button', { name: 'Limpar' });
    expect(clear.disabled).toBe(true);
    await userEvent.click(within(filters).getByRole('radio', { name: 'Servidor' }));
    await waitFor(() => {
      expect(backend.callsOf('search_projects').at(-1)?.args.request).toMatchObject({
        environment: 'server',
      });
    });
    expect(clear.disabled).toBe(false);
    await userEvent.click(clear);
    expect(await group('Populares')).toBeDefined();
    expect(
      within(filters).getByRole('radio', { name: 'Qualquer' }).getAttribute('aria-checked'),
    ).toBe('true');
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
});
