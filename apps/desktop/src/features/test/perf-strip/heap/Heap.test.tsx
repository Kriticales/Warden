/**
 * Memória do jogo na faixa, os avisos abaixo dela e "Memória no último teste" em Problemas
 * (SPEC T13; L-11; CA-T13-10 e CA-T13-11, parte da interface): números no formato do pt-BR,
 * o gráfico, "Pausas de coleta", "na última coleta", "sem leitura" com o motivo, o aviso de
 * memória quase cheia com Ajustes do teste (o foco volta ao botão), os achados em Problemas
 * pelo mecanismo da D-03 e a atualização no fim de um teste. Com o axe.
 */
import { createMemoryHistory } from '@tanstack/react-router';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { App, createQueryClient } from '../../../../app/App';
import type {
  Finding,
  HeapDetails,
  SessionPerf,
  TestSessionSummary,
} from '../../../../lib/ipc/bindings';
import { axeComponent, axePage } from '../../../../test/axe';
import { deferred } from '../../../../test/backend';
import { makeAppError } from '../../../../test/factories';
import { renderApp, renderWithProviders } from '../../../../test/render';
import { editorBackend } from '../../../pack-editor/testing';
import { invalidateAfterTest, testKeys } from '../../api';
import { useTestStore } from '../../store';
import { makeSession, sendOnChannel } from '../../test.fixtures';
import type { PerfSampleEvent } from '../model';
import { applyPerfSample } from '../model';
import { PerfStrip } from '../PerfStrip';
import { countText, gcTimeText, memoryPhraseParams, seriesLabel } from './format';

const SERIES = [2458, 2662, 2560, 2867, 2765, 2970, 3072, 2662, 2867, 3174];

function details(overrides: Partial<HeapDetails> = {}): HeapDetails {
  return {
    countsPauses: false,
    atLastGc: false,
    seriesMb: SERIES,
    lowHeapS: null,
    suggestedMb: null,
    ...overrides,
  };
}

function sample(overrides: Partial<PerfSampleEvent> = {}): PerfSampleEvent {
  return {
    type: 'perfSample',
    rssMb: 4300,
    peakRssMb: 4710,
    loadedMs: 102_000,
    worldMs: 131_000,
    previous: null,
    heapUsedMb: 3174,
    heapMaxMb: 6144,
    gcCount: 214,
    gcMs: 1234,
    heap: details(),
    heapUnavailable: null,
    ...overrides,
  };
}

/** Sem leitura: nenhum número da memória do jogo. */
const NO_HEAP = {
  heapUsedMb: null,
  heapMaxMb: null,
  gcCount: null,
  gcMs: null,
  heap: null,
} as const;

/** O texto de um item da faixa, pelo rótulo. */
function item(strip: HTMLElement, label: string): string {
  const lbl = within(strip).getByText(label);
  return lbl.parentElement?.textContent ?? '';
}

function strip(): HTMLElement {
  return screen.getByRole('region', { name: 'Desempenho do jogo' });
}

const LOW_HEAP: Finding = {
  rule: 'W_LOW_HEAP',
  severity: 'warning',
  titleKey: 'diagnostico.regras.W_LOW_HEAP',
  params: { maxMb: '6144', suggestedMb: '8192', percent: '93', seconds: '72' },
  items: [],
  evidence: [
    {
      kind: 'measurement',
      sessionId: '2026-10-07_14-20-00',
      metric: 'heapAfterGc',
      values: { percent: '93', seconds: '72', maxMb: '6144' },
    },
  ],
  fixes: [{ kind: 'changeMemory' }],
};

const OVERSIZED: Finding = {
  rule: 'I_HEAP_OVERSIZED',
  severity: 'info',
  titleKey: 'diagnostico.regras.I_HEAP_OVERSIZED',
  params: { maxMb: '12288', peakMb: '3072', percent: '25' },
  items: [],
  evidence: [
    {
      kind: 'measurement',
      sessionId: '2026-10-07_14-20-00',
      metric: 'heapPeak',
      values: { peakMb: '3072', percent: '25', maxMb: '12288' },
    },
  ],
  fixes: [{ kind: 'changeMemory' }],
};

function perfWith(findings: Finding[]): SessionPerf {
  return { loadedMs: 102_000, worldMs: null, findings };
}

describe('números da memória do jogo no formato do pt-BR', () => {
  it('coletas, tempo parado, gráfico e as frases dos achados', () => {
    expect(countText(1902)).toBe('1.902');
    expect(gcTimeText(1234)).toBe('1,2 s');
    expect(gcTimeText(18_400)).toBe('18,4 s');
    expect(gcTimeText(125_000)).toBe('2 min 5 s');
    expect(seriesLabel(SERIES)).toBe('Memória nos últimos 5 minutos: entre 2,4 GB e 3,1 GB');
    expect(seriesLabel([3174, 3174])).toBe('Memória nos últimos 5 minutos: 3,1 GB');
    expect(seriesLabel([])).toBe('');
    expect(
      memoryPhraseParams({
        maxMb: '6144',
        suggestedMb: '8192',
        seconds: '72',
        percent: '93',
        peakMb: '3072',
        outro: 'x',
      }),
    ).toEqual({
      memoria: '6 GB',
      sugerida: '8 GB',
      duracao: '1 min 12 s',
      percentual: '93%',
      pico: '3 GB',
    });
    expect(memoryPhraseParams({ maxMb: 'abc' })).toEqual({});
  });
});

describe('"Memória do jogo" e "Coletas de memória" na faixa', () => {
  it('com leitura: memória de máximo, gráfico e coletas com o tempo parado; axe', async () => {
    const { container } = renderWithProviders(<PerfStrip perf={applyPerfSample(sample())} />);
    expect(item(strip(), 'Memória do jogo')).toBe('Memória do jogo3,1 GB de 6 GB');
    expect(
      within(strip()).getByRole('img', {
        name: 'Memória nos últimos 5 minutos: entre 2,4 GB e 3,1 GB',
      }),
    ).toBeDefined();
    expect(item(strip(), 'Coletas de memória')).toBe(
      'Coletas de memória2141,2 s parado em coletas',
    );
    // O que a L-10 já mostrava continua igual.
    expect(item(strip(), 'RAM do processo')).toBe('RAM do processo4,2 GBpico de 4,6 GB');
    expect(item(strip(), 'Entrou no mundo em')).toBe('Entrou no mundo em2 min 11 s');
    expect(strip().className).not.toContain('perf--warn');
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('antes da primeira leitura: medindo, sem número', () => {
    renderWithProviders(<PerfStrip perf={applyPerfSample(sample(NO_HEAP))} />);
    expect(item(strip(), 'Memória do jogo')).toBe('Memória do jogomedindo…');
    expect(item(strip(), 'Coletas de memória')).toBe('Coletas de memóriamedindo…');
    expect(within(strip()).queryByRole('img')).toBeNull();
  });

  it('ZGC e Shenandoah contam pausas: "Pausas de coleta"', () => {
    renderWithProviders(
      <PerfStrip perf={applyPerfSample(sample({ heap: details({ countsPauses: true }) }))} />,
    );
    expect(within(strip()).queryByText('Coletas de memória')).toBeNull();
    expect(item(strip(), 'Pausas de coleta')).toBe('Pausas de coleta2141,2 s parado em pausas');
  });

  it('Java 25 com Parallel ou Serial: a memória da última coleta, com a legenda', () => {
    renderWithProviders(
      <PerfStrip perf={applyPerfSample(sample({ heap: details({ atLastGc: true }) }))} />,
    );
    expect(item(strip(), 'Memória do jogo')).toContain('na última coleta');
  });

  it('sem leitura: "sem leitura" e o motivo, sem número inventado (CA-T13-10)', async () => {
    const { container } = renderWithProviders(
      <PerfStrip
        perf={applyPerfSample(
          sample({
            ...NO_HEAP,
            heapUnavailable: { kind: 'argument', argument: '-XX:+PerfDisableSharedMem' },
          }),
        )}
      />,
    );
    expect(item(strip(), 'Memória do jogo')).toBe(
      'Memória do jogosem leituraOs argumentos do teste têm -XX:+PerfDisableSharedMem.',
    );
    expect(item(strip(), 'Coletas de memória')).toBe('Coletas de memória—sem leitura');
    expect(await axeComponent(container)).toHaveNoViolations();
  });
});

describe('avisos abaixo da faixa, no app (CA-T13-10 e CA-T13-11)', () => {
  beforeEach(() => {
    vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (
      this: HTMLElement,
    ) {
      if (this.classList.contains('console__log')) return 600;
      return this.classList.contains('console__line') ? 20 : 0;
    });
  });

  afterEach(() => {
    useTestStore.setState({ game: null, runs: {}, packChanged: {} });
    vi.restoreAllMocks();
  });

  /** Abre o app, começa o teste e devolve o canal com o jogo aberto. */
  async function openGame(): Promise<{
    channel: unknown;
    finished: ReturnType<typeof deferred<TestSessionSummary>>;
  }> {
    const finished = deferred<TestSessionSummary>();
    let channel: unknown = null;
    const { url } = editorBackend({
      handlers: {
        test_start: ({ onEvent }) => {
          channel = onEvent;
          return finished.promise;
        },
      },
    });
    renderApp(url);
    await screen.findByRole('heading', { level: 1, name: 'Mods' }, { timeout: 10_000 });
    await userEvent.setup().click(
      within(screen.getByRole('banner', { name: 'Pack aberto' })).getByRole('button', {
        name: 'Testar',
      }),
    );
    await screen.findByRole('heading', { level: 1, name: 'Testando o pack' });
    act(() => {
      sendOnChannel(channel, [
        { type: 'stage', stage: 'test.launch', labelKey: 'teste:etapa.abrir' },
        { type: 'console', lines: [] },
      ]);
    });
    await screen.findByRole('heading', { level: 1, name: 'Jogo aberto' });
    return { channel, finished };
  }

  it('memória quase cheia: faixa em aviso, quanto aumentar e Ajustes do teste (o foco volta)', async () => {
    const { channel } = await openGame();
    act(() => {
      sendOnChannel(channel, [sample()]);
    });
    expect(screen.queryByText('O jogo está quase sem memória.')).toBeNull();
    act(() => {
      sendOnChannel(channel, [
        sample({
          heapUsedMb: 5734,
          heap: details({ lowHeapS: 125, suggestedMb: 8192, seriesMb: [5000, 5734] }),
        }),
      ]);
    });
    expect(strip().className).toContain('perf--warn');
    const alert = screen.getByText('O jogo está quase sem memória.').closest('.alert');
    if (!(alert instanceof HTMLElement)) throw new Error('aviso de memória');
    expect(alert.textContent).toContain(
      'Depois de limpar a memória, ele continua usando mais de 90% dos 6 GB há 2 min 5 s. Aumente para 8 GB em Ajustes do teste.',
    );
    expect(await axePage(document.body)).toHaveNoViolations();

    const user = userEvent.setup();
    const button = within(alert).getByRole('button', { name: 'Ajustes do teste…' });
    await user.click(button);
    const dialog = await screen.findByRole('dialog', { name: 'Ajustes do teste neste computador' });
    expect(dialog).toBeDefined();
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(button);
    });
  });

  it('argumento que desliga a leitura: o motivo, Ajustes do teste e sem o aviso repetido', async () => {
    const { channel } = await openGame();
    const argument = { kind: 'argument', argument: '-XX:+PerfDisableSharedMem' } as const;
    act(() => {
      sendOnChannel(channel, [
        {
          type: 'warning',
          error: makeAppError(
            { domain: 'app', code: 'TEST_PERF_FLAGS' },
            { params: { args: '-XX:+PerfDisableSharedMem' } },
          ),
        },
      ]);
    });
    // Antes da primeira amostra, o aviso do começo do teste (L-08).
    expect(screen.getByText(/impedem o Warden de ler a memória do jogo/)).toBeDefined();
    act(() => {
      sendOnChannel(channel, [sample({ ...NO_HEAP, heapUnavailable: argument })]);
    });
    const alert = screen
      .getByText('A memória do jogo não pode ser lida neste teste.')
      .closest('.alert');
    if (!(alert instanceof HTMLElement)) throw new Error('aviso sem leitura');
    expect(alert.textContent).toContain(
      'O argumento -XX:+PerfDisableSharedMem, comum nas “flags do Aikar”, desliga a leitura. Remova-o em Ajustes do teste.',
    );
    expect(within(alert).getByRole('button', { name: 'Ajustes do teste…' })).toBeDefined();
    expect(screen.queryByText(/impedem o Warden de ler a memória do jogo/)).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('o Java não publica: o motivo, sem botão (não há o que remover)', async () => {
    const { channel } = await openGame();
    act(() => {
      sendOnChannel(channel, [sample({ ...NO_HEAP, heapUnavailable: { kind: 'notPublished' } })]);
    });
    expect(item(strip(), 'Memória do jogo')).toContain(
      'O Java deste teste não publica a memória do jogo.',
    );
    const alert = screen
      .getByText('A memória do jogo não pode ser lida neste teste.')
      .closest('.alert');
    if (!(alert instanceof HTMLElement)) throw new Error('aviso sem leitura');
    expect(within(alert).queryByRole('button')).toBeNull();
  });
});

describe('"Memória no último teste" em Problemas', () => {
  function renderBlock(sessions: TestSessionSummary[]) {
    return editorBackend({ handlers: { test_sessions_list: () => sessions } });
  }

  it('os achados do último teste com a evidência e a correção; axe', async () => {
    const { url } = renderBlock([makeSession({ perf: perfWith([LOW_HEAP, OVERSIZED]) })]);
    renderApp(`${url}/problemas`);
    const block = await screen.findByRole(
      'region',
      { name: 'Memória no último teste' },
      { timeout: 10_000 },
    );
    expect(
      within(block).getByRole('heading', { name: 'O jogo está quase sem memória' }),
    ).toBeDefined();
    expect(block.textContent).toContain(
      'Depois das coletas, ele continuou usando 93% dos 6 GB por 1 min 12 s. Aumente para 8 GB em Ajustes do teste.',
    );
    expect(block.textContent).toContain('Medido pelo Warden durante o teste:');
    expect(block.textContent).toContain(
      'Memória depois das coletas: 93% de 6 GB, por 1 min 12 s seguidos.',
    );
    expect(within(block).getByRole('heading', { name: 'Memória sobrando' })).toBeDefined();
    expect(block.textContent).toContain('O jogo nunca passou de 3 GB (25%) dos 12 GB do teste.');
    expect(
      within(block).getByRole('button', { name: 'Diminuir a memória do teste' }),
    ).toBeDefined();
    const user = userEvent.setup();
    await user.click(within(block).getByRole('button', { name: 'Aumentar a memória do teste' }));
    expect(
      await screen.findByRole('dialog', { name: 'Ajustes do teste neste computador' }),
    ).toBeDefined();
    await user.keyboard('{Escape}');
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('só o teste mais novo conta: um teste sem achados depois de um com achados esconde o bloco', async () => {
    const older = makeSession({ id: '2026-10-07_14-20-00', perf: perfWith([LOW_HEAP]) });
    const newer = makeSession({ id: '2026-10-07_15-00-00', perf: perfWith([]) });
    const { url } = renderBlock([newer, older]);
    renderApp(`${url}/problemas`);
    await screen.findByRole('heading', { level: 1, name: 'Problemas' }, { timeout: 10_000 });
    await screen.findByRole('region', { name: 'Último travamento' });
    expect(screen.queryByRole('region', { name: 'Memória no último teste' })).toBeNull();
  });

  it('o fim de um teste relê as sessões e o bloco acompanha (dados sempre atuais)', async () => {
    let sessions: TestSessionSummary[] = [makeSession({ perf: perfWith([]) })];
    const { row, url } = editorBackend({ handlers: { test_sessions_list: () => sessions } });
    const queryClient = createQueryClient();
    render(
      <App
        history={createMemoryHistory({ initialEntries: [`${url}/problemas`] })}
        queryClient={queryClient}
      />,
    );
    await screen.findByRole('region', { name: 'Último travamento' }, { timeout: 10_000 });
    expect(screen.queryByRole('region', { name: 'Memória no último teste' })).toBeNull();
    // O teste terminou com um achado de memória: o fim do teste invalida as sessões (e o resto
    // que mostra o teste), e o bloco aparece sem sair da seção.
    sessions = [
      makeSession({ id: '2026-10-07_15-00-00', perf: perfWith([LOW_HEAP]) }),
      ...sessions,
    ];
    await act(async () => {
      await invalidateAfterTest(queryClient, row.id);
    });
    expect(await screen.findByRole('region', { name: 'Memória no último teste' })).toBeDefined();
    expect(queryClient.getQueryState(testKeys.sessions(row.id))?.isInvalidated).toBe(false);
  });
});
