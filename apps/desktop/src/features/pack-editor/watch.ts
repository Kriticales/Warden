/**
 * Mudanças externas no pack aberto (SPEC T05; ARCHITECTURE §15; ROADMAP A-05).
 *
 * Enquanto a tela do pack está aberta, o Rust vigia a pasta. Quando outro programa mexe nela
 * (editor de texto, `git pull`), ele emite `pack-changed` com `external: true` e as áreas que
 * mudaram; a raiz do app invalida essas áreas (`usePackChangedInvalidation`), sem trocar de seção.
 *
 * As áreas não bastam numa mudança externa: muita coisa que a tela mostra do pack fica fora delas
 * (a linha do cabeçalho, a higiene, as alterações não salvas do Histórico, a prévia e as análises
 * do Exportar, os opcionais...). As escritas do próprio Warden cuidam disso pelas `invalidates`
 * de cada mutation; uma mudança por fora não passa por nenhuma. Por isso a regra aqui é uma só:
 * invalidar tudo o que está sob o prefixo do pack (`queryKeys.pack`), mais a lista de Meus packs,
 * que fica fora dele. Só as consultas da tela aberta são relidas na hora; as outras ficam velhas
 * e são relidas quando voltarem à tela. Uma consulta nova do pack entra na regra sozinha, desde
 * que a chave comece pelo prefixo (ARCHITECTURE §18).
 */
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { dismissToast, showToast } from '../../components/ui/toast';
import { log } from '../../lib/log';
import { commands, events, type PackId } from '../../lib/ipc/bindings';
import { useTauriEvent } from '../../lib/ipc/events';
import { queryKeys } from '../../lib/ipc/keys';
import { packsKeys } from '../packs/api';

/** Liga o vigia da pasta do pack enquanto o componente estiver montado. */
export function usePackWatch(packId: PackId): void {
  const { t } = useTranslation('editor');
  const queryClient = useQueryClient();
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
    // Tudo do pack (cabeçalho, seções, Histórico, Exportar...) e a lista de Meus packs.
    for (const queryKey of [queryKeys.pack(packId), packsKeys.list]) {
      void queryClient.invalidateQueries({ queryKey });
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
