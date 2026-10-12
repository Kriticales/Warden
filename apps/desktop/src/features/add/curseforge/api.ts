/**
 * Link da CurseForge (P1-10): `curseforge_link_resolve` e `curseforge_link_import`. São
 * consultas que não gravam nada no pack (o packwiz lê o item numa cópia); nunca ficam em
 * cache, e a chave do cache é o link (ou o arquivo escolhido), para o mesmo pedido não ser
 * lido duas vezes enquanto está aberto.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId } from '../../../lib/ipc/bindings';
import { commandQuery } from '../../../lib/ipc/query';

export function useCurseforgeLink(packId: PackId, url: string | null) {
  return useQuery({
    ...commandQuery(['curseforgeLink', packId, url ?? ''] as const, () =>
      commands.curseforgeLinkResolve(packId, url ?? ''),
    ),
    enabled: url !== null,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

/** O arquivo escolhido na pré-visualização de um link de projeto. */
export interface LinkPick {
  projectId: string;
  fileId: string;
}

/** O packwiz lê o arquivo escolhido (`--addon-id`/`--file-id`) numa cópia do pack. */
export function useCurseforgeLinkImport(packId: PackId, pick: LinkPick | null) {
  return useQuery({
    ...commandQuery(
      ['curseforgeLinkImport', packId, pick?.projectId ?? '', pick?.fileId ?? ''] as const,
      () => commands.curseforgeLinkImport(packId, pick?.projectId ?? '', pick?.fileId ?? ''),
    ),
    enabled: pick !== null,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}
