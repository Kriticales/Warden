/**
 * Dados das atualizações (ARCHITECTURE §18): o relatório do pack (só memória, produzido por
 * `updates_check`), a revisão e a aplicação.
 *
 * O relatório fica sob a área `inventory` do pack: o evento `pack-changed` e as mutations do
 * inventário o releem (é uma leitura de memória, sem rede). Verificar e aplicar não fazem
 * atualização otimista: o disco é a verdade (a aplicação invalida o inventário).
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import {
  commands,
  type PackId,
  type ModUpdateReport,
  type UpdateSelection,
} from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';
import { unwrap } from '../../lib/ipc/result';
import { packsKeys } from '../packs/api';

export const updatesKeys = {
  /** O último relatório do pack. */
  report: (packId: PackId) => [...queryKeys.packArea(packId, 'inventory'), 'updates'] as const,
  /** A revisão de um conjunto de itens. */
  plan: (packId: PackId, paths: readonly string[]) =>
    [...queryKeys.packArea(packId, 'inventory'), 'updatePlan', ...paths] as const,
};

export function useUpdatesReport(packId: PackId) {
  return useQuery({
    ...commandQuery(updatesKeys.report(packId), () => commands.updatesReport(packId)),
    staleTime: 0,
  });
}

/** "Verificar atualizações": consulta as plataformas e guarda o relatório. */
export function useCheckUpdates(packId: PackId) {
  const queryClient = useQueryClient();
  return useCommandMutation(() => commands.updatesCheck(packId, 'manual'), {
    onSuccess: (report) => {
      queryClient.setQueryData(updatesKeys.report(packId), report);
    },
  });
}

/**
 * Verificação ao abrir o pack: o Rust decide se já passou o intervalo das Configurações (e se
 * o usuário quer verificar sozinho); aqui só pedimos, sem barulho, uma vez por pack aberto.
 */
export function useAutoCheckUpdates(packId: PackId) {
  const queryClient = useQueryClient();
  useEffect(() => {
    let active = true;
    void commands
      .updatesCheck(packId, 'auto')
      .then((result) => {
        if (!active) return;
        const report = unwrap(result);
        queryClient.setQueryData(updatesKeys.report(packId), report);
      })
      .catch(() => {
        // Verificação automática que falha é silenciosa: o botão Verificar atualizações mostra
        // o erro quando o usuário pedir.
      });
    return () => {
      active = false;
    };
  }, [packId, queryClient]);
}

export function useUpdatePlan(packId: PackId, paths: readonly string[] | null) {
  return useQuery({
    ...commandQuery(updatesKeys.plan(packId, paths ?? []), () =>
      commands.updatesPlan(packId, [...(paths ?? [])]),
    ),
    enabled: paths !== null && paths.length > 0,
    staleTime: 0,
    gcTime: 0,
  });
}

export function useApplyUpdates(packId: PackId) {
  return useCommandMutation(
    (selections: UpdateSelection[]) => commands.updatesApply(packId, selections),
    {
      invalidates: [
        queryKeys.packArea(packId, 'inventory'),
        queryKeys.packArea(packId, 'history'),
        packsKeys.row(packId),
        packsKeys.list,
      ],
    },
  );
}

export type { ModUpdateReport };
