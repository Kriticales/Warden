/**
 * Dados da seção Exportar (ARCHITECTURE §12 e §18): a prévia (`export_preview`), "Excluir do
 * pack" (`export_exclude`), a exportação (`export_run`, com o diálogo nativo do Rust) e o
 * "Abrir pasta" do resultado (`export_reveal`).
 *
 * A prévia é relida do disco ao abrir a seção e ao voltar para a janela: ela mostra exatamente
 * o que a exportação vai levar naquele momento.
 */
import { useQuery } from '@tanstack/react-query';

import {
  commands,
  type ExportFormat,
  type ExportSource,
  type OperationSnapshot,
  type PackId,
} from '../../lib/ipc/bindings';
import { queryKeys } from '../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../lib/ipc/query';
import { useOperations } from '../../app/tasks/useOperations';
import { packsKeys } from '../packs/api';

/** Tipo da operação de `export_run` (`OperationKind` no Rust). */
export const EXPORT_OPERATION = 'export.run';

/** O estado atual do pack; "Exportar esta versão" (versão salva) é P1. */
export const CURRENT: ExportSource = { kind: 'current' };

export const exportKeys = {
  preview: (packId: PackId) => [...queryKeys.pack(packId), 'export', 'preview'] as const,
};

export function useExportPreview(packId: PackId) {
  return useQuery({
    ...commandQuery(exportKeys.preview(packId), () => commands.exportPreview(packId, CURRENT)),
    staleTime: 0,
    refetchOnMount: 'always',
    refetchOnWindowFocus: 'always',
  });
}

/** Excluir do pack muda o `.packwizignore` e o índice: tudo do pack é relido. */
export function useExcludeFromPack(packId: PackId) {
  return useCommandMutation((path: string) => commands.exportExclude(packId, path), {
    invalidates: [queryKeys.pack(packId), packsKeys.row(packId), packsKeys.list],
  });
}

/** `null` = a pessoa fechou o diálogo de destino sem escolher. */
export function useExportRun(packId: PackId) {
  return useCommandMutation((format: ExportFormat) => commands.exportRun(packId, CURRENT, format));
}

export function useRevealExport() {
  return useCommandMutation((path: string) => commands.exportReveal(path));
}

export function useCancelOperation() {
  return useCommandMutation(commands.operationCancel, { invalidates: [queryKeys.operations] });
}

/** A exportação deste pack em andamento, vista pela gaveta de Tarefas. */
export function useRunningExport(packId: PackId): OperationSnapshot | null {
  const operations = useOperations();
  return (
    operations.data?.find(
      (operation) =>
        operation.kind === EXPORT_OPERATION &&
        operation.packId === packId &&
        operation.finishedAtMs === null,
    ) ?? null
  );
}
