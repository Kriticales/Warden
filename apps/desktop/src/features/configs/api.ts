/**
 * Dados da seção Configs (ARCHITECTURE §18): a lista de arquivos (`config_tree`), o conteúdo de
 * um arquivo (`config_read`) e a gravação (`config_write`).
 *
 * Regra do disco como verdade: a lista e o arquivo são relidos ao abrir a seção e ao voltar
 * para a janela (um arquivo mudado por fora aparece sem reiniciar o app); a gravação não faz
 * atualização otimista, só põe o resultado confirmado no cache.
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import {
  commands,
  type ConfigContent,
  type ConfigOrigin,
  type ConfigSaved,
  type PackId,
} from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';
import { packsKeys } from '../packs/api';

export const configsKeys = {
  /** Tudo de configs de um pack. */
  all: (packId: PackId) => queryKeys.packArea(packId, 'configs'),
  tree: (packId: PackId, origin: ConfigOrigin) =>
    [...queryKeys.packArea(packId, 'configs'), origin, 'tree'] as const,
  file: (packId: PackId, origin: ConfigOrigin, path: string) =>
    [...queryKeys.packArea(packId, 'configs'), origin, 'file', path] as const,
};

/** Sempre relido do disco ao abrir a seção e ao voltar para a janela. */
const fromDisk = {
  staleTime: 0,
  refetchOnMount: 'always',
  refetchOnWindowFocus: 'always',
} as const;

export function useConfigTree(packId: PackId, origin: ConfigOrigin) {
  return useQuery({
    ...commandQuery(configsKeys.tree(packId, origin), () => commands.configTree(packId, origin)),
    ...fromDisk,
  });
}

export function useConfigFile(packId: PackId, origin: ConfigOrigin, path: string | null) {
  return useQuery({
    ...commandQuery(configsKeys.file(packId, origin, path ?? ''), () =>
      commands.configRead(packId, origin, path ?? ''),
    ),
    ...fromDisk,
    enabled: path !== null,
  });
}

export interface WriteRequest {
  origin: ConfigOrigin;
  path: string;
  text: string;
  /** O `hash` da leitura em que o texto se baseia. */
  expectedHash: string;
}

/**
 * Grava um arquivo. Ao terminar, o cache do arquivo recebe o resultado confirmado (o `hash` novo
 * é o esperado da próxima gravação) e a lista de arquivos e a linha do pack (alterações não
 * salvas) são relidas.
 */
export function useConfigWrite(packId: PackId) {
  const queryClient = useQueryClient();
  return useCommandMutation(
    ({ origin, path, text, expectedHash }: WriteRequest) =>
      commands.configWrite(packId, origin, path, text, expectedHash),
    {
      onSuccess: (saved: ConfigSaved, { origin, path, text }) => {
        queryClient.setQueryData<ConfigContent>(configsKeys.file(packId, origin, path), (old) =>
          old ? { ...old, text, hash: saved.hash, size: saved.size } : old,
        );
      },
      invalidates: [configsKeys.all(packId), packsKeys.row(packId), packsKeys.list],
    },
  );
}
