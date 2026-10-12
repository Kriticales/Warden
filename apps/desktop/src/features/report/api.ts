/**
 * Comandos do relato (UI-02): a pré-visualização (montada e redigida no Rust) e a abertura no
 * navegador do endereço que a pré-visualização mostrou.
 */
import { keepPreviousData, useQuery } from '@tanstack/react-query';

import { commands, type ProblemReportInput } from '../../lib/ipc/bindings';
import { useCommandMutation } from '../../lib/ipc/query';
import { unwrap } from '../../lib/ipc/result';

export const reportKeys = {
  preview: (input: ProblemReportInput) => ['app', 'problemReport', input] as const,
};

/**
 * A pré-visualização. Lê os registros de novo a cada mudança do texto (com o atraso de quem
 * chama) e nunca fica em cache depois de fechar o diálogo.
 */
export function useReportPreview(input: ProblemReportInput) {
  return useQuery({
    queryKey: reportKeys.preview(input),
    queryFn: async () => unwrap(await commands.problemReportPreview(input)),
    placeholderData: keepPreviousData,
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: 0,
  });
}

/** Abre o endereço da pré-visualização no navegador padrão. */
export function useOpenReport() {
  return useCommandMutation((url: string) => commands.problemReportOpen(url));
}
