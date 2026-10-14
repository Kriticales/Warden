/**
 * "Recarregar no jogo" (SPEC T26), usado pelo editor de scripts e pelo "Recarregar scripts" da
 * tela do teste:
 *
 * - KubeJS 7 com o jogo aberto: o Warden recarrega pelo servidor web do KubeJS (o token fica no
 *   Rust) e avisa;
 * - servidor local aberto: o Warden manda o comando pelo console e mostra a resposta;
 * - nos outros casos, o aviso com o comando certo para a versão e **Copiar**; no CraftTweaker do
 *   1.12.2, o aviso de que é preciso reiniciar.
 */
import { Copy, X } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import type { PackId, ReloadOutcome } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useReloadScripts } from './api';

/** O que fica à mostra depois de "Recarregar no jogo". */
export type ReloadNote = { kind: 'copy'; command: string } | { kind: 'restart' } | null;

/** Quantas linhas da resposta do servidor o aviso mostra. */
const RESPONSE_LINES = 3;

/**
 * A recarga e o aviso que ela deixa à mostra. Com `onNote`, o aviso vai para quem chamou (a
 * tela do teste o mostra num toast) em vez de ficar guardado aqui.
 */
export function useReloadAction(packId: PackId, onNote?: (note: ReloadNote) => void) {
  const { t } = useTranslation('scripts');
  const reload = useReloadScripts(packId);
  const [kept, setKept] = useState<ReloadNote>(null);
  const setNote = (note: ReloadNote) => {
    if (onNote) {
      if (note) onNote(note);
    } else {
      setKept(note);
    }
  };

  const done = (outcome: ReloadOutcome) => {
    const route = outcome.route;
    switch (route.kind) {
      case 'webServer':
        setNote(null);
        showToast({ kind: 'ok', title: t('recarregado.jogo') });
        break;
      case 'serverConsole':
        setNote(null);
        showToast({
          kind: 'ok',
          title: t('recarregado.servidor'),
          text: [
            t('recarregado.servidorTexto', { comando: route.command }),
            ...outcome.response.slice(-RESPONSE_LINES),
          ].join('\n'),
        });
        break;
      case 'copyCommand':
        setNote({ kind: 'copy', command: route.command });
        break;
      case 'restartGame':
        setNote({ kind: 'restart' });
        break;
      case 'noGame':
      case 'notAScript':
        setNote(null);
        break;
    }
  };

  const run = async (path: string): Promise<void> => {
    try {
      done(await reload.mutateAsync(path));
    } catch (error) {
      showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
    }
  };

  return {
    run,
    pending: reload.isPending,
    note: kept,
    closeNote: () => {
      setKept(null);
    },
  };
}

export interface ReloadNoteViewProps {
  note: ReloadNote;
  /** "KubeJS 6" (o título do aviso do comando). */
  tool: string;
  onClose: () => void;
}

/** O aviso do comando para copiar ou de reiniciar o jogo. */
export function ReloadNoteView({ note, tool, onClose }: ReloadNoteViewProps) {
  const { t } = useTranslation('scripts');
  if (note === null) {
    return null;
  }
  const close = (
    <Button size="sm" variant="ghost" icon={X} iconOnly onClick={onClose}>
      {t('comando.fechar')}
    </Button>
  );
  if (note.kind === 'restart') {
    return (
      <div className="script-note">
        <Alert kind="warn" compact role="status" title={t('reiniciar.titulo')} actions={close}>
          {t('reiniciar.texto')}
        </Alert>
      </div>
    );
  }
  return (
    <div className="script-note">
      <Alert
        kind="info"
        compact
        role="status"
        title={t('comando.titulo', { ferramenta: tool })}
        actionsBelow
        actions={
          <>
            <Button
              size="sm"
              icon={Copy}
              onClick={() => {
                void navigator.clipboard.writeText(note.command).then(() => {
                  showToast({ kind: 'ok', title: t('comando.copiado') });
                });
              }}
            >
              {t('comando.copiar', { comando: note.command })}
            </Button>
            {close}
          </>
        }
      >
        {t('comando.texto')}
      </Alert>
    </div>
  );
}
