/**
 * Pasta dos packs (T01 passo 3; T21 → Geral). O caminho só é mostrado: quem escolhe é o
 * diálogo nativo aberto pelo `settings_choose_packs_dir`, que valida e grava (D7; ARCHITECTURE
 * §4.1). Uma pasta recusada mostra o motivo no próprio campo.
 */
import { CircleAlert, FolderOpen } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Button } from '../../../components/ui/button';
import { Icon } from '../../../components/ui/icon';
import { commandError } from '../../../lib/ipc/query';
import { packsDirProblem, useChoosePacksDir, useSettingsStatus } from '../api';
import { useRowLabel } from '../layout/SettingsFrame';
import { describedBy, FieldShell } from './fields';

const REASONS = ['notFound', 'notDirectory', 'symlink', 'insideAppData', 'notWritable'] as const;
type Reason = (typeof REASONS)[number];

function isReason(value: string | null): value is Reason {
  return REASONS.some((reason) => reason === value);
}

/**
 * `inRow`: dentro de uma linha de Configurações → Geral, onde o título e a descrição da linha
 * já são o rótulo e a dica do campo.
 */
export function PacksDirField({ hint, inRow = false }: { hint?: string; inRow?: boolean }) {
  const { t } = useTranslation('configuracoes');
  const id = useId();
  const rowLabel = useRowLabel();
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

  const errorId = `${id}-erro`;
  const control = (
    <>
      <div className="field__row">
        <input
          id={id}
          className="input input--mono"
          readOnly
          value={status.data.packsDir}
          aria-invalid={reasonText ? true : undefined}
          {...(inRow
            ? {
                'aria-labelledby': rowLabel['aria-labelledby'],
                'aria-describedby':
                  [rowLabel['aria-describedby'], reasonText ? errorId : null]
                    .filter(Boolean)
                    .join(' ') || undefined,
              }
            : { 'aria-describedby': describedBy(id, hintText, reasonText) })}
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
    </>
  );
  if (inRow) {
    return (
      <div className="stack-2">
        {control}
        {reasonText ? (
          <div className="field__error" id={errorId} role="alert">
            <Icon icon={CircleAlert} size="sm" />
            <span>{reasonText}</span>
          </div>
        ) : null}
      </div>
    );
  }
  return (
    <FieldShell id={id} label={t('pasta.rotulo')} hint={hintText} error={reasonText}>
      {control}
    </FieldShell>
  );
}
