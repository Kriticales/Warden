/**
 * Chamadas da busca do culpado (ARCHITECTURE §4.1, domínio `bisect`; SPEC T25; ROADMAP D-12).
 *
 * - `useBisect`: a busca guardada do pack (rodando, pausada ou a última terminada).
 * - `useBisectPlan`: o que o diálogo "Encontrar o mod culpado" mostra.
 * - `runBisect`: começa, continua ou "continua sem" o conjunto achado. O comando só volta
 *   quando a busca termina ou pausa (pode levar 40 minutos); enquanto isso o canal manda as
 *   rodadas e cada uma atualiza a busca guardada. Funciona fora de qualquer tela: dá para ir a
 *   outras seções e voltar pelo botão do cabeçalho.
 * - Pausar, cancelar e responder (modo assistido) são mutations que atualizam a busca.
 *
 * Quando a busca termina, **tudo o que mostra o resultado** é atualizado: a própria busca, a
 * lista de Travamentos (o resultado fica ligado ao travamento de origem) e o grafo (a busca
 * guarda as dependências que descobriu).
 */
import { useQuery, type QueryClient } from '@tanstack/react-query';

import {
  commands,
  type BisectRequest,
  type OperationEvent,
  type PackId,
} from '../../lib/ipc/bindings';
import { operationChannel } from '../../lib/ipc/channel';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';
import { unwrap } from '../../lib/ipc/result';
import { crashesKeys } from '../diagnostics/crashes/api';

/** Tipo da operação da busca (`OperationKind` no Rust). */
export const BISECT_OPERATION = 'bisect.run';

export const bisectKeys = {
  /** Tudo da busca do pack. */
  all: (packId: PackId) => [...queryKeys.pack(packId), 'bisect'] as const,
  /** A busca guardada. */
  current: (packId: PackId) => [...queryKeys.pack(packId), 'bisect', 'current'] as const,
  /** O plano do diálogo. */
  plan: (packId: PackId) => [...queryKeys.pack(packId), 'bisect', 'plan'] as const,
  /** O log de uma rodada. */
  log: (packId: PackId, round: number) =>
    [...queryKeys.pack(packId), 'bisect', 'log', round] as const,
};

/** O grafo do pack (a busca guarda as dependências inferidas nele). */
function graphKey(packId: PackId) {
  return [...queryKeys.packArea(packId, 'inventory'), 'graph'] as const;
}

export function useBisect(packId: PackId) {
  return useQuery({
    ...commandQuery(bisectKeys.current(packId), () => commands.bisectGet(packId)),
    staleTime: 0,
  });
}

export function useBisectPlan(packId: PackId, enabled: boolean) {
  return useQuery({
    ...commandQuery(bisectKeys.plan(packId), () => commands.bisectPlan(packId)),
    enabled,
    staleTime: 0,
  });
}

export function useRoundLog(packId: PackId, round: number | null) {
  return useQuery({
    ...commandQuery(bisectKeys.log(packId, round ?? -1), () =>
      commands.bisectRoundLog(packId, round ?? 0),
    ),
    enabled: round !== null,
    // O log de uma rodada terminada não muda.
    staleTime: Infinity,
  });
}

/** O que muda quando uma rodada termina. */
async function invalidateRound(queryClient: QueryClient, packId: PackId): Promise<void> {
  await queryClient.invalidateQueries({ queryKey: bisectKeys.current(packId) });
}

/** O que muda quando a busca termina ou pausa. */
export async function invalidateAfterBisect(
  queryClient: QueryClient,
  packId: PackId,
): Promise<void> {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: bisectKeys.all(packId) }),
    queryClient.invalidateQueries({ queryKey: crashesKeys.list(packId) }),
    queryClient.invalidateQueries({ queryKey: graphKey(packId) }),
  ]);
}

export type BisectRunKind =
  { kind: 'start'; request: BisectRequest } | { kind: 'resume' } | { kind: 'continueWithout' };

/**
 * Roda a busca até ela terminar ou pausar. Cada rodada atualiza a busca guardada; o fim
 * atualiza também os Travamentos e o grafo. Lança o erro do comando (o chamador avisa).
 */
export async function runBisect(
  queryClient: QueryClient,
  packId: PackId,
  run: BisectRunKind,
): Promise<void> {
  const channel = operationChannel((event: OperationEvent) => {
    if (event.type === 'round') {
      void invalidateRound(queryClient, packId);
    }
  });
  try {
    const call =
      run.kind === 'start'
        ? commands.bisectStart(packId, run.request, channel)
        : run.kind === 'resume'
          ? commands.bisectResume(packId, channel)
          : commands.bisectContinueWithout(packId, channel);
    unwrap(await call);
  } finally {
    await invalidateAfterBisect(queryClient, packId);
  }
}

export function usePauseBisect(packId: PackId) {
  return useCommandMutation(() => commands.bisectPause(packId), {
    invalidates: [bisectKeys.current(packId)],
  });
}

export function useCancelBisect(packId: PackId) {
  return useCommandMutation(() => commands.bisectCancel(packId), {
    invalidates: [bisectKeys.all(packId), crashesKeys.list(packId)],
  });
}

export function useAnswerBisect(packId: PackId) {
  return useCommandMutation(
    (answer: 'happened' | 'notHappened' | 'couldNotTest') => commands.bisectAnswer(packId, answer),
    { invalidates: [bisectKeys.current(packId)] },
  );
}
