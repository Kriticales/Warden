/**
 * Dados do início da descoberta (ARCHITECTURE §18): populares e atualizados para o pack e a
 * lista curada de categorias. Sob a área `inventory` do pack: o que entrou no pack sai das
 * listas quando o inventário muda.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId, type ProjectKind } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery } from '../../../lib/ipc/query';

/** O início guarda as listas na memória por 5 minutos, como a busca. */
const HOME_STALE_MS = 5 * 60_000;

export const discoverKeys = {
  home: (packId: PackId, kind: ProjectKind) =>
    [...queryKeys.packArea(packId, 'inventory'), 'home', kind] as const,
  categories: (kind: ProjectKind) => ['discover', 'categories', kind] as const,
};

/** Populares e atualizados para o pack; só pede quando o início está à vista. */
export function useDiscoverHome(packId: PackId, kind: ProjectKind, enabled: boolean) {
  return useQuery({
    ...commandQuery(discoverKeys.home(packId, kind), () => commands.discoverHome(packId, kind)),
    enabled,
    staleTime: HOME_STALE_MS,
  });
}

/** As categorias curadas do tipo (sem rede: é o mapeamento embutido no app). */
export function useDiscoverCategories(kind: ProjectKind) {
  return useQuery({
    ...commandQuery(discoverKeys.categories(kind), () => commands.discoverCategories(kind)),
    staleTime: Infinity,
  });
}
