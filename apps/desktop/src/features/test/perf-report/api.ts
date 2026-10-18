/**
 * Testar com perfil de desempenho (SPEC T13; ROADMAP L-12): o pedido do teste nesse modo, o
 * perfil gravado na sessão (tempo por mod e resumo do spark) e "Abrir no visualizador do
 * spark".
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId, type TestRequest } from '../../../lib/ipc/bindings';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { testKeys } from '../api';
import { DEFAULT_REQUEST } from '../store';

/** O pedido do `test_start` neste modo. */
export const PERF_REQUEST: TestRequest = { ...DEFAULT_REQUEST, mode: 'performance' };

export const perfReportKeys = {
  /** O perfil de uma sessão (sob a sessão: some com o pack). */
  report: (packId: PackId, sessionId: string) =>
    [...testKeys.session(packId, sessionId), 'perf-profile'] as const,
};

/** O perfil de desempenho gravado de uma sessão (`null` nas sessões sem ele). */
export function usePerfReport(packId: PackId, sessionId: string) {
  return useQuery({
    ...commandQuery(perfReportKeys.report(packId, sessionId), () =>
      commands.perfReportGet(packId, sessionId),
    ),
    // Uma sessão gravada não muda.
    staleTime: Infinity,
  });
}

/**
 * "Abrir no visualizador do spark": não muda nada do pack nem da sessão (nada a invalidar).
 */
export function useOpenSparkViewer(packId: PackId, sessionId: string) {
  return useCommandMutation((file: string) =>
    commands.perfOpenSparkViewer(packId, sessionId, file),
  );
}
