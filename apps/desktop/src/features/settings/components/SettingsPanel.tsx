/**
 * Moldura de uma seção de Configurações (`.panel` com o `<h2>` que dá nome à região) e o
 * jeito comum de gravar uma configuração: erro vira toast com a frase do catálogo.
 */
import { useId, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { showToast } from '../../../components/ui/toast';
import type { SettingsPatch } from '../../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useUpdateSettings } from '../api';

export function SettingsPanel({ title, children }: { title: ReactNode; children: ReactNode }) {
  const titleId = useId();
  return (
    <section className="panel" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        {title}
      </h2>
      <div className="stack mt-3">{children}</div>
    </section>
  );
}

/** Grava uma mudança de configuração; avisa por toast quando o Warden recusa. */
export function useSaveSetting() {
  const { t } = useTranslation('configuracoes');
  const update = useUpdateSettings();
  const save = (patch: SettingsPatch, onSaved?: () => void) => {
    update.mutate(patch, {
      onSuccess: () => onSaved?.(),
      onError: (error) => {
        showToast({
          kind: 'danger',
          title: t('naoSalvou'),
          text: appErrorMessage(toAppError(error)),
        });
      },
    });
  };
  return { save, pending: update.isPending };
}
