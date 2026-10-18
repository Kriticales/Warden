/**
 * Dados dos testes de componente da busca do culpado (D-12), como o backend manda.
 */
import type {
  BisectCurrent,
  BisectItem,
  BisectPlan,
  BisectRoundView,
  BisectView,
  Estimate,
} from '../../lib/ipc/bindings';

const NOW = Date.now();

export function item(name: string): BisectItem {
  return { path: `mods/${name.toLowerCase().replace(/\s+/g, '-')}.pw.toml`, name };
}

export const SUSPECTS = [
  'Epic Fight',
  "Farmer's Delight",
  'Jade',
  'Sophisticated Core',
  'Supplementaries',
  'Waystones',
  "Xaero's World Map",
].map(item);

export function makeEstimate(overrides: Partial<Estimate> = {}): Estimate {
  return {
    roundsMin: 11,
    roundsMax: 19,
    minutesMin: 15,
    minutesMax: 26,
    worldMs: 69_000,
    measured: true,
    ...overrides,
  };
}

export function makePlan(overrides: Partial<BisectPlan> = {}): BisectPlan {
  return {
    searched: 118,
    alwaysOn: [item('Fabric API')],
    disabled: [item('spark'), item('Crash Assistant')],
    estimate: makeEstimate(),
    minecraft: '1.20.1',
    loader: 'forge',
    quickPlay: true,
    world: 'Mundo de teste',
    ...overrides,
  };
}

export function makeRound(
  number: number,
  overrides: Partial<BisectRoundView> = {},
): BisectRoundView {
  return {
    number,
    kind: { kind: 'prefix', size: 59, of: 118, withFound: false },
    repeat: 1,
    enabledCount: 61,
    verdict: { kind: 'passed' },
    durationMs: 88_000,
    anchor: null,
    hasLog: true,
    ...overrides,
  };
}

/** A rodada 3 em andamento: os primeiros 69 mods, 3 acrescentados agora. */
export function makeCurrent(overrides: Partial<BisectCurrent> = {}): BisectCurrent {
  return {
    number: 3,
    kind: { kind: 'prefix', size: 69, of: 118, withFound: false },
    repeat: 1,
    enabledCount: 71,
    added: ["Farmer's Delight", 'Jade', 'Supplementaries'],
    found: [],
    asking: false,
    startedAtMs: NOW - 48_000,
    ...overrides,
  };
}

/** Busca em andamento na rodada 3 com 7 suspeitos. */
export function makeView(overrides: Partial<BisectView> = {}): BisectView {
  return {
    id: '20261018-150200-000',
    status: 'running',
    symptom: { kind: 'crashOnWorld' },
    originCrash: '0a1b2c3d4e5f6071',
    rounds: [
      makeRound(0, {
        kind: { kind: 'reproduce', attempt: 1 },
        enabledCount: 121,
        verdict: { kind: 'failedSame' },
        anchor: 'joinWorld',
      }),
      makeRound(1, { kind: { kind: 'control' }, enabledCount: 1, durationMs: 44_000 }),
      makeRound(2, { verdict: { kind: 'failedOther' }, anchor: 'crashReport' }),
    ],
    current: makeCurrent(),
    suspects: SUSPECTS,
    searched: 118,
    alwaysOn: [item('Fabric API')],
    disabled: [item('spark'), item('Crash Assistant')],
    estimate: makeEstimate(),
    repetitions: 1,
    elapsedMs: 9 * 60_000,
    startedAtMs: NOW - 10 * 60_000,
    finishedAtMs: null,
    outcome: null,
    outcomeItems: [],
    inferred: [],
    earlier: [],
    ...overrides,
  };
}

/** Busca terminada com o par Epic Fight + Supplementaries. */
export function makeFound(overrides: Partial<BisectView> = {}): BisectView {
  const culprits = [item('Epic Fight'), item('Supplementaries')];
  return makeView({
    status: 'finished',
    current: null,
    suspects: culprits,
    finishedAtMs: NOW,
    outcome: {
      kind: 'found',
      culprits: culprits.map((culprit) => culprit.path),
      confidence: 'high',
      anotherProblem: false,
    },
    outcomeItems: culprits,
    ...overrides,
  });
}
