/**
 * O desempenho do teste em andamento, a partir das amostras `perfSample` do canal do
 * `test_start` (ARCHITECTURE §7.7; L-10): a RAM do processo, o pico, os tempos até carregar e
 * até entrar no mundo e a última sessão de outra versão do pack para a comparação. A L-11
 * acrescenta a memória do jogo e as coletas.
 */
import type { OperationEvent, PreviousLoad, SessionPerf } from '../../../lib/ipc/bindings';

export type PerfSampleEvent = Extract<OperationEvent, { type: 'perfSample' }>;

export interface PerfState {
  /** RAM do processo e dos filhos agora, em MB (`null` antes da primeira medição). */
  rssMb: number | null;
  /** Pico da RAM do processo, em MB. */
  peakRssMb: number | null;
  /** Até carregar, em ms. */
  loadedMs: number | null;
  /** Até entrar no mundo, em ms. */
  worldMs: number | null;
  /** A última sessão de outra versão do pack. */
  previous: PreviousLoad | null;
}

/** O estado depois de uma amostra (cada amostra traz tudo; a mais nova vale). */
export function applyPerfSample(event: PerfSampleEvent): PerfState {
  return {
    rssMb: event.rssMb,
    peakRssMb: event.peakRssMb,
    loadedMs: event.loadedMs,
    worldMs: event.worldMs,
    previous: event.previous,
  };
}

/** O desempenho gravado de uma sessão, no mesmo formato. */
export function fromSession(perf: SessionPerf): PerfState {
  return {
    rssMb: null,
    peakRssMb: perf.peakRssMb ?? null,
    loadedMs: perf.loadedMs,
    worldMs: perf.worldMs,
    previous: perf.previous ?? null,
  };
}
