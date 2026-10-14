/**
 * Dados de "Restaurar padrão" (C-06): `config_restore_default` devolve o texto com o padrão nas
 * chaves pedidas (ou em todas as alteradas), sem gravar nada.
 *
 * - No editor, a prévia é calculada sobre o texto atual (com as alterações não salvas) e o
 *   texto restaurado vai para o editor: salvar e desfazer seguem o caminho de sempre.
 * - Em Problemas (`W_CONFIG_RANGE`), a prévia lê o arquivo do pack e a gravação usa o
 *   `config_write` com o `hash` da leitura (alteração externa recusada). Depois de gravar, tudo
 *   o que mostra o arquivo, a linha do pack, a lista de Meus packs e o relatório de Problemas
 *   (que fica "velho") é relido.
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import { commands, type PackId } from '../../../../lib/ipc/bindings';
import { unwrap } from '../../../../lib/ipc/result';
import { diagnosticsKeys } from '../../../diagnostics/api';
import { configsKeys, useConfigWrite } from '../../api';

export const defaultsKeys = {
  /** Prévia sobre o texto do editor. */
  preview: (packId: PackId, path: string, text: string, keys: readonly string[] | null) =>
    [...configsKeys.all(packId), 'restoreDefault', path, text, keys] as const,
  /** Prévia sobre o arquivo do pack (Problemas). */
  packPreview: (packId: PackId, path: string, key: string) =>
    [...configsKeys.all(packId), 'pack', 'restoreDefault', path, key] as const,
};

/** O texto com os padrões, para a diferença antes de mudar o editor. */
export function useRestorePreview(
  packId: PackId,
  path: string,
  text: string,
  keys: readonly string[] | null,
  enabled: boolean,
) {
  return useQuery({
    queryKey: defaultsKeys.preview(packId, path, text, keys),
    queryFn: async () =>
      unwrap(await commands.configRestoreDefault(path, text, keys ? [...keys] : null)),
    enabled,
    staleTime: Infinity,
    // A chave leva o texto inteiro: prévias antigas saem logo do cache.
    gcTime: 10_000,
    retry: false,
  });
}

export interface PackRestorePreview {
  before: string;
  after: string;
  hash: string;
}

/** Lê o arquivo do pack e calcula a chave restaurada (sempre do disco, ao abrir). */
export function usePackRestorePreview(packId: PackId, path: string, key: string, enabled: boolean) {
  return useQuery({
    queryKey: defaultsKeys.packPreview(packId, path, key),
    queryFn: async (): Promise<PackRestorePreview> => {
      const content = unwrap(await commands.configRead(packId, 'pack', path));
      const before = content.text ?? '';
      const restored = unwrap(await commands.configRestoreDefault(path, before, [key]));
      return { before, after: restored.text, hash: content.hash };
    },
    enabled,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

/** Grava a prévia no pack; o relatório de Problemas também é relido. */
export function usePackRestoreWrite(packId: PackId) {
  const queryClient = useQueryClient();
  const write = useConfigWrite(packId);
  return {
    ...write,
    restore: async (path: string, preview: PackRestorePreview) => {
      const saved = await write.mutateAsync({
        origin: 'pack',
        path,
        text: preview.after,
        expectedHash: preview.hash,
      });
      await queryClient.invalidateQueries({ queryKey: diagnosticsKeys.report(packId) });
      return saved;
    },
  };
}
