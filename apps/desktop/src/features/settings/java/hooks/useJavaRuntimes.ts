/**
 * Dados da seção Java de Configurações: a tabela (`java_runtimes_list`) e as ações "Procurar
 * atualizações do Java" (operação longa, aparece em Tarefas), "Remover Javas sem uso" e
 * remover um Java. As remoções devolvem a tabela já atualizada, que entra direto no cache.
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import { commands, type JavaOverview, type RuntimeId } from '../../../../lib/ipc/bindings';
import { commandQuery, useCommandMutation } from '../../../../lib/ipc/query';

export const javaQueryKeys = {
  runtimes: ['java', 'runtimes'] as const,
};

export const javaRuntimesQuery = commandQuery(javaQueryKeys.runtimes, commands.javaRuntimesList);

export function useJavaRuntimes() {
  return useQuery(javaRuntimesQuery);
}

/** "Procurar atualizações do Java". */
export function useCheckJavaUpdates() {
  return useCommandMutation(() => commands.javaRuntimesCheckUpdates(), {
    invalidates: [javaQueryKeys.runtimes],
  });
}

function useOverviewMutation<V>(
  command: (variables: V) => ReturnType<typeof commands.javaRuntimesList>,
) {
  const queryClient = useQueryClient();
  return useCommandMutation(command, {
    onSuccess: (overview: JavaOverview) => {
      queryClient.setQueryData(javaQueryKeys.runtimes, overview);
    },
    // Com erro, a tabela é relida: parte da remoção pode ter acontecido.
    onError: () => queryClient.invalidateQueries({ queryKey: javaQueryKeys.runtimes }),
  });
}

/** Remove um Java. */
export function useRemoveJava() {
  return useOverviewMutation((id: RuntimeId) => commands.javaRuntimeRemove(id));
}

/** "Remover Javas sem uso". */
export function useRemoveUnusedJavas() {
  return useOverviewMutation(() => commands.javaRuntimesRemoveUnused());
}
