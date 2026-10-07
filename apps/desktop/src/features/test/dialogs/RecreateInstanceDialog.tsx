/**
 * "Recriar instância de teste…" do menu ▾ (SPEC T11 e T13): apaga a instância de teste; os
 * mundos ficam se o usuário pedir. Usa o comando e os textos dos Ajustes do teste (P1-08).
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { showToast } from '../../../components/ui/toast';
import type { PackId, PackRow } from '../../../lib/ipc/bindings';
import { useRecreateInstance, useTestSettings } from '../../pack-editor/api';

export interface InstanceDialogProps {
  packId: PackId;
  pack: PackRow;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function RecreateInstanceDialog({ packId, open, onOpenChange }: InstanceDialogProps) {
  const { t } = useTranslation('editor');
  const view = useTestSettings(packId, open);
  const recreate = useRecreateInstance(packId);
  const [keepWorlds, setKeepWorlds] = useState(true);
  const hasWorlds = view.data?.hasWorlds ?? false;
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t('ajustesTeste.recriarTitulo')}
      description={t('ajustesTeste.recriarTexto')}
      confirmLabel={t('ajustesTeste.recriarConfirmar')}
      confirmingLabel={t('ajustesTeste.recriando')}
      cancelLabel={t('ajustesTeste.cancelar')}
      onConfirm={async () => {
        await recreate.mutateAsync(hasWorlds && keepWorlds);
        showToast({ kind: 'ok', title: t('ajustesTeste.recriada') });
      }}
    >
      {hasWorlds ? (
        // O texto do rótulo está no <span> dentro dele (padrão `.check` do design system).
        // eslint-disable-next-line jsx-a11y/label-has-associated-control
        <label className="check">
          <input
            type="checkbox"
            checked={keepWorlds}
            onChange={(event) => {
              setKeepWorlds(event.target.checked);
            }}
          />
          <span className="check__text">
            <span>{t('ajustesTeste.manterMundos')}</span>
          </span>
        </label>
      ) : null}
    </ConfirmDialog>
  );
}
