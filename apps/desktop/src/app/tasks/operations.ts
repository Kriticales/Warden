/**
 * Lista de operações da gaveta de Tarefas (T22): o que `operations_list` devolve, atualizado
 * pelo evento `operation-updated` (ARCHITECTURE §4.2 e §15).
 *
 * Mesma ordem do backend: em andamento (mais antigas primeiro) e depois as concluídas (mais
 * recentes primeiro), com no máximo `FINISHED_KEPT` concluídas.
 */
import type { OperationSnapshot, OperationState } from '../../lib/ipc/bindings';

/** Quantas operações concluídas a lista guarda (igual ao backend). */
export const FINISHED_KEPT = 50;

const FINISHED_STATES: ReadonlySet<OperationState> = new Set(['succeeded', 'failed', 'cancelled']);

export function isFinished(operation: OperationSnapshot): boolean {
  return FINISHED_STATES.has(operation.state);
}

/** Aplica uma atualização à lista, mantendo a ordem e o limite. */
export function applyOperationUpdate(
  list: readonly OperationSnapshot[],
  update: OperationSnapshot,
): OperationSnapshot[] {
  const rest = list.filter((operation) => operation.id !== update.id);
  const running = rest.filter((operation) => !isFinished(operation));
  const finished = rest.filter(isFinished);
  if (isFinished(update)) {
    return [...running, ...[update, ...finished].slice(0, FINISHED_KEPT)];
  }
  const previous = list.find((operation) => operation.id === update.id);
  if (previous && !isFinished(previous)) {
    // Atualização de uma em andamento: fica no mesmo lugar.
    return list.map((operation) => (operation.id === update.id ? update : operation));
  }
  const ordered = [...running, update].sort((a, b) => a.startedAtMs - b.startedAtMs);
  return [...ordered, ...finished];
}

/** Situação do indicador do rodapé. */
export type TasksIndicatorState =
  { kind: 'idle' } | { kind: 'busy'; count: number } | { kind: 'error'; count: number };

/**
 * `busy` se há operações em andamento; senão `error` se alguma falhou depois de
 * `seenUntilMs` (a última vez que a gaveta foi aberta); senão `idle`.
 */
export function indicatorState(
  list: readonly OperationSnapshot[],
  seenUntilMs: number,
): TasksIndicatorState {
  const running = list.filter((operation) => !isFinished(operation)).length;
  if (running > 0) {
    return { kind: 'busy', count: running };
  }
  const failed = list.filter(
    (operation) => operation.state === 'failed' && (operation.finishedAtMs ?? 0) > seenUntilMs,
  ).length;
  return failed > 0 ? { kind: 'error', count: failed } : { kind: 'idle' };
}
