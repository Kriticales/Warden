/**
 * Mudanças externas no pack aberto (SPEC T05; ARCHITECTURE §15; ROADMAP A-05).
 *
 * Enquanto a tela do pack está aberta, o Rust vigia a pasta. Quando outro programa mexe nela
 * (editor de texto, `git pull`), ele emite `pack-changed` com `external: true`: a invalidação
 * das queries já acontece na raiz do app (`usePackChangedInvalidation`), sem trocar de seção;
 * aqui fica o aviso discreto de que a tela foi atualizada.
 */
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { dismissToast, showToast } from '../../components/ui/toast';
import { log } from '../../lib/log';
import { commands, events, type PackId } from '../../lib/ipc/bindings';
import { useTauriEvent } from '../../lib/ipc/events';

/** Liga o vigia da pasta do pack enquanto o componente estiver montado. */
export function usePackWatch(packId: PackId): void {
  const { t } = useTranslation('editor');
  const toast = useRef<number | null>(null);

  useEffect(() => {
    const report = (cause: unknown) => {
      log.warn('não foi possível vigiar a pasta do pack', cause);
    };
    commands.packWatchStart(packId).then((result) => {
      if (result.status === 'error') {
        report(result.error);
      }
    }, report);
    return () => {
      commands.packWatchStop(packId).catch(report);
    };
  }, [packId]);

  useTauriEvent(events.packChanged, (change) => {
    if (!change.external || change.packId !== packId) {
      return;
    }
    // Rajadas seguidas mostram um aviso só.
    if (toast.current !== null) {
      dismissToast(toast.current);
    }
    toast.current = showToast({
      kind: 'info',
      title: t('mudancaExterna.titulo'),
      text: t('mudancaExterna.texto'),
    });
  });
}
