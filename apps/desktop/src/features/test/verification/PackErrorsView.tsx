/**
 * Os erros do pré-teste que pararam o Testar (SPEC T13 passos 1 e 4; protótipo
 * `teste-checagem`): um diálogo com os erros e a correção principal de cada um, **Testar mesmo
 * assim** e **Cancelar e ver Problemas**. Fechado o diálogo, a tela do teste diz em que etapa o
 * teste parou e deixa reabrir a lista, testar de novo ou testar mesmo assim.
 *
 * Os erros vêm do último relatório do pack (`diagnostics_report_get`), o mesmo que o teste
 * acabou de gravar. As correções passam pelos fluxos normais (`features/diagnostics/fixes`).
 */
import { useNavigate } from '@tanstack/react-router';
import { ListChecks, Play, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../../app/layout/PageHead';
import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Alert } from '../../../components/ui/alert';
import { Button } from '../../../components/ui/button';
import { Dialog, DialogContent } from '../../../components/ui/dialog';
import type { AppError, PackId, SuggestedFix, TestRequest } from '../../../lib/ipc/bindings';
import { useDiagnosticsReport } from '../../diagnostics/api';
import { FindingCard } from '../../diagnostics/components/FindingCard';
import { useFixes } from '../../diagnostics/fixes';
import { countBySeverity } from '../../diagnostics/text';
import '../../diagnostics/diagnostics.css';

export interface PackErrorsViewProps {
  packId: PackId;
  /** O erro `app.TEST_PACK_HAS_ERRORS` (`count`, `step`). */
  error: AppError;
  /** O pedido do teste que parou. */
  request: TestRequest;
  /** Começa o teste de novo com o pedido dado. */
  onRetry: (request: TestRequest) => void;
}

export function PackErrorsView({ packId, error, request, onRetry }: PackErrorsViewProps) {
  const { t } = useTranslation('diagnostico');
  const navigate = useNavigate();
  const report = useDiagnosticsReport(packId);
  const [open, setOpen] = useState(true);
  const [fixed, setFixed] = useState(false);
  const fixes = useFixes(packId, () => {
    setFixed(true);
  });

  const stopped =
    error.params.step === 'final' ? t('dialogo.paradoFinal') : t('dialogo.paradoCheck');
  const stoppedCount = Number(error.params.count ?? '0');
  const errors =
    report.data?.findings.filter((entry) => !entry.ignored && entry.finding.severity === 'error') ??
    [];
  const counts = report.data ? countBySeverity(report.data.findings) : null;
  const count = report.data ? errors.length : stoppedCount;

  const testAgain = () => {
    onRetry({ ...request, ignorePackErrors: false });
  };
  const testAnyway = () => {
    onRetry({ ...request, ignorePackErrors: true });
  };
  const runFix = (entry: (typeof errors)[number], fix: SuggestedFix) => {
    // A correção abre o fluxo normal (diálogo de dependências, Remover, Ajustes do teste)
    // ou grava com confirmação: este diálogo fecha para não ficar um sobre o outro.
    setOpen(false);
    fixes.run(entry, fix);
  };

  let body;
  if (report.isPending) {
    body = <LoadingState label={t('dialogo.carregando')} />;
  } else if (report.isError) {
    body = <ErrorPanel error={report.error} compact />;
  } else {
    body = (
      <div className="stack-3">
        <div className="diag-dialog__list">
          {errors.map((entry) => (
            <FindingCard
              key={entry.id}
              packId={packId}
              entry={entry}
              maxFixes={1}
              onFix={runFix}
              busyFix={fixes.busy}
            />
          ))}
        </div>
        {counts && counts.warning > 0 ? (
          <p className="t-sm t-3">{t('dialogo.avisos', { count: counts.warning })}</p>
        ) : null}
      </div>
    );
  }

  return (
    <>
      <PageHead title={stopped} />
      <section className="panel panel--strong stack-3" aria-labelledby="pack-errors-title">
        <h2 className="panel__title" id="pack-errors-title">
          {t('dialogo.titulo', { count })}
        </h2>
        {fixed ? (
          <Alert kind="ok" compact>
            {t('ajuda.corrigido')}
          </Alert>
        ) : (
          <p>{t('dialogo.paradoTexto', { count })}</p>
        )}
        <div className="btn-row">
          <Button variant={fixed ? 'primary' : 'secondary'} icon={Play} onClick={testAgain}>
            {t('dialogo.testarDeNovo')}
          </Button>
          <Button
            icon={ListChecks}
            onClick={() => {
              setOpen(true);
            }}
          >
            {t('dialogo.verErros')}
          </Button>
          <Button variant="ghost" icon={TriangleAlert} onClick={testAnyway}>
            {t('dialogo.testarMesmoAssim')}
          </Button>
        </div>
      </section>
      <Dialog open={open} onOpenChange={setOpen}>
        {open ? (
          <DialogContent
            size="lg"
            title={t('dialogo.titulo', { count })}
            description={t('dialogo.sub')}
            footer={
              <>
                <Button
                  variant="ghost"
                  onClick={() => {
                    setOpen(false);
                    void navigate({ to: '/packs/$packId/problemas', params: { packId } });
                  }}
                >
                  {t('dialogo.cancelar')}
                </Button>
                <Button icon={Play} onClick={testAnyway}>
                  {t('dialogo.testarMesmoAssim')}
                </Button>
              </>
            }
          >
            {body}
          </DialogContent>
        ) : null}
      </Dialog>
      {fixes.dialogs}
    </>
  );
}
