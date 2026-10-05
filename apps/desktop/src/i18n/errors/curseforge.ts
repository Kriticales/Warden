import type { CurseforgeErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `curseforge` (QUALITY §3): chave, busca, projetos, arquivos e
 * distribuição bloqueada. Dona: P1-04. As frases de chave seguem a SPEC (T08 e CA-T21-02).
 */
export const curseforge: Record<CurseforgeErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao falar com a CurseForge. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  CURSEFORGE_KEY_MISSING:
    'Para usar a CurseForge, informe a sua chave da CurseForge em Configurações.',
  CURSEFORGE_KEY_INVALID:
    'A CurseForge recusou a chave. Confira se ela foi copiada inteira ou gere outra no console da CurseForge.',
  MOD_NOT_FOUND:
    'O projeto {{modId}} não existe na CurseForge ou deixou de estar disponível. Procure outro na busca.',
  FILE_NOT_FOUND:
    'O arquivo {{fileId}} não existe mais na CurseForge. Escolha outra versão do mod.',
  DISTRIBUTION_BLOCKED:
    'O autor de {{file}} não deixa apps de terceiros baixarem este arquivo da CurseForge. Baixe-o pela página indicada nos detalhes técnicos ou troque pelo mesmo mod do Modrinth.',
  INVALID_QUERY:
    'A busca na CurseForge passou dos limites dela (filtros demais ou além dos primeiros 10 mil resultados). Refine a busca e tente de novo.',
  NOT_FOUND:
    'A CurseForge não encontrou o que foi pedido. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  RATE_LIMITED:
    'A CurseForge pediu para esperar antes de receber mais pedidos. Aguarde um minuto e tente de novo.',
  UNAVAILABLE:
    'A CurseForge está fora do ar ou com problemas agora. Tente de novo em alguns minutos.',
  REQUEST_REJECTED:
    'A CurseForge recusou o pedido (erro {{status}}). Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_RESPONSE:
    'A CurseForge respondeu algo que o Warden não entende. Tente de novo mais tarde; se continuar, copie os detalhes técnicos e registre o problema.',
};
