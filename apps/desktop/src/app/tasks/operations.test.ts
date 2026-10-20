import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';

import { queryKeys } from '../../lib/ipc/keys';
import { BASE_TIME_MS, makeOperation } from '../../test/factories';
import {
  applyOperationUpdate,
  FINISHED_KEPT,
  indicatorState,
  isFinished,
  mergeOperationLists,
} from './operations';
import { operationsQuery } from './useOperations';

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

  it('retrato mais velho que o da lista não desfaz o fim da operação', () => {
    const running = makeOperation({ revision: 4 });
    const done = {
      ...running,
      state: 'succeeded' as const,
      finishedAtMs: BASE_TIME_MS + 1,
      revision: 6,
    };
    const list = applyOperationUpdate([running], done);
    // Um aviso de progresso que saiu antes do fim e chegou depois.
    const late = {
      ...running,
      progress: { current: 1, total: 2, unit: 'items' as const },
      revision: 5,
    };
    expect(applyOperationUpdate(list, late)).toEqual([done]);
    expect(indicatorState(applyOperationUpdate(list, late), BASE_TIME_MS)).toEqual({
      kind: 'idle',
    });
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

describe('mergeOperationLists', () => {
  it('lista velha que chega depois do fim não traz a operação de volta', () => {
    const running = makeOperation({ revision: 1 });
    const done = {
      ...running,
      state: 'succeeded' as const,
      finishedAtMs: BASE_TIME_MS + 1,
      revision: 2,
    };
    expect(mergeOperationLists([done], [running])).toEqual([done]);
  });

  it('fica com o mais novo de cada lado, na ordem e no limite do backend', () => {
    const a = makeOperation({ startedAtMs: BASE_TIME_MS + 2, revision: 5 });
    const b = makeOperation({ startedAtMs: BASE_TIME_MS + 1, revision: 1 });
    const bNewer = { ...b, stage: { id: 'baixar', labelKey: 'x' }, revision: 4 };
    const finished = Array.from({ length: FINISHED_KEPT + 1 }, (_, index) =>
      makeOperation({ state: 'failed', finishedAtMs: BASE_TIME_MS - index, revision: 1 }),
    );
    const merged = mergeOperationLists([a, b, ...finished.slice(0, 10)], [bNewer, ...finished]);
    expect(merged.slice(0, 2)).toEqual([bNewer, a]);
    expect(merged.slice(2)).toEqual(finished.slice(0, FINISHED_KEPT));
  });

  it('a lista manda no que existe; só fica a operação que começou depois dela', () => {
    const listed = makeOperation({ revision: 5 });
    const gone = makeOperation({ state: 'succeeded', finishedAtMs: BASE_TIME_MS, revision: 3 });
    const startedLater = makeOperation({ startedAtMs: BASE_TIME_MS + 9, revision: 6 });
    expect(mergeOperationLists([gone, startedLater], [listed])).toEqual([listed, startedLater]);
  });

  it('resposta de operations_list pedida antes do fim não sobrescreve o aviso', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const running = makeOperation({ revision: 1 });
    const done = {
      ...running,
      state: 'succeeded' as const,
      finishedAtMs: BASE_TIME_MS + 1,
      revision: 2,
    };
    client.setQueryData(queryKeys.operations, [done]);
    await client.query({ ...operationsQuery, queryFn: () => Promise.resolve([running]) });
    expect(client.getQueryData(queryKeys.operations)).toEqual([done]);
  });
});
