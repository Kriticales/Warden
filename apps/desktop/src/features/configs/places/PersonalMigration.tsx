/**
 * A migração das configs pessoais (ADR-0067): quando o pack aberto ainda leva o `options.txt`
 * (ou outra preferência pessoal), o Warden avisa ao abrir o pack, numa faixa logo abaixo do
 * cabeçalho (não bloqueia o resto do pack), e pergunta antes de mexer. Nada some sem o dono ver:
 *
 * - **Tirar do pack e guardar aqui**: a cópia fica guardada neste computador (a do teste local,
 *   se houver; senão a do pack) e o arquivo sai do pack;
 * - **Manter no pack**: o arquivo deixa de ser pessoal e continua indo para os jogadores;
 * - **Agora não**: pergunta de novo na próxima vez que o pack abrir.
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
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

  if (!open) return null;
  return (
    <section className="pack-migrate" aria-labelledby="pack-migrate-title">
      <Alert
        kind="warn"
        title={
          <span id="pack-migrate-title">
            {t('migracao.titulo', { count: paths.length, arquivos: paths[0] ?? '' })}
          </span>
        }
        actionsBelow
        actions={
          <>
            <Button
              size="sm"
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
            <Button
              size="sm"
              disabled={busy}
              loading={unmark.isPending}
              onClick={() => {
                void Promise.all(paths.map((path) => unmark.mutateAsync(path))).catch(() => {
                  // O erro aparece no aviso padrão das ações; a faixa continua.
                });
              }}
            >
              {t('migracao.manter')}
            </Button>
            <Button size="sm" variant="ghost" disabled={busy} onClick={postpone}>
              {t('migracao.depois')}
            </Button>
          </>
        }
      >
        <p>
          {t('migracao.texto')} <span className="t-mono">{paths.join(', ')}</span>.
        </p>
        <p className="t-2">{t('migracao.nada')}</p>
      </Alert>
    </section>
  );
}
