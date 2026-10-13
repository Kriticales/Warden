/**
 * Console agrupado (D-09; CA-T13-12 na interface): o seletor "Mostrar" no console, os grupos
 * por mod com "Ver no pack" e "Copiar", o item "×1.284", o stack trace recolhido com a primeira
 * linha de mod, "Só problemas", a releitura com o jogo aberto e os nomes atualizados quando o
 * pack muda.
 */
import { QueryClientProvider, useQueryClient } from '@tanstack/react-query';
import {
  RouterProvider,
  createMemoryHistory,
  createRootRoute,
  createRouter,
} from '@tanstack/react-router';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { createQueryClient } from '../../../app/App';
import { Toaster } from '../../../components/ui/toast';
import { TooltipProvider } from '../../../components/ui/tooltip';
import type {
  ConsoleLine,
  GroupRow,
  GroupedRequest,
  GroupedView,
  Inventory,
  ItemView,
  PackId,
} from '../../../lib/ipc/bindings';
import { usePackChangedInvalidation } from '../../../lib/ipc/events';
import { axeComponent } from '../../../test/axe';
import { mockBackend, type MockBackend } from '../../../test/backend';
import { makeInventory, makeItem } from '../../pack-editor/editor.fixtures';
import { Console, type ConsoleProps } from '../console/Console';

const PACK = 'pack-console' as PackId;
const SESSION = '2026-10-13_18-02-00';

const SUPPLEMENTARIES = makeItem({
  path: 'mods/supplementaries.pw.toml',
  name: 'Supplementaries',
});

function item(overrides: Partial<ItemView> = {}): ItemView {
  return {
    id: 1,
    group: 'item:mods/supplementaries.pw.toml',
    severity: 'warning',
    level: 'warn',
    logger: 'net.mehvahdjukaar.supplementaries.Supplementaries',
    text: 'Missing texture for {id} at {n}',
    count: 1284,
    first: { atMs: null, time: '18:02:16' },
    last: { atMs: null, time: '18:04:51' },
    noise: null,
    trace: false,
    extraLines: 0,
    modFrame: null,
    ...overrides,
  };
}

const TRACE = item({
  id: 2,
  group: 'item:mods/epic-fight.pw.toml',
  severity: 'error',
  level: 'error',
  logger: 'net.minecraft.server.MinecraftServer',
  text: 'Encountered an unexpected exception',
  count: 1,
  first: { atMs: null, time: '18:03:00' },
  last: { atMs: null, time: '18:03:00' },
  trace: true,
  extraLines: 3,
  modFrame:
    'at yesman.epicfight.world.capabilities.entitypatch.LivingEntityPatch.onTravel(LivingEntityPatch.java:412) ~[epicfight-forge-20.9.4-1.20.1.jar%23154!/:20.9.4]',
});

const NOISE = item({
  id: 3,
  group: 'game',
  severity: 'warning',
  logger: 'net.minecraft.client.sounds.SoundEngine',
  text: 'Missing sound for event: {id}',
  count: 2,
  noise: 'sound.missing_event',
});

function row(key: string, overrides: Partial<GroupRow['group']>, items: ItemView[] = []): GroupRow {
  return {
    group: {
      key,
      kind: 'item',
      item: null,
      modId: null,
      entries: 0,
      errors: 0,
      warnings: 0,
      repeated: 0,
      items: items.length,
      ...overrides,
    },
    name: null,
    items,
  };
}

/** O backend agrupa; aqui ele devolve os grupos abertos pedidos. */
function groupedView(request: GroupedRequest, extra = 0): GroupedView {
  const open = new Set(request.open.map((group) => group.key));
  const supplementaries = row(
    'item:mods/supplementaries.pw.toml',
    {
      item: 'mods/supplementaries.pw.toml',
      modId: 'supplementaries',
      entries: 1284 + extra,
      warnings: 1284 + extra,
      repeated: 1284 + extra,
      items: 1,
    },
    open.has('item:mods/supplementaries.pw.toml') ? [item({ count: 1284 + extra })] : [],
  );
  supplementaries.name = 'Supplementaries (nome antigo)';
  const epic = row(
    'item:mods/epic-fight.pw.toml',
    { item: 'mods/epic-fight.pw.toml', modId: 'epicfight', entries: 1, errors: 1, items: 1 },
    open.has('item:mods/epic-fight.pw.toml') ? [TRACE] : [],
  );
  epic.name = 'Epic Fight';
  const game = row(
    'game',
    { kind: 'game', entries: 412, warnings: 2, repeated: 2, items: 300 },
    open.has('game') ? [NOISE] : [],
  );
  if (request.mode === 'problems') {
    return {
      lines: 1700,
      groups: [],
      problems: [TRACE, item({ count: 1284 + extra })],
      problemsTotal: 2,
      names: { 'item:mods/epic-fight.pw.toml': 'Epic Fight' },
    };
  }
  return {
    lines: 1700 + extra,
    groups: [supplementaries, epic, game],
    problems: [],
    problemsTotal: 0,
    names: {},
  };
}

const ORIGINAL: ConsoleLine = {
  seq: 1,
  atMs: null,
  time: '18:02:16',
  level: 'warn',
  logger: 'net.mehvahdjukaar.supplementaries.Supplementaries',
  thread: null,
  text: 'Missing texture for supplementaries:block/jar at 0',
  params: {},
  origin: 'game',
  stream: null,
};

function Invalidation() {
  usePackChangedInvalidation(useQueryClient());
  return null;
}

interface Setup {
  backend: MockBackend;
  /** Onde o roteador está (para conferir "Ver no pack"). */
  location: () => { pathname: string; search: unknown };
  setInventory: (inventory: Inventory) => void;
  setExtra: (extra: number) => void;
}

function renderGrouped(props: Partial<ConsoleProps> = {}, handlers = {}): Setup {
  let inventory = makeInventory([SUPPLEMENTARIES]);
  let extra = 0;
  const backend = mockBackend({
    inventory_list: () => inventory,
    console_grouped: (args) => groupedView(args.request as GroupedRequest, extra),
    console_grouped_lines: (args) => {
      const target = args.target as { type: string };
      if (target.type === 'group') {
        return [
          ORIGINAL,
          { ...ORIGINAL, seq: 2, text: 'Missing texture for supplementaries:item/rope at 1' },
        ];
      }
      return [
        {
          ...ORIGINAL,
          level: 'error',
          text: `Encountered an unexpected exception\njava.lang.NullPointerException: x\n\t${TRACE.modFrame ?? ''}\n\tat net.minecraft.world.entity.LivingEntity.aiStep(LivingEntity.java:2541)`,
        },
      ];
    },
    ...handlers,
  });
  const queryClient = createQueryClient();
  const root = createRootRoute({
    component: () => (
      <>
        <Invalidation />
        <Console
          lines={[]}
          state="ended"
          grouped={{ packId: PACK, sessionId: SESSION }}
          {...props}
        />
      </>
    ),
  });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: ['/'] }),
  });
  render(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <RouterProvider router={router} />
        <Toaster />
      </TooltipProvider>
    </QueryClientProvider>,
  );
  return {
    backend,
    location: () => router.state.location,
    setInventory: (next) => {
      inventory = next;
    },
    setExtra: (next) => {
      extra = next;
    },
  };
}

const showSelect = () => screen.findByRole('combobox', { name: 'Mostrar' });
const groupedRegion = () => screen.findByRole('region', { name: 'Linhas agrupadas por mod' });

/** O jsdom não mede: altura da lista (600 px) e de cada linha (20 px). */
beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains('console__log')) return 600;
    return this.classList.contains('cgrouped__row') ? 20 : 0;
  });
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.classList.contains('console__log') ? 1000 : 0;
  });
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('Console agrupado', () => {
  it('o modo linha a linha continua igual e sem o pedido ao backend', async () => {
    const { backend } = renderGrouped();
    const select = await showSelect();
    expect((select as HTMLSelectElement).value).toBe('lines');
    expect(screen.getByRole('combobox', { name: 'Nível das linhas' })).toBeDefined();
    expect(screen.getByRole('log', { name: 'Console do jogo' })).toBeDefined();
    expect(backend.callsOf('console_grouped')).toHaveLength(0);
  });

  it('agrupa por mod: nome do pack, contagens, ×1.284 e ruído conhecido', async () => {
    const { backend, location } = renderGrouped();
    fireEvent.change(await showSelect(), { target: { value: 'grouped' } });
    const region = await groupedRegion();
    // O nível some no agrupado (o seletor "Mostrar" decide o que aparece).
    expect(screen.queryByRole('combobox', { name: 'Nível das linhas' })).toBeNull();
    const toggle = await within(region).findByRole('button', {
      name: 'Mostrar as linhas de Supplementaries',
    });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(within(region).getByText('1.284 avisos')).toBeDefined();
    expect(within(region).getByText('1.284 repetidas')).toBeDefined();
    expect(within(region).getByText('Jogo e loader')).toBeDefined();
    // Sem o item no pack, nada de "Ver no pack" para o Epic Fight.
    expect(within(region).queryByRole('button', { name: 'Ver Epic Fight no pack' })).toBeNull();

    fireEvent.click(toggle);
    expect(await within(region).findByText('×1.284')).toBeDefined();
    expect(within(region).getByText('Missing texture for {id} at {n}')).toBeDefined();
    expect(within(region).getByText(/primeira às 18:02:16, última às 18:04:51/)).toBeDefined();
    const request = backend.callsOf('console_grouped').at(-1)?.args.request as GroupedRequest;
    expect(request).toEqual({
      mode: 'byMod',
      open: [{ key: 'item:mods/supplementaries.pw.toml', limit: 200 }],
      limit: 0,
      query: '',
    });

    fireEvent.click(
      within(region).getByRole('button', { name: 'Mostrar as linhas de Jogo e loader' }),
    );
    expect(await within(region).findByText('comum, geralmente inofensivo')).toBeDefined();

    fireEvent.click(within(region).getByRole('button', { name: 'Ver Supplementaries no pack' }));
    await waitFor(() => {
      expect(location().pathname).toBe(`/packs/${PACK}/mods`);
    });
    expect(location().search).toEqual({ item: 'mods/supplementaries.pw.toml' });
  });

  it('stack trace recolhido com a primeira linha de mod; abrir lê as linhas originais', async () => {
    const { backend } = renderGrouped();
    fireEvent.change(await showSelect(), { target: { value: 'grouped' } });
    const region = await groupedRegion();
    fireEvent.click(
      await within(region).findByRole('button', { name: 'Mostrar as linhas de Epic Fight' }),
    );
    const highlighted = (await within(region).findByText(/yesman\.epicfight/)).closest(
      '.cstack__f--mod',
    );
    expect(highlighted).not.toBeNull();
    expect(within(highlighted as HTMLElement).getByText('Primeira linha de mod:')).toBeDefined();
    expect(within(region).queryByText(/LivingEntity\.aiStep/)).toBeNull();
    expect(backend.callsOf('console_grouped_lines')).toHaveLength(0);

    const open = within(region).getByRole('button', { name: 'stack trace, mais 3 linhas' });
    fireEvent.click(open);
    expect(open.getAttribute('aria-expanded')).toBe('true');
    expect(await within(region).findByText(/LivingEntity\.aiStep/)).toBeDefined();
    expect(backend.callsOf('console_grouped_lines')[0]?.args).toMatchObject({
      sessionId: SESSION,
      target: { type: 'item', id: 2 },
      limit: 1,
    });
  });

  it('copiar um grupo copia as linhas originais', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.assign(navigator, { clipboard: { writeText } });
    renderGrouped();
    fireEvent.change(await showSelect(), { target: { value: 'grouped' } });
    const region = await groupedRegion();
    fireEvent.click(
      await within(region).findByRole('button', { name: 'Copiar as linhas de Supplementaries' }),
    );
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(
        '18:02:16 [WARN] [net.mehvahdjukaar.supplementaries.Supplementaries] Missing texture for supplementaries:block/jar at 0\n' +
          '18:02:16 [WARN] [net.mehvahdjukaar.supplementaries.Supplementaries] Missing texture for supplementaries:item/rope at 1',
      );
    });
    expect(await screen.findByText('2 linhas copiadas.')).toBeDefined();
  });

  it('só problemas: a lista vem do backend com o nome do mod de cada item', async () => {
    const { backend } = renderGrouped();
    fireEvent.change(await showSelect(), { target: { value: 'problems' } });
    const region = await screen.findByRole('region', {
      name: 'Erros, avisos e stack traces do console',
    });
    expect(await within(region).findByText('Encountered an unexpected exception')).toBeDefined();
    expect(within(region).getByText('Epic Fight')).toBeDefined();
    expect(within(region).getByText('Supplementaries')).toBeDefined();
    const request = backend.callsOf('console_grouped').at(-1)?.args.request as GroupedRequest;
    expect(request.mode).toBe('problems');
    expect(request.limit).toBe(200);
  });

  it('a busca do console vai para o backend', async () => {
    const { backend } = renderGrouped();
    fireEvent.change(await showSelect(), { target: { value: 'grouped' } });
    await groupedRegion();
    fireEvent.change(screen.getByRole('searchbox', { name: 'Buscar no console' }), {
      target: { value: 'texture' },
    });
    await waitFor(() => {
      const request = backend.callsOf('console_grouped').at(-1)?.args.request as GroupedRequest;
      expect(request.query).toBe('texture');
    });
  });

  it('com o jogo aberto, relê os grupos e as contagens mudam', async () => {
    const setup = renderGrouped({ state: 'live' });
    fireEvent.change(await showSelect(), { target: { value: 'grouped' } });
    const region = await groupedRegion();
    expect(await within(region).findByText('1.284 avisos')).toBeDefined();
    setup.setExtra(16);
    expect(await within(region).findByText('1.300 avisos', {}, { timeout: 3000 })).toBeDefined();
  });

  it('o nome e o link do mod acompanham o pack (pack-changed)', async () => {
    const setup = renderGrouped();
    fireEvent.change(await showSelect(), { target: { value: 'grouped' } });
    const region = await groupedRegion();
    expect(
      await within(region).findByRole('button', { name: 'Ver Supplementaries no pack' }),
    ).toBeDefined();

    setup.setInventory(makeInventory([{ ...SUPPLEMENTARIES, name: 'Supplementaries Plus' }]));
    await act(() => setup.backend.emit('pack-changed', { packId: PACK, areas: ['inventory'] }));
    expect(
      await within(region).findByRole('button', {
        name: 'Mostrar as linhas de Supplementaries Plus',
      }),
    ).toBeDefined();

    // Item removido do pack: o grupo fica (é o log), com o nome gravado e sem o link.
    setup.setInventory(makeInventory([]));
    await act(() => setup.backend.emit('pack-changed', { packId: PACK, areas: ['inventory'] }));
    expect(
      await within(region).findByRole('button', {
        name: 'Mostrar as linhas de Supplementaries (nome antigo)',
      }),
    ).toBeDefined();
    expect(within(region).queryByRole('button', { name: /no pack$/ })).toBeNull();
  });

  it('esperando o jogo abrir, sem sessão', async () => {
    renderGrouped({ state: 'waiting', grouped: { packId: PACK, sessionId: null } });
    fireEvent.change(await showSelect(), { target: { value: 'grouped' } });
    expect(
      await screen.findByText(
        'Esperando o jogo abrir. As linhas aparecem agrupadas assim que ele começar.',
      ),
    ).toBeDefined();
  });

  it('sem violações de acessibilidade, com grupo e stack trace abertos', async () => {
    renderGrouped();
    fireEvent.change(await showSelect(), { target: { value: 'grouped' } });
    const region = await groupedRegion();
    fireEvent.click(
      await within(region).findByRole('button', { name: 'Mostrar as linhas de Epic Fight' }),
    );
    fireEvent.click(
      await within(region).findByRole('button', { name: 'stack trace, mais 3 linhas' }),
    );
    await within(region).findByText(/LivingEntity\.aiStep/);
    expect(await axeComponent(document.body)).toHaveNoViolations();
  });
});
