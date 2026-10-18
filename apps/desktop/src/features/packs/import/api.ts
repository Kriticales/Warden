/**
 * Dados de Importar modpack (SPEC T24; ARCHITECTURE §12.2): a leitura do modpack
 * (`import_analyze`, que devolve o resumo e o token do plano guardado no Rust), a importação
 * (`import_apply`) e o descarte do plano (`import_discard`).
 *
 * Regra do disco como verdade: a importação cria um pack, então invalida a lista de Meus packs e
 * tudo o que é de pack (contagens, cabeçalho, perfis do teste).
 *
 * Os itens para revisar do pack importado (`import_review`) ficam sob a área `inventory` do
 * pack: quando o inventário muda (um item sai do pack), a contagem é relida.
 */
import { useQuery } from '@tanstack/react-query';

import { useOperations } from '../../../app/tasks/useOperations';
import {
  commands,
  type ImportApply,
  type OperationSnapshot,
  type PackId,
} from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { packsKeys } from '../api';

/** Tipo da operação de `import_analyze` (`OperationKind` no Rust). */
export const ANALYZE_OPERATION = 'import.analyze';

export const importKeys = {
  analysis: (origin: string) => ['import', 'analysis', origin] as const,
  /** Os itens para revisar de um pack importado. */
  review: (packId: PackId) =>
    [...queryKeys.packArea(packId, 'inventory'), 'import', 'review'] as const,
};

/**
 * A leitura do modpack. Cada leitura guarda um plano novo no Rust: não relê sozinha (foco da
 * janela, remontagem) e some do cache ao sair da página (quem usa descarta o token).
 */
export function useImportAnalysis(origin: string) {
  return useQuery({
    ...commandQuery(importKeys.analysis(origin), () => commands.importAnalyze(origin)),
    staleTime: Infinity,
    gcTime: 0,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
}

/** Esquece o plano guardado no Rust (e a cópia de staging do zip da CurseForge). */
export function discardPlan(token: string) {
  void commands.importDiscard(token);
}

export function useImportApply() {
  return useCommandMutation(
    ({ token, request }: { token: string; request: ImportApply }) =>
      commands.importApply(token, request),
    { invalidates: [packsKeys.list, ['pack']] },
  );
}

/** A leitura em andamento, vista pela gaveta de Tarefas (para o Cancelar). */
export function useRunningAnalysis(): OperationSnapshot | null {
  const operations = useOperations();
  return (
    operations.data?.find(
      (operation) => operation.kind === ANALYZE_OPERATION && operation.finishedAtMs === null,
    ) ?? null
  );
}

export function cancelOperation(operation: OperationSnapshot) {
  void commands.operationCancel(operation.id);
}

/** Os itens que a importação marcou para revisar (`null`: nada a revisar). */
export function useImportReview(packId: PackId) {
  return useQuery(commandQuery(importKeys.review(packId), () => commands.importReview(packId)));
}

/** "Já revisei": esconde o aviso deste pack. */
export function useImportReviewDone(packId: PackId) {
  return useCommandMutation(() => commands.importReviewDone(packId), {
    invalidates: [importKeys.review(packId)],
  });
}
