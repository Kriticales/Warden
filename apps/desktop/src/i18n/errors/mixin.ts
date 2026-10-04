import type { MixinErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `mixin` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const mixin: Record<MixinErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao ver o que um mod altera no jogo. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
