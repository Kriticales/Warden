/**
 * Dados dos kits de desempenho (ADR-0033): a lista vem dos dados embutidos no app
 * (`kits_list`), sem rede, então nunca muda durante a sessão.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type Loader } from '../../../lib/ipc/bindings';
import { commandQuery } from '../../../lib/ipc/query';

export const kitsKeys = {
  list: (minecraft: string, loader: Loader | null) => ['kits', minecraft, loader] as const,
};

/** Os kits que servem ao Minecraft e ao loader (nenhum para vanilla). */
export function useKits(minecraft: string | null, loader: Loader | null) {
  return useQuery({
    ...commandQuery(kitsKeys.list(minecraft ?? '', loader), async () => ({
      status: 'ok' as const,
      data: await commands.kitsList(minecraft ?? '', loader),
    })),
    enabled: minecraft !== null,
    staleTime: Infinity,
  });
}
