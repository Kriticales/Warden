/**
 * Pasta dos packs (T01 passo 3; T21 → Geral). O caminho só é mostrado: quem escolhe é o
 * diálogo nativo aberto pelo `settings_choose_packs_dir`, que valida e grava (D7; ARCHITECTURE
 * §4.1). Uma pasta recusada mostra o motivo no próprio campo.
 */
import { FolderOpen } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { commandError } from '../../../lib/ipc/query';
import { packsDirProblem, useChoosePacksDir, useSettingsStatus } from '../api';
import { describedBy, FieldShell } from './fields';

const REASONS = ['notFound', 'notDirectory', 'symlink', 'insideAppData', 'notWritable'] as const;
type Reason = (typeof REASONS)[number];

function isReason(value: string | null): value is Reason {
  return REASONS.some((reason) => reason === value);
}

export function PacksDirField({ hint }: { hint?: string }) {
  const { t } = useTranslation('configuracoes');
  const id = useId();
  const status = useSettingsStatus();
  const choose = useChoosePacksDir();

  if (status.isPending) {
    return <LoadingState inline label={t('carregando')} />;
  }
  if (status.isError) {
    return (
      <ErrorPanel
        compact
        error={status.error}
        onRetry={() => {
          void status.refetch();
        }}
      />
    );
  }

  const appError = commandError(choose.error);
  const reason = packsDirProblem(appError);
  const reasonText = isReason(reason) ? t(`pasta.motivo.${reason}`) : null;
  const hintText = hint ?? t('pasta.dica');

  return (
    <FieldShell id={id} label={t('pasta.rotulo')} hint={hintText} error={reasonText}>
      <div className="field__row">
        <input
          id={id}
          className="input input--mono"
          readOnly
          value={status.data.packsDir}
          aria-invalid={reasonText ? true : undefined}
          aria-describedby={describedBy(id, hintText, reasonText)}
        />
        <Button
          icon={FolderOpen}
          loading={choose.isPending}
          onClick={() => {
            choose.mutate(undefined);
          }}
        >
          {choose.isPending ? t('pasta.escolhendo') : t('pasta.escolher')}
        </Button>
      </div>
      {choose.isError && !reasonText ? <ErrorPanel compact error={choose.error} /> : null}
    </FieldShell>
  );
}
