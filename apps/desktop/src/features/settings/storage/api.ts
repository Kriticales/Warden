/**
 * Dados de Configurações → Armazenamento (A-06): o espaço usado (`storage_usage`, medido no
 * Rust fora da thread da interface) e "Limpar cache" (`storage_clear_cache`).
 *
 * O espaço muda por fora desta seção: um teste baixa o Minecraft, o Java e os mods e enche a
 * instância; adicionar mods enche o cache; remover Javas libera espaço. Por isso a medição é
 * refeita ao abrir a página (`staleTime: 0`), quando qualquer tarefa termina, quando o jogo
 * fecha e quando a tabela de Java muda ([`useStorageUsageSync`]).
 */
import { hashKey, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { commands, events } from '../../../lib/ipc/bindings';
import { useTauriEvent } from '../../../lib/ipc/events';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { isFinished } from '../../../app/tasks/operations';
import { javaQueryKeys } from '../java/hooks/useJavaRuntimes';

const JAVA_RUNTIMES_HASH = hashKey(javaQueryKeys.runtimes);

export const storageKeys = {
  usage: ['storage', 'usage'] as const,
};

export function useStorageUsage() {
  return useQuery({
    ...commandQuery(storageKeys.usage, commands.storageUsage),
    staleTime: 0,
  });
}

/** "Limpar cache". Mede de novo ao terminar, com sucesso ou erro (parte pode ter saído). */
export function useClearCache() {
  return useCommandMutation(() => commands.storageClearCache(), {
    invalidates: [storageKeys.usage],
  });
}

/** Mede de novo quando algo que ocupa espaço muda fora desta seção. */
export function useStorageUsageSync(): void {
  const queryClient = useQueryClient();
  const remeasure = () => {
    void queryClient.invalidateQueries({ queryKey: storageKeys.usage });
  };
  useTauriEvent(events.operationUpdated, (operation) => {
    if (isFinished(operation)) {
      remeasure();
    }
  });
  useTauriEvent(events.gameState, (game) => {
    if (game.state === 'exited') {
      remeasure();
    }
  });
  // A remoção de Javas não é uma tarefa: a tabela de Java recebe a lista nova direto no cache.
  useEffect(
    () =>
      queryClient.getQueryCache().subscribe((event) => {
        if (
          event.type === 'updated' &&
          event.action.type === 'success' &&
          // A primeira leitura da tabela (ao abrir a página) não é mudança.
          event.query.state.dataUpdateCount > 1 &&
          event.query.queryHash === JAVA_RUNTIMES_HASH
        ) {
          void queryClient.invalidateQueries({ queryKey: storageKeys.usage });
        }
      }),
    [queryClient],
  );
}
