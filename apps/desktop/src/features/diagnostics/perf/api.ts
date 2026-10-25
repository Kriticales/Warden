/**
 * O desempenho do último teste que carregou (`perf_last_test`), para o bloco da seção
 * Problemas. A chave fica **sob** as sessões gravadas do pack (`testKeys.sessions`): o fim de um
 * teste, que invalida as sessões (`invalidateAfterTest`), atualiza o bloco sem clique.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId } from '../../../lib/ipc/bindings';
import { commandQuery } from '../../../lib/ipc/query';
import { testKeys } from '../../test/api';

export const perfKeys = {
  last: (packId: PackId) => [...testKeys.sessions(packId), 'perf-last'] as const,
};

export function useLastTestPerf(packId: PackId) {
  return useQuery(commandQuery(perfKeys.last(packId), () => commands.perfLastTest(packId)));
}
