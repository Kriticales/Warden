/**
 * Faixa de desempenho e "Abriu em" no resultado (SPEC T13; CA-T13-09, parte da interface;
 * L-10): números no formato do pt-BR, estados antes da medição, a comparação com a última
 * versão testada, o fluxo do app com as amostras do canal e o axe.
 */
import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { OperationEvent, SessionPerf, TestSessionSummary } from '../../../lib/ipc/bindings';
import { axeComponent, axePage } from '../../../test/axe';
import { deferred } from '../../../test/backend';
import { renderApp, renderWithProviders } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { useTestStore } from '../store';
import { makeSession, sendOnChannel } from '../test.fixtures';
import { comparisonText, loadTimeText, memoryParts, memoryText } from './format';
import { applyPerfSample, fromSession, type PerfSampleEvent } from './model';
import { PerfStrip, PerfSummary } from './PerfStrip';

const PREVIOUS = {
  sessionId: '2026-10-12_20-00-00',
  packVersion: '1.4.2',
  loadedMs: 84_000,
} as const;

function sample(overrides: Partial<PerfSampleEvent> = {}): PerfSampleEvent {
  return {
    type: 'perfSample',
    rssMb: 4300,
    peakRssMb: 4710,
    loadedMs: 102_000,
    worldMs: 131_000,
    previous: PREVIOUS,
    heapUsedMb: null,
    heapMaxMb: null,
    gcCount: null,
    gcMs: null,
    ...overrides,
  };
}

function sessionPerf(overrides: Partial<SessionPerf> = {}): SessionPerf {
  return {
    peakRssMb: 4710,
    peakCommitMb: null,
    loadedMs: 102_000,
    worldMs: 131_000,
    samples: 720,
    previous: PREVIOUS,
    ...overrides,
  };
}

/** O texto de um item da faixa, pelo rótulo. */
function item(strip: HTMLElement, label: string): string {
  const lbl = within(strip).getByText(label);
  return lbl.parentElement?.textContent ?? '';
}

describe('números da faixa no formato do pt-BR', () => {
  it('memória com vírgula decimal e unidade', () => {
    expect(memoryParts(4300)).toEqual({ value: '4,2', unit: 'GB' });
    expect(memoryParts(1536)).toEqual({ value: '1,5', unit: 'GB' });
    expect(memoryParts(1024)).toEqual({ value: '1,0', unit: 'GB' });
    expect(memoryParts(512)).toEqual({ value: '512', unit: 'MB' });
    expect(memoryText(12_800)).toBe('12,5 GB');
  });

  it('tempos e comparação com a última versão', () => {
    expect(loadTimeText(300)).toBe('menos de 1 s');
    expect(loadTimeText(48_000)).toBe('48 s');
    expect(loadTimeText(102_000)).toBe('1 min 42 s');
    expect(comparisonText(102_000, PREVIOUS)).toBe('18 s a mais que na 1.4.2');
    expect(comparisonText(78_000, PREVIOUS)).toBe('6 s a menos que na 1.4.2');
    expect(comparisonText(84_400, PREVIOUS)).toBe('o mesmo tempo da 1.4.2');
  });

  it('cada amostra traz tudo; a sessão gravada vira o mesmo formato', () => {
    expect(applyPerfSample(sample({ rssMb: null, peakRssMb: null }))).toEqual({
      rssMb: null,
      peakRssMb: null,
      loadedMs: 102_000,
      worldMs: 131_000,
      previous: PREVIOUS,
    });
    expect(fromSession({ loadedMs: null, worldMs: null })).toEqual({
      rssMb: null,
      peakRssMb: null,
      loadedMs: null,
      worldMs: null,
      previous: null,
    });
  });
});

describe('faixa de desempenho com o jogo aberto', () => {
  it('antes da primeira amostra: medindo e carregando, sem violações do axe', async () => {
    const { container } = renderWithProviders(<PerfStrip perf={null} />);
    const strip = screen.getByRole('region', { name: 'Desempenho do jogo' });
    expect(item(strip, 'RAM do processo')).toContain('medindo…');
    expect(item(strip, 'Abriu em')).toContain('carregando…');
    expect(item(strip, 'Entrou no mundo em')).toContain('ainda não entrou num mundo');
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('com amostras: RAM e pico, "Abriu em" com a diferença e o mundo', async () => {
    const { container } = renderWithProviders(<PerfStrip perf={applyPerfSample(sample())} />);
    const strip = screen.getByRole('region', { name: 'Desempenho do jogo' });
    expect(item(strip, 'RAM do processo')).toBe('RAM do processo4,2 GBpico de 4,6 GB');
    expect(within(strip).getByText('GB', { selector: 'small' })).toBeDefined();
    expect(item(strip, 'Abriu em')).toBe('Abriu em1 min 42 s18 s a mais que na 1.4.2');
    expect(item(strip, 'Entrou no mundo em')).toBe('Entrou no mundo em2 min 11 s');
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('sem versão anterior para comparar, "Abriu em" fica sem a diferença', () => {
    renderWithProviders(
      <PerfStrip perf={applyPerfSample(sample({ previous: null, worldMs: null }))} />,
    );
    const strip = screen.getByRole('region', { name: 'Desempenho do jogo' });
    expect(item(strip, 'Abriu em')).toBe('Abriu em1 min 42 s');
  });
});

describe('"Abriu em" no resultado (CA-T13-09)', () => {
  const props = (session: TestSessionSummary) => ({
    packId: session.id as never,
    session,
    lines: [],
  });

  it('mostra o pico de RAM, "Abriu em" com a diferença e o mundo', async () => {
    const session = makeSession({ perf: sessionPerf() });
    const { container } = renderWithProviders(<PerfSummary {...props(session)} />);
    const block = screen.getByRole('region', { name: 'Desempenho do teste' });
    expect(item(block, 'Pico de RAM do processo')).toContain('4,6 GB');
    expect(item(block, 'Abriu em')).toBe('Abriu em1 min 42 s18 s a mais que na 1.4.2');
    expect(item(block, 'Entrou no mundo em')).toBe('Entrou no mundo em2 min 11 s');
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('travou antes de carregar: diz isso no lugar do tempo', () => {
    const session = makeSession({
      outcome: 'crashed',
      perf: sessionPerf({ loadedMs: null, worldMs: null, previous: null }),
    });
    renderWithProviders(<PerfSummary {...props(session)} />);
    const block = screen.getByRole('region', { name: 'Desempenho do teste' });
    expect(item(block, 'Abriu em')).toBe('Abriu em—o jogo travou antes de carregar');
    expect(item(block, 'Entrou no mundo em')).toContain('não entrou num mundo');
  });

  it('sessão de antes da medição não mostra nada', () => {
    const { container } = renderWithProviders(<PerfSummary {...props(makeSession())} />);
    expect(container.textContent).toBe('');
  });
});

describe('no app: amostras do canal na faixa e o resultado', () => {
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

  it('a faixa acompanha as amostras com o jogo aberto e o resultado mostra "Abriu em"', async () => {
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
    const user = userEvent.setup();
    await user.click(
      within(screen.getByRole('banner', { name: 'Pack aberto' })).getByRole('button', {
        name: 'Testar',
      }),
    );
    await screen.findByRole('heading', { level: 1, name: 'Testando o pack' });

    const opening: OperationEvent[] = [
      { type: 'stage', stage: 'test.launch', labelKey: 'teste:etapa.abrir' },
      { type: 'console', lines: [] },
      sample({ rssMb: null, peakRssMb: null, loadedMs: null, worldMs: null, previous: null }),
    ];
    act(() => {
      sendOnChannel(channel, opening);
    });
    await screen.findByRole('heading', { level: 1, name: 'Jogo aberto' });
    const strip = screen.getByRole('region', { name: 'Desempenho do jogo' });
    expect(item(strip, 'Abriu em')).toContain('carregando…');

    act(() => {
      sendOnChannel(channel, [sample({ rssMb: 3200, worldMs: null })]);
    });
    expect(item(strip, 'RAM do processo')).toContain('3,1 GB');
    expect(item(strip, 'Abriu em')).toBe('Abriu em1 min 42 s18 s a mais que na 1.4.2');
    expect(await axePage(document.body)).toHaveNoViolations();

    await act(async () => {
      finished.resolve(makeSession({ perf: sessionPerf() }));
      await Promise.resolve();
    });
    await screen.findByRole('heading', { level: 1, name: 'O jogo fechou normalmente' });
    const block = screen.getByRole('region', { name: 'Desempenho do teste' });
    expect(item(block, 'Abriu em')).toContain('18 s a mais que na 1.4.2');
    expect(screen.queryByRole('region', { name: 'Desempenho do jogo' })).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });
});
