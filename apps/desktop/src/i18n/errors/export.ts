import type { ExportErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `export` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const exportErrors: Record<ExportErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao exportar o pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  DESTINATION_NOT_EMPTY:
    'O destino escolhido já existe. Escolha uma pasta vazia ou um nome de arquivo novo. Nada foi gravado.',
  DESTINATION_INSIDE_PACK:
    'O destino fica dentro da pasta do pack. Escolha um lugar fora dela, para a exportação não entrar no próprio pack.',
  PACK_OUT_OF_DATE:
    'O índice do pack está desatualizado: o packwiz mudaria arquivos ao conferir. Reabra o pack no Warden para atualizar o índice e exporte de novo. Nada foi gravado.',
  EXCLUDE_NEEDS_MANUAL_RULE:
    'Este nome tem colchetes, que o .packwizignore lê como padrão. Para excluir este arquivo, acrescente a regra à mão no .packwizignore do pack.',
};
