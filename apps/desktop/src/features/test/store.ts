/**
 * O teste em andamento, fora de qualquer tela (Zustand; ARCHITECTURE §18): dá para ir a outras
 * seções com o jogo aberto e voltar pelo botão do cabeçalho sem perder etapas nem console.
 *
 * - `game`: o jogo aberto (ou em preparação) no Warden inteiro, pelo evento `game-state`
 *   (`useGameStateSync`, ligado uma vez na raiz). É o que o botão Testar de todos os packs
 *   lê ("um jogo por vez").
 * - `runs`: o teste de cada pack começado nesta sessão do app, com etapa, progresso, avisos,
 *   console e o resultado, pelo canal do `test_start`.
 * - `packChanged`: o pack mudou (`pack-changed`) com o jogo dele aberto: o cabeçalho avisa que
 *   as mudanças valem no próximo teste.
 */
import { create } from 'zustand';

import {
  commands,
  type AppError,
  type ConsoleLine,
  type GameState,
  type OperationEvent,
  type OperationId,
  type PackId,
  type Progress,
  type TestRequest,
  type TestSessionSummary,
} from '../../lib/ipc/bindings';
import { operationChannel } from '../../lib/ipc/channel';

/** Linhas guardadas por teste (SPEC T13: "Mostra até 50.000 linhas na tela"). */
export const MAX_CONSOLE_LINES = 50_000;

export type RunStatus = 'preparing' | 'running' | 'finished' | 'failed' | 'cancelled';

/** Um teste começado nesta sessão do app. */
export interface TestRun {
  packId: PackId;
  status: RunStatus;
  request: TestRequest;
  operationId: OperationId | null;
  stage: { id: string; labelKey: string } | null;
  progress: Progress | null;
  /** Etapas já vistas (para o indicador de etapas não voltar). */
  stagesSeen: string[];
  lines: ConsoleLine[];
  warnings: AppError[];
  error: AppError | null;
  result: TestSessionSummary | null;
  startedAtMs: number;
  stopping: boolean;
  /** Teste reencontrado depois de a interface recarregar: sem canal, o console vem por
   * consulta e o resultado, da lista de sessões. */
  adopted: boolean;
}

interface TestStore {
  game: GameState | null;
  runs: Partial<Record<PackId, TestRun>>;
  packChanged: Partial<Record<PackId, boolean>>;
  /** Aplica um `game-state`. */
  setGame: (state: GameState | null) => void;
  /** O pack mudou (evento `pack-changed`). */
  notePackChanged: (packId: PackId) => void;
  /** Começa o teste do pack. Volta quando o jogo fechou (ou o teste falhou). */
  start: (packId: PackId, request?: TestRequest) => Promise<void>;
  /** "Parar jogo" ou cancelar a preparação. */
  stop: (packId: PackId) => Promise<void>;
  /** Linhas do teste aberto lidas do backend (a interface recarregou no meio do teste). */
  adoptLiveConsole: (packId: PackId, lines: ConsoleLine[]) => void;
  /** Esquece o resultado do teste anterior (outra tela de resultado foi escolhida). */
  clearRun: (packId: PackId) => void;
}

export const DEFAULT_REQUEST: TestRequest = {
  mode: 'normal',
  profile: null,
  replaceInstanceChanges: false,
};

function newRun(packId: PackId, request: TestRequest): TestRun {
  return {
    packId,
    status: 'preparing',
    request,
    operationId: null,
    stage: null,
    progress: null,
    stagesSeen: [],
    lines: [],
    warnings: [],
    error: null,
    result: null,
    startedAtMs: Date.now(),
    stopping: false,
    adopted: false,
  };
}

/** Junta linhas novas mantendo só as últimas `MAX_CONSOLE_LINES`. */
export function appendLines(lines: ConsoleLine[], more: readonly ConsoleLine[]): ConsoleLine[] {
  const joined = lines.concat(more);
  return joined.length > MAX_CONSOLE_LINES
    ? joined.slice(joined.length - MAX_CONSOLE_LINES)
    : joined;
}

/** O efeito de um evento do canal no teste. */
export function applyEvent(run: TestRun, event: OperationEvent): TestRun {
  switch (event.type) {
    case 'started':
      return { ...run, operationId: event.operationId };
    case 'stage':
      return {
        ...run,
        stage: { id: event.stage, labelKey: event.labelKey },
        progress: null,
        stagesSeen: run.stagesSeen.includes(event.stage)
          ? run.stagesSeen
          : [...run.stagesSeen, event.stage],
      };
    case 'progress':
      return { ...run, progress: { current: event.current, total: event.total, unit: event.unit } };
    case 'warning':
      return { ...run, warnings: [...run.warnings, event.error] };
    case 'console':
      return {
        ...run,
        status: run.status === 'preparing' ? 'running' : run.status,
        lines: appendLines(run.lines, event.lines),
      };
    case 'log':
    case 'perfSample':
    case 'finished':
      return run;
  }
}

/** Cópia do registro sem a chave. */
function without<T>(record: Partial<Record<PackId, T>>, key: PackId): Partial<Record<PackId, T>> {
  return Object.fromEntries(Object.entries(record).filter(([id]) => id !== key));
}

function isCancelled(error: AppError): boolean {
  return error.code.domain === 'core' && error.code.code === 'CANCELLED';
}

export const useTestStore = create<TestStore>()((set, get) => {
  const update = (packId: PackId, change: (run: TestRun) => TestRun) => {
    set((store) => {
      const run = store.runs[packId];
      return run ? { runs: { ...store.runs, [packId]: change(run) } } : {};
    });
  };

  return {
    game: null,
    runs: {},
    packChanged: {},
    setGame: (state) => {
      set((store) => {
        const game = state && state.state !== 'exited' ? state : null;
        // Um teste novo (ou o fim do anterior) zera o aviso.
        const packChanged =
          state && (state.state === 'exited' || state.state === 'preparing')
            ? without(store.packChanged, state.packId)
            : store.packChanged;
        let runs = store.runs;
        const run = state ? runs[state.packId] : undefined;
        if (state?.state === 'running' && run?.status === 'preparing') {
          runs = { ...runs, [state.packId]: { ...run, status: 'running' } };
        }
        if (state?.state === 'exited' && run?.adopted && isActive(run)) {
          runs = { ...runs, [state.packId]: { ...run, status: 'finished' } };
        }
        return { game, packChanged, runs };
      });
    },
    notePackChanged: (packId) => {
      if (get().game?.packId === packId) {
        set((store) => ({ packChanged: { ...store.packChanged, [packId]: true } }));
      }
    },
    start: async (packId, request = DEFAULT_REQUEST) => {
      set((store) => ({ runs: { ...store.runs, [packId]: newRun(packId, request) } }));
      const channel = operationChannel((event) => {
        update(packId, (run) => applyEvent(run, event));
      });
      const result = await commands.testStart(packId, request, channel);
      if (result.status === 'ok') {
        update(packId, (run) => ({
          ...run,
          status: 'finished',
          result: result.data,
          stopping: false,
        }));
      } else {
        const error = result.error;
        update(packId, (run) => ({
          ...run,
          status: isCancelled(error) ? 'cancelled' : 'failed',
          error: isCancelled(error) ? null : error,
          stopping: false,
        }));
      }
    },
    stop: async (packId) => {
      update(packId, (run) => ({ ...run, stopping: true }));
      const result = await commands.testStop(packId);
      if (result.status === 'error') {
        update(packId, (run) => ({ ...run, stopping: false }));
      }
    },
    adoptLiveConsole: (packId, lines) => {
      set((store) => {
        const current = store.runs[packId];
        if (current?.adopted && isActive(current)) {
          return {
            runs: { ...store.runs, [packId]: { ...current, lines: appendLines([], lines) } },
          };
        }
        if (current) {
          return {};
        }
        const game = store.game?.packId === packId ? store.game : null;
        const run: TestRun = {
          ...newRun(packId, DEFAULT_REQUEST),
          status: game?.state === 'running' ? 'running' : 'preparing',
          operationId: game?.operationId ?? null,
          startedAtMs: game?.startedAtMs ?? Date.now(),
          lines: appendLines([], lines),
          adopted: true,
        };
        return { runs: { ...store.runs, [packId]: run } };
      });
    },
    clearRun: (packId) => {
      set((store) => ({ runs: without(store.runs, packId) }));
    },
  };
});

/** O teste deste pack, se houver. */
export function useTestRun(packId: PackId): TestRun | undefined {
  return useTestStore((store) => store.runs[packId]);
}

/** O jogo aberto no Warden. */
export function useGame(): GameState | null {
  return useTestStore((store) => store.game);
}

/** Se o teste está em andamento (preparando ou com o jogo aberto). */
export function isActive(run: TestRun | undefined): boolean {
  return run?.status === 'preparing' || run?.status === 'running';
}
