/**
 * Lista de operações da gaveta de Tarefas (T22): o que `operations_list` devolve, atualizado
 * pelo evento `operation-updated` (ARCHITECTURE §4.2 e §15).
 *
 * Mesma ordem do backend: em andamento (mais antigas primeiro) e depois as concluídas (mais
 * recentes primeiro), com no máximo `FINISHED_KEPT` concluídas.
 *
 * O backend avisa fora da trava e a lista pode chegar depois de um aviso mais novo: entre dois
 * retratos da mesma operação vale sempre o de `revision` maior (ARCHITECTURE §15). Sem isso, uma
 * lista velha que ainda via a operação "em andamento" sobrescrevia o fim dela, e o rodapé
 * ficava em "1 tarefa em andamento" sem tarefa nenhuma.
 */
import type { OperationSnapshot, OperationState } from '../../lib/ipc/bindings';

/** Quantas operações concluídas a lista guarda (igual ao backend). */
export const FINISHED_KEPT = 50;

const FINISHED_STATES: ReadonlySet<OperationState> = new Set(['succeeded', 'failed', 'cancelled']);

export function isFinished(operation: OperationSnapshot): boolean {
  return FINISHED_STATES.has(operation.state);
}

/** Aplica uma atualização à lista, mantendo a ordem e o limite. Retrato mais velho que o da
 * lista não muda nada. */
export function applyOperationUpdate(
  list: readonly OperationSnapshot[],
  update: OperationSnapshot,
): OperationSnapshot[] {
  const current = list.find((operation) => operation.id === update.id);
  if (current && current.revision > update.revision) {
    return list.slice();
  }
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

/**
 * Junta a lista que já estava na tela com a que acabou de chegar de `operations_list`: de cada
 * operação fica o retrato de `revision` maior, na ordem e no limite do backend. A lista que
 * chega manda no que existe: uma operação que só a tela conhece fica apenas se o retrato dela
 * é mais novo que a lista inteira (começou depois que a lista foi lida).
 */
export function mergeOperationLists(
  current: readonly OperationSnapshot[],
  incoming: readonly OperationSnapshot[],
): OperationSnapshot[] {
  const listRevision = Math.max(0, ...incoming.map((operation) => operation.revision));
  const newest = new Map<string, OperationSnapshot>();
  for (const operation of incoming) {
    newest.set(operation.id, operation);
  }
  for (const operation of current) {
    const seen = newest.get(operation.id);
    if (seen ? operation.revision > seen.revision : operation.revision > listRevision) {
      newest.set(operation.id, operation);
    }
  }
  const all = [...newest.values()];
  const running = all
    .filter((operation) => !isFinished(operation))
    .sort((a, b) => a.startedAtMs - b.startedAtMs);
  const finished = all
    .filter(isFinished)
    .sort((a, b) => (b.finishedAtMs ?? 0) - (a.finishedAtMs ?? 0))
    .slice(0, FINISHED_KEPT);
  return [...running, ...finished];
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
