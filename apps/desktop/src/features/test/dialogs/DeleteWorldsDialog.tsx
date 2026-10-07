/**
 * "Apagar mundos de teste…" do menu ▾ (SPEC T13; decisão D6): lista os mundos da instância de
 * teste e apaga todos depois de confirmar. Os mundos nunca vão para o pack.
 */
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { showToast } from '../../../components/ui/toast';
import { useDeleteWorlds, useInstanceWorlds } from '../api';
import type { InstanceDialogProps } from './RecreateInstanceDialog';

export function DeleteWorldsDialog({ packId, open, onOpenChange }: InstanceDialogProps) {
  const { t } = useTranslation('teste');
  const worlds = useInstanceWorlds(packId, open);
  const remove = useDeleteWorlds(packId);
  const names = worlds.data ?? [];
  const count = names.length;
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t('mundos.titulo')}
      description={t('mundos.texto')}
      confirmLabel={t('mundos.confirmar', { count })}
      confirmingLabel={t('mundos.apagando')}
      cancelLabel={t('mundos.cancelar')}
      onConfirm={async () => {
        if (count === 0) {
          return;
        }
        const removed = await remove.mutateAsync(undefined);
        showToast({ kind: 'ok', title: t('mundos.feito', { count: removed }) });
      }}
    >
      {worlds.isPending ? (
        <p className="t-sm t-3">{t('mundos.lendo')}</p>
      ) : count === 0 ? (
        <p className="t-sm t-3">{t('mundos.nenhum')}</p>
      ) : (
        <ul className="worlds-list">
          {names.map((name) => (
            <li key={name} className="path">
              {name}
            </li>
          ))}
        </ul>
      )}
    </ConfirmDialog>
  );
}
