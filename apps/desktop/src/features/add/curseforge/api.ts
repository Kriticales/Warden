/**
 * Link da CurseForge (P1-10): `curseforge_link_resolve`. É uma consulta que não grava nada no
 * pack (o packwiz lê o link de arquivo numa cópia); nunca fica em cache, e a chave do cache é a
 * URL, para o mesmo link não ser lido duas vezes enquanto está aberto.
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
