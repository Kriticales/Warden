/**
 * Modo Grafo da seção Mods (SPEC T06; CA-T06-07; D-11), com o backend simulado: o Create no
 * centro, as dependências à esquerda (com as embutidas dentro do nó), os dependentes à
 * direita, o tipo de cada ligação na legenda e na lista em texto, teclado, `axe`, "Ver no
 * grafo" nos detalhes, profundidade, busca do centro, dados atualizados por `pack-changed` e
 * "Ver o pack inteiro".
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type {
  FocusEdge,
  FocusNode,
  FocusReport,
  InventoryItem,
  RelationKind,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { makeDetails, makeInventory, makeItem } from '../editor.fixtures';
import { editorBackend } from '../testing';

// O canvas do Cytoscape não existe no jsdom: o desenho do pack inteiro vira um marcador.
vi.mock('./WholeGraphCanvas', () => ({
  default: ({ label }: { label: string }) => <div role="img" aria-label={label} />,
}));

const CREATE = makeItem({ name: 'Create' });
const JEI = makeItem({ name: 'JEI' });
const SLICE = makeItem({ name: 'Slice and Dice' });
const CRAFTS = makeItem({ name: 'Crafts and Additions' });
const RIVAL = makeItem({ name: 'Rival' });
const KOTLIN = makeItem({ name: 'Kotlin for Forge' });
const ITEMS = [CREATE, JEI, SLICE, CRAFTS, RIVAL, KOTLIN];

function node(item: InventoryItem, level: number, extra: Partial<FocusNode> = {}): FocusNode {
  return {
    key: item.path,
    item: { path: item.path, name: item.name },
    label: item.name,
    level,
    embedded: [],
    library: false,
    ...extra,
  };
}

function edge(from: string, to: string, kind: RelationKind, extra: Partial<FocusEdge> = {}) {
  return { from, to, kind, range: null, embedded: null, note: null, ...extra };
}

const MISSING: FocusNode = {
  key: 'falta:ponder',
  item: null,
  label: 'ponder',
  level: -1,
  embedded: [],
  library: false,
};

function createReport(center: InventoryItem = CREATE): FocusReport {
  return {
    center: { path: center.path, name: center.name },
    depth: 1,
    nodes: [
      node(JEI, -1),
      MISSING,
      node(CREATE, 0, { embedded: ['flywheel', 'registrate'] }),
      node(CRAFTS, 1),
      node(RIVAL, 1),
      node(SLICE, 1),
    ],
    edges: [
      edge(CREATE.path, JEI.path, 'optional'),
      edge(CREATE.path, MISSING.key, 'required'),
      edge(CRAFTS.path, CREATE.path, 'required'),
      edge(RIVAL.path, CREATE.path, 'breaks'),
      edge(SLICE.path, CREATE.path, 'inferred', { note: 'o log pediu o Create' }),
    ],
    totalItems: ITEMS.length,
  };
}

function backendFor(focus: (args: Record<string, unknown>) => FocusReport = () => createReport()) {
  return editorBackend({
    inventory: makeInventory(ITEMS),
    handlers: {
      item_details: (args) => makeDetails(ITEMS.find((i) => i.path === args.path) ?? CREATE),
      graph_focus: (args) => (args.path === null ? wholeReport(ITEMS.length) : focus(args)),
      graph_orphans: () => [{ item: { path: KOTLIN.path, name: KOTLIN.name }, optionalUsers: [] }],
      graph_dependents: () => ({
        targets: [],
        dependsOn: [],
        usedBy: [],
        affected: [{ item: { path: CRAFTS.path, name: CRAFTS.name }, depth: 1, needs: [] }],
      }),
      graph_why_in_pack: () => ({
        item: { path: CREATE.path, name: CREATE.name },
        why: { kind: 'noDependents' },
        cycle: [],
      }),
    },
  });
}

function wholeReport(count: number): FocusReport {
  const nodes = Array.from({ length: count }, (_, index) => {
    const item = ITEMS[index];
    return item
      ? node(item, 0, { library: item === KOTLIN })
      : { ...node(CREATE, 0), key: `mods/m${String(index)}.pw.toml`, label: `M${String(index)}` };
  });
  return {
    center: null,
    depth: 0,
    nodes,
    edges: [edge(CRAFTS.path, CREATE.path, 'required')],
    totalItems: count,
  };
}

async function openGraph(url: string, centro = CREATE.path) {
  renderApp(`${url}/mods?ver=grafo&centro=${encodeURIComponent(centro)}`);
  return screen.findByRole('group', { name: /^Grafo de dependências de / }, { timeout: 10_000 });
}

describe('modo Grafo de Mods (D-11)', () => {
  it('CA-T06-07: Create no centro, dependências à esquerda e dependentes à direita; axe', async () => {
    const { url, backend } = backendFor();
    const graph = await openGraph(url);
    expect(graph.getAttribute('aria-label')).toBe('Grafo de dependências de Create');
    expect(backend.callsOf('graph_focus')[0]?.args).toMatchObject({
      path: CREATE.path,
      depth: 1,
    });

    // Colunas: à esquerda o que o Create exige, à direita quem depende dele.
    const [left, right] = within(graph)
      .getAllByRole('list')
      .filter((list) => list.classList.contains('gcol'));
    expect(left?.textContent).toContain('JEI');
    expect(left?.textContent).toContain('ponder');
    expect(right?.textContent).toContain('Crafts and Additions');
    expect(right?.textContent).toContain('Slice and Dice');
    expect(right?.textContent).toContain('Rival');
    // As embutidas ficam dentro do nó do Create.
    const center = within(graph).getByRole('button', { name: /^Create/ });
    expect(center.textContent).toContain('Dentro dele: flywheel e registrate');
    // Tipo de cada ligação no nó e na legenda.
    expect(within(graph).getByRole('button', { name: /^JEI/ }).textContent).toContain('opcional');
    expect(within(graph).getByRole('button', { name: /^Rival/ }).textContent).toContain(
      'incompatível',
    );
    const legend = within(graph).getByRole('list', { name: 'Tipos de ligação' });
    for (const kind of ['Obrigatória', 'Opcional ou recomendada', 'Incompatível', 'Inferida']) {
      expect(legend.textContent).toContain(kind);
    }
    // E na lista em texto, uma linha por ligação com o tipo.
    const links = screen.getByRole('region', { name: '5 ligações' });
    const lines = within(links)
      .getAllByRole('listitem')
      .map((item) => item.textContent);
    expect(lines).toEqual([
      'Create usa JEI se estiver no pack · opcional',
      'Create precisa de ponder · obrigatória · não está no pack',
      'Crafts and Additions precisa de Create · obrigatória',
      'Rival é incompatível com Create · incompatível',
      'Slice and Dice parece precisar de Create · inferidao log pediu o Create',
    ]);
    // Em palavras.
    const words = screen.getByRole('region', { name: 'Em palavras' });
    expect(words.textContent).toContain('Create precisa de: ponder (não está no pack)');
    expect(words.textContent).toContain('Vêm dentro do próprio Create: flywheel e registrate');
    expect(words.textContent).toContain(
      'Precisam do Create: Crafts and Additions e Slice and Dice',
    );
    expect(
      await within(words).findByText(
        'Se você remover o Create, 1 mod para de funcionar: Crafts and Additions',
      ),
    ).toBeDefined();
    expect(words.textContent).toContain('Você adicionou: nenhum outro item o exige');
    // Bibliotecas sem uso.
    expect(await screen.findByText('1 biblioteca sem uso: Kotlin for Forge.')).toBeDefined();

    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T06-07: todos os nós do pack são alcançáveis por teclado e abrem os detalhes', async () => {
    const { url } = backendFor();
    const graph = await openGraph(url);
    const user = userEvent.setup();
    const buttons = within(graph).getAllByRole('button');
    // JEI, Create, Crafts, Rival, Slice: os 5 itens do pack (ponder falta: é texto).
    expect(
      buttons.map((button) => button.querySelector('.gnode__name, .gcenter__name')?.textContent),
    ).toEqual(['JEI', 'Create', 'Crafts and Additions', 'Rival', 'Slice and Dice']);
    expect(within(graph).getByText('ponder').closest('button')).toBeNull();

    buttons[0]?.focus();
    const reached: string[] = [];
    for (const button of buttons) {
      expect(document.activeElement).toBe(button);
      reached.push(button.textContent);
      await user.tab();
    }
    expect(reached).toHaveLength(5);

    // Enter num nó abre os detalhes dele.
    within(graph)
      .getByRole('button', { name: /^Rival/ })
      .focus();
    await user.keyboard('{Enter}');
    const drawer = await screen.findByRole('dialog', { name: 'Detalhes do mod' });
    expect(await within(drawer).findByText('Rival', { selector: '.itemhead__name' })).toBeDefined();
  });

  it('"Ver no grafo" nos detalhes põe o mod no centro', async () => {
    const { url, backend } = backendFor((args) =>
      createReport(ITEMS.find((item) => item.path === args.path) ?? CREATE),
    );
    renderApp(`${url}/mods`);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'JEI' }, { timeout: 10_000 }));
    const drawer = await screen.findByRole('dialog', { name: 'Detalhes do mod' });
    await user.click(await within(drawer).findByRole('link', { name: 'Ver no grafo' }));
    expect(
      await screen.findByRole('group', { name: 'Grafo de dependências de JEI' }),
    ).toBeDefined();
    expect(backend.callsOf('graph_focus').at(-1)?.args).toMatchObject({ path: JEI.path });
    expect(screen.getByRole('radio', { name: 'Grafo' }).getAttribute('aria-checked')).toBe('true');
  });

  it('"Ver como" troca de modo por clique e pelas setas, sem perder o foco', async () => {
    const { url } = backendFor();
    renderApp(`${url}/mods`);
    const user = userEvent.setup();
    const graphRadio = await screen.findByRole('radio', { name: 'Grafo' }, { timeout: 10_000 });
    await user.click(graphRadio);
    expect(await screen.findByRole('group', { name: /^Grafo de dependências/ })).toBeDefined();
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole('radio', { name: 'Grafo' }));
    });
    await user.keyboard('{ArrowLeft}');
    expect(await screen.findByRole('searchbox', { name: 'Buscar no pack' })).toBeDefined();
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole('radio', { name: 'Lista' }));
    });
  });

  it('profundidade e busca do mod no centro', async () => {
    const { url, backend } = backendFor((args) =>
      createReport(ITEMS.find((item) => item.path === args.path) ?? CREATE),
    );
    await openGraph(url);
    const user = userEvent.setup();
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Profundidade' }),
      '2 níveis de cada lado',
    );
    await waitFor(() => {
      expect(backend.callsOf('graph_focus').at(-1)?.args).toMatchObject({ depth: 2 });
    });

    await user.type(screen.getByRole('searchbox', { name: 'Buscar mod para o centro' }), 'sli');
    const select = screen.getByRole('combobox', { name: 'Mod no centro' });
    const options = within(select)
      .getAllByRole('option')
      .map((option) => option.textContent);
    expect(options).toEqual(['Mod no centro', 'Slice and Dice']);
    await user.selectOptions(select, 'Slice and Dice');
    expect(
      await screen.findByRole('group', { name: 'Grafo de dependências de Slice and Dice' }),
    ).toBeDefined();
  });

  it('pack-changed na lista atualiza o grafo, a explicação e as bibliotecas sem uso', async () => {
    let report = createReport();
    let orphans = [{ item: { path: KOTLIN.path, name: KOTLIN.name }, optionalUsers: [] }];
    const { url, backend, row } = editorBackend({
      inventory: makeInventory(ITEMS),
      handlers: {
        graph_focus: () => report,
        graph_orphans: () => orphans,
      },
    });
    await openGraph(url);
    expect(await screen.findByText('1 biblioteca sem uso: Kotlin for Forge.')).toBeDefined();
    expect(screen.getByRole('region', { name: '5 ligações' })).toBeDefined();

    // Removeram o Rival e a biblioteca: o Rust avisa que a lista mudou.
    report = {
      ...report,
      nodes: report.nodes.filter((n) => n.key !== RIVAL.path),
      edges: report.edges.filter((e) => e.from !== RIVAL.path),
    };
    orphans = [];
    const before = backend.callsOf('graph_focus').length;
    await act(() => backend.emit('pack-changed', { packId: row.id, areas: ['inventory'] }));
    expect(await screen.findByRole('region', { name: '4 ligações' })).toBeDefined();
    await waitFor(() => {
      expect(screen.queryByText(/biblioteca sem uso/)).toBeNull();
    });
    expect(screen.queryByRole('button', { name: /^Rival/ })).toBeNull();
    expect(backend.callsOf('graph_focus').length).toBeGreaterThan(before);
  });

  it('erro do backend aparece com "Tentar de novo"', async () => {
    let fail = true;
    const { url } = editorBackend({
      inventory: makeInventory(ITEMS),
      handlers: {
        graph_orphans: () => [],
        graph_focus: () => {
          if (fail) ipcError(makeAppError({ domain: 'project', code: 'ITEM_NOT_FOUND' }));
          return createReport();
        },
      },
    });
    renderApp(`${url}/mods?ver=grafo`);
    const retry = await screen.findByRole('button', { name: /Tentar/ }, { timeout: 10_000 });
    fail = false;
    await userEvent.setup().click(retry);
    expect(await screen.findByRole('group', { name: /^Grafo de dependências/ })).toBeDefined();
  });

  it('"Ver o pack inteiro" mostra o desenho e a lista em texto; o foco volta ao fechar', async () => {
    const { url } = backendFor();
    await openGraph(url);
    const user = userEvent.setup();
    const open = screen.getByRole('button', { name: 'Ver o pack inteiro' });
    await user.click(open);
    const dialog = await screen.findByRole('dialog', { name: 'O pack inteiro' });
    expect(await within(dialog).findByText('6 mods e 1 ligação.')).toBeDefined();
    expect(
      await within(dialog).findByRole('img', { name: /^Desenho do pack inteiro/ }),
    ).toBeDefined();
    const list = within(dialog).getByRole('region', { name: 'Lista em texto' });
    expect(list.textContent).toContain('Crafts and Additions precisa de Create');
    expect(list.textContent).toContain('Kotlin for Forge sem ligações · biblioteca');
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Fechar' }));
    await waitFor(() => {
      expect(document.activeElement).toBe(open);
    });
  });

  it('acima de 150 mods avisa antes de desenhar', async () => {
    const { url } = editorBackend({
      inventory: makeInventory(ITEMS),
      handlers: {
        graph_focus: (args) => (args.path === null ? wholeReport(151) : createReport()),
        graph_orphans: () => [],
      },
    });
    await openGraph(url);
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Ver o pack inteiro' }));
    const dialog = await screen.findByRole('dialog', { name: 'O pack inteiro' });
    expect(
      await within(dialog).findByText(/^Este pack tem 151 mods\. Com mais de 150/),
    ).toBeDefined();
    expect(within(dialog).queryByRole('img')).toBeNull();
    await user.click(within(dialog).getByRole('button', { name: 'Desenhar assim mesmo' }));
    expect(await within(dialog).findByRole('img')).toBeDefined();
  });
});
