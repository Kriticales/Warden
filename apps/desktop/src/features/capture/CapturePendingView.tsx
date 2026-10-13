/**
 * Testar de novo com mudanças do teste anterior não revisadas (SPEC T15 "Ações"): o teste não
 * começa (`app.TEST_CAPTURE_PENDING`, antes de qualquer etapa) e a tela pergunta "Revisar
 * agora", "Descartar e testar" ou "Cancelar". Mostrada pela tela do teste no lugar do motivo de
 * o teste não ter começado.
 */
import { useNavigate, useRouter } from '@tanstack/react-router';
import { ListChecks, Play } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../app/layout/PageHead';
import { Button } from '../../components/ui/button';
import { showToast } from '../../components/ui/toast';
import type { AppError, PackId, TestRequest } from '../../lib/ipc/bindings';
import { appErrorMessage, toAppError } from '../../lib/ipc/errors';
import { useTestStore } from '../test/store';
import { useCaptureDiscard } from './api';
import './capture.css';

export interface CapturePendingViewProps {
  packId: PackId;
  error: AppError;
  request: TestRequest;
  /** Começa o teste de novo. */
  onRetry: (request: TestRequest) => void;
}

export function CapturePendingView({ packId, error, request, onRetry }: CapturePendingViewProps) {
  const { t } = useTranslation('captura');
  const navigate = useNavigate();
  const router = useRouter();
  const clearRun = useTestStore((store) => store.clearRun);
  const discard = useCaptureDiscard(packId);
  const sessionId = error.params.sessionId ?? '';

  return (
    <>
      <PageHead title={t('pendente.titulo')} />
      <section className="panel panel--strong stack-3" aria-labelledby="capture-pending-title">
        <h2 className="panel__title sr-only" id="capture-pending-title">
          {t('pendente.titulo')}
        </h2>
        <p>{t('pendente.texto')}</p>
        <div className="btn-row">
          <Button
            variant="primary"
            icon={ListChecks}
            onClick={() => {
              clearRun(packId);
              void navigate({
                to: '/packs/$packId/teste',
                params: { packId },
                search: sessionId ? { sessao: sessionId } : {},
              });
            }}
          >
            {t('pendente.revisar')}
          </Button>
          <Button
            icon={Play}
            loading={discard.isPending}
            onClick={() => {
              discard.mutate(sessionId, {
                onSuccess: () => {
                  onRetry(request);
                },
                onError: (cause) => {
                  showToast({ kind: 'danger', title: appErrorMessage(toAppError(cause)) });
                },
              });
            }}
          >
            {t('pendente.descartarTestar')}
          </Button>
          <Button
            variant="ghost"
            onClick={() => {
              clearRun(packId);
              router.history.back();
            }}
          >
            {t('pendente.cancelar')}
          </Button>
        </div>
      </section>
    </>
  );
}
