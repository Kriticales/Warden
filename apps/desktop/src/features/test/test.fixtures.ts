/**
 * Dados dos testes de componente do Testar: sessões, jogo aberto, linhas de console e o canal
 * do `test_start` simulado.
 */
import type {
  ConsoleLine,
  GameState,
  OperationEvent,
  PackId,
  TestSessionSummary,
  TestSessionView,
} from '../../lib/ipc/bindings';

const NOW = Date.now();

export function makeSession(overrides: Partial<TestSessionSummary> = {}): TestSessionSummary {
  return {
    id: '2026-10-07_14-20-00',
    startedAtMs: NOW - 60 * 60_000,
    endedAtMs: NOW - 48 * 60_000,
    durationMs: 12 * 60_000,
    outcome: 'closedNormally',
    exitCode: 0,
    packVersion: '1.4.2',
    minecraft: '1.20.1',
    loader: 'Forge',
    loaderVersion: '47.3.0',
    javaMajor: 17,
    memoryMb: 6144,
    mode: 'normal',
    crashReports: [],
    hsErrFiles: [],
    ...overrides,
  };
}

export function gameLine(text: string, overrides: Partial<ConsoleLine> = {}): ConsoleLine {
  return {
    seq: 0,
    atMs: NOW,
    time: null,
    level: 'info',
    logger: 'minecraft/Minecraft',
    thread: 'Render thread',
    text,
    params: {},
    origin: 'game',
    stream: 'stdout',
    ...overrides,
  };
}

export function wardenLine(
  key: string,
  params: Record<string, string>,
  overrides: Partial<ConsoleLine> = {},
): ConsoleLine {
  return {
    ...gameLine(key),
    logger: null,
    thread: null,
    origin: 'warden',
    stream: null,
    params,
    ...overrides,
  };
}

export function makeSessionView(
  summary: TestSessionSummary,
  lines: ConsoleLine[] = [],
): TestSessionView {
  return { summary, lines, truncated: false };
}

export function makeGame(
  packId: PackId,
  state: GameState['state'],
  overrides: Partial<GameState> = {},
): GameState {
  return {
    packId,
    packName: 'Vale Sereno',
    state,
    sessionId: state === 'running' ? '2026-10-07_15-00-00' : null,
    operationId: null,
    profile: null,
    startedAtMs: NOW,
    ...overrides,
  };
}

interface TauriInternals {
  runCallback: (id: number, data: unknown) => void;
}

/** Manda eventos pelo canal (`Channel`) que o `test_start` recebeu, como o Rust faria. */
export function sendOnChannel(channel: unknown, events: readonly OperationEvent[]): void {
  const { id } = channel as { id: number };
  const internals = (window as unknown as { __TAURI_INTERNALS__: TauriInternals })
    .__TAURI_INTERNALS__;
  const sent = sentCount.get(id) ?? 0;
  events.forEach((message, offset) => {
    internals.runCallback(id, { index: sent + offset, message });
  });
  sentCount.set(id, sent + events.length);
}

const sentCount = new Map<number, number>();
