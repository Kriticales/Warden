/**
 * "Abrir ou importar…" (T02, T04; decisão D20): abre o diálogo nativo de pasta e leva a
 * pasta escolhida para a página Abrir pack, que verifica antes de escrever qualquer coisa.
 * Desistir do diálogo não faz nada.
 */
import { useNavigate } from '@tanstack/react-router';

import { showToast } from '../../../components/ui/toast';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useChooseFolder } from '../api';

export function useOpenOrImport() {
  const navigate = useNavigate();
  const choose = useChooseFolder();
  const start = () => {
    choose.mutate('openPack', {
      onSuccess: (path) => {
        if (path !== null) {
          void navigate({ to: '/packs/abrir', search: { pasta: path } });
        }
      },
      onError: (error) => {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      },
    });
  };
  return { start, pending: choose.isPending };
}
