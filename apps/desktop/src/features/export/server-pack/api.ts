/**
 * Dados do pacote para servidor (ARCHITECTURE §12.1): a leitura do que vai em cada variante
 * (`server_pack_analyze`), a geração com a validação opcional (`server_pack_run`, com o diálogo
 * nativo do Rust) e o "Mostrar arquivo" do resultado.
 *
 * A leitura é relida ao abrir e ao voltar para a janela (o pack, a publicação e o cache de
 * downloads mudam por fora); a geração a invalida no fim, porque os downloads manuais da T20
 * mudam o que ainda falta baixar.
 */
import { useQuery } from '@tanstack/react-query';

import { useOperations } from '../../../app/tasks/useOperations';
import {
  commands,
  type OperationSnapshot,
  type PackId,
  type ServerPackRunRequest,
} from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';

/** Tipo da operação de `server_pack_run` (`OperationKind` no Rust). */
export const SERVER_PACK_OPERATION = 'export.serverPack';

export const serverPackKeys = {
  info: (packId: PackId) => [...queryKeys.pack(packId), 'export', 'serverPack'] as const,
};

export function useServerPackInfo(packId: PackId) {
  return useQuery({
    ...commandQuery(serverPackKeys.info(packId), () => commands.serverPackAnalyze(packId)),
    staleTime: 0,
    refetchOnMount: 'always',
    refetchOnWindowFocus: 'always',
  });
}

/** `null` = a pessoa fechou o diálogo de destino sem escolher. */
export function useServerPackRun(packId: PackId) {
  return useCommandMutation(
    (request: ServerPackRunRequest) => commands.serverPackRun(packId, request),
    { invalidates: [serverPackKeys.info(packId)] },
  );
}

export function useRevealServerPack() {
  return useCommandMutation((path: string) => commands.serverPackReveal(path));
}

/** A geração deste pack em andamento, vista pela gaveta de Tarefas. */
export function useRunningServerPack(packId: PackId): OperationSnapshot | null {
  const operations = useOperations();
  return (
    operations.data?.find(
      (operation) =>
        operation.kind === SERVER_PACK_OPERATION &&
        operation.packId === packId &&
        operation.finishedAtMs === null,
    ) ?? null
  );
}
