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
  NO_TEST_WORLD:
    'Para buscar um travamento ao entrar no mundo, a instância de teste precisa de um mundo. Abra o jogo pelo Testar, crie um mundo e feche o jogo.',
  NEEDS_SERVER:
    'No Minecraft {{minecraft}} o jogo não entra sozinho num mundo: a busca precisa abrir um servidor neste computador. Aceite o servidor local no diálogo, use “Trava ao abrir o jogo” ou o modo em que você diz se aconteceu.',
};
