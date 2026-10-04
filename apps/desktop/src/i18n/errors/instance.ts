import type { InstanceErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `instance` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const instance: Record<InstanceErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao preparar a instância de teste. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
