import type { ProjectErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `project` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a tarefa
 * dona do domínio acrescenta uma frase para cada código novo.
 */
export const project: Record<ProjectErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao trabalhar no pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_INPUT: 'Confira os dados informados para o pack.',
  DESTINATION_NOT_EMPTY: 'A pasta de destino já contém arquivos. Escolha uma pasta vazia.',
  PACK_NOT_FOUND: 'Este pack não está na lista. Atualize Meus packs.',
  FOLDER_MISSING: 'Pasta não encontrada. Use Localizar… para indicar a nova pasta.',
  INVALID_PACK: 'Não foi possível ler este pack. Veja os detalhes técnicos.',
  UNSUPPORTED_LOADER: 'Este pack usa um loader que o Warden ainda não suporta.',
  INVALID_LOADER_VERSION: 'A versão escolhida não está disponível para esse loader e Minecraft.',
  ALREADY_REGISTERED: 'Este pack já está em Meus packs.',
  PACK_CHANGED_EXTERNALLY: 'O pack mudou fora do Warden. Atualize e tente de novo.',
  READ_ONLY: 'O repositório está em um estado que permite apenas leitura.',
  TRASH_CONFIRMATION: 'Digite o nome exato do pack para enviá-lo à Lixeira.',
  ITEM_NOT_FOUND: 'Este item não está mais no pack. Atualize a lista e tente de novo.',
  FILE_CHANGED_ON_DISK:
    'Este arquivo foi alterado fora do Warden depois que você o abriu. Nada foi gravado. Escolha Recarregar, Ver diferenças ou Sobrescrever.',
  CONFIG_NOT_FOUND:
    'O arquivo {{path}} não existe mais. Atualize a lista de arquivos e escolha outro.',
  CONFIG_NOT_EDITABLE:
    'Este arquivo só abre para leitura (binário, acima de 2 MB ou fora de UTF-8), então o Warden não grava nele.',
};
