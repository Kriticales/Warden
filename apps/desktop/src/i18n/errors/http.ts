import type { HttpErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `http` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const http: Record<HttpErrorCode, string> = {
  INTERNAL:
    'Algo deu errado numa conexão com a internet. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
