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
    'O índice do pack não confere com os arquivos da pasta: algo mudou fora do Warden. Em Mods, confira os arquivos fora do índice (Incluir no pack) e exporte de novo. Nada foi gravado.',
  EXCLUDE_NEEDS_MANUAL_RULE:
    'Este nome tem colchetes, que o .packwizignore lê como padrão. Para excluir este arquivo, acrescente a regra à mão no .packwizignore do pack.',
  PRISM_VERSION_REQUIRED:
    'O pack ainda não tem versão. Salve uma versão em Salvar versão (ou preencha a versão no pack.toml) e gere a instância de novo. Nada foi gravado.',
  PRISM_BLOCKED:
    'Estes mods da CurseForge não deixam apps de terceiros baixar o arquivo: {{mods}}. O Prism não conseguiria baixá-los. Use Trocar pelo Modrinth quando houver, ou tire o mod do pack. Nada foi gravado.',
  PRISM_LOCAL_NEEDS_CONFIRMATION:
    'Arquivos de terceiros vão dentro da instância e falta a sua confirmação. Marque que você pode distribuí-los e gere de novo. Nada foi gravado.',
  PRISM_CURSEFORGE_KEY_MISSING:
    'O pack tem mods da CurseForge e falta a chave para conferir se podem ser distribuídos. Digite a chave em Configurações e gere de novo. Nada foi gravado.',
  PRISM_LOOKUP_FAILED:
    'Não foi possível conferir os mods no Modrinth ou na CurseForge. Veja a conexão com a internet e tente de novo. Nada foi gravado.',
  PRISM_DOWNLOAD_FAILED:
    'Não foi possível baixar ou conferir o arquivo de {{name}} para conferir e registrar cada um. Veja a conexão e tente de novo; se continuar, atualize ou troque o mod. Nada foi gravado.',
  PRISM_INVALID_OUTPUT:
    'O arquivo gerado não passou na conferência e foi descartado. Copie os detalhes técnicos e registre o problema. Nada foi gravado.',
  PRISM_UNSUPPORTED_PACK:
    'Este pack usa algo que a instância pronta para o Prism não sabe reproduzir (veja os detalhes técnicos). Use a pasta ou o zip do pack. Nada foi gravado.',
};
