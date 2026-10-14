import type { ScriptsErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `scripts` (QUALITY §3; C-07): editor de scripts KubeJS e
 * CraftTweaker, servidor web do KubeJS 7, console do servidor local e VS Code.
 */
export const scripts: Record<ScriptsErrorCode, string> = {
  INTERNAL:
    'Algo deu errado nos scripts do pack. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  WEB_SERVER_UNAVAILABLE:
    'O KubeJS não respondeu no jogo aberto. Espere o jogo terminar de carregar o mundo e tente de novo.',
  WEB_SERVER_REJECTED:
    'O KubeJS recusou o pedido de recarga (código {{status}}). Feche o jogo, abra o teste de novo e tente outra vez.',
  SERVER_CONSOLE_FAILED:
    'O servidor local não aceitou o comando. Confira se ele ainda está aberto e tente de novo.',
  VS_CODE_NOT_FOUND:
    'O VS Code não foi encontrado neste computador. Instale o VS Code e tente de novo.',
  VS_CODE_FAILED:
    'O VS Code não abriu. Tente de novo; se continuar, abra a pasta da instância por fora.',
};
