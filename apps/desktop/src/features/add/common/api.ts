/**
 * Dados da página Adicionar (ARCHITECTURE §18): busca combinada com rolagem infinita,
 * pré-visualização, versões, plano e gravação.
 *
 * As chaves ficam sob a área `inventory` do pack: quando o inventário muda (`pack-changed`, ou
 * depois de Adicionar), a busca, o plano e as marcas "Já no pack" são relidos. Sem atualização
 * otimista: o disco é a verdade.
 */
import { useInfiniteQuery, useQuery } from '@tanstack/react-query';

import {
  commands,
  type AddApplyRequest,
  type AddChoice,
  type PackId,
  type SearchCursor,
  type SearchPage,
  type SearchRequest,
  type SourceId,
} from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { unwrap } from '../../../lib/ipc/result';
import { packsKeys } from '../../packs/api';

/** A busca sem o cursor (o cursor é a página). */
export type SearchFilters = Omit<SearchRequest, 'cursor'>;

/** A busca guarda os resultados na memória por 5 minutos (ARCHITECTURE §17.1). */
const SEARCH_STALE_MS = 5 * 60_000;

export const addKeys = {
  search: (packId: PackId, filters: SearchFilters) =>
    [...queryKeys.packArea(packId, 'inventory'), 'search', filters] as const,
  details: (packId: PackId, source: SourceId, projectId: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'project', source, projectId] as const,
  versions: (packId: PackId, source: SourceId, projectId: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'versions', source, projectId] as const,
  plan: (packId: PackId, choices: readonly AddChoice[]) =>
    [...queryKeys.packArea(packId, 'inventory'), 'addPlan', choices] as const,
};

/** Busca combinada, 20 itens por página, seguindo o `next` de cada página. */
export function useSearch(packId: PackId, filters: SearchFilters) {
  return useInfiniteQuery({
    queryKey: addKeys.search(packId, filters),
    queryFn: async ({ pageParam }): Promise<SearchPage> =>
      unwrap(await commands.searchProjects(packId, { ...filters, cursor: pageParam })),
    initialPageParam: null as SearchCursor | null,
    getNextPageParam: (last) => last.next,
    staleTime: SEARCH_STALE_MS,
  });
}

export function useProjectDetails(packId: PackId, source: SourceId, projectId: string) {
  return useQuery({
    ...commandQuery(addKeys.details(packId, source, projectId), () =>
      commands.projectDetails(packId, source, projectId),
    ),
    staleTime: SEARCH_STALE_MS,
  });
}

export function useProjectVersions(packId: PackId, source: SourceId, projectId: string) {
  return useQuery({
    ...commandQuery(addKeys.versions(packId, source, projectId), () =>
      commands.projectVersions(packId, source, projectId),
    ),
    staleTime: SEARCH_STALE_MS,
  });
}

/** O plano do diálogo de dependências; nada é gravado. Relido sempre que o diálogo abre. */
export function useAddPlan(packId: PackId, choices: readonly AddChoice[] | null) {
  return useQuery({
    ...commandQuery(addKeys.plan(packId, choices ?? []), () =>
      commands.addPlan(packId, { items: [...(choices ?? [])] }),
    ),
    enabled: choices !== null && choices.length > 0,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

/** Grava os itens confirmados; o inventário e a linha do pack são relidos ao terminar. */
export function useAddApply(packId: PackId) {
  return useCommandMutation((request: AddApplyRequest) => commands.addApply(packId, request), {
    invalidates: [queryKeys.packArea(packId, 'inventory'), packsKeys.row(packId), packsKeys.list],
  });
}
