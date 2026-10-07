/**
 * Dados dos formatos de outros launchers (E-02): a leitura do pack para o formato
 * (`export_format_analyze`) e a geração (`export_format_run`, com o diálogo nativo do Rust).
 * "Mostrar o arquivo" reaproveita `export_reveal` da E-01.
 *
 * A leitura é refeita ao abrir o formato e ao voltar para a janela, como a prévia da E-01: ela
 * diz o que a exportação vai fazer naquele momento.
 */
import { useQuery } from '@tanstack/react-query';

import { useOperations } from '../../../app/tasks/useOperations';
import {
  commands,
  type FormatChoices,
  type LauncherFormat,
  type OperationSnapshot,
  type PackId,
} from '../../../lib/ipc/bindings';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { CURRENT } from '../api';

/** Tipo da operação de `export_format_run` (`OperationKind` no Rust). */
export const EXPORT_FORMAT_OPERATION = 'export.formatRun';

export const formatKeys = {
  analysis: (packId: PackId, format: LauncherFormat) =>
    [...queryKeys.pack(packId), 'export', 'format', format] as const,
};

export function useFormatAnalysis(packId: PackId, format: LauncherFormat) {
  return useQuery({
    ...commandQuery(formatKeys.analysis(packId, format), () =>
      commands.exportFormatAnalyze(packId, CURRENT, format),
    ),
    staleTime: 0,
    refetchOnMount: 'always',
    refetchOnWindowFocus: 'always',
  });
}

/** `null` = a pessoa fechou o diálogo de destino sem escolher. */
export function useFormatRun(packId: PackId, format: LauncherFormat) {
  return useCommandMutation((choices: FormatChoices) =>
    commands.exportFormatRun(packId, CURRENT, format, choices),
  );
}

/** A exportação de formato deste pack em andamento, vista pela gaveta de Tarefas. */
export function useRunningFormatExport(packId: PackId): OperationSnapshot | null {
  const operations = useOperations();
  return (
    operations.data?.find(
      (operation) =>
        operation.kind === EXPORT_FORMAT_OPERATION &&
        operation.packId === packId &&
        operation.finishedAtMs === null,
    ) ?? null
  );
}
