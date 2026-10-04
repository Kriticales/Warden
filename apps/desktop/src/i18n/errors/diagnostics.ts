import type { DiagnosticsErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `diagnostics` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const diagnostics: Record<DiagnosticsErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao analisar os problemas do pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
