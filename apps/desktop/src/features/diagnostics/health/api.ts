/**
 * Chamadas da saúde do pack (ARCHITECTURE §4.1, domínio `health`; ROADMAP D-08): a nota do pack
 * agora (topo de Problemas) e as últimas notas guardadas (coluna Saúde de Meus packs).
 *
 * A nota do pack fica sob a área `inventory` (mudar o pack a invalida) e leva na chave o que ela
 * usa e muda sem `pack-changed`: o relatório do pré-teste (verificação nova, "Ignorar neste
 * pack") e os travamentos (fim de um teste, travamentos apagados). Cada cálculo grava a nota em
 * `packs.json`, por isso invalida a lista de Meus packs: a linha do pack nunca fica com outra
 * nota.
 */
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';

import {
  commands,
  type DiagnosticsReport,
  type HealthSummary,
  type PackId,
} from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { unwrap } from '../../../lib/ipc/result';

export const healthKeys = {
  /** A nota de um pack (o resto da chave diz de que relatório e travamentos ela é). */
  pack: (packId: PackId) => [...queryKeys.packArea(packId, 'inventory'), 'health'] as const,
  /** As últimas notas guardadas (sob `['packs']`, que o fim de um teste invalida). */
  list: ['packs', 'health'] as const,
};

/**
 * O que a nota usa do relatório: quando foi verificado e o que está ignorado (`nunca`: sem
 * relatório; `lendo`: ainda lendo).
 */
export function reportStamp(report: DiagnosticsReport | null | undefined): string {
  if (report === undefined) return 'lendo';
  if (report === null) return 'nunca';
  const ignored = report.findings.filter((entry) => entry.ignored).map((entry) => entry.id);
  return `${String(report.checkedAtMs)}|${ignored.join('\n')}`;
}

/**
 * A nota do pack agora (`null`: nunca verificado). `crashesAt` é a última leitura dos
 * travamentos (`undefined`: ainda lendo; a nota espera, para não calcular duas vezes).
 */
export function usePackHealth(
  packId: PackId,
  report: DiagnosticsReport | null | undefined,
  crashesAt: number | undefined,
) {
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: [...healthKeys.pack(packId), reportStamp(report), crashesAt] as const,
    queryFn: async () => {
      const health = unwrap(await commands.healthGet(packId));
      void queryClient.invalidateQueries({ queryKey: healthKeys.list });
      return health;
    },
    enabled: report !== undefined && crashesAt !== undefined,
    staleTime: 0,
    // Relatório novo muda a chave: a nota anterior fica na tela até a nova chegar.
    placeholderData: keepPreviousData,
  });
}

/** As últimas notas guardadas, por pack. */
export function useHealthList() {
  return useQuery({
    queryKey: healthKeys.list,
    queryFn: async () => {
      const list = await commands.healthList();
      return new Map<PackId, HealthSummary>(list.map((entry) => [entry.packId, entry.health]));
    },
    staleTime: 0,
  });
}
