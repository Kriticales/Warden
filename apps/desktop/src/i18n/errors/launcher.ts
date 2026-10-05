import type { LauncherErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `launcher` (QUALITY §3): instalar o Minecraft e o loader, montar a
 * linha de comando e abrir o jogo. Dona: L-02. Parâmetros: `name`, `loader`, `version`,
 * `minecraft`, `path`, `javaMajor`.
 */
export const launcher: Record<LauncherErrorCode, string> = {
  INTERNAL:
    'Algo deu errado ao abrir o jogo. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  INVALID_PLAYER_NAME:
    'O nome do jogador "{{name}}" não serve: use de 3 a 16 letras sem acento, números ou _. Troque em Configurações.',
  LOADER_VERSION_MISSING:
    'O pack não diz a versão exata do {{loader}}. Escolha a versão do {{loader}} em Configurações do pack e teste de novo.',
  MINECRAFT_VERSION_NOT_FOUND:
    'A versão {{minecraft}} do Minecraft não existe. Confira a versão do Minecraft em Configurações do pack.',
  LOADER_VERSION_NOT_FOUND:
    'O {{loader}} {{version}} não existe para o Minecraft {{minecraft}}. Escolha outra versão do {{loader}} em Configurações do pack.',
  LOADER_INSTALL_FAILED:
    'A instalação do {{loader}} falhou. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  DOWNLOAD_FAILED:
    'Alguns arquivos do jogo não puderam ser baixados. Confira a conexão e tente de novo.',
  GAME_FILES_INVALID:
    'Os arquivos do jogo instalados estão estragados. Tente de novo; o Warden baixa de novo o que faltar.',
  JAVA_UNUSABLE:
    'O Java em {{path}} não pôde ser usado para instalar o jogo. Escolha outro Java em Ajustes do teste.',
  QUICK_PLAY_UNSUPPORTED:
    'Abrir direto no mundo só existe a partir do Minecraft 1.20; o {{minecraft}} abre no menu. Em Ajustes do teste, troque "Ao abrir o jogo" para "Menu principal".',
  COMMAND_TOO_LONG:
    'O pack tem arquivos demais para o Java {{javaMajor}} abrir o jogo no Windows. Escolha um Java mais novo em Ajustes do teste.',
  LAUNCH_FAILED:
    'O jogo não abriu: o Java não pôde ser iniciado. Confira se um antivírus não bloqueou o Java e tente de novo.',
  PROCESS_CONTROL_FAILED:
    'O Warden não conseguiu controlar o processo do jogo. Feche o jogo, se estiver aberto, e tente de novo.',
};
