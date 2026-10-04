import type { DiscoveryErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `discovery` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const discovery: Record<DiscoveryErrorCode, string> = {
  INTERNAL:
    'Algo deu errado na página de descoberta. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
