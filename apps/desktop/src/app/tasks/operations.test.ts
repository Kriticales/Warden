import { describe, expect, it } from 'vitest';

import { BASE_TIME_MS, makeOperation } from '../../test/factories';
import { applyOperationUpdate, FINISHED_KEPT, indicatorState, isFinished } from './operations';

describe('applyOperationUpdate', () => {
  it('nova em andamento entra por ordem de início, antes das concluídas', () => {
    const older = makeOperation({ startedAtMs: BASE_TIME_MS });
    const done = makeOperation({ state: 'succeeded', finishedAtMs: BASE_TIME_MS + 5 });
    const newer = makeOperation({ startedAtMs: BASE_TIME_MS + 10 });
    const middle = makeOperation({ startedAtMs: BASE_TIME_MS + 5 });
    let list = applyOperationUpdate([older, done], newer);
    list = applyOperationUpdate(list, middle);
    expect(list.map((operation) => operation.id)).toEqual([older.id, middle.id, newer.id, done.id]);
  });

  it('atualização de uma em andamento fica no mesmo lugar', () => {
    const a = makeOperation();
    const b = makeOperation();
    const updated = { ...a, stage: { id: 'download', labelKey: 'x' } };
    expect(applyOperationUpdate([a, b], updated)).toEqual([updated, b]);
  });

  it('a que terminou vai para o topo das concluídas, com no máximo 50', () => {
    const running = makeOperation();
    const finished = Array.from({ length: FINISHED_KEPT }, (_, index) =>
      makeOperation({ state: 'succeeded', finishedAtMs: BASE_TIME_MS - index }),
    );
    const list = applyOperationUpdate([running, ...finished], {
      ...running,
      state: 'failed',
      finishedAtMs: BASE_TIME_MS + 1,
    });
    expect(list).toHaveLength(FINISHED_KEPT);
    expect(list[0]?.id).toBe(running.id);
    expect(list.every(isFinished)).toBe(true);
    expect(list.at(-1)?.id).toBe(finished[FINISHED_KEPT - 2]?.id);
  });

  it('evento de uma operação desconhecida já concluída também entra', () => {
    const done = makeOperation({ state: 'cancelled', finishedAtMs: BASE_TIME_MS });
    expect(applyOperationUpdate([], done)).toEqual([done]);
  });
});

describe('indicatorState', () => {
  it('ocupado conta as em andamento (inclusive esperando a trava e cancelando)', () => {
    const list = [
      makeOperation(),
      makeOperation({ state: 'waitingForLock' }),
      makeOperation({ state: 'cancelling' }),
      makeOperation({ state: 'failed', finishedAtMs: BASE_TIME_MS }),
    ];
    expect(indicatorState(list, 0)).toEqual({ kind: 'busy', count: 3 });
  });

  it('falha nova acende o erro; falha já vista não', () => {
    const failed = makeOperation({ state: 'failed', finishedAtMs: BASE_TIME_MS + 100 });
    expect(indicatorState([failed], BASE_TIME_MS)).toEqual({ kind: 'error', count: 1 });
    expect(indicatorState([failed], BASE_TIME_MS + 100)).toEqual({ kind: 'idle' });
    expect(indicatorState([], 0)).toEqual({ kind: 'idle' });
  });
});
