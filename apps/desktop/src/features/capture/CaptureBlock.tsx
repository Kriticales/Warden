/**
 * "O que mudou durante o teste" no resultado do teste (SPEC T15: "no próprio resultado do teste,
 * não numa tela separada"). Registrado em `test/result-blocks.ts` (ordem 100, acima de "Por que
 * travou").
 *
 * Só aparece no resultado do teste que gerou a captura: a revisão (mudanças esperando decisão),
 * "nada mudou" ou o resumo depois de trazer ou descartar. Sessões mais antigas não mostram nada.
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Alert } from '../../components/ui/alert';
import type { ResultBlockProps } from '../test/result-blocks';
import { useCapture } from './api';
import { AppliedDialog } from './components/AppliedDialog';
import { CaptureReview } from './components/CaptureReview';
import './capture.css';

export function CaptureBlock({ packId, session }: ResultBlockProps) {
  const { t } = useTranslation('captura');
  const capture = useCapture(packId);
  const [applied, setApplied] = useState<number | null>(null);
  const dialog = (
    <AppliedDialog
      packId={packId}
      count={applied}
      onClose={() => {
        setApplied(null);
      }}
    />
  );

  if (capture.isPending) {
    return <LoadingState inline label={t('carregando')} />;
  }
  if (capture.isError) {
    return <ErrorPanel compact error={capture.error} />;
  }
  const view = capture.data;
  if (view?.sessionId !== session.id) {
    return dialog;
  }
  let body;
  if (view.status.kind === 'applied') {
    body = (
      <Alert kind="ok" compact role="status">
        {t('trazidas', { count: view.status.count })}
      </Alert>
    );
  } else if (view.status.kind === 'discarded') {
    body = (
      <Alert kind="neutral" compact role="status">
        {t('descartadas')}
      </Alert>
    );
  } else if (view.files.length === 0) {
    body = (
      <Alert kind="neutral" compact role="status">
        {t('nada')}
        {view.reformatted > 0 ? ` ${t('reformatados', { count: view.reformatted })}` : null}
      </Alert>
    );
  } else {
    body = (
      <CaptureReview key={view.sessionId} packId={packId} view={view} onApplied={setApplied} />
    );
  }
  return (
    <>
      {body}
      {dialog}
    </>
  );
}
