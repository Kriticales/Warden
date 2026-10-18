/**
 * Busca do culpado (SPEC T25; ROADMAP D-12) no app inteiro com o backend simulado: o diálogo
 * pelo menu ▾, pelo resultado do travamento e por Travamentos; o modo da tela do teste com as
 * rodadas, o modo assistido (CA-T25-04), pausar e cancelar; o resultado; o botão do cabeçalho
 * durante a busca; e os dados que mudam a cada rodada.
 */
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';

import type { BisectRequest, BisectView, OperationEvent } from '../../lib/ipc/bindings';
import { axePage } from '../../test/axe';
import { deferred, type Handler } from '../../test/backend';
import { renderApp } from '../../test/render';
import { makeCrashList, makeGroup } from '../diagnostics/crashes/crashes.fixtures';
import { makeReport } from '../diagnostics/diagnostics.fixtures';
import { editorBackend } from '../pack-editor/testing';
import { useTestStore } from '../test/store';
import { makeGame, makeSession, makeSessionView, sendOnChannel } from '../test/test.fixtures';
import { item, makeCurrent, makeFound, makePlan, makeRound, makeView } from './bisect.fixtures';

afterEach(() => {
  useTestStore.setState({ game: null, runs: {}, packChanged: {} });
});

function setup(handlers: Record<string, Handler> = {}) {
  return editorBackend({
    handlers: {
      bisect_plan: () => makePlan(),
      bisect_get: () => null,
      ...handlers,
    },
  });
}

/** Abre a rota e espera o título dela (a seção Mods, por padrão). */
async function openPage(url: string, suffix = '', title: string | RegExp = 'Mods') {
  renderApp(`${url}${suffix}`);
  await screen.findByRole('heading', { level: 1, name: title }, { timeout: 10_000 });
}

function header() {
  return screen.getByRole('banner', { name: 'Pack aberto' });
}

describe('menu ▾ e diálogo "Encontrar o mod culpado"', () => {
  it('mostra o que acontece, a estimativa e começa a busca no modo da tela do teste', async () => {
    const run = deferred<BisectView>();
    const { backend, url } = setup({ bisect_start: () => run.promise });
    await openPage(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    const menu = await screen.findByRole('menu');
    await user.click(within(menu).getByRole('menuitem', { name: /Encontrar o mod culpado…/ }));

    const dialog = await screen.findByRole('dialog', { name: 'Encontrar o mod culpado' });
    expect(
      await within(dialog).findByText('11 a 19 rodadas · cerca de 15 a 26 minutos'),
    ).toBeDefined();
    expect(
      within(dialog).getByText('118 mods entram na busca. Fabric API fica sempre ligado.'),
    ).toBeDefined();
    expect(
      within(dialog).getByText(
        'spark e Crash Assistant ficam desligados durante a busca, para não atrapalhar.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByRole('radio', { name: /Trava ao abrir o jogo/ })).toHaveProperty(
      'checked',
      true,
    );
    expect(await axePage(document.body)).toHaveNoViolations();
    await user.click(within(dialog).getByRole('radio', { name: /Trava ao entrar no mundo/ }));
    await user.click(within(dialog).getByRole('button', { name: 'Começar a busca' }));

    await waitFor(() => {
      expect(backend.callsOf('bisect_start')).toHaveLength(1);
    });
    expect(backend.callsOf('bisect_start')[0]?.args.request).toEqual({
      symptom: { kind: 'crashOnWorld' },
      originCrash: null,
      suspects: [],
    } satisfies BisectRequest);
    // Foi para a tela do teste no modo da busca (vazia até a primeira rodada).
    expect(await screen.findByText('Nenhuma busca do culpado neste pack')).toBeDefined();
  });

  it('sem mundo de teste, "Trava ao entrar no mundo" explica e fica indisponível', async () => {
    const { url } = setup({ bisect_plan: () => makePlan({ world: null }) });
    await openPage(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', {
        name: /Encontrar o mod culpado…/,
      }),
    );
    const dialog = await screen.findByRole('dialog', { name: 'Encontrar o mod culpado' });
    const world = await within(dialog).findByRole('radio', { name: /Trava ao entrar no mundo/ });
    expect(world).toHaveProperty('disabled', true);
    expect(within(dialog).getByText(/Precisa de um mundo de teste/)).toBeDefined();
    // "Aparece uma mensagem no log" pede a mensagem antes de começar.
    await user.click(within(dialog).getByRole('radio', { name: /Aparece uma mensagem no log/ }));
    const start = within(dialog).getByRole('button', { name: 'Começar a busca' });
    expect(start).toHaveProperty('disabled', true);
    await user.type(within(dialog).getByLabelText('Mensagem do log'), 'textura 42');
    expect(start).toHaveProperty('disabled', false);
  });

  it('com o jogo aberto, o item espera (um jogo por vez)', async () => {
    const { row, url } = setup();
    useTestStore.setState({ game: makeGame(row.id, 'running') });
    await openPage(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    const item = within(await screen.findByRole('menu')).getByRole('menuitem', {
      name: /Encontrar o mod culpado…/,
    });
    expect(item.getAttribute('aria-disabled')).toBe('true');
  });
});

describe('modo da busca na tela do teste', () => {
  it('mostra a trilha, o que o Warden está fazendo, a tabela, os suspeitos e o botão do cabeçalho', async () => {
    const { row, url } = setup({ bisect_get: () => makeView() });
    useTestStore.setState({ game: makeGame(row.id, 'bisecting') });
    await openPage(url, '/teste?modo=busca', 'Buscando o mod culpado');
    expect(
      screen.getByText('Rodada 3 de 11 a 19 · 7 mods ainda suspeitos · 9 min até agora'),
    ).toBeDefined();
    expect(
      screen.getByText(
        'Rodada 3: abrindo o jogo com 71 dos 118 mods. Se travar igual, o culpado está entre os 3 mods acrescentados agora.',
      ),
    ).toBeDefined();
    const track = screen.getByRole('list', { name: 'Rodadas da busca' });
    expect(within(track).getByText('Rodada 0, 121 mods: Travou igual')).toBeDefined();
    expect(
      within(track).getByText('Rodada 2, 61 mods: Travou diferente (não conta)'),
    ).toBeDefined();
    const table = screen.getByRole('table', { name: 'Rodadas da busca do culpado' });
    expect(within(table).getByText('Pack inteiro')).toBeDefined();
    expect(within(table).getByText('Sem mods')).toBeDefined();
    expect(screen.getByText('7 de 118 mods ainda suspeitos')).toBeDefined();
    expect(
      within(header()).getByRole('button', { name: /Buscando o culpado: ver progresso/ }),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('cada rodada que termina atualiza a busca na tela', async () => {
    const run = deferred<BisectView>();
    let view = makeView({ rounds: [], current: null });
    const { backend, url } = setup({
      bisect_get: () => view,
      bisect_start: () => run.promise,
    });
    await openPage(url);
    const user = userEvent.setup();
    await user.click(within(header()).getByRole('button', { name: 'Mais opções do teste' }));
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', {
        name: /Encontrar o mod culpado…/,
      }),
    );
    const dialog = await screen.findByRole('dialog', { name: 'Encontrar o mod culpado' });
    await within(dialog).findByText('11 a 19 rodadas · cerca de 15 a 26 minutos');
    await user.click(within(dialog).getByRole('button', { name: 'Começar a busca' }));
    await screen.findByText('Preparando a cópia do pack para a próxima rodada…');
    const channel = backend.callsOf('bisect_start')[0]?.args.onEvent;
    const reads = backend.callsOf('bisect_get').length;
    view = makeView();
    const round: OperationEvent = {
      type: 'round',
      index: 2,
      totalEstimate: [11, 19],
      enabled: 61,
      suspects: 7,
      result: { kind: 'failedOther' },
      asking: false,
    };
    act(() => {
      sendOnChannel(channel, [round]);
    });
    expect(await screen.findByText(/Rodada 3: abrindo o jogo com 71 dos 118 mods/)).toBeDefined();
    expect(backend.callsOf('bisect_get').length).toBeGreaterThan(reads);
    // No fim, a busca, os Travamentos e o grafo são lidos de novo.
    view = makeFound();
    const crashes = backend.callsOf('crashes_list').length;
    await act(async () => {
      run.resolve(view);
      await run.promise;
    });
    expect(
      await screen.findByRole('heading', {
        level: 1,
        name: /Culpado encontrado: Epic Fight \(junto com Supplementaries\)/,
      }),
    ).toBeDefined();
    await waitFor(() => {
      expect(backend.callsOf('crashes_list').length).toBeGreaterThanOrEqual(crashes);
    });
  });

  it('CA-T25-04: no modo assistido a rodada mostra as três respostas', async () => {
    const asking = makeView({
      symptom: { kind: 'assisted' },
      current: makeCurrent({ asking: true }),
    });
    const { backend, url } = setup({
      bisect_get: () => asking,
      bisect_answer: () => null,
    });
    await openPage(
      url,
      '/teste?modo=busca',
      /Buscando o mod culpado|Culpado encontrado|Busca cancelada/,
    );
    const panel = screen.getByRole('region', { name: 'Rodada 3 · modo assistido' });
    expect(
      within(panel).getByText(
        'O jogo está aberto com 71 mods. Faça no jogo o que causa o problema e diga o que aconteceu.',
      ),
    ).toBeDefined();
    const user = userEvent.setup();
    expect(within(panel).getByRole('button', { name: 'Não aconteceu' })).toBeDefined();
    expect(within(panel).getByRole('button', { name: 'Não deu para testar' })).toBeDefined();
    await user.click(within(panel).getByRole('button', { name: 'Aconteceu' }));
    await waitFor(() => {
      expect(backend.callsOf('bisect_answer')[0]?.args.answer).toBe('happened');
    });
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T25-04: um travamento detectado sozinho marca a rodada sem perguntar', async () => {
    const crashed = makeView({
      symptom: { kind: 'assisted' },
      rounds: [
        ...makeView().rounds,
        makeRound(3, { verdict: { kind: 'failedSame' }, anchor: 'crashReport' }),
      ],
      current: makeCurrent({ number: 4 }),
    });
    const { url } = setup({ bisect_get: () => crashed });
    await openPage(
      url,
      '/teste?modo=busca',
      /Buscando o mod culpado|Culpado encontrado|Busca cancelada/,
    );
    const table = screen.getByRole('table', { name: 'Rodadas da busca do culpado' });
    const row = within(table).getByRole('cell', { name: '3' }).closest('tr');
    expect(row?.textContent).toContain('Travou igual');
    expect(screen.queryByRole('button', { name: 'Aconteceu' })).toBeNull();
    expect(
      screen.getByText('Quando o jogo entrar, o Warden pergunta o que aconteceu.'),
    ).toBeDefined();
  });

  it('pausar espera a rodada e avisa', async () => {
    const { backend, url } = setup({
      bisect_get: () => makeView(),
      bisect_pause: () => null,
    });
    await openPage(
      url,
      '/teste?modo=busca',
      /Buscando o mod culpado|Culpado encontrado|Busca cancelada/,
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Pausar depois desta rodada' }));
    expect(await screen.findByText('A busca pausa quando esta rodada terminar.')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Pausa pedida' })).toHaveProperty('disabled', true);
    expect(backend.callsOf('bisect_pause')).toHaveLength(1);
  });

  it('pausada: "Continuar a busca" continua do estado guardado', async () => {
    const { backend, url } = setup({
      bisect_get: () => makeView({ status: 'paused', current: null }),
      bisect_resume: () => deferred<BisectView>().promise,
    });
    await openPage(
      url,
      '/teste?modo=busca',
      /Buscando o mod culpado|Culpado encontrado|Busca cancelada/,
    );
    expect(
      screen.getByText('Pausada antes da rodada 3 · 7 mods ainda suspeitos · 9 min até agora'),
    ).toBeDefined();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Continuar a busca' }));
    await waitFor(() => {
      expect(backend.callsOf('bisect_resume')).toHaveLength(1);
    });
  });

  it('cancelar: confirmação, parcial e "O culpado está entre estes 7 mods"', async () => {
    let view = makeView({ status: 'paused', current: null });
    const { backend, url } = setup({
      bisect_get: () => view,
      bisect_cancel: () => {
        view = makeView({
          status: 'finished',
          current: null,
          outcome: { kind: 'cancelled', suspects: makeView().suspects.map((entry) => entry.path) },
          outcomeItems: makeView().suspects,
        });
        return null;
      },
    });
    await openPage(
      url,
      '/teste?modo=busca',
      /Buscando o mod culpado|Culpado encontrado|Busca cancelada/,
    );
    expect(screen.getByText('Busca pausada.')).toBeDefined();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Cancelar busca' }));
    const confirm = await screen.findByRole('alertdialog', {
      name: 'Cancelar a busca do culpado?',
    });
    await user.click(within(confirm).getByRole('button', { name: 'Cancelar busca' }));
    expect(await screen.findByText('O culpado está entre estes 7 mods.')).toBeDefined();
    expect(
      screen.getByText('A busca parou antes do fim, mas já descartou 111 mods.'),
    ).toBeDefined();
    expect(backend.callsOf('bisect_cancel')).toHaveLength(1);
  });
});

describe('resultado', () => {
  it('culpado encontrado: confiança, ações e outro problema', async () => {
    const found = makeFound({
      outcome: {
        kind: 'found',
        culprits: ['mods/epic-fight.pw.toml'],
        confidence: 'high',
        anotherProblem: true,
      },
      outcomeItems: [item('Epic Fight')],
    });
    const { backend, url } = setup({
      bisect_get: () => found,
      bisect_continue_without: () => deferred<BisectView>().promise,
    });
    await openPage(url, '/teste?modo=busca', /Culpado encontrado: Epic Fight$/);
    expect(screen.getByText('Alta: confirmado em 2 testes de controle')).toBeDefined();
    expect(
      screen.getByRole('button', { name: 'Ver o que o Epic Fight altera no jogo' }),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Remover Epic Fight do pack' })).toBeDefined();
    expect(screen.getByText('Há outro problema além deste.')).toBeDefined();
    expect(
      screen.getByText(
        'O resultado fica em Problemas → Travamentos, junto com o travamento de origem.',
      ),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Continuar a busca sem o Epic Fight' }));
    await waitFor(() => {
      expect(backend.callsOf('bisect_continue_without')).toHaveLength(1);
    });
  });

  it.each([
    [{ kind: 'notReproduced' as const }, 'O problema não se repetiu'],
    [{ kind: 'notAMod' as const }, 'Mesmo sem mods o jogo trava'],
    [{ kind: 'environment' as const }, 'O problema não é de um mod'],
    [{ kind: 'contradiction' as const, preventers: [] }, 'Os testes se contradizem'],
  ])('mostra o fim %o', async (outcome, title) => {
    const { url } = setup({
      bisect_get: () => makeView({ status: 'finished', current: null, outcome }),
    });
    await openPage(url, '/teste?modo=busca', title);
    expect(screen.getByRole('button', { name: 'Começar de novo' })).toBeDefined();
  });
});

describe('entradas pelo travamento', () => {
  it('Travamentos: "Encontrar o mod culpado" leva o travamento de origem; "Ver a busca do culpado"', async () => {
    const group = makeGroup({
      links: { culpritSearches: ['20261018-150200-000'], conversations: [] },
    });
    const { backend, url } = setup({
      diagnostics_report_get: () => makeReport({ findings: [] }),
      crashes_list: () => makeCrashList({ groups: [group] }),
      bisect_start: () => deferred<BisectView>().promise,
    });
    renderApp(`${url}/problemas`);
    const block = await screen.findByRole('region', { name: /^Travamentos/ }, { timeout: 10_000 });
    const user = userEvent.setup();
    const find = await within(block).findByRole('button', {
      name: /Encontrar o mod culpado do travamento de/,
    });
    expect(within(block).getByRole('link', { name: 'Ver a busca do culpado' })).toBeDefined();
    await user.click(find);
    const dialog = await screen.findByRole('dialog', { name: 'Encontrar o mod culpado' });
    expect(within(dialog).getByRole('radio', { name: /Trava ao entrar no mundo/ })).toHaveProperty(
      'checked',
      true,
    );
    await within(dialog).findByText('11 a 19 rodadas · cerca de 15 a 26 minutos');
    await user.click(within(dialog).getByRole('button', { name: 'Começar a busca' }));
    await waitFor(() => {
      expect(backend.callsOf('bisect_start')[0]?.args.request).toMatchObject({
        originCrash: group.signature.hash,
      });
    });
  });

  it('resultado "O jogo travou": oferece a busca com o travamento de origem', async () => {
    const session = makeSession({ outcome: 'crashed', exitCode: -1 });
    const { url } = setup({
      test_sessions_list: () => [session],
      test_session_get: () => makeSessionView(session, []),
      diagnostics_crash_get: () => null,
      crashes_list: () =>
        makeCrashList({ groups: [makeGroup({ latestSessionId: session.id, count: 3 })] }),
    });
    await openPage(url, '/teste', /travou/i);
    expect(
      await screen.findByText(/Este travamento já aconteceu 3 vezes\./, undefined, {
        timeout: 5_000,
      }),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Encontrar o mod culpado…' })).toBeDefined();
  });
});
