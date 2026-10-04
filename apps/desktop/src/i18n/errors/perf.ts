import type { PerfErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `perf` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const perf: Record<PerfErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao medir o desempenho do jogo. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
