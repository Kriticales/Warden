import type { ModrinthErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `modrinth` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const modrinth: Record<ModrinthErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao falar com o Modrinth. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
