import type { VersioningErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `versioning` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const versioning: Record<VersioningErrorCode, string> = {
  INTERNAL:
    'Algo deu errado no histórico de versões do pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
