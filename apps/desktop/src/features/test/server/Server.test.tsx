/**
 * "Testar como servidor" (SPEC T27 e T21; ROADMAP L-09) no app inteiro com o backend simulado:
 * a EULA na primeira vez, as opções com a conta de memória, a tela com o servidor aberto (os
 * dois consoles e a linha de comando), o resultado por lado e a EULA em Configurações. Os
 * critérios estão no nome de cada teste.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
  ServerPlan,
  ServerSide,
  ServerState,
  Settings,
  TestRequest,
  TestSessionSummary,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { deferred, type Handler } from '../../../test/backend';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { makeSettings, mockSettingsBackend } from '../../settings/testing';
import { stepsFor } from '../steps';
import { useTestStore } from '../store';
import {
  gameLine,
  makeSession,
  makeSessionView,
  sendOnChannel,
  wardenLine,
} from '../test.fixtures';
import { splitLines } from './ServerRunningView';
import { clientOnlyText, gameSideText, serverResultTitle, serverSideText } from './text';

beforeEach(() => {
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

function makePlan(overrides: Partial<ServerPlan> = {}): ServerPlan {
  return {
    memory: { serverMb: 4096, gameMb: 6144, totalMb: 10_240, freeMb: 13_312, fits: true },
    clientOnly: ['Embeddium', 'Rubidium', 'Jade', 'Oculus'],
    clientOnlyCount: 8,
    eulaAcceptedAt: '2026-10-01T17:05:09Z',
    loaderSupported: true,
    ...overrides,
  };
}

function makeSide(overrides: Partial<ServerSide> = {}): ServerSide {
  return {
    only: false,
    outcome: 'closedNormally',
    exitCode: 0,
    durationMs: 120_000,
    readyMs: 14_000,
    port: 25_566,
    memoryMb: 4096,
    game: 'joined',
    stopForced: false,
    crash: null,
    ...overrides,
  };
}

/** O pack aberto com o backend do servidor: a EULA mora no `settings`, como no Rust. */
async function openPack(
  settings: Partial<Settings> = {},
  plan: (withGame: boolean, settings: Settings) => ServerPlan = (withGame, current) =>
    makePlan({
      eulaAcceptedAt: current.eulaAcceptedAt ?? null,
      memory: {
        ...makePlan().memory,
        gameMb: withGame ? 6144 : null,
        totalMb: withGame ? 10_240 : 4096,
      },
    }),
  handlers: Record<string, Handler> = {},
) {
  const state = { settings: makeSettings(settings) };
  const setup = editorBackend({
    handlers: {
      settings_get: () => state.settings,
      server_plan: ({ withGame }) => plan(withGame as boolean, state.settings),
      server_eula_accept: () => {
        state.settings = { ...state.settings, eulaAcceptedAt: '2026-10-13T12:00:00Z' };
        return state.settings;
      },
      server_current: () => null,
      ...handlers,
    },
  });
  renderApp(setup.url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
  return { ...setup, state, user: userEvent.setup() };
}

function header() {
  return screen.getByRole('banner', { name: 'Pack aberto' });
}

async function openServerDialog(user: ReturnType<typeof userEvent.setup>) {
  await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
  const menu = await screen.findByRole('menu');
  await user.click(await within(menu).findByRole('menuitem', { name: /Testar como servidor…/ }));
}

describe('EULA na primeira vez (CA-T27-02, CA-T21-05)', () => {
  it('sem aceite, pergunta com o link; "Agora não" não abre nada; "Aceito a EULA" grava e segue para as opções; a pergunta não volta', async () => {
    const { backend, user } = await openPack();
    await openServerDialog(user);
    const eula = await screen.findByRole('dialog', { name: 'Aceitar a EULA do Minecraft?' });
    expect(within(eula).getByRole('button', { name: /aka\.ms\/MinecraftEULA/ })).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(eula).getByRole('button', { name: 'Agora não' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(backend.callsOf('server_eula_accept')).toHaveLength(0);
    expect(backend.callsOf('test_start')).toHaveLength(0);
    // O foco volta para quem abriu o menu.
    expect(document.activeElement).toBe(
      within(header()).getByRole('button', { name: 'Mais opções do teste' }),
    );

    await openServerDialog(user);
    const again = await screen.findByRole('dialog', { name: 'Aceitar a EULA do Minecraft?' });
    await user.click(within(again).getByRole('button', { name: 'Aceito a EULA' }));
    expect(await screen.findByRole('dialog', { name: 'Testar como servidor' })).toBeDefined();
    expect(backend.callsOf('server_eula_accept')).toHaveLength(1);
    expect(backend.callsOf('test_start')).toHaveLength(0);
    await user.click(screen.getByRole('button', { name: 'Cancelar' }));

    // A conta foi lida de novo com o aceite: a pergunta não volta.
    await openServerDialog(user);
    expect(await screen.findByRole('dialog', { name: 'Testar como servidor' })).toBeDefined();
    expect(screen.queryByRole('dialog', { name: 'Aceitar a EULA do Minecraft?' })).toBeNull();
  });
});

describe('opções do Testar como servidor (CA-T27-03)', () => {
  it('mostra a soma da memória e a livre agora, os mods de fora e as garantias; com memória livre menor, avisa e sugere "Só o servidor"; nada abre sem o clique', async () => {
    const { backend, user } = await openPack(
      { eulaAcceptedAt: '2026-10-01T17:05:09Z' },
      (withGame) =>
        makePlan({
          memory: {
            serverMb: 4096,
            gameMb: withGame ? 6144 : null,
            totalMb: withGame ? 10_240 : 4096,
            freeMb: 7168,
            fits: !withGame,
          },
        }),
    );
    await openServerDialog(user);
    const dialog = await screen.findByRole('dialog', { name: 'Testar como servidor' });
    expect(await within(dialog).findByText('servidor 4 GB + jogo 6 GB = 10 GB')).toBeDefined();
    expect(within(dialog).getByText('7 GB')).toBeDefined();
    expect(within(dialog).getByText('Pode faltar memória.')).toBeDefined();
    expect(within(dialog).getByText(/escolha "Só o servidor"/)).toBeDefined();
    expect(
      within(dialog).getByText(
        '8 mods só de cliente ficam de fora: Embeddium, Rubidium, Jade e mais 5.',
      ),
    ).toBeDefined();
    expect(
      within(dialog).getByText(
        'Só aceita conexões deste computador (127.0.0.1) e fecha sozinho quando o teste acabar.',
      ),
    ).toBeDefined();
    expect(backend.callsOf('test_start')).toHaveLength(0);
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.click(within(dialog).getByRole('radio', { name: /Só o servidor/ }));
    expect(await within(dialog).findByText('servidor 4 GB')).toBeDefined();
    await waitFor(() => {
      expect(within(dialog).queryByText('Pode faltar memória.')).toBeNull();
    });
    expect(backend.callsOf('server_plan').some((call) => call.args.withGame === false)).toBe(true);
    expect(backend.callsOf('test_start')).toHaveLength(0);
  });

  it('"Abrir servidor e testar" começa o teste no modo servidor, com o jogo ou só o servidor', async () => {
    const { backend, user } = await openPack(
      { eulaAcceptedAt: '2026-10-01T17:05:09Z' },
      undefined,
      { test_start: () => new Promise(() => undefined) },
    );
    await openServerDialog(user);
    const dialog = await screen.findByRole('dialog', { name: 'Testar como servidor' });
    await within(dialog).findByText('servidor 4 GB + jogo 6 GB = 10 GB');
    await user.click(within(dialog).getByRole('button', { name: 'Abrir servidor e testar' }));
    await waitFor(() => {
      expect(backend.callsOf('test_start')).toHaveLength(1);
    });
    expect(backend.callsOf('test_start')[0]?.args.request).toEqual({
      mode: 'server',
      profile: null,
      replaceInstanceChanges: false,
      serverOnly: false,
      // L-12: o perfil de desempenho no servidor vem desmarcado.
      performance: false,
    } satisfies TestRequest);
    expect(await screen.findByRole('heading', { level: 1, name: 'Testando o pack' })).toBeDefined();
    // As etapas do teste com servidor.
    const steps = screen.getByRole('list', { name: 'Etapas do teste' });
    expect(within(steps).getByText('Preparar o servidor')).toBeDefined();
    expect(within(steps).getByText('Abrir o servidor e o jogo')).toBeDefined();
  });
});

describe('servidor aberto (SPEC T27 passo 3)', () => {
  it('console do servidor separado do do jogo ("Mostrando"), linha de comando e "Parar jogo e servidor"; axe', async () => {
    let channel: unknown = null;
    const ready: ServerState = {
      packId: '',
      phase: 'ready',
      port: 25_566,
      startedAtMs: Date.now(),
      readyMs: 14_000,
    };
    const { backend, user, row } = await openPack(
      { eulaAcceptedAt: '2026-10-01T17:05:09Z' },
      undefined,
      {
        test_start: (args) => {
          channel = args.onEvent;
          return deferred<TestSessionSummary>().promise;
        },
        server_current: () => ({ ...ready, packId: row.id }),
        server_send_command: () => null,
        test_stop: () => null,
      },
    );
    await openServerDialog(user);
    const dialog = await screen.findByRole('dialog', { name: 'Testar como servidor' });
    await within(dialog).findByText('servidor 4 GB + jogo 6 GB = 10 GB');
    await user.click(within(dialog).getByRole('button', { name: 'Abrir servidor e testar' }));
    await waitFor(() => {
      expect(channel).not.toBeNull();
    });
    act(() => {
      sendOnChannel(channel, [
        {
          type: 'console',
          lines: [
            wardenLine(
              'servidor:console.abrindo',
              { jogo: 'forge 1.20.1-47.3.0', java: '17', memoriaMb: '4096', porta: '25566' },
              { seq: 0 },
            ),
            gameLine('Done (6.912s)! For help, type "help"', { seq: 1, origin: 'server' }),
            gameLine('Jogador joined the game', { seq: 2, origin: 'server' }),
            gameLine('[Render thread/INFO]: Connecting to 127.0.0.1, 25566', { seq: 3 }),
          ],
        },
      ]);
    });
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Testando como servidor' }),
    ).toBeDefined();
    expect(await screen.findByText(/Servidor em 127\.0\.0\.1, porta 25566/)).toBeDefined();
    const serverConsole = screen.getByRole('region', { name: 'Console do servidor' });
    expect(within(serverConsole).getByText('Done (6.912s)! For help, type "help"')).toBeDefined();
    expect(
      within(serverConsole).getByText(
        'Abrindo o servidor: forge 1.20.1-47.3.0 · Java 17 · 4 GB · só em 127.0.0.1, porta 25566',
      ),
    ).toBeDefined();
    expect(within(serverConsole).queryByText(/Connecting to/)).toBeNull();
    // O foco voltou ao ▾ do Testar e abriu a dica dele (fora das regiões, como toda dica).
    act(() => {
      (document.activeElement as HTMLElement | null)?.blur();
    });
    await waitFor(() => {
      expect(document.querySelector('[data-radix-popper-content-wrapper]')).toBeNull();
    });
    expect(await axePage(document.body)).toHaveNoViolations();

    const command = screen.getByRole('textbox', { name: 'Enviar comando ao servidor' });
    await waitFor(() => {
      expect((command as HTMLInputElement).disabled).toBe(false);
    });
    await user.type(command, 'say oi{Enter}');
    await waitFor(() => {
      expect(backend.callsOf('server_send_command')[0]?.args).toEqual({
        packId: row.id,
        command: 'say oi',
      });
    });

    await user.selectOptions(screen.getByRole('combobox', { name: 'Mostrando' }), 'game');
    const gameConsole = await screen.findByRole('region', { name: 'Console do jogo' });
    expect(within(gameConsole).getByText(/Connecting to 127\.0\.0\.1/)).toBeDefined();
    expect(within(gameConsole).queryByText(/Done \(6\.912s\)/)).toBeNull();
    expect(screen.queryByRole('textbox', { name: 'Enviar comando ao servidor' })).toBeNull();

    await user.click(screen.getByRole('button', { name: 'Parar jogo e servidor' }));
    const confirm = await screen.findByRole('alertdialog', { name: 'Parar o jogo e o servidor?' });
    await user.click(within(confirm).getByRole('button', { name: 'Parar jogo e servidor' }));
    await waitFor(() => {
      expect(backend.callsOf('test_stop')).toHaveLength(1);
    });
  });
});

describe('servidor aberto sem o evento "pronto"', () => {
  it('a tela relê o servidor enquanto ele carrega: perdido o `server-state`, a linha de comando liga mesmo assim', async () => {
    let channel: unknown = null;
    let reads = 0;
    const { user, row } = await openPack({ eulaAcceptedAt: '2026-10-01T17:05:09Z' }, undefined, {
      test_start: (args) => {
        channel = args.onEvent;
        return deferred<TestSessionSummary>().promise;
      },
      // A primeira leitura é de antes do "Done"; o evento "pronto" nunca chega à tela.
      server_current: (): ServerState => {
        reads += 1;
        return {
          packId: row.id,
          phase: reads === 1 ? 'starting' : 'ready',
          port: 25_566,
          startedAtMs: Date.now(),
          readyMs: reads === 1 ? null : 14_000,
        };
      },
    });
    await openServerDialog(user);
    const dialog = await screen.findByRole('dialog', { name: 'Testar como servidor' });
    await user.click(
      await within(dialog).findByRole('button', { name: 'Abrir servidor e testar' }),
    );
    await waitFor(() => {
      expect(channel).not.toBeNull();
    });
    act(() => {
      sendOnChannel(channel, [
        {
          type: 'console',
          lines: [gameLine('Done (6.912s)! For help, type "help"', { seq: 0, origin: 'server' })],
        },
      ]);
    });
    const command = await screen.findByRole('textbox', { name: 'Enviar comando ao servidor' });
    await waitFor(
      () => {
        expect((command as HTMLInputElement).disabled).toBe(false);
      },
      { timeout: 5_000 },
    );
    expect(reads).toBeGreaterThan(1);
  });
});

describe('resultado por lado (SPEC T27 passo 4; CA-T27-04)', () => {
  it('o servidor que travou com um mod de cliente: título, o mod apontado e o trecho do log; o "Por que travou" do jogo não aparece', async () => {
    const crashed = makeSession({
      mode: 'server',
      outcome: 'crashed',
      exitCode: 1,
      server: makeSide({
        only: true,
        outcome: 'crashed',
        exitCode: 1,
        readyMs: null,
        game: 'notOpened',
        crash: {
          foundCause: true,
          mods: [{ name: 'Zoom de Cliente', path: 'mods/zoom.pw.toml', side: 'both' }],
          clientClass: 'net.minecraft.client.gui.screens.Screen',
          excerpt: [
            '[Server thread/ERROR]: Encountered an unexpected exception',
            'java.lang.RuntimeException: Attempted to load class net/minecraft/client/gui/screens/Screen for invalid dist DEDICATED_SERVER',
          ],
          titleKey: 'diagnostico.achados.crashReport',
        },
      }),
    });
    const setup = editorBackend({
      handlers: {
        test_sessions_list: () => [crashed],
        test_session_get: () => makeSessionView(crashed),
        settings_get: () => makeSettings(),
      },
    });
    renderApp(`${setup.url}/teste`);
    expect(
      await screen.findByRole('heading', { level: 1, name: 'O servidor travou' }),
    ).toBeDefined();
    const result = screen.getByRole('region', { name: /O que aconteceu em cada lado/ });
    expect(within(result).getByText('Travou antes de ficar pronto.')).toBeDefined();
    expect(within(result).getByText('Não abriu (Só o servidor).')).toBeDefined();
    expect(
      within(result).getByText('O mod Zoom de Cliente é de cliente e derrubou o servidor.'),
    ).toBeDefined();
    expect(within(result).getByText(/invalid dist DEDICATED_SERVER/)).toBeDefined();
    expect(screen.queryByRole('region', { name: /Por que travou/ })).toBeNull();
    expect(setup.backend.callsOf('diagnostics_crash')).toHaveLength(0);
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('textos por lado e títulos', () => {
    const side = makeSide();
    expect(serverSideText(side)).toBe('Abriu em 14 s e fechou normalmente.');
    expect(gameSideText(side)).toBe('Entrou no servidor.');
    expect(serverSideText(makeSide({ stopForced: true }))).toMatch(/encerrado à força/);
    expect(gameSideText(makeSide({ game: 'opened' }))).toBe('Abriu, mas não entrou no servidor.');
    expect(serverResultTitle(makeSession())).toBeNull();
    expect(serverResultTitle(makeSession({ mode: 'server', server: side }))).toBeNull();
    expect(
      serverResultTitle(
        makeSession({ mode: 'server', outcome: 'stoppedByUser', server: makeSide({ only: true }) }),
      ),
    ).toBe('Servidor encerrado por você');
    expect(clientOnlyText([], 0)).toBe(
      'Nenhum mod é só de cliente: o pack inteiro vai para o servidor.',
    );
    expect(clientOnlyText(['Jade'], 1)).toBe('1 mod só de cliente fica de fora: Jade.');
  });

  it('separa as linhas do servidor das do jogo e as etapas por modo', () => {
    const lines = [
      wardenLine('console.abrindo', {}),
      wardenLine('servidor:console.pronto', {}),
      gameLine('x', { origin: 'server' }),
      gameLine('y'),
    ];
    const { server, game } = splitLines(lines);
    expect(server.map((line) => line.text)).toEqual(['servidor:console.pronto', 'x']);
    expect(game.map((line) => line.text)).toEqual(['console.abrindo', 'y']);
    const ids = (request: TestRequest) => stepsFor(request).map((step) => step.id);
    expect(ids({ mode: 'normal' })).not.toContain('server');
    expect(ids({ mode: 'server' })).toEqual([
      'check',
      'prepare',
      'sync',
      'server',
      'final-check',
      'launch',
    ]);
    expect(ids({ mode: 'server', serverOnly: true })).toEqual([
      'check',
      'prepare',
      'server',
      'launch',
    ]);
  });
});

describe('EULA em Configurações → Teste (CA-T21-05)', () => {
  it('mostra "Aceita em" com Revogar; revogar grava e mostra "Ainda não aceita" na hora', async () => {
    const backend = mockSettingsBackend(
      { settings: makeSettings({ eulaAcceptedAt: '2026-10-01T17:05:09Z' }) },
      {
        server_eula_revoke: () => {
          backend.state.settings = { ...backend.state.settings, eulaAcceptedAt: null };
          return backend.state.settings;
        },
      },
    );
    renderApp('/configuracoes');
    const group = await screen.findByRole('group', { name: 'EULA do Minecraft' });
    expect(within(group).getByText('Aceita em 01/10/2026')).toBeDefined();
    await userEvent.setup().click(within(group).getByRole('button', { name: 'Revogar' }));
    expect(await within(group).findByText('Ainda não aceita')).toBeDefined();
    expect(backend.callsOf('server_eula_revoke')).toHaveLength(1);
    expect(within(group).queryByRole('button', { name: 'Revogar' })).toBeNull();
  });
});
