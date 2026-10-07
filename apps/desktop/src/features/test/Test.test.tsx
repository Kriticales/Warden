/**
 * Testar (SPEC T13) no app inteiro com o backend simulado: botão do cabeçalho e seus estados,
 * menu ▾, tela do teste (preparação, jogo aberto, resultado), sessões anteriores, um jogo por
 * vez e fechar o Warden com o jogo aberto. Os critérios estão no nome de cada teste.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
  GameState,
  OperationEvent,
  PackId,
  TestRequest,
  TestSessionSummary,
} from '../../lib/ipc/bindings';
import { deferred, ipcError, type Handler } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { axePage } from '../../test/axe';
import { makeTestSettingsView } from '../pack-editor/editor.fixtures';
import { editorBackend } from '../pack-editor/testing';
import { useTestStore } from './store';
import {
  gameLine,
  makeGame,
  makeSession,
  makeSessionView,
  sendOnChannel,
  wardenLine,
} from './test.fixtures';

/**
 * O jsdom não mede elementos: o console virtualizado precisa da altura do log (600 px) e das
 * linhas (20 px), como no teste do próprio console.
 */
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

async function openPack(url: string) {
  renderApp(url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
}

/** Backend do pack com um jogo dele já aberto (ou em preparação) no Warden. */
function withGame(state: GameState['state'], handlers: Record<string, Handler> = {}) {
  let packId = '' as PackId;
  const setup = editorBackend({
    handlers: { test_game_state: () => makeGame(packId, state), ...handlers },
  });
  packId = setup.row.id;
  return setup;
}

function header() {
  return screen.getByRole('banner', { name: 'Pack aberto' });
}

describe('botão Testar do cabeçalho (CA-T13-08)', () => {
  it('preparando: mostra "Testando… ver progresso" e, a partir de Mods, leva às etapas do teste', async () => {
    const { url } = withGame('preparing');
    await openPack(url);
    const button = await within(header()).findByRole('button', {
      name: 'Testando… ver progresso',
    });
    await userEvent.setup().click(button);
    expect(await screen.findByRole('heading', { level: 1, name: 'Testando o pack' })).toBeDefined();
    expect(screen.getByRole('list', { name: 'Etapas do teste' })).toBeDefined();
    expect(screen.getByText('Preparar o Minecraft')).toBeDefined();
    expect(screen.getByText('Copiar o pack para o teste')).toBeDefined();
    expect(screen.getByText('Abrir o jogo')).toBeDefined();
  });

  it('jogo aberto: mostra "Jogo aberto: ver teste" e leva ao console ao vivo', async () => {
    const { url } = withGame('running', {
      test_live_console: () => [
        wardenLine('console.abrindo', {
          jogo: '1.20.1 Forge 47.3.0',
          java: '17',
          memoriaMb: '6144',
        }),
        gameLine('Ação no carregamento do mundo', { seq: 1 }),
      ],
    });
    await openPack(url);
    const button = await within(header()).findByRole('button', { name: 'Jogo aberto: ver teste' });
    await userEvent.setup().click(button);
    expect(await screen.findByRole('heading', { level: 1, name: 'Jogo aberto' })).toBeDefined();
    expect(await screen.findByText('Ação no carregamento do mundo')).toBeDefined();
    expect(
      screen.getByText('Abrindo o jogo: 1.20.1 Forge 47.3.0, Java 17, 6 GB de memória.'),
    ).toBeDefined();
  });

  it('"Ver último teste" no menu ▾ abre o resultado da última sessão, com "Por que travou" (CA-T13-06)', async () => {
    const crashed = makeSession({
      outcome: 'crashed',
      exitCode: 255,
      durationMs: 48_000,
      crashReports: ['crash-reports/crash-2026-10-07_14.20.48-client.txt'],
    });
    const { backend, url } = editorBackend({
      handlers: {
        test_sessions_list: () => [crashed],
        test_session_get: () =>
          makeSessionView(crashed, [
            gameLine('Missing or unsupported mandatory dependencies', { level: 'error' }),
          ]),
        test_artifact_open: () => null,
      },
    });
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    const item = await screen.findByRole('menuitem', { name: /Ver último teste/ });
    expect(item.textContent).toContain('travou');
    await user.click(item);

    expect(await screen.findByRole('heading', { level: 1, name: 'O jogo travou' })).toBeDefined();
    const why = screen.getByRole('region', { name: 'Por que travou' });
    expect(within(why).getByText('O jogo fechou com o código 255 depois de 48 s.')).toBeDefined();
    expect(within(why).getByText('Missing or unsupported mandatory dependencies')).toBeDefined();
    await user.click(within(why).getByRole('button', { name: 'Abrir crash report' }));
    expect(backend.callsOf('test_artifact_open')[0]?.args).toMatchObject({
      sessionId: crashed.id,
      path: crashed.crashReports[0],
    });
    // A sessão aparece em Testes anteriores, marcada.
    const table = screen.getByRole('table');
    expect(within(table).getByText('travou')).toBeDefined();
  });
});

describe('fluxo do teste', () => {
  it('Testar começa o teste: etapas com progresso, console ao vivo e o resultado ao fechar; axe', async () => {
    const finished = deferred<TestSessionSummary>();
    let channel: unknown = null;
    const { backend, url } = editorBackend({
      handlers: {
        test_start: (args) => {
          channel = args.onEvent;
          return finished.promise;
        },
      },
    });
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Testar' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Testando o pack' })).toBeDefined();
    expect(backend.callsOf('test_start')[0]?.args.request).toEqual({
      mode: 'normal',
      profile: null,
      replaceInstanceChanges: false,
    } satisfies TestRequest);
    expect(within(header()).getByRole('button', { name: 'Testando… ver progresso' })).toBeDefined();

    const stages: OperationEvent[] = [
      { type: 'started', operationId: '01J9ZQ0000000000000000000A', kind: 'test.start' },
      { type: 'stage', stage: 'launcher.download', labelKey: 'launcher.download' },
      { type: 'progress', current: 812, total: 2140, unit: 'items' },
    ];
    act(() => {
      sendOnChannel(channel, stages);
    });
    expect(await screen.findByText('Baixando os arquivos do jogo')).toBeDefined();
    expect(screen.getByText('812 de 2.140')).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();

    act(() => {
      sendOnChannel(channel, [
        { type: 'stage', stage: 'syncPack', labelKey: 'test.stage.syncPack' },
        { type: 'stage', stage: 'test.launch', labelKey: 'teste:etapa.abrir' },
        {
          type: 'console',
          lines: [gameLine('[Render thread/INFO]: Setting user: Jogador', { seq: 0 })],
        },
      ]);
    });
    expect(await screen.findByRole('heading', { level: 1, name: 'Jogo aberto' })).toBeDefined();
    expect(screen.getByText('[Render thread/INFO]: Setting user: Jogador')).toBeDefined();

    await act(async () => {
      finished.resolve(makeSession({ outcome: 'closedNormally', durationMs: 102_000 }));
      await Promise.resolve();
    });
    expect(
      await screen.findByRole('heading', { level: 1, name: 'O jogo fechou normalmente' }),
    ).toBeDefined();
    expect(screen.getByText(/1 min 42 s de jogo/)).toBeDefined();
    expect(within(header()).getByRole('button', { name: 'Testar' })).toBeDefined();
  });

  it('Parar jogo pede confirmação e chama test_stop', async () => {
    const setup = withGame('running', { test_stop: () => null });
    renderApp(`${setup.url}/teste`);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Parar jogo' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Parar o jogo?' });
    expect(
      within(dialog).getByText('O progresso não salvo do mundo de teste pode ser perdido.'),
    ).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Parar jogo' }));
    await waitFor(() => {
      expect(setup.backend.callsOf('test_stop')).toHaveLength(1);
    });
  });

  it('arquivos do pack mudados na instância: lista e "Substituir e testar" repete com a substituição', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        test_start: ({ request }) => {
          if ((request as TestRequest).replaceInstanceChanges) {
            return makeSession();
          }
          return ipcError(
            makeAppError(
              { domain: 'app', code: 'TEST_INSTANCE_CHANGED' },
              { params: { count: '2', files: 'config/a.toml\noptions.txt' } },
            ),
          );
        },
      },
    });
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Testar' }));
    expect(
      await screen.findByRole('heading', {
        name: '2 arquivos do pack mudaram na instância de teste',
      }),
    ).toBeDefined();
    expect(screen.getByText('config/a.toml')).toBeDefined();
    expect(screen.getByText('options.txt')).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Substituir e testar' }));
    await waitFor(() => {
      expect(backend.callsOf('test_start')).toHaveLength(2);
    });
    expect(
      (backend.callsOf('test_start')[1]?.args.request as TestRequest).replaceInstanceChanges,
    ).toBe(true);
    expect(
      await screen.findByRole('heading', { level: 1, name: 'O jogo fechou normalmente' }),
    ).toBeDefined();
  });

  it('um jogo por vez: com o jogo de outro pack aberto, Testar avisa e oferece ir para o teste dele', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        test_game_state: () =>
          makeGame('01J9ZQ0000000000000000OUTRO', 'running', { packName: 'Outro Pack' }),
      },
    });
    await openPack(url);
    const user = userEvent.setup();
    await waitFor(() => {
      expect(backend.callsOf('test_game_state').length).toBeGreaterThan(0);
    });
    await user.click(within(header()).getByRole('button', { name: 'Testar' }));
    const dialog = await screen.findByRole('alertdialog', {
      name: 'O Outro Pack está com o jogo aberto',
    });
    expect(backend.callsOf('test_start')).toHaveLength(0);
    expect(within(dialog).getByRole('button', { name: 'Ir para o teste' })).toBeDefined();
  });

  it('o pack mudou com o jogo aberto: o cabeçalho avisa que vale no próximo teste', async () => {
    const setup = withGame('running');
    await openPack(setup.url);
    await within(header()).findByRole('button', { name: 'Jogo aberto: ver teste' });
    await act(async () => {
      await setup.backend.emit('pack-changed', { packId: setup.row.id, areas: ['inventory'] });
    });
    expect(
      await within(header()).findByRole('button', {
        name: 'O pack mudou desde o início do teste; as mudanças valem no próximo teste.',
      }),
    ).toBeDefined();
  });
});

describe('menu ▾ e instância de teste', () => {
  it('grupos do menu e "Apagar mundos de teste…" lista e apaga os mundos', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        pack_test_settings_get: () =>
          makeTestSettingsView({ instanceExists: true, hasWorlds: true }),
        instance_worlds_list: () => ['Mundo de teste', 'Outro mundo'],
        instance_delete_worlds: () => 2,
      },
    });
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByText('Perfil do teste')).toBeDefined();
    expect(within(menu).getByText('Instância de teste')).toBeDefined();
    expect(
      within(menu).getByRole('menuitem', { name: /Ajustes do teste neste computador/ }),
    ).toBeDefined();
    expect(
      within(menu).getByRole('menuitem', { name: /Recriar instância de teste/ }),
    ).toBeDefined();
    await user.click(await within(menu).findByRole('menuitem', { name: /Apagar mundos de teste/ }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Apagar mundos de teste?' });
    expect(await within(dialog).findByText('Mundo de teste')).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Apagar 2 mundos' }));
    await waitFor(() => {
      expect(backend.callsOf('instance_delete_worlds')).toHaveLength(1);
    });
    expect(await screen.findByText('2 mundos de teste apagados.')).toBeDefined();
  });

  it('sem instância, as ações da instância ficam indisponíveis e dizem por quê', async () => {
    const { url } = editorBackend();
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    const reveal = await screen.findByRole('menuitem', {
      name: /Abrir pasta da instância de teste/,
    });
    await waitFor(() => {
      expect(reveal.getAttribute('aria-disabled')).toBe('true');
    });
    expect(reveal.textContent).toContain('Ainda não existe');
    expect(
      screen.getByRole('menuitem', { name: /Ver último teste/ }).getAttribute('aria-disabled'),
    ).toBe('true');
  });
});

describe('fechar o Warden com o jogo aberto (CA-T13-07, interface)', () => {
  it('pergunta antes e, ao confirmar, chama test_quit_app', async () => {
    const setup = withGame('running', { test_quit_app: () => null });
    await openPack(setup.url);
    await act(async () => {
      await setup.backend.emit('game-quit-requested', {
        game: makeGame(setup.row.id, 'running'),
      });
    });
    const dialog = await screen.findByRole('alertdialog', {
      name: 'Fechar o Warden com o jogo aberto?',
    });
    expect(dialog.textContent).toContain('Vale Sereno');
    await userEvent
      .setup()
      .click(within(dialog).getByRole('button', { name: 'Fechar o jogo e o Warden' }));
    await waitFor(() => {
      expect(setup.backend.callsOf('test_quit_app')).toHaveLength(1);
    });
  });
});

describe('tela do teste sem testes', () => {
  it('pack nunca testado: explica e oferece Testar', async () => {
    const { url } = editorBackend();
    renderApp(`${url}/teste`);
    expect(
      await screen.findByRole('heading', { name: 'Este pack ainda não foi testado' }),
    ).toBeDefined();
    expect(
      screen.getByText('Os testes deste pack aparecem aqui depois do primeiro.'),
    ).toBeDefined();
  });
});
