/**
 * "Abrir ou importar…" (T02, T04; decisão D20): abre o diálogo nativo de pasta e leva a
 * pasta escolhida para o diálogo Abrir pack, por cima de Meus packs (decisão D38), que
 * verifica antes de escrever qualquer coisa. Pasta de instância do Prism, do MultiMC ou do app
 * da CurseForge vai para Importar modpack (T24; P1-19), reconhecida pelo conteúdo.
 * Desistir do diálogo não faz nada.
 */
import { useNavigate } from '@tanstack/react-router';

import { showToast } from '../../../components/ui/toast';
import { commands } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useChooseFolder } from '../api';

export function useOpenOrImport() {
  const navigate = useNavigate();
  const choose = useChooseFolder();
  const start = () => {
    choose.mutate('openPack', {
      onSuccess: (path) => {
        if (path === null) return;
        void commands.importDetect(path).then((detected) => {
          const instance =
            detected.status === 'ok' &&
            (detected.data === 'prism' || detected.data === 'curseforgeApp');
          if (instance) {
            void navigate({ to: '/packs/importar', search: { origem: path } });
          } else {
            // Pasta packwiz ou não reconhecida: o diálogo Abrir pack verifica e explica.
            void navigate({ to: '/packs', search: { pasta: path } });
          }
        });
      },
      onError: (error) => {
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
      },
    });
  };
  return { start, pending: choose.isPending };
}
