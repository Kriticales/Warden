/**
 * Dados de "Navegar por modpacks" (ARCHITECTURE §18): a lista de mods de um modpack.
 *
 * A chave fica sob a área `inventory` do pack, como a busca e a pré-visualização: quando o
 * inventário muda (`pack-changed`, ou depois de Adicionar), os estados "Já no seu pack" são
 * relidos junto com a lista de resultados, a linha do pack e as contagens.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId, type SourceId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery } from '../../../lib/ipc/query';

/** A lista fica na memória por 5 minutos, como a busca (ARCHITECTURE §17.1). */
const MODPACK_STALE_MS = 5 * 60_000;

export const modpackKeys = {
  contents: (packId: PackId, source: SourceId, projectId: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'modpack', source, projectId] as const,
};

export function useModpackContents(packId: PackId, source: SourceId, projectId: string) {
  return useQuery({
    ...commandQuery(modpackKeys.contents(packId, source, projectId), () =>
      commands.modpackContents(packId, source, projectId),
    ),
    staleTime: MODPACK_STALE_MS,
    retry: false,
  });
}
