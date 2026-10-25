/**
 * A migração das configs pessoais (ADR-0067): quando o pack aberto ainda leva o `options.txt`
 * (ou outra preferência pessoal), o Warden avisa ao abrir o pack e pergunta antes de mexer.
 * Nada some sem o dono ver:
 *
 * - **Tirar do pack e guardar aqui**: a cópia fica guardada neste computador (a do teste local,
 *   se houver; senão a do pack) e o arquivo sai do pack;
 * - **Manter no pack**: o arquivo deixa de ser pessoal e continua indo para os jogadores;
 * - **Agora não**: pergunta de novo na próxima vez que o pack abrir.
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import { showToast } from '../../../components/ui/toast';
import type { PackId } from '../../../lib/ipc/bindings';
import { useConfigOverview, useKeepPersonalOut, useUnmarkPersonal } from './api';

/** Packs em que a pessoa escolheu "Agora não" nesta abertura do app. */
const postponed = new Set<PackId>();

export function PersonalMigration({ packId }: { packId: PackId }) {
  const { t } = useTranslation('configs');
  const overview = useConfigOverview(packId);
  const keepOut = useKeepPersonalOut(packId);
  const unmark = useUnmarkPersonal(packId);
  const [later, setLater] = useState(() => postponed.has(packId));
  const paths = overview.data?.personalInPack ?? [];
  const open = !later && paths.length > 0;
  const busy = keepOut.isPending || unmark.isPending;

  const postpone = () => {
    postponed.add(packId);
    setLater(true);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next && !busy) postpone();
      }}
    >
      <DialogContent
        title={t('migracao.titulo', { count: paths.length, arquivos: paths[0] ?? '' })}
        description={t('migracao.texto')}
        footer={
          <>
            <Button variant="ghost" disabled={busy} onClick={postpone}>
              {t('migracao.depois')}
            </Button>
            <Button
              disabled={busy}
              loading={unmark.isPending}
              onClick={() => {
                void Promise.all(paths.map((path) => unmark.mutateAsync(path))).catch(() => {
                  // O erro aparece no aviso padrão das ações; o diálogo continua aberto.
                });
              }}
            >
              {t('migracao.manter')}
            </Button>
            <Button
              variant="primary"
              disabled={busy}
              loading={keepOut.isPending}
              onClick={() => {
                keepOut.mutate(paths, {
                  onSuccess: (removed) => {
                    showToast({ kind: 'ok', title: t('migracao.feitoToast', { count: removed }) });
                  },
                });
              }}
            >
              {t('migracao.tirar')}
            </Button>
          </>
        }
      >
        <ul className="cfgx__migrate">
          {paths.map((path) => (
            <li key={path} className="t-mono">
              {path}
            </li>
          ))}
        </ul>
        <p className="t-sm t-2">{t('migracao.nada')}</p>
      </DialogContent>
    </Dialog>
  );
}
