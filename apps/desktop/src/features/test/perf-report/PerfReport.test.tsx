/**
 * "Testar com perfil de desempenho" (SPEC T13; CA-T13-13; ROADMAP L-12) no app inteiro com o
 * backend simulado: o item do menu ▾, o teste começado nesse modo, a parte "Desempenho" do
 * resultado (tempo por mod e resumo do spark), "Abrir no visualizador do spark", a opção do
 * "Testar como servidor" e os dados atualizados depois que o teste termina.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';

import type {
  PerfReport,
  ServerPlan,
  SparkReport,
  TestRequest,
  TestSessionSummary,
} from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { deferred } from '../../../test/backend';
import { makeSettings } from '../../settings/testing';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { useTestStore } from '../store';
import { makeGame, makeSession, makeSessionView } from '../test.fixtures';

afterEach(() => {
  useTestStore.setState({ game: null, runs: {}, packChanged: {} });
});

/** 18/10/2026 00:42:48 no horário local do teste. */
const SAVED_AT = new Date(2026, 9, 18, 0, 42, 48).getTime();

function makeSpark(overrides: Partial<SparkReport> = {}): SparkReport {
  return {
    file: 'profile-2026-10-18_00.42.48.sparkprofile',
    kind: 'profile',
    savedAtMs: SAVED_AT,
    readable: true,
    platform: { side: 'client', name: 'Forge', version: '47.4.10', minecraft: '1.20.1' },
    durationMs: 40_234,
    ticks: { msptMedian: 1.6, msptMax: 213.7, msptP95: null, tps: 20, ticks: 800 },
    top: [
      { name: 'Create', item: 'mods/create.pw.toml', modId: 'create', percent: 31 },
      { name: 'Epic Fight', item: 'mods/epicfight.pw.toml', modId: 'epicfight', percent: 12 },
      { name: 'Supplementaries', item: null, modId: 'supplementaries', percent: 6.2 },
      { name: 'Jade', item: null, modId: 'jade', percent: 1 },
    ],
    gamePercent: 49.8,
    ...overrides,
  };
}

function makeReport(overrides: Partial<PerfReport> = {}): PerfReport {
  return {
    side: 'client',
    load: {
      status: 'measured',
      source: 'trace',
      loader: 'forge',
      log: 'logs/debug.log',
      mods: [
        { key: 'create', name: 'Create', item: 'mods/create.pw.toml', ms: 14_200, phases: [] },
        { key: 'jei', name: 'Just Enough Items', item: null, ms: 288, phases: [] },
      ],
      gameAndLoaderMs: 1345,
      modsMs: 14_488,
    },
    serverSpark: null,
    spark: [makeSpark()],
    sparkSkipped: 0,
    ...overrides,
  };
}

const PERF: TestSessionSummary = makeSession({ mode: 'performance' });

function header() {
  return screen.getByRole('banner', { name: 'Pack aberto' });
}

async function openPack(url: string) {
  renderApp(url);
  await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
}

/** A tela do teste com a última sessão e o perfil dado. */
function withPerfSession(report: PerfReport | null, session = PERF) {
  return editorBackend({
    handlers: {
      test_sessions_list: () => [session],
      test_session_get: () => makeSessionView(session, []),
      perf_report_get: () => report,
      perf_open_spark_viewer: () => null,
      test_start: () => deferred<TestSessionSummary>().promise,
    },
  });
}

async function region() {
  return screen.findByRole('region', { name: 'Desempenho' });
}

describe('menu ▾: Testar com perfil de desempenho', () => {
  it('começa o teste nesse modo e abre a tela do teste', async () => {
    const { backend, url } = editorBackend({
      handlers: { test_start: () => deferred<TestSessionSummary>().promise },
    });
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    const menu = await screen.findByRole('menu');
    const item = within(menu).getByRole('menuitem', { name: /Testar com perfil de desempenho/ });
    expect(item.textContent).toContain('Mede o que mais pesa para carregar e rodar');
    // Na ordem do protótipo: depois de "Testar como servidor…".
    const labels = within(menu)
      .getAllByRole('menuitem')
      .map((entry) => entry.textContent);
    const server = labels.findIndex((text) => text.includes('Testar como servidor'));
    const perf = labels.findIndex((text) => text.includes('Testar com perfil de desempenho'));
    expect(perf).toBe(server + 1);
    await user.click(item);
    expect(await screen.findByRole('heading', { level: 1, name: 'Testando o pack' })).toBeDefined();
    expect(backend.callsOf('test_start')[0]?.args.request).toEqual({
      mode: 'performance',
      profile: null,
      replaceInstanceChanges: false,
    } satisfies TestRequest);
  });

  it('com um jogo aberto, o item fica indisponível e diz por quê', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        test_game_state: () =>
          makeGame('01J9ZQ0000000000000000OUTRO', 'running', { packName: 'Outro Pack' }),
      },
    });
    await openPack(url);
    await waitFor(() => {
      expect(backend.callsOf('test_game_state').length).toBeGreaterThan(0);
    });
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    const item = await screen.findByRole('menuitem', { name: /Testar com perfil de desempenho/ });
    expect(item.getAttribute('aria-disabled')).toBe('true');
    expect(item.textContent).toContain('Feche o jogo antes');
    expect(backend.callsOf('test_start')).toHaveLength(0);
  });
});

describe('resultado: Desempenho', () => {
  it('mostra o que mais demorou para carregar, com a ressalva, e o resumo do spark; axe', async () => {
    const { url } = withPerfSession(makeReport());
    renderApp(`${url}/teste`);
    const panel = await region();
    expect(within(panel).getByText('Teste com perfil de desempenho')).toBeDefined();
    const table = within(panel).getByRole('table', { name: 'Tempo de carregamento de cada mod' });
    const rows = within(table).getAllByRole('row').slice(1);
    expect(rows.map((row) => row.textContent)).toEqual(['Create14,2 s', 'Just Enough Items288 ms']);
    expect(
      within(panel).getByText('O Minecraft e o Forge levaram 1,3 s nessas fases.'),
    ).toBeDefined();
    expect(
      within(panel).getByText(
        /Mede o código de cada mod nas fases do Forge\. Não entram as texturas, os modelos/,
      ),
    ).toBeDefined();
    expect(within(panel).getByText('Perfil do spark salvo às 00:42')).toBeDefined();
    expect(within(panel).getByText('1,6 ms na mediana, 213,7 ms no pior tick')).toBeDefined();
    expect(within(panel).getByText('20 em média (o normal é 20)')).toBeDefined();
    expect(within(panel).getByText('40 s')).toBeDefined();
    // Os três que mais usaram processamento, e o resto é do jogo.
    expect(
      within(panel).getByText('Create (31%), Epic Fight (12%), Supplementaries (6,2%)', {
        exact: false,
      }),
    ).toBeDefined();
    expect(within(panel).getByText(/O jogo, o loader e o Java: 49,8%\./)).toBeDefined();
    expect(within(panel).queryByText(/Jade/)).toBeNull();
    expect(
      within(panel).getByText(/Nada é enviado: o spark lê o arquivo aí mesmo\./),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('"Abrir no visualizador do spark" pede o arquivo daquele perfil', async () => {
    const heap = makeSpark({
      file: 'heapsummary-1.sparkheap',
      kind: 'heap',
      durationMs: null,
      ticks: null,
      top: [{ name: 'FerriteCore', item: null, modId: 'ferritecore', percent: 3.4 }],
      gamePercent: 94,
    });
    const { backend, url } = withPerfSession(makeReport({ spark: [makeSpark(), heap] }));
    renderApp(`${url}/teste`);
    const panel = await region();
    // O mais novo primeiro.
    const titles = within(panel)
      .getAllByRole('heading', { level: 3 })
      .map((heading) => heading.textContent);
    expect(titles).toEqual([
      'O que mais demorou para carregar',
      'Memória do spark salva às 00:42',
      'Perfil do spark salvo às 00:42',
    ]);
    expect(within(panel).getByText('Mais memória')).toBeDefined();
    const buttons = within(panel).getAllByRole('button', {
      name: 'Abrir no visualizador do spark',
    });
    const older = buttons[1];
    if (!older) throw new Error('faltou o botão do perfil');
    await userEvent.setup().click(older);
    await waitFor(() => {
      expect(backend.callsOf('perf_open_spark_viewer')).toHaveLength(1);
    });
    expect(backend.callsOf('perf_open_spark_viewer')[0]?.args).toMatchObject({
      sessionId: PERF.id,
      file: 'profile-2026-10-18_00.42.48.sparkprofile',
    });
  });

  it('Fabric: diz que o loader não registra; sem perfil do spark, ensina a salvar', async () => {
    const { url } = withPerfSession(
      makeReport({
        load: {
          status: 'notRecorded',
          source: null,
          loader: 'fabric',
          log: null,
          mods: [],
          gameAndLoaderMs: 0,
          modsMs: 0,
        },
        spark: [],
      }),
    );
    renderApp(`${url}/teste`);
    const panel = await region();
    expect(
      within(panel).getByText('O Fabric não registra o tempo de carregamento de cada mod.'),
    ).toBeDefined();
    expect(within(panel).queryByRole('table')).toBeNull();
    expect(within(panel).getByText('Nenhum perfil do spark salvo neste teste.')).toBeDefined();
    expect(within(panel).getByText(/\/sparkc profiler stop --save-to-file/)).toBeDefined();
    expect(within(panel).queryByRole('button')).toBeNull();
  });

  it('log sem as linhas de tempo e arquivo do spark ilegível', async () => {
    const { url } = withPerfSession(
      makeReport({
        load: {
          status: 'noLines',
          source: 'trace',
          loader: 'neoforge',
          log: 'logs/debug.log',
          mods: [],
          gameAndLoaderMs: 0,
          modsMs: 0,
        },
        spark: [makeSpark({ readable: false, ticks: null, top: [], platform: null })],
        sparkSkipped: 2,
      }),
    );
    renderApp(`${url}/teste`);
    const panel = await region();
    expect(
      within(panel).getByText(
        'O log do NeoForge (logs/debug.log) não trouxe o tempo de cada mod. O jogo fechou antes de terminar de carregar?',
      ),
    ).toBeDefined();
    expect(within(panel).getByText('Não deu para ler este arquivo do spark.')).toBeDefined();
    expect(
      within(panel).getByText('2 arquivos mais antigos do spark ficaram de fora.'),
    ).toBeDefined();
  });

  it('no servidor local sem o spark: diz para adicionar', async () => {
    const server = makeSession({ mode: 'server' });
    const { url } = withPerfSession(
      makeReport({ side: 'server', load: null, serverSpark: 'notInstalled', spark: [] }),
      server,
    );
    renderApp(`${url}/teste`);
    const panel = await region();
    expect(within(panel).getByText('Teste como servidor com o perfil de desempenho')).toBeDefined();
    expect(within(panel).getByText('No servidor, o Warden mede só com o spark.')).toBeDefined();
    expect(within(panel).getByText(/O spark não está entre os mods do servidor/)).toBeDefined();
  });

  it('sessão de um teste normal: nada aparece e o perfil nem é pedido', async () => {
    const normal = makeSession({ mode: 'normal' });
    const { backend, url } = withPerfSession(null, normal);
    renderApp(`${url}/teste`);
    expect(
      await screen.findByRole('heading', { level: 1, name: 'O jogo fechou normalmente' }),
    ).toBeDefined();
    expect(screen.queryByRole('region', { name: 'Desempenho' })).toBeNull();
    expect(backend.callsOf('perf_report_get')).toHaveLength(0);
  });

  it('teste como servidor sem o perfil: o bloco não aparece', async () => {
    const server = makeSession({ mode: 'server' });
    const { backend, url } = withPerfSession(null, server);
    renderApp(`${url}/teste`);
    await waitFor(() => {
      expect(backend.callsOf('perf_report_get')).toHaveLength(1);
    });
    expect(screen.queryByRole('region', { name: 'Desempenho' })).toBeNull();
  });
});

describe('dados atuais depois do teste', () => {
  it('ao terminar, o perfil novo, a lista de testes e o "último teste" de Meus packs são lidos de novo', async () => {
    const finished = deferred<TestSessionSummary>();
    let done = false;
    const setup = editorBackend({
      handlers: {
        test_start: () => finished.promise,
        perf_report_get: () => makeReport(),
        test_sessions_list: () => (done ? [PERF] : []),
        packs_list: () => [{ ...setup.row, lastTest: done ? 'ok' : null }],
      },
    });
    const { backend, url } = setup;
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    await user.click(
      await screen.findByRole('menuitem', { name: /Testar com perfil de desempenho/ }),
    );
    expect(await screen.findByRole('heading', { level: 1, name: 'Testando o pack' })).toBeDefined();
    const sessionsBefore = backend.callsOf('test_sessions_list').length;
    done = true;
    await act(async () => {
      finished.resolve(PERF);
      await Promise.resolve();
    });
    expect(await region()).toBeDefined();
    expect(backend.callsOf('perf_report_get')[0]?.args).toMatchObject({ sessionId: PERF.id });
    await waitFor(() => {
      expect(backend.callsOf('test_sessions_list').length).toBeGreaterThan(sessionsBefore);
    });
    await user.click(within(header()).getByRole('link', { name: 'Meus packs' }));
    await screen.findByRole('heading', { level: 1, name: 'Meus packs' }, { timeout: 10_000 });
    expect(await screen.findByText('Abriu normalmente')).toBeDefined();
  });
});

describe('Testar como servidor com o perfil de desempenho', () => {
  function makePlan(withGame: boolean): ServerPlan {
    return {
      eulaAcceptedAt: '2026-10-01T17:05:09Z',
      loaderSupported: true,
      memory: {
        serverMb: 4096,
        gameMb: withGame ? 6144 : null,
        totalMb: withGame ? 10_240 : 4096,
        freeMb: 20_000,
        fits: true,
      },
      clientOnly: [],
      clientOnlyCount: 0,
    };
  }

  it('a opção vem desmarcada, liga o perfil no pedido e fica desligada em "Só o servidor"', async () => {
    const { backend, url } = editorBackend({
      handlers: {
        settings_get: () => makeSettings({ eulaAcceptedAt: '2026-10-01T17:05:09Z' }),
        server_plan: ({ withGame }) => makePlan(withGame as boolean),
        server_current: () => null,
        test_start: () => deferred<TestSessionSummary>().promise,
      },
    });
    await openPack(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    await user.click(await screen.findByRole('menuitem', { name: /Testar como servidor…/ }));
    const dialog = await screen.findByRole('dialog', { name: 'Testar como servidor' });
    const option = () =>
      within(dialog).findByRole<HTMLInputElement>('checkbox', {
        name: 'Medir o desempenho do servidor com o spark',
      });
    expect((await option()).checked).toBe(false);
    // A conta de memória é lida de novo para cada escolha: a opção aparece de novo depois.
    await user.click(within(dialog).getByRole('radio', { name: /Só o servidor/ }));
    expect(await within(dialog).findByText('Só com o jogo aberto junto.')).toBeDefined();
    expect((await option()).disabled).toBe(true);
    await user.click(
      await within(dialog).findByRole('radio', { name: /Abrir o servidor e entrar/ }),
    );
    await within(dialog).findByText(/o Warden liga o spark do servidor pelo console/);
    await user.click(await option());
    expect((await option()).checked).toBe(true);
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(dialog).getByRole('button', { name: 'Abrir servidor e testar' }));
    await waitFor(() => {
      expect(backend.callsOf('test_start')).toHaveLength(1);
    });
    expect(backend.callsOf('test_start')[0]?.args.request).toEqual({
      mode: 'server',
      profile: null,
      replaceInstanceChanges: false,
      serverOnly: false,
      performance: true,
    } satisfies TestRequest);
  });
});
