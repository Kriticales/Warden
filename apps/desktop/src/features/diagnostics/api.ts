/**
 * Chamadas do diagnóstico (ARCHITECTURE §4.1, domínio `diagnostics`): o último relatório do
 * pack, "Verificar agora", "Ignorar neste pack", "Por que travou" de uma sessão, a dependência
 * que falta e a redação do "Copiar relatório".
 *
 * O relatório fica sob a área `inventory` do pack: quando o inventário muda (`pack-changed`), ele
 * é relido e o backend diz se ficou velho (`stale`).
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import {
  commands,
  type DiagnosticsReport,
  type PackId,
  type SourceId,
} from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';

export const diagnosticsKeys = {
  /** O último relatório do pré-teste. */
  report: (packId: PackId) =>
    [...queryKeys.packArea(packId, 'inventory'), 'diagnostics', 'report'] as const,
  /** "Por que travou" de uma sessão (não muda depois de gravada). */
  crash: (packId: PackId, sessionId: string) =>
    [...queryKeys.pack(packId), 'test', 'crash', sessionId] as const,
};

/** O último relatório (`null`: nunca verificado). Sempre relido ao abrir a tela. */
export function useDiagnosticsReport(packId: PackId) {
  return useQuery({
    ...commandQuery(diagnosticsKeys.report(packId), () => commands.diagnosticsReportGet(packId)),
    staleTime: 0,
  });
}

/** "Verificar agora": o relatório novo entra direto no cache. */
export function useRunCheck(packId: PackId) {
  const queryClient = useQueryClient();
  return useCommandMutation(() => commands.diagnosticsRun(packId), {
    onSuccess: (report) => {
      queryClient.setQueryData<DiagnosticsReport | null>(diagnosticsKeys.report(packId), report);
    },
  });
}

/** "Ignorar neste pack" (ou desfazer). */
export function useIgnoreFinding(packId: PackId) {
  const queryClient = useQueryClient();
  return useCommandMutation(
    ({ findingId, ignored }: { findingId: string; ignored: boolean }) =>
      commands.diagnosticsIgnore(packId, findingId, ignored),
    {
      onSuccess: (report) => {
        queryClient.setQueryData<DiagnosticsReport | null>(diagnosticsKeys.report(packId), report);
      },
    },
  );
}

/** "Por que travou" de uma sessão que travou. */
export function useCrashDiagnosis(packId: PackId, sessionId: string, enabled: boolean) {
  return useQuery({
    ...commandQuery(diagnosticsKeys.crash(packId, sessionId), () =>
      commands.diagnosticsCrashGet(packId, sessionId),
    ),
    enabled,
    staleTime: Infinity,
    retry: false,
  });
}

/** O projeto que resolve uma dependência que falta. */
export function useFindDependency(packId: PackId) {
  return useCommandMutation(({ modId, source }: { modId: string; source: SourceId | null }) =>
    commands.diagnosticsDependencyFind(packId, modId, source),
  );
}

/** O texto do relatório, sem dados pessoais. */
export function useRedact(packId: PackId) {
  return useCommandMutation((text: string) => commands.diagnosticsRedact(packId, text));
}
