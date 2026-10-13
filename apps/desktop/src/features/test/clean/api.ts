/**
 * "Testar como o jogador recebe" (SPEC T13; ROADMAP L-07): o pedido do teste nesse modo e o
 * resultado gravado na sessão (o que o jogador recebeu × o pack).
 */
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';

import { showToast } from '../../../components/ui/toast';
import { commands, type PackId, type TestRequest } from '../../../lib/ipc/bindings';
import { commandQuery } from '../../../lib/ipc/query';
import { testKeys } from '../api';
import { DEFAULT_REQUEST, useGame, useTestStore } from '../store';

/** O pedido do `test_start` neste modo. */
export const PLAYER_REQUEST: TestRequest = { ...DEFAULT_REQUEST, mode: 'asPlayer' };

export const playerKeys = {
  /** O resultado de uma sessão (sob a sessão: some com o pack). */
  report: (packId: PackId, sessionId: string) =>
    [...testKeys.session(packId, sessionId), 'player'] as const,
};

/** O resultado gravado de uma sessão "como o jogador recebe" (`null` nas outras). */
export function usePlayerReport(packId: PackId, sessionId: string) {
  return useQuery({
    ...commandQuery(playerKeys.report(packId, sessionId), () =>
      commands.testPlayerReport(packId, sessionId),
    ),
    // Uma sessão gravada não muda.
    staleTime: Infinity,
  });
}

/**
 * Começa o teste como o jogador recebe. Com o jogo de outro pack aberto, avisa (um jogo por
 * vez) em vez de começar.
 */
export function useTestAsPlayer(packId: PackId) {
  const { t } = useTranslation('teste');
  const start = useTestStore((store) => store.start);
  const game = useGame();
  return () => {
    if (game) {
      showToast({ kind: 'warn', title: t('botao.outroJogo', { pack: game.packName }) });
      return;
    }
    void start(packId, PLAYER_REQUEST);
  };
}
