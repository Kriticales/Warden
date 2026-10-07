/**
 * O fim da página (SPEC T19): Exportar… abre o diálogo nativo de destino (no Rust); durante a
 * exportação, o progresso com Cancelar (a operação `export.run` da gaveta de Tarefas); no fim,
 * "Pack exportado" com Abrir pasta. Com erros no diagnóstico, pede "Exportar mesmo assim".
 */
import { FolderOpen, Package } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ConfirmDialog } from '../../../components/common/ConfirmDialog';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { ProgressBar } from '../../../components/common/ProgressBar';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogClose, DialogContent } from '../../../components/ui/dialog';
import type {
  ExportFormat,
  ExportResult,
  OperationSnapshot,
  PackId,
} from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { commandError } from '../../../lib/ipc/query';
import { useCancelOperation, useExportRun, useRevealExport, useRunningExport } from '../api';

function isCancelled(error: unknown): boolean {
  const code = commandError(error)?.code;
  return code?.domain === 'core' && code.code === 'CANCELLED';
}

export function ExportAction({
  packId,
  format,
  diagnosticErrors,
  readOnly,
}: {
  packId: PackId;
  format: ExportFormat;
  diagnosticErrors: number | null;
  readOnly: boolean;
}) {
  const { t } = useTranslation('exportar');
  const run = useExportRun(packId);
  const running = useRunningExport(packId);
  const [result, setResult] = useState<{ format: ExportFormat; value: ExportResult } | null>(null);
  const [confirmErrors, setConfirmErrors] = useState(false);
  const errors = diagnosticErrors ?? 0;

  const start = () => {
    run.mutate(format, {
      onSuccess: (value) => {
        if (value) setResult({ format, value });
      },
    });
  };

  return (
    <div>
      {run.isError && !isCancelled(run.error) ? (
        <ErrorPanel title={t('falha')} error={run.error} className="mt-4" />
      ) : null}
      {running ? (
        <RunningPanel operation={running} />
      ) : (
        <div className="export-foot">
          <p className="grow t-sm t-2">
            {format === 'zip' ? t('acao.dicaZip') : t('acao.dicaPasta')}
            {readOnly ? <span className="t-3 block">{t('acao.somenteLeitura')}</span> : null}
          </p>
          <Button
            variant="primary"
            size="lg"
            icon={Package}
            loading={run.isPending}
            onClick={() => {
              if (errors > 0) setConfirmErrors(true);
              else start();
            }}
          >
            {t('acao.exportar')}
          </Button>
        </div>
      )}
      <ConfirmDialog
        open={confirmErrors}
        onOpenChange={setConfirmErrors}
        title={t('erros.titulo', { count: errors })}
        description={t('erros.texto')}
        confirmLabel={t('erros.confirmar')}
        onConfirm={start}
      />
      <Dialog
        open={result !== null}
        onOpenChange={(open) => {
          if (!open) setResult(null);
        }}
      >
        {result ? <ResultDialog format={result.format} result={result.value} /> : null}
      </Dialog>
    </div>
  );
}

function RunningPanel({ operation }: { operation: OperationSnapshot }) {
  const { t } = useTranslation('exportar');
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

function ResultDialog({ format, result }: { format: ExportFormat; result: ExportResult }) {
  const { t } = useTranslation('exportar');
  const reveal = useRevealExport();
  return (
    <DialogContent
      size="sm"
      title={t('resultado.titulo')}
      footer={
        <>
          <DialogClose asChild>
            <Button variant="ghost">{t('resultado.fechar')}</Button>
          </DialogClose>
          <Button
            variant="primary"
            icon={FolderOpen}
            loading={reveal.isPending}
            onClick={() => {
              reveal.mutate(result.path);
            }}
          >
            {format === 'zip' ? t('resultado.mostrarArquivo') : t('resultado.abrirPasta')}
          </Button>
        </>
      }
    >
      <p>
        {t('resultado.resumo', {
          arquivos: t('conteudo.arquivos', { count: result.files.length }),
          tamanho: formatBytes(result.bytes),
        })}{' '}
        <span className="path">{result.path}</span>.
      </p>
      <Alert kind="ok" compact title={t('resultado.conferido')} className="mt-3">
        {t('resultado.conferidoTexto')}
      </Alert>
      {reveal.isError ? <ErrorPanel compact error={reveal.error} className="mt-3" /> : null}
    </DialogContent>
  );
}
