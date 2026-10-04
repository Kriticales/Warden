import type { ServerErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `server` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const server: Record<ServerErrorCode, string> = {
  INTERNAL:
    'Algo deu errado no servidor local. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
