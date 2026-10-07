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
};
