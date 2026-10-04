import type { ConfigsErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `configs` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const configs: Record<ConfigsErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao ler ou gravar uma config. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
