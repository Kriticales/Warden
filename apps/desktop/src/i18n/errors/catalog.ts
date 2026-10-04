import type { CatalogErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `catalog` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const catalog: Record<CatalogErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao buscar as versões do Minecraft e dos loaders. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
};
