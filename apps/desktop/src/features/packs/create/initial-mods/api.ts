/**
 * Dados da etapa "Mods iniciais" (SPEC T03; ADR-0033): a oferta para o Minecraft e o loader
 * escolhidos, antes de o pack existir, e a gravação depois do "Pack criado".
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type Loader, type PackId } from '../../../../lib/ipc/bindings';
import type { InitialModsRequest } from '../../../../lib/ipc/bindings';
import { commandQuery, useCommandMutation } from '../../../../lib/ipc/query';
import { packsKeys } from '../../api';

export const initialModsKeys = {
  offer: (minecraft: string, loader: Loader | null) =>
    ['initialMods', 'offer', minecraft, loader] as const,
};

/**
 * O que a etapa oferece. Consulta o Modrinth; erro de rede vira o aviso "Não foi possível
 * consultar os mods iniciais agora" (a etapa deixa seguir). Sem loader, nada a consultar.
 */
export function useInitialOffer(minecraft: string | null, loader: Loader | null) {
  return useQuery({
    ...commandQuery(initialModsKeys.offer(minecraft ?? '', loader), () =>
      commands.initialModsOffer(minecraft ?? '', loader),
    ),
    enabled: minecraft !== null && loader !== null,
    // Os dados mudam devagar e o assistente volta para esta etapa: não reconsulta a cada visita.
    staleTime: 5 * 60_000,
    retry: false,
  });
}

/** Grava os mods iniciais no pack recém-criado. */
export function useInitialModsApply() {
  return useCommandMutation(
    ({ packId, request }: { packId: PackId; request: InitialModsRequest }) =>
      commands.initialModsApply(packId, request),
    { invalidates: [packsKeys.list] },
  );
}
