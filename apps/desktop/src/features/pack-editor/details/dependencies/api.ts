/**
 * Consultas do grafo de dependências para o bloco dos detalhes (D-07): `graph_dependents` e
 * `graph_why_in_pack`. O grafo é refeito no Rust quando o inventário ou os jars mudam; aqui as
 * respostas ficam na área `inventory` do pack e somem junto com ela quando o pack muda.
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId } from '../../../../lib/ipc/bindings';
import { queryKeys } from '../../../../lib/ipc/keys';
import { commandQuery } from '../../../../lib/ipc/query';

export const graphKeys = {
  dependents: (packId: PackId, path: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'graph', 'dependents', path] as const,
  why: (packId: PackId, path: string) =>
    [...queryKeys.packArea(packId, 'inventory'), 'graph', 'why', path] as const,
};

/** Relações diretas do item e o que deixa de funcionar se ele sair. */
export function useDependents(packId: PackId, path: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(graphKeys.dependents(packId, path), () =>
      commands.graphDependents(packId, [path]),
    ),
    enabled,
    staleTime: 30_000,
  });
}

/** "Por que está no pack": você adicionou, ou a cadeia até o que você adicionou. */
export function useWhyInPack(packId: PackId, path: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(graphKeys.why(packId, path), () => commands.graphWhyInPack(packId, path)),
    enabled,
    staleTime: 30_000,
  });
}
