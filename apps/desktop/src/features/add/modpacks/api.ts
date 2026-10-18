/**
 * Dados de "Navegar por modpacks" (ARCHITECTURE §18): a lista de mods de um modpack.
 *
 * A chave fica sob a área `inventory` do pack, como a busca e a pré-visualização: quando o
 * inventário muda (`pack-changed`, ou depois de Adicionar), os estados "Já no seu pack" são
 * relidos junto com a lista de resultados, a linha do pack e as contagens.
 *
 * A leitura é uma operação cancelável no backend (`add.modpackContents`): quando a consulta é
 * abandonada (o painel fechou, outro modpack foi aberto, ou a lista foi pedida de novo), o
 * TanStack Query aborta o `signal`, e a operação é cancelada; a leitura do arquivo do modpack
 * para na hora e fecha a conexão.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type OperationId, type PackId, type SourceId } from '../../../lib/ipc/bindings';
import { operationChannel } from '../../../lib/ipc/channel';
import { queryKeys } from '../../../lib/ipc/keys';
import { unwrap } from '../../../lib/ipc/result';

/** A lista fica na memória por 5 minutos, como a busca (ARCHITECTURE §17.1). */
const MODPACK_STALE_MS = 5 * 60_000;

export const modpackKeys = {
  contents: (packId: PackId, source: SourceId, projectId: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'modpack', source, projectId] as const,
};

/**
 * Roda um comando que manda o ID da operação pelo canal (`started`) e o cancela no backend
 * quando `signal` aborta, antes ou depois de o ID chegar.
 */
async function cancellable<T>(
  signal: AbortSignal,
  run: (channel: ReturnType<typeof operationChannel>) => Promise<T>,
): Promise<T> {
  let operation: OperationId | null = null;
  const cancel = () => {
    if (operation !== null) void commands.operationCancel(operation);
  };
  const channel = operationChannel((event) => {
    if (event.type !== 'started') return;
    operation = event.operationId;
    if (signal.aborted) cancel();
  });
  signal.addEventListener('abort', cancel, { once: true });
  try {
    return await run(channel);
  } finally {
    signal.removeEventListener('abort', cancel);
  }
}

export function useModpackContents(packId: PackId, source: SourceId, projectId: string) {
  return useQuery({
    queryKey: modpackKeys.contents(packId, source, projectId),
    queryFn: async ({ signal }) =>
      unwrap(
        await cancellable(signal, (channel) =>
          commands.modpackContents(packId, source, projectId, channel),
        ),
      ),
    staleTime: MODPACK_STALE_MS,
    retry: false,
  });
}
