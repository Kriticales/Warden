/**
 * Chamadas dos downloads manuais da CurseForge (SPEC T20; ROADMAP L-06): a lista que o teste
 * está esperando, "Selecionar arquivo baixado…" (o diálogo e a conferência do hash são do
 * backend) e "Abrir página de download" (o endereço não sai do backend). O evento
 * `manual-downloads-changed` relê a lista.
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import { commands, events, type ManualDownloadItem, type PackId } from '../../../lib/ipc/bindings';
import { useTauriEvent } from '../../../lib/ipc/events';
import { queryKeys } from '../../../lib/ipc/keys';
import { useCommandMutation } from '../../../lib/ipc/query';

export const manualDownloadKeys = {
  /** A lista que o teste do pack está esperando. */
  list: (packId: PackId) => [...queryKeys.pack(packId), 'test', 'manualDownloads'] as const,
};

/** A lista de downloads manuais do pack (vazia quando o teste não está esperando nenhum). */
export function useManualDownloads(packId: PackId) {
  const queryClient = useQueryClient();
  useTauriEvent(events.manualDownloadsChanged, (change) => {
    if (change.packId === packId) {
      void queryClient.invalidateQueries({ queryKey: manualDownloadKeys.list(packId) });
    }
  });
  return useQuery({
    queryKey: manualDownloadKeys.list(packId),
    queryFn: (): Promise<ManualDownloadItem[]> => commands.cfBlockedList(packId),
    staleTime: 0,
  });
}

/** "Selecionar arquivo baixado…": `null` quando a pessoa fecha o diálogo sem escolher. */
export function useProvideManualFile(packId: PackId) {
  return useCommandMutation((metafile: string) => commands.cfBlockedProvideFile(packId, metafile), {
    invalidates: [manualDownloadKeys.list(packId)],
  });
}

/** "Abrir página de download". */
export function useOpenManualPage(packId: PackId) {
  return useCommandMutation((metafile: string) => commands.cfBlockedOpenPage(packId, metafile));
}
