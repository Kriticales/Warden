import type { JarmetaErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `jarmeta` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const jarmeta: Record<JarmetaErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao ler as informações de um mod. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
