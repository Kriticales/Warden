/**
 * Operações da gaveta de Tarefas: a query de `operations_list` e a sincronização pelo evento
 * `operation-updated` (T22; ARCHITECTURE §4.2).
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import { showOperationToast } from '../../components/common/OperationToast';
import { commands, events, type OperationSnapshot } from '../../lib/ipc/bindings';
import { useTauriEvent } from '../../lib/ipc/events';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery } from '../../lib/ipc/query';
import { operationName } from './operation-text';
import { applyOperationUpdate, isFinished } from './operations';
import { useTasksUi } from './tasks-store';

export const operationsQuery = commandQuery(queryKeys.operations, commands.operationsList);

/** A lista de operações (em andamento e concluídas). */
export function useOperations() {
  return useQuery(operationsQuery);
}

/**
 * Mantém a lista em dia com `operation-updated`. Uma vez, na raiz do app. Quando uma operação
 * vista em andamento termina com a gaveta fechada, mostra o toast do resultado.
 */
export function useOperationsSync(): void {
  const queryClient = useQueryClient();
  useTauriEvent(events.operationUpdated, (update) => {
    const previous = queryClient.getQueryData<OperationSnapshot[]>(queryKeys.operations);
    const before = previous?.find((operation) => operation.id === update.id);
    queryClient.setQueryData<OperationSnapshot[]>(queryKeys.operations, (list) =>
      applyOperationUpdate(list ?? [], update),
    );
    const { open, showDetails } = useTasksUi.getState();
    if (before && !isFinished(before) && isFinished(update) && !open) {
      showOperationToast(update, {
        name: operationName(update.kind),
        onShowDetails: () => {
          showDetails(update.id);
        },
      });
    }
  });
}
