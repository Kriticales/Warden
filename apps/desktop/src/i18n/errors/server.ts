import type { ServerErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `server` (QUALITY §3; ROADMAP L-09). Criado pela F0-05 só com
 * `INTERNAL`; a L-09 acrescentou os códigos do servidor local.
 */
export const server: Record<ServerErrorCode, string> = {
  INTERNAL:
    'Algo deu errado no servidor local. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  EULA_NOT_ACCEPTED:
    'A EULA do Minecraft ainda não foi aceita, então o servidor local não abre. Use "Testar como servidor…" no menu ▾ do Testar para aceitar.',
  LOADER_UNSUPPORTED: 'O servidor local ainda não abre packs com {{loader}}.',
  LOADER_VERSION_NOT_FOUND:
    'O {{loader}} {{version}} não existe para o Minecraft {{minecraft}}. Confira a versão do loader nas informações do pack.',
  NO_SERVER_DOWNLOAD: 'O Minecraft {{minecraft}} não tem servidor oficial para baixar.',
  DOWNLOAD_FAILED:
    'Não deu para baixar o servidor ({{source}}). Confira a internet e tente de novo.',
  INSTALLER_FAILED:
    'O instalador do {{loader}} terminou com erro (código {{code}}). Veja os detalhes técnicos e tente de novo.',
  INSTALLER_OUTPUT_MISSING:
    'O instalador do {{loader}} terminou, mas o servidor não ficou instalado. Veja os detalhes técnicos e tente de novo.',
  NO_FREE_PORT:
    'Nenhuma porta livre neste computador para o servidor local. Feche outros servidores e tente de novo.',
  EXITED_BEFORE_READY: 'O servidor fechou antes de ficar pronto (código {{code}}).',
  READY_TIMEOUT: 'O servidor não ficou pronto em {{seconds}} s. Veja o console do servidor.',
  NOT_RUNNING: 'O servidor local não está aberto.',
  INVALID_COMMAND: 'Comando inválido: escreva um comando de uma linha, com até 256 caracteres.',
  PROCESS_FAILED: 'O servidor local não pôde ser aberto. Veja os detalhes técnicos.',
};
