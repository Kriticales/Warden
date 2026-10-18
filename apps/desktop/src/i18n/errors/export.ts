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
  VERSION_REQUIRED:
    'O pack não tem versão no pack.toml e este formato exige uma. Digite a versão (por exemplo, 1.0.0) na tela de exportação. O pack não muda.',
  EMBED_NEEDS_CONFIRMATION:
    'Há arquivos de terceiros que iriam dentro do arquivo gerado. Confirme que você pode incluí-los, ou troque pelo Modrinth, e exporte de novo. Nada foi gravado.',
  BLOCKED_BY_AUTHOR:
    'Há mods da CurseForge que o autor não deixa outros apps distribuírem e que não existem no Modrinth. Este formato não pode levá-los. Veja a lista na tela de exportação. Nada foi gravado.',
  SWAP_NOT_AVAILABLE:
    'Um dos mods escolhidos para trocar pelo Modrinth não tem mais um equivalente lá. Leia a tela de exportação de novo e escolha outra vez. Nada foi gravado.',
  CURSEFORGE_KEY_MISSING:
    'Para conferir os mods da CurseForge deste pack é preciso a chave da CurseForge. Cadastre-a em Configurações, em Chaves e contas, e tente de novo.',
  LOOKUP_FAILED:
    'Não foi possível consultar o Modrinth ou a CurseForge para conferir os arquivos. Confira a conexão com a internet e tente de novo. Nada foi gravado.',
  FORMAT_TOOL_FAILED:
    'O packwiz não conseguiu gerar o arquivo. Os detalhes técnicos dizem o que ele respondeu. Nada foi gravado.',
  INVALID_OUTPUT:
    'O arquivo gerado não passou na conferência e foi descartado. Copie os detalhes técnicos e registre o problema. Nada foi gravado.',
  UNREADABLE_ARCHIVE:
    'Não foi possível ler este arquivo como o formato esperado. Confira se ele é um .mrpack ou um zip da CurseForge inteiro.',
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
    'Não foi possível baixar ou conferir o arquivo de {{name}}. Veja a conexão e tente de novo; se continuar, atualize ou troque o mod. Nada foi gravado.',
  PRISM_INVALID_OUTPUT:
    'O arquivo gerado não passou na conferência e foi descartado. Copie os detalhes técnicos e registre o problema. Nada foi gravado.',
  PRISM_UNSUPPORTED_PACK:
    'Este pack usa algo que a instância pronta para o Prism não sabe reproduzir (veja os detalhes técnicos). Use a pasta ou o zip do pack. Nada foi gravado.',
  SERVER_PACK_UNSUPPORTED_PACK:
    'Este pack usa algo que o pacote para servidor não sabe reproduzir (veja os detalhes técnicos). Nada foi gravado.',
  SERVER_PACK_UNSUPPORTED_LOADER:
    'O pacote para servidor ainda não sai para packs com {{loader}}. Nada foi gravado.',
  SERVER_PACK_NOT_PUBLISHED:
    'Para baixar os mods pelo link do pack, publique uma versão antes (Publicar versão, no Histórico) ou escolha Mods dentro do zip. Nada foi gravado.',
  SERVER_PACK_NEEDS_CONFIRMATION:
    'Os mods iriam dentro do zip e falta a sua confirmação de que pode distribuí-los. Marque a confirmação e gere de novo. Nada foi gravado.',
  SERVER_PACK_MANUAL_DOWNLOADS:
    'Estes mods da CurseForge precisam ser baixados à mão antes de gerar: {{mods}}. Gere de novo e baixe cada um pela lista que aparece. Nada foi gravado.',
  SERVER_PACK_DOWNLOAD_FAILED:
    'Não foi possível baixar ou conferir um dos mods do servidor; os detalhes técnicos dizem qual. Veja a conexão e tente de novo; se continuar, atualize ou troque o mod.',
  SERVER_PACK_LOOKUP_FAILED:
    'Não foi possível conferir os mods na CurseForge. Veja a conexão com a internet e tente de novo.',
  SERVER_PACK_INVALID_OUTPUT:
    'O pacote gerado não passou na conferência e foi descartado. Copie os detalhes técnicos e registre o problema. Nada foi gravado.',
};
