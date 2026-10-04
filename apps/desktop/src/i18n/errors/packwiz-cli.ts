import type { PackwizCliErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `packwizCli` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const packwizCli: Record<PackwizCliErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao executar o packwiz. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
