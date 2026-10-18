/**
 * "Abrir ou importar…" (T02, T04, T24; decisões D20 e D38): o único caminho para abrir um pack
 * packwiz ou importar um modpack de outro app. O botão de Meus packs oferece **Escolher
 * pasta…** e **Escolher arquivo…**, porque o diálogo nativo do Windows escolhe pasta ou arquivo,
 * nunca os dois; depois, o Warden reconhece o que foi escolhido pelo conteúdo:
 *
 * - pasta packwiz (ou não reconhecida): o diálogo Abrir pack, por cima de Meus packs, que
 *   verifica antes de escrever qualquer coisa e explica o que falta;
 * - pasta de instância do Prism, do MultiMC ou do app da CurseForge: Importar modpack;
 * - arquivo (`.mrpack`, zip da CurseForge, zip de instância do Prism): Importar modpack, que
 *   lê o conteúdo e recusa o que não for modpack.
 *
 * Desistir do diálogo não faz nada.
 */
import { useNavigate } from '@tanstack/react-router';

import { showToast } from '../../../components/ui/toast';
import { commands } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useCommandMutation } from '../../../lib/ipc/query';
import { useChooseFolder } from '../api';

export function useOpenOrImport() {
  const navigate = useNavigate();
  const folder = useChooseFolder();
  const file = useCommandMutation(() => commands.importChooseFile());
  const onError = (error: unknown) => {
    showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
  };
  const toImport = (path: string) => {
    void navigate({ to: '/packs/importar', search: { origem: path } });
  };
  const chooseFolder = () => {
    folder.mutate('openPack', {
      onSuccess: (path) => {
        if (path === null) return;
        void commands.importDetect(path).then((detected) => {
          const instance =
            detected.status === 'ok' &&
            (detected.data === 'prism' || detected.data === 'curseforgeApp');
          if (instance) {
            toImport(path);
          } else {
            // Pasta packwiz ou não reconhecida: o diálogo Abrir pack verifica e explica.
            void navigate({ to: '/packs', search: { pasta: path } });
          }
        });
      },
      onError,
    });
  };
  const chooseFile = () => {
    file.mutate(undefined, {
      onSuccess: (path) => {
        if (path !== null) toImport(path);
      },
      onError,
    });
  };
  return {
    chooseFolder,
    chooseFile,
    /** Qual diálogo está aberto (`null`: nenhum). */
    pending: folder.isPending ? ('folder' as const) : file.isPending ? ('file' as const) : null,
  };
}
