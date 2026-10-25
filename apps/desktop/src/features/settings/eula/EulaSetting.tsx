/**
 * "EULA do Minecraft" em Configurações → Teste (SPEC T21; CA-T21-05; decisão D19): "Aceita em
 * 01/10/2026" com **Revogar**, ou "Ainda não aceita". O aceite é dado no diálogo do "Testar
 * como servidor"; depois de revogar, o próximo servidor local pergunta de novo.
 */
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '../../../components/ui/button';
import { showToast } from '../../../components/ui/toast';
import { formatDate } from '../../../lib/format';
import { appErrorMessage, toAppError } from '../../../lib/ipc/errors';
import { useRevokeEula } from '../../test/server/api';
import { useSettings } from '../api';
import { StatusText } from '../components/fields';

/** `bare`: só a situação e o **Revogar**, dentro da linha "EULA do Minecraft" de Teste. */
export function EulaSetting({ bare = false }: { bare?: boolean }) {
  const { t } = useTranslation('servidor');
  const settings = useSettings();
  const revoke = useRevokeEula();
  const labelId = useId();
  const accepted = settings.data?.eulaAcceptedAt ?? null;
  const acceptedMs = accepted === null ? Number.NaN : Date.parse(accepted);
  const status = (
    <div className="row">
      {accepted === null ? (
        <StatusText kind="muted">{t('configuracoes.naoAceita')}</StatusText>
      ) : (
        <>
          <StatusText kind="ok">
            {Number.isNaN(acceptedMs)
              ? t('configuracoes.aceita')
              : t('configuracoes.aceitaEm', { data: formatDate(acceptedMs) })}
          </StatusText>
          <Button
            size="sm"
            variant="ghost"
            loading={revoke.isPending}
            onClick={() => {
              revoke.mutate(undefined, {
                onSuccess: () => {
                  showToast({ kind: 'ok', title: t('configuracoes.revogada') });
                },
                onError: (error) => {
                  showToast({ kind: 'danger', title: appErrorMessage(toAppError(error)) });
                },
              });
            }}
          >
            {t('configuracoes.revogar')}
          </Button>
        </>
      )}
    </div>
  );
  if (bare) {
    return status;
  }
  return (
    <div className="field" role="group" aria-labelledby={labelId}>
      <span className="field__label" id={labelId}>
        {t('configuracoes.rotulo')}
      </span>
      {status}
      <div className="field__hint">{t('configuracoes.dica')}</div>
    </div>
  );
}
