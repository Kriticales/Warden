/**
 * Busca do culpado com o servidor local (SPEC T25 e T27; ROADMAP D-13; CA-T25-06) no app
 * inteiro com o backend simulado: o pedido no diálogo (por que, a memória, a EULA, "Entendi,
 * pode abrir"; sem o aceite, o modo assistido), a entrada pelo "Testar como servidor" que travou
 * no servidor, o servidor de cada rodada na tela da busca, as rodadas que travaram nele, o log
 * dos dois lados e o resultado. Também as regras puras (`need.ts`, `origin.ts`).
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
  BisectView,
  ServerPlan,
  ServerSide,
  ServerState,
  Settings,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { deferred, type Handler } from '../../../test/backend';
import { renderApp } from '../../../test/render';
import { makeCrashList } from '../../diagnostics/crashes/crashes.fixtures';
import { editorBackend } from '../../pack-editor/testing';
import { makeSettings } from '../../settings/testing';
import { useTestStore } from '../../test/store';
import { gameLine, makeGame, makeSession, makeSessionView } from '../../test/test.fixtures';
import { makeFound, makePlan, makeRound, makeView } from '../bisect.fixtures';
import { serverReason, withServer } from './need';
import { serverCrashOrigin } from './origin';

beforeEach(() => {
  // O console virtual mede as linhas.
  vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains('console__log')) return 600;
    return this.classList.contains('console__line') ? 20 : 0;
  });
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.classList.contains('console__log') ? 1000 : 0;
  });
});

afterEach(() => {
  useTestStore.setState({ game: null, runs: {}, packChanged: {} });
  vi.restoreAllMocks();
});

function makeServerPlan(overrides: Partial<ServerPlan> = {}): ServerPlan {
  return {
    memory: { serverMb: 4096, gameMb: 6144, totalMb: 10_240, freeMb: 13_312, fits: true },
    clientOnly: ['Embeddium', 'Oculus'],
    clientOnlyCount: 2,
    eulaAcceptedAt: '2026-10-01T17:05:09Z',
    loaderSupported: true,
    ...overrides,
  };
}

/** O pack aberto, com o plano da busca de um Forge 1.16.5 e a EULA guardada nas Configurações. */
function setup(
  options: {
    plan?: Partial<ServerPlan>;
    settings?: Partial<Settings>;
    handlers?: Record<string, Handler>;
  } = {},
) {
  // A EULA já aceita, salvo quando o teste diz o contrário.
  const state = {
    settings: makeSettings({ eulaAcceptedAt: '2026-10-01T17:05:09Z', ...options.settings }),
  };
  const setupResult = editorBackend({
    handlers: {
      settings_get: () => state.settings,
      bisect_plan: () => makePlan({ minecraft: '1.16.5', quickPlay: false }),
      bisect_get: () => null,
      server_plan: () =>
        makeServerPlan({ eulaAcceptedAt: state.settings.eulaAcceptedAt ?? null, ...options.plan }),
      server_eula_accept: () => {
        state.settings = { ...state.settings, eulaAcceptedAt: '2026-10-18T12:00:00Z' };
        return state.settings;
      },
      server_current: () => null,
      ...options.handlers,
    },
  });
  return { ...setupResult, state };
}

async function openPage(url: string, suffix = '', title: string | RegExp = 'Mods') {
  renderApp(`${url}${suffix}`);
  await screen.findByRole('heading', { level: 1, name: title }, { timeout: 10_000 });
}

function header() {
  return screen.getByRole('banner', { name: 'Pack aberto' });
}

async function openBisectDialog(user: ReturnType<typeof userEvent.setup>) {
  await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
  await user.click(
    within(await screen.findByRole('menu')).getByRole('menuitem', {
      name: /Encontrar o mod culpado…/,
    }),
  );
  return screen.findByRole('dialog', { name: 'Encontrar o mod culpado' });
}

describe('pedido do servidor local no diálogo (CA-T25-06)', () => {
  it('num Forge 1.16.5, "Trava ao entrar no mundo" explica, mostra a memória, pede a EULA e só abre o servidor com o aceite', async () => {
    const { backend, url } = setup({
      settings: { eulaAcceptedAt: null },
      handlers: { bisect_start: () => deferred<BisectView>().promise },
    });
    await openPage(url);
    const user = userEvent.setup();
    const dialog = await openBisectDialog(user);
    const world = await within(dialog).findByRole('radio', { name: /Trava ao entrar no mundo/ });
    expect(world).toHaveProperty('disabled', false);
    expect(
      within(dialog).getByText(
        'No Minecraft 1.16.5 o jogo não entra sozinho num mundo: em cada rodada o Warden abre um servidor neste computador, depois de pedir.',
      ),
    ).toBeDefined();
    // Sem escolher "ao entrar no mundo", nada de servidor.
    expect(
      within(dialog).queryByText(
        'Para este problema, o Warden precisa abrir um servidor neste computador.',
      ),
    ).toBeNull();
    await user.click(world);
    expect(
      await within(dialog).findByText(
        'Para este problema, o Warden precisa abrir um servidor neste computador.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByText(/No Minecraft 1\.16\.5 o jogo não entra direto/)).toBeDefined();
    expect(within(dialog).getByText('servidor 4 GB + jogo 6 GB = 10 GB')).toBeDefined();
    expect(within(dialog).getByText('13 GB')).toBeDefined();
    expect(within(dialog).getByText('2 mods só de cliente ficam fora do servidor.')).toBeDefined();
    expect(within(dialog).getByRole('button', { name: /aka\.ms\/MinecraftEULA/ })).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();

    // A EULA primeiro; o foco fica no mesmo botão a cada passo.
    const action = within(dialog).getByRole('button', { name: 'Aceito a EULA' });
    await user.click(action);
    await waitFor(() => {
      expect(backend.callsOf('server_eula_accept')).toHaveLength(1);
    });
    const allow = await within(dialog).findByRole('button', { name: 'Entendi, pode abrir' });
    expect(allow).toBe(action);
    expect(document.activeElement).toBe(allow);
    expect(backend.callsOf('bisect_start')).toHaveLength(0);
    await user.click(allow);
    expect(within(dialog).getByText('Combinado: cada rodada abre o servidor local.')).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Não abrir o servidor' })).toBe(action);
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Começar a busca' }));
    await waitFor(() => {
      expect(backend.callsOf('bisect_start')).toHaveLength(1);
    });
    expect(backend.callsOf('bisect_start')[0]?.args.request).toEqual({
      symptom: { kind: 'crashOnWorld' },
      originCrash: null,
      suspects: [],
      server: true,
    });
  });

  it('sem o aceite, a busca usa o modo assistido e não abre o servidor', async () => {
    const { backend, url } = setup({
      handlers: { bisect_start: () => deferred<BisectView>().promise },
    });
    await openPage(url);
    const user = userEvent.setup();
    const dialog = await openBisectDialog(user);
    await user.click(
      await within(dialog).findByRole('radio', { name: /Trava ao entrar no mundo/ }),
    );
    expect(
      await within(dialog).findByText(/Sem o servidor, a busca usa o modo em que você diz/),
    ).toBeDefined();
    // Aceitar e desfazer volta ao assistido.
    await user.click(within(dialog).getByRole('button', { name: 'Entendi, pode abrir' }));
    await user.click(within(dialog).getByRole('button', { name: 'Não abrir o servidor' }));
    await user.click(within(dialog).getByRole('button', { name: 'Começar a busca' }));
    await waitFor(() => {
      expect(backend.callsOf('bisect_start')).toHaveLength(1);
    });
    expect(backend.callsOf('bisect_start')[0]?.args.request).toMatchObject({
      symptom: { kind: 'assisted' },
      server: false,
    });
  });

  it('com pouca memória livre, avisa antes de abrir; com o aceite, o mundo de teste é criado antes', async () => {
    const { backend, url } = setup({
      plan: {
        memory: { serverMb: 4096, gameMb: 6144, totalMb: 10_240, freeMb: 7168, fits: false },
      },
      handlers: {
        bisect_plan: () => makePlan({ minecraft: '1.12.2', quickPlay: false, worldReady: false }),
      },
    });
    await openPage(url);
    const user = userEvent.setup();
    const dialog = await openBisectDialog(user);
    await user.click(
      await within(dialog).findByRole('radio', { name: /Trava ao entrar no mundo/ }),
    );
    expect(await within(dialog).findByText('Pode faltar memória.')).toBeDefined();
    expect(
      within(dialog).getByText(/Com 7 GB livres, o computador pode ficar lento/),
    ).toBeDefined();
    // O servidor abre uma cópia do mesmo mundo de teste da D-12, criado antes da 1ª rodada.
    const creates =
      /Antes da primeira rodada, o Warden cria o “Mundo de teste” com o servidor do Minecraft 1\.12\.2/;
    expect(within(dialog).queryByText(creates)).toBeNull();
    await user.click(within(dialog).getByRole('button', { name: 'Entendi, pode abrir' }));
    expect(within(dialog).getByText(creates)).toBeDefined();
    expect(backend.callsOf('bisect_start')).toHaveLength(0);
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('num loader sem servidor local, explica e não oferece abrir', async () => {
    const { url } = setup({ plan: { loaderSupported: false } });
    await openPage(url);
    const user = userEvent.setup();
    const dialog = await openBisectDialog(user);
    await user.click(
      await within(dialog).findByRole('radio', { name: /Trava ao entrar no mundo/ }),
    );
    expect(
      await within(dialog).findByText(
        'O servidor local ainda não abre packs com Quilt ou LiteLoader.',
      ),
    ).toBeDefined();
    expect(within(dialog).queryByRole('button', { name: 'Entendi, pode abrir' })).toBeNull();
  });

  it('a EULA aceita na busca vale para o "Testar como servidor" (as contas do servidor são relidas)', async () => {
    const { url } = setup({ settings: { eulaAcceptedAt: null } });
    await openPage(url);
    const user = userEvent.setup();
    const dialog = await openBisectDialog(user);
    await user.click(
      await within(dialog).findByRole('radio', { name: /Trava ao entrar no mundo/ }),
    );
    await user.click(await within(dialog).findByRole('button', { name: 'Aceito a EULA' }));
    await within(dialog).findByRole('button', { name: 'Entendi, pode abrir' });
    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    await user.click(
      await within(await screen.findByRole('menu')).findByRole('menuitem', {
        name: /Testar como servidor…/,
      }),
    );
    expect(await screen.findByRole('dialog', { name: 'Testar como servidor' })).toBeDefined();
    expect(screen.queryByRole('dialog', { name: 'Aceitar a EULA do Minecraft?' })).toBeNull();
  });
});

function makeSide(overrides: Partial<ServerSide> = {}): ServerSide {
  return {
    only: false,
    outcome: 'crashed',
    exitCode: 1,
    durationMs: 9000,
    readyMs: null,
    port: 25_566,
    memoryMb: 4096,
    game: 'notOpened',
    stopForced: false,
    crash: null,
    ...overrides,
  };
}

describe('entrada pelo "Testar como servidor" que travou no servidor', () => {
  it('a oferta pede o servidor local, com "trava ao abrir" quando ele caiu antes do Done', async () => {
    const session = makeSession({ mode: 'server', outcome: 'crashed', server: makeSide() });
    const { backend, url } = setup({
      handlers: {
        bisect_plan: () => makePlan(),
        test_sessions_list: () => [session],
        test_session_get: () => makeSessionView(session, []),
        diagnostics_crash_get: () => null,
        crashes_list: () => makeCrashList({ groups: [] }),
        bisect_start: () => deferred<BisectView>().promise,
      },
    });
    await openPage(url, '/teste', /travou/i);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Encontrar o mod culpado…' }));
    const dialog = await screen.findByRole('dialog', { name: 'Encontrar o mod culpado' });
    expect(within(dialog).getByRole('radio', { name: /Trava ao abrir o jogo/ })).toHaveProperty(
      'checked',
      true,
    );
    expect(await within(dialog).findByText(/Este travamento foi no servidor\./)).toBeDefined();
    await user.click(await within(dialog).findByRole('button', { name: 'Entendi, pode abrir' }));
    await user.click(within(dialog).getByRole('button', { name: 'Começar a busca' }));
    await waitFor(() => {
      expect(backend.callsOf('bisect_start')[0]?.args.request).toMatchObject({
        symptom: { kind: 'crashOnOpen' },
        server: true,
      });
    });
  });
});

describe('tela da busca com o servidor local', () => {
  it('"O que o Warden está fazendo agora" mostra o servidor da rodada', async () => {
    const ready: ServerState = {
      packId: 'qualquer',
      phase: 'ready',
      port: 25_566,
      startedAtMs: Date.now() - 20_000,
      readyMs: 14_000,
    };
    const { row, url } = setup({
      handlers: {
        bisect_get: () => makeView({ server: true }),
        server_current: () => ({ ...ready, packId: row.id }),
      },
    });
    useTestStore.setState({ game: makeGame(row.id, 'bisecting') });
    await openPage(url, '/teste?modo=busca', 'Buscando o mod culpado');
    expect(
      await screen.findByText(
        'Servidor local pronto em 127.0.0.1, porta 25566; o jogo entra nele.',
      ),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('as rodadas que travaram no servidor; "Ver log" com os dois lados; o resultado', async () => {
    const rounds = [
      makeRound(0, {
        kind: { kind: 'reproduce', attempt: 1 },
        verdict: { kind: 'failedSame' },
        anchor: 'joinWorld',
        hasServerLog: true,
        serverFailed: true,
      }),
      makeRound(1, { kind: { kind: 'control' }, hasServerLog: true }),
      // O servidor travou antes do "Done": o jogo nem abriu.
      makeRound(2, {
        verdict: { kind: 'failedSame' },
        hasLog: false,
        hasServerLog: true,
        serverFailed: true,
      }),
    ];
    const found = makeFound({ server: true, rounds });
    const { backend, url } = setup({
      handlers: {
        bisect_get: () => found,
        bisect_round_log: () => [
          gameLine('Connecting to 127.0.0.1, 25566'),
          gameLine("Couldn't place player in world", {
            origin: 'server',
            thread: 'Server thread',
            level: 'error',
          }),
        ],
      },
    });
    await openPage(url, '/teste?modo=busca', /Culpado encontrado/);
    expect(
      screen.getByText(/2 rodadas travaram no servidor; o log de cada rodada tem os dois lados\./),
    ).toBeDefined();
    const user = userEvent.setup();
    await user.click(screen.getByText('Ver as 3 rodadas'));
    const table = screen.getByRole('table', { name: 'Rodadas da busca do culpado' });
    expect(within(table).getAllByText('· no servidor')).toHaveLength(2);
    // A rodada 2 só tem o log do servidor e mesmo assim tem "Ver log".
    await user.click(within(table).getByRole('button', { name: /rodada 2/ }));
    const log = await screen.findByRole('dialog', { name: /rodada 2/ });
    const shown = await within(log).findByRole('combobox', { name: 'Mostrando' });
    expect(within(log).getByText('Connecting to 127.0.0.1, 25566')).toBeDefined();
    expect(within(log).queryByText("Couldn't place player in world")).toBeNull();
    await user.selectOptions(shown, 'server');
    expect(await within(log).findByText("Couldn't place player in world")).toBeDefined();
    expect(within(log).queryByText('Connecting to 127.0.0.1, 25566')).toBeNull();
    expect(backend.callsOf('bisect_round_log')[0]?.args.round).toBe(2);
    expect(await axePage(document.body)).toHaveNoViolations();
  });
});

describe('regras', () => {
  it('quando precisa do servidor e o que o pedido leva', () => {
    expect(serverReason({ quickPlay: false }, 'crashOnWorld', false)).toBe('version');
    expect(serverReason({ quickPlay: false }, 'crashOnOpen', false)).toBeNull();
    expect(serverReason({ quickPlay: true }, 'crashOnWorld', false)).toBeNull();
    expect(serverReason({ quickPlay: true }, 'logLine', true)).toBe('server');
    const symptom = { kind: 'crashOnWorld' } as const;
    expect(withServer(symptom, null, true)).toEqual({ symptom, server: false });
    expect(withServer(symptom, 'version', true)).toEqual({ symptom, server: true });
    expect(withServer(symptom, 'version', false)).toEqual({
      symptom: { kind: 'assisted' },
      server: false,
    });
  });

  it('a origem no servidor só vale para o "Testar como servidor" que travou nele', () => {
    expect(serverCrashOrigin(makeSession({ outcome: 'crashed' }))).toBeNull();
    expect(
      serverCrashOrigin(
        makeSession({
          mode: 'server',
          outcome: 'crashed',
          server: makeSide({ outcome: 'closedNormally' }),
        }),
      ),
    ).toBeNull();
    expect(
      serverCrashOrigin(makeSession({ mode: 'server', outcome: 'crashed', server: makeSide() })),
    ).toEqual({ server: true, symptom: 'crashOnOpen' });
    expect(
      serverCrashOrigin(
        makeSession({ mode: 'server', outcome: 'crashed', server: makeSide({ readyMs: 14_000 }) }),
      ),
    ).toEqual({ server: true, symptom: 'crashOnWorld' });
  });
});
