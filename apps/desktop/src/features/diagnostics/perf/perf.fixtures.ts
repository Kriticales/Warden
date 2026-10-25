/**
 * Dados de teste do bloco "Desempenho do último teste": a 0.1.2 abre 24% mais lenta que a 0.1.1
 * (critério da T33 cumprido), com a memória do jogo no limite e um perfil do spark.
 */
import type { LastTestPerf } from '../../../lib/ipc/bindings';

export const PERF_SESSION_ID = '2026-10-20_11-02-00';

export function makeLastTestPerf(overrides: Partial<LastTestPerf> = {}): LastTestPerf {
  return {
    sessionId: PERF_SESSION_ID,
    startedAtMs: Date.UTC(2026, 9, 20, 14, 2),
    packVersion: '0.1.2',
    mode: 'normal',
    loadedMs: 108_000,
    worldMs: 151_000,
    previous: { sessionId: '2026-10-19_10-00-00', packVersion: '0.1.1', loadedMs: 87_000 },
    memory: { usedMb: 5939, maxMb: 6144, nearLimit: true },
    ramPeakMb: 7100,
    tick: { msptMedian: 38, msptMax: 120, msptP95: 52, tps: 19.6, ticks: 1200 },
    sparkTop: [
      { name: 'Create', item: 'mods/create.pw.toml', modId: 'create', percent: 21 },
      { name: 'Distant Horizons', item: null, modId: 'distanthorizons', percent: 14 },
      { name: 'Iris', item: 'mods/iris.pw.toml', modId: 'iris', percent: 9 },
    ],
    comparison: {
      status: 'heavier',
      version: '0.1.2',
      previousVersion: '0.1.1',
      metric: 'loadTime',
      current: 108_000,
      previous: 87_000,
      percent: 24,
    },
    ...overrides,
  };
}
