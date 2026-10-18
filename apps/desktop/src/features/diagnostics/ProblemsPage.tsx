/**
 * Seção Problemas (SPEC T14 "Seção Problemas"; protótipo `problemas`; descrição "Saúde do pack,
 * problemas e travamentos"): "Verificar agora" e os blocos registrados em `problems-blocks.ts`
 * (os achados do pré-teste e os Travamentos da D-06; a saúde do pack chega com a D-08).
 *
 * O pack nunca verificado, ou que mudou desde a última verificação (`stale`), é verificado
 * sozinho ao abrir a seção: depois de uma correção, a lista se atualiza sem clique.
 */
import { RefreshCw } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { PageHead } from '../../app/layout/PageHead';
import { ErrorPanel } from '../../components/common/ErrorPanel';
import { LoadingState } from '../../components/common/LoadingState';
import { Button } from '../../components/ui/button';
import type { PackId } from '../../lib/ipc/bindings';
import { useDiagnosticsReport, useRunCheck } from './api';
import { orderedProblemsBlocks } from './problems-blocks';
import { countBySeverity, formatAgo } from './text';
import './diagnostics.css';

export function ProblemsPage({ packId }: { packId: PackId }) {
  const { t } = useTranslation('diagnostico');
  const report = useDiagnosticsReport(packId);
  const check = useRunCheck(packId);
  const autoChecked = useRef<string | null>(null);

  // Verifica sozinho uma vez por relatório velho (ou ausente).
  const data = report.data;
  const needsCheck = report.isSuccess && (!data || data.stale);
  const reportKey = data ? `${String(data.checkedAtMs)}:${data.packHash ?? ''}` : 'nunca';
  const { mutate, isPending } = check;
  useEffect(() => {
    if (needsCheck && !isPending && autoChecked.current !== reportKey) {
      autoChecked.current = reportKey;
      mutate(undefined);
    }
  }, [needsCheck, isPending, reportKey, mutate]);

  // O título da página fica também enquanto lê e quando falha: é nele que o foco cai ao abrir a
  // seção (HANDOFF §7).
  if (report.isPending || report.isError) {
    return (
      <div className="stack diag-page">
        <PageHead title={t('secao.titulo')} />
        {report.isPending ? (
          <LoadingState label={t('secao.verificando')} />
        ) : (
          <ErrorPanel
            title={t('secao.erroLer')}
            error={report.error}
            onRetry={() => {
              void report.refetch();
            }}
          />
        )}
      </div>
    );
  }

  const counts = data ? countBySeverity(data.findings) : null;
  const total = counts ? counts.error + counts.warning + counts.info : 0;
  let sub: string;
  if (check.isPending) {
    sub = t('secao.verificando');
  } else if (!data) {
    sub = t('secao.nunca');
  } else {
    const quando = formatAgo(data.checkedAtMs);
    sub = `${
      total > 0 ? t('secao.resumo', { count: total, quando }) : t('secao.resumoVazio', { quando })
    }. ${t('secao.tambem')}`;
  }

  return (
    <div className="stack diag-page">
      <PageHead
        title={t('secao.titulo')}
        sub={sub}
        actions={
          <Button
            icon={RefreshCw}
            loading={check.isPending}
            onClick={() => {
              check.mutate(undefined);
            }}
          >
            {check.isPending ? t('secao.verificando') : t('secao.verificarAgora')}
          </Button>
        }
      />
      {check.isError ? (
        <ErrorPanel
          title={t('secao.erroVerificar')}
          error={check.error}
          compact
          onRetry={() => {
            check.mutate(undefined);
          }}
        />
      ) : null}
      {orderedProblemsBlocks().map((block) => (
        <block.component key={block.id} packId={packId} report={data} checking={check.isPending} />
      ))}
    </div>
  );
}
