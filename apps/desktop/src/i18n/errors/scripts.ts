import type { ScriptsErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `scripts` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const scripts: Record<ScriptsErrorCode, string> = {
  INTERNAL:
    'Algo deu errado nos scripts do pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
