/**
 * Chamadas dos Travamentos (ARCHITECTURE §4.1, domínio `crashes`; ROADMAP D-06): a lista
 * agrupada pela causa, as vezes de um grupo e "Apagar os mais antigos".
 *
 * Os travamentos vêm das sessões gravadas do Testar, por isso as chaves ficam **sob** as
 * sessões do pack (`testKeys.sessions`): o fim de um teste, que invalida as sessões
 * (`invalidateAfterTest`), invalida também os travamentos, sem uma lista à parte.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { storageKeys } from '../../settings/storage/api';
import { testKeys } from '../../test/api';

export const crashesKeys = {
  /** A lista agrupada (sob as sessões gravadas). */
  list: (packId: PackId) => [...testKeys.sessions(packId), 'crashes'] as const,
  /** Um grupo com cada vez que travou. */
  detail: (packId: PackId, signature: string) =>
    [...testKeys.sessions(packId), 'crashes', signature] as const,
};

export function useCrashes(packId: PackId) {
  return useQuery({
    ...commandQuery(crashesKeys.list(packId), () => commands.crashesList(packId)),
    staleTime: 0,
  });
}

export function useCrash(packId: PackId, signature: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(crashesKeys.detail(packId, signature), () =>
      commands.crashGet(packId, signature),
    ),
    enabled,
  });
}

/**
 * "Apagar os mais antigos": manda as sessões que a pessoa confirmou (o `prunable` da lista); o
 * backend apaga só as que ainda passam do limite. As sessões apagadas somem de tudo o que mostra
 * testes do pack (Travamentos, Testes anteriores, o resultado e o "Por que travou" de cada
 * sessão) e do espaço usado em Configurações → Armazenamento.
 */
export function usePruneCrashes(packId: PackId) {
  return useCommandMutation((sessions: string[]) => commands.crashesPrune(packId, sessions), {
    invalidates: [[...queryKeys.pack(packId), 'test'], storageKeys.usage],
  });
}
