import type { ImportErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `import` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const importErrors: Record<ImportErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao importar o pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
