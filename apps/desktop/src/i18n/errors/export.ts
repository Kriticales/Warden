import type { ExportErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `export` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const exportErrors: Record<ExportErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao exportar o pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
