/**
 * Wrappers de TanStack Query sobre os comandos gerados (ARCHITECTURE §18).
 *
 * Os comandos do `bindings.ts` devolvem `{ status, data | error }`; aqui o resultado vira dado
 * ou `CommandError` (com o `AppError`), que é o que as telas mostram com `ErrorPanel`.
 *
 * Regra do disco como verdade: mutations não fazem atualização otimista de dados do pack;
 * invalidam as chaves indicadas ao terminar (com sucesso ou não, porque uma escrita pode ter
 * mudado parte do pack antes de falhar).
 */
import {
  queryOptions,
  useMutation,
  useQueryClient,
  type QueryKey,
  type UseMutationOptions,
} from '@tanstack/react-query';

import type { AppError } from './bindings';
import { CommandError, unwrap, type CommandResult } from './result';

/** Opções de query para um comando: `useQuery(commandQuery(['app', 'info'], commands.appInfo))`. */
export function commandQuery<T, K extends QueryKey>(
  queryKey: K,
  command: () => Promise<CommandResult<T>>,
) {
  return queryOptions({
    queryKey,
    queryFn: async () => unwrap(await command()),
  });
}

/** Opções de `useCommandMutation`, sem `mutationFn` (vem do comando). */
export type CommandMutationOptions<T, V> = Omit<
  UseMutationOptions<T, CommandError, V>,
  'mutationFn'
> & {
  /** Chaves a invalidar quando a mutation termina, com sucesso ou erro. */
  invalidates?: readonly QueryKey[];
};

/**
 * Mutation de um comando. O erro é sempre `CommandError` (com `appError`); as chaves de
 * `invalidates` são invalidadas ao terminar.
 */
export function useCommandMutation<T, V>(
  command: (variables: V) => Promise<CommandResult<T>>,
  { invalidates = [], onSettled, ...options }: CommandMutationOptions<T, V> = {},
) {
  const queryClient = useQueryClient();
  return useMutation<T, CommandError, V>({
    ...options,
    mutationFn: async (variables) => unwrap(await command(variables)),
    onSettled: async (...args) => {
      await Promise.all(invalidates.map((queryKey) => queryClient.invalidateQueries({ queryKey })));
      await onSettled?.(...args);
    },
  });
}

/** O `AppError` de um erro de query ou mutation, quando houver. */
export function commandError(error: unknown): AppError | null {
  return error instanceof CommandError ? error.appError : null;
}
