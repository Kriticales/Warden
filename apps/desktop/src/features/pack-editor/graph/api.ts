/**
 * Consultas do modo Grafo de Mods (D-11): `graph_focus` (o mod no centro, ou o pack inteiro) e
 * `graph_orphans` (bibliotecas sem uso). As chaves ficam na área `inventory` do pack, como as do
 * bloco de dependências dos detalhes (D-07): o evento `pack-changed` que muda a lista muda
 * também o grafo, a explicação e as bibliotecas sem uso de uma vez.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId } from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery } from '../../../lib/ipc/query';

/** Profundidade do grafo focado: níveis de cada lado do mod no centro. */
export type Depth = 1 | 2;

export const drawKeys = {
  focus: (packId: PackId, path: string, depth: Depth) =>
    [...queryKeys.packArea(packId, 'inventory'), 'graph', 'focus', path, depth] as const,
  whole: (packId: PackId) =>
    [...queryKeys.packArea(packId, 'inventory'), 'graph', 'whole'] as const,
  orphans: (packId: PackId) =>
    [...queryKeys.packArea(packId, 'inventory'), 'graph', 'orphans'] as const,
};

/** O subgrafo em volta do mod no centro. */
export function useGraphFocus(packId: PackId, path: string | null, depth: Depth) {
  return useQuery({
    ...commandQuery(drawKeys.focus(packId, path ?? '', depth), () =>
      commands.graphFocus(packId, path, depth),
    ),
    enabled: path !== null,
    staleTime: 30_000,
  });
}

/** O pack inteiro, só quando "Ver o pack inteiro" está aberto. */
export function useWholeGraph(packId: PackId, enabled: boolean) {
  return useQuery({
    ...commandQuery(drawKeys.whole(packId), () => commands.graphFocus(packId, null, 0)),
    enabled,
    staleTime: 30_000,
  });
}

/** Bibliotecas que nenhum mod mantido usa mais. */
export function useOrphans(packId: PackId) {
  return useQuery({
    ...commandQuery(drawKeys.orphans(packId), () => commands.graphOrphans(packId)),
    staleTime: 30_000,
  });
}
