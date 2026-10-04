import type { ProjectErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `project` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const project: Record<ProjectErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao trabalhar no pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
