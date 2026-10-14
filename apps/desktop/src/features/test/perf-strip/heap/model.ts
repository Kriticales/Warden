/**
 * A memória do jogo a partir das amostras `perfSample` (L-11; ARCHITECTURE §7.7): a memória
 * usada e a máxima, as coletas, o gráfico dos últimos 5 minutos, o aviso de memória quase
 * cheia e, sem leitura, o motivo.
 */
import type { HeapDetails, HeapUnavailable } from '../../../../lib/ipc/bindings';
import type { PerfSampleEvent } from '../model';

export interface HeapState {
  /** Memória do jogo usada, em MB. */
  usedMb: number;
  /** Máximo, em MB. */
  maxMb: number;
  /** Coletas (ou pausas, no ZGC e no Shenandoah). */
  gcCount: number;
  /** Tempo parado nelas, em ms. */
  gcMs: number;
  /** Gráfico, pausas, "na última coleta" e o aviso. */
  details: HeapDetails;
}

/** A memória do jogo da amostra (`null` enquanto não há leitura). */
export function heapOf(event: PerfSampleEvent): HeapState | null {
  if (
    event.heap === null ||
    event.heapUsedMb === null ||
    event.heapMaxMb === null ||
    event.gcCount === null ||
    event.gcMs === null
  ) {
    return null;
  }
  return {
    usedMb: event.heapUsedMb,
    maxMb: event.heapMaxMb,
    gcCount: event.gcCount,
    gcMs: event.gcMs,
    details: event.heap,
  };
}

/** O que a faixa mostra da memória do jogo. */
export type HeapView =
  | { kind: 'measuring' }
  | { kind: 'reading'; heap: HeapState }
  | { kind: 'unavailable'; reason: HeapUnavailable };

export function heapView(heap: HeapState | null, reason: HeapUnavailable | null): HeapView {
  if (reason) return { kind: 'unavailable', reason };
  if (heap) return { kind: 'reading', heap };
  return { kind: 'measuring' };
}
