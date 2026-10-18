import type { BisectErrorCode } from '../../lib/ipc/bindings';

/**
 * Frases dos erros do domínio `bisect` (QUALITY §3). Criado pela F0-05 só com `INTERNAL`; a D-12
 * acrescentou os outros códigos.
 */
export const bisect: Record<BisectErrorCode, string> = {
  INTERNAL:
    'Algo deu errado na busca do culpado. Tente de novo; se continuar, copie os detalhes técnicos e registre o problema.',
  NOT_FOUND: 'Não há busca do culpado guardada para este pack.',
  NOTHING_TO_SEARCH: 'Este pack não tem mods para procurar.',
  STATE_UNREADABLE:
    'Não foi possível ler a busca do culpado guardada ({{path}}). Comece uma busca nova.',
  NO_QUESTION: 'A busca do culpado não está esperando uma resposta agora.',
  NOT_RESUMABLE: 'Esta busca do culpado já terminou ou não está rodando.',
  EULA_NEEDED:
    'Para entrar no mundo, a busca usa um mundo de teste que o Warden cria com o servidor do Minecraft, e a Mojang exige que você aceite a EULA antes. Aceite a EULA no diálogo “Encontrar o mod culpado” ou escolha “Trava ao abrir o jogo”.',
  TEST_WORLD_FAILED:
    'O Warden não conseguiu criar o mundo de teste com o servidor do Minecraft {{minecraft}}. Confira a internet e tente de novo; os detalhes técnicos dizem o que falhou.',
  NEEDS_SERVER:
    'No Minecraft {{minecraft}} o jogo não entra sozinho num mundo; para isso o Warden vai precisar abrir um servidor neste computador, que ainda não está disponível. Use “Trava ao abrir o jogo” ou o modo em que você diz se aconteceu.',
};
