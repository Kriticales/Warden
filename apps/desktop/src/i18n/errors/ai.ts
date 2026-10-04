import type { AiErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `ai` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const ai: Record<AiErrorCode, string> = {
  INTERNAL:
    'Algo deu errado no Diagnóstico com IA. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
