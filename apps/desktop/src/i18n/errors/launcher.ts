import type { LauncherErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `launcher` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const launcher: Record<LauncherErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao abrir o jogo. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
