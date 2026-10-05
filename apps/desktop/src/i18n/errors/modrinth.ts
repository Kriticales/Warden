import type { ModrinthErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `modrinth` (QUALITY §3): busca, projetos, versões e o cache local
 * do Modrinth. Dona: P1-03.
 */
export const modrinth: Record<ModrinthErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao falar com o Modrinth. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  PROJECT_NOT_FOUND:
    'O projeto {{id}} não existe no Modrinth ou deixou de ser público. Confira o nome ou procure outro mod na busca.',
  VERSION_NOT_FOUND: 'A versão {{id}} não existe mais no Modrinth. Escolha outra versão do mod.',
  NOT_FOUND:
    'O Modrinth não encontrou o que foi pedido. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  RATE_LIMITED:
    'O Modrinth pediu para esperar antes de receber mais pedidos. Aguarde um minuto e tente de novo.',
  UNAVAILABLE:
    'O Modrinth está fora do ar ou com problemas agora. Tente de novo em alguns minutos.',
  REQUEST_REJECTED:
    'O Modrinth recusou o pedido (erro {{status}}). Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_RESPONSE:
    'O Modrinth respondeu algo que o Warden não entende. Tente de novo mais tarde; se continuar, copie os detalhes técnicos e registre o problema.',
  CACHE_UNAVAILABLE:
    'O Warden não conseguiu usar o cache do Modrinth no disco. Confira se há espaço livre e tente de novo.',
};
