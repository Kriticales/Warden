/**
 * Começa (ou continua) a busca do culpado: vai para a tela do teste no modo da busca e roda a
 * busca fora da tela (dá para ir a outras seções). Um erro do começo (outro jogo aberto, sem
 * mundo de teste, versão que precisa do servidor local) vira um aviso.
 */
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';

import { showToast } from '../../components/ui/toast';
import type { PackId } from '../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { runBisect, type BisectRunKind } from './api';

function isCancelled(error: unknown): boolean {
  const app = toAppError(error);
  return app.code.domain === 'core' && app.code.code === 'CANCELLED';
}

export function useStartBisect(packId: PackId) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  return (run: BisectRunKind) => {
    void navigate({ to: '/packs/$packId/teste', params: { packId }, search: { modo: 'busca' } });
    runBisect(queryClient, packId, run).catch((error: unknown) => {
      if (!isCancelled(error)) {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      }
    });
  };
}
