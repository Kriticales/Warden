import type { JavaErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `java` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const java: Record<JavaErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao preparar o Java. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
