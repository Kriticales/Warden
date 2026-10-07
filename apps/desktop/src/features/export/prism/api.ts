/**
 * Dados da instância pronta para o Prism (ARCHITECTURE §12.3): a análise (`prism_analyze`),
 * a geração (`prism_run`, com o diálogo nativo do Rust) e o "Mostrar arquivo" do resultado.
 */
import { useQuery } from '@tanstack/react-query';

import { useOperations } from '../../../app/tasks/useOperations';
import {
  commands,
  type OperationSnapshot,
  type PackId,
  type PrismOptions,
} from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { CURRENT } from '../api';

/** Tipo da operação de `prism_run` (`OperationKind` no Rust). */
export const PRISM_OPERATION = 'export.prism';

export const prismKeys = {
  analysis: (packId: PackId) => [...queryKeys.pack(packId), 'export', 'prism'] as const,
};

/** Relida ao abrir e ao voltar para a janela: reflete o pack e a chave de agora. */
export function usePrismAnalysis(packId: PackId, enabled: boolean) {
  return useQuery({
    ...commandQuery(prismKeys.analysis(packId), () => commands.prismAnalyze(packId, CURRENT)),
    enabled,
    staleTime: 0,
    refetchOnMount: 'always',
    refetchOnWindowFocus: 'always',
  });
}

/** `null` = a pessoa fechou o diálogo de destino sem escolher. */
export function usePrismRun(packId: PackId) {
  return useCommandMutation((options: PrismOptions) => commands.prismRun(packId, CURRENT, options));
}

export function useRevealPrism() {
  return useCommandMutation((path: string) => commands.prismReveal(path));
}

/** A geração deste pack em andamento, vista pela gaveta de Tarefas. */
export function useRunningPrism(packId: PackId): OperationSnapshot | null {
  const operations = useOperations();
  return (
    operations.data?.find(
      (operation) =>
        operation.kind === PRISM_OPERATION &&
        operation.packId === packId &&
        operation.finishedAtMs === null,
    ) ?? null
  );
}
