/**
 * O fim da página nos formatos de outros launchers: Exportar… abre o diálogo nativo (no Rust);
 * durante a geração, o progresso com Cancelar; no fim, o resultado conferido. Com erros no
 * diagnóstico, pede "Exportar mesmo assim", como a E-01.
 */
import { Package } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Button } from '../../../components/ui/button';
import { Dialog } from '../../../components/ui/dialog';
import type {
  FormatAnalysis,
  FormatExportResult,
  LauncherFormat,
  OperationSnapshot,
  PackId,
} from '../../../lib/ipc/bindings';
import { commandError } from '../../../lib/ipc/query';
import { useCancelOperation } from '../api';
import { useFormatRun, useRunningFormatExport } from './api';
import { obstacle, toChoices, type FormatDecisions } from './model';
import { FormatResultDialog } from './FormatResultDialog';

function isCancelled(error: unknown): boolean {
  const code = commandError(error)?.code;
  return code?.domain === 'core' && code.code === 'CANCELLED';
}

export function FormatAction({
  packId,
  format,
  analysis,
  decisions,
  diagnosticErrors,
  readOnly,
}: {
  packId: PackId;
  format: LauncherFormat;
  analysis: FormatAnalysis;
  decisions: FormatDecisions;
  diagnosticErrors: number | null;
  readOnly: boolean;
}) {
  const { t } = useTranslation(['exportarFormatos', 'exportar']);
  const hintId = useId();
  const run = useFormatRun(packId, format);
  const running = useRunningFormatExport(packId);
  const [result, setResult] = useState<FormatExportResult | null>(null);
  const [confirmErrors, setConfirmErrors] = useState(false);
  const errors = diagnosticErrors ?? 0;
  const blocked = obstacle(analysis, decisions);

  const start = () => {
    run.mutate(toChoices(analysis, decisions), {
      onSuccess: (value) => {
        if (value) setResult(value);
      },
    });
  };
  const hint = (() => {
    if (blocked === 'blocked') return t('exportarFormatos:acao.bloqueado');
    if (blocked === 'version') return t('exportarFormatos:acao.faltaVersao');
    if (blocked === 'confirmation') return t('exportarFormatos:acao.faltaConfirmacao');
    return format === 'mrpack'
      ? t('exportarFormatos:acao.dicaMrpack')
      : t('exportarFormatos:acao.dicaCurseforge');
  })();

  return (
    <div>
      {run.isError && !isCancelled(run.error) ? (
        <ErrorPanel title={t('exportarFormatos:falha')} error={run.error} className="mt-4" />
      ) : null}
      {running ? (
        <RunningPanel operation={running} />
      ) : (
        <div className="export-foot">
          <p className="grow t-sm t-2" id={hintId}>
            {hint}
            {readOnly ? (
              <span className="t-3 block">{t('exportarFormatos:acao.somenteLeitura')}</span>
            ) : null}
          </p>
          <Button
            variant="primary"
            size="lg"
            icon={Package}
            loading={run.isPending}
            disabled={blocked !== null}
            aria-describedby={hintId}
            onClick={() => {
              if (errors > 0) setConfirmErrors(true);
              else start();
            }}
          >
            {t('exportarFormatos:acao.exportar')}
          </Button>
        </div>
      )}
      <ConfirmDialog
        open={confirmErrors}
        onOpenChange={setConfirmErrors}
        title={t('exportar:erros.titulo', { count: errors })}
        description={t('exportar:erros.texto')}
        confirmLabel={t('exportar:erros.confirmar')}
        onConfirm={start}
      />
      <Dialog
        open={result !== null}
        onOpenChange={(open) => {
          if (!open) setResult(null);
        }}
      >
        {result ? <FormatResultDialog result={result} /> : null}
      </Dialog>
    </div>
  );
}

function RunningPanel({ operation }: { operation: OperationSnapshot }) {
  const { t } = useTranslation('exportarFormatos');
  const cancel = useCancelOperation();
  const waiting = operation.state === 'waitingForLock';
  const progress = operation.progress;
  const value =
    progress && progress.total !== null && progress.total > 0
      ? (progress.current / progress.total) * 100
      : null;
  return (
    <div className="panel mt-4">
      <ProgressBar
        value={value}
        label={t('andamento.rotulo')}
        meta={waiting ? t('andamento.aguardando') : t('andamento.meta')}
        state={waiting ? 'waiting' : 'running'}
      />
      {operation.cancellable ? (
        <Button
          variant="ghost"
          size="sm"
          className="mt-3"
          loading={cancel.isPending || operation.state === 'cancelling'}
          onClick={() => {
            cancel.mutate(operation.id);
          }}
        >
          {t('andamento.cancelar')}
        </Button>
      ) : null}
    </div>
  );
}
