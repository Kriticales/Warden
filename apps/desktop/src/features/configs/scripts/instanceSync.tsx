/**
 * Salvar um script do pack com o jogo aberto (SPEC T26: "Com o jogo aberto, salvar grava na
 * instância de teste e oferece Salvar e recarregar"):
 *
 * - depois de gravar no pack, o Warden copia o script para a instância de teste e oferece
 *   **Recarregar no jogo** (ou recarrega direto, em **Salvar e recarregar**);
 * - se o script da instância foi mudado direto nela, nada é apagado: o aviso explica e a pessoa
 *   escolhe **Substituir na instância** ou abrir o da instância.
 */
import { RefreshCw, X } from 'lucide-react';
import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { PackId } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useCopyToInstance } from './api';

/** O aviso que fica à mostra depois de salvar no pack com o jogo aberto. */
export type SyncNote =
  | { kind: 'copied'; path: string }
  | { kind: 'conflict'; path: string; previousHash: string; reload: boolean }
  | null;

export interface CopyOptions {
  overwrite: boolean;
  /** Recarregar no jogo logo depois de copiar (**Salvar e recarregar**). */
  reload: boolean;
}

/**
 * A cópia do pack para a instância e o aviso que ela deixa. `copy` responde `true` quando a
 * instância ficou com o texto do pack.
 */
export function useInstanceSync(packId: PackId) {
  const mutation = useCopyToInstance(packId);
  const [note, setNote] = useState<SyncNote>(null);
  const { mutateAsync } = mutation;

  const copy = useCallback(
    async (path: string, previousHash: string, options: CopyOptions): Promise<boolean> => {
      try {
        const result = await mutateAsync({ path, previousHash, overwrite: options.overwrite });
        switch (result) {
          case 'copied':
          case 'unchanged':
            setNote(options.reload ? null : { kind: 'copied', path });
            return true;
          case 'instanceChanged':
            setNote({ kind: 'conflict', path, previousHash, reload: options.reload });
            return false;
          case 'noInstance':
            setNote(null);
            return false;
        }
      } catch (error) {
        setNote(null);
        showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
        return false;
      }
    },
    [mutateAsync],
  );

  return {
    copy,
    pending: mutation.isPending,
    note,
    closeNote: useCallback(() => {
      setNote(null);
    }, []),
  };
}

export interface InstanceSyncNoteProps {
  note: SyncNote;
  /** O arquivo aberto (o aviso de outro arquivo não aparece). */
  path: string;
  pending: boolean;
  reloading: boolean;
  /** A ferramenta não recarrega com o jogo aberto (CraftTweaker do 1.12.2): sem Recarregar. */
  restart: boolean;
  onReload: () => void;
  onOverwrite: (previousHash: string, reload: boolean) => void;
  onOpenInInstance: () => void;
  onClose: () => void;
}

export function InstanceSyncNote({
  note,
  path,
  pending,
  reloading,
  restart,
  onReload,
  onOverwrite,
  onOpenInInstance,
  onClose,
}: InstanceSyncNoteProps) {
  const { t } = useTranslation('scripts');
  if (note?.path !== path) {
    return null;
  }
  const close = (
    <Button size="sm" variant="ghost" icon={X} iconOnly onClick={onClose}>
      {t('comando.fechar')}
    </Button>
  );
  if (note.kind === 'copied') {
    return (
      <div className="script-note">
        <Alert
          kind="ok"
          compact
          role="status"
          title={t('copia.titulo')}
          actions={
            restart ? (
              close
            ) : (
              <>
                <Button size="sm" icon={RefreshCw} loading={reloading} onClick={onReload}>
                  {t('barra.recarregar')}
                </Button>
                {close}
              </>
            )
          }
        >
          {restart ? t('copia.textoReiniciar') : t('copia.texto')}
        </Alert>
      </div>
    );
  }
  return (
    <div className="script-note">
      <Alert
        kind="warn"
        compact
        role="status"
        title={t('conflito.titulo')}
        actionsBelow
        actions={
          <>
            <Button
              size="sm"
              loading={pending}
              onClick={() => {
                onOverwrite(note.previousHash, note.reload);
              }}
            >
              {note.reload ? t('conflito.substituirERecarregar') : t('conflito.substituir')}
            </Button>
            <Button size="sm" variant="ghost" onClick={onOpenInInstance}>
              {t('conflito.abrir')}
            </Button>
            {close}
          </>
        }
      >
        {t('conflito.texto')}
      </Alert>
    </div>
  );
}
