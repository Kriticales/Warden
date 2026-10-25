/**
 * Seção Problemas (SPEC T14 "Seção Problemas"; esboço aprovado do bug-01, "Separadas"):
 * cabeçalho com a descrição "Saúde do pack, problemas e travamentos" e "Verificar agora", e os
 * blocos registrados em `problems-blocks.ts` em duas colunas: na larga, os achados do pré-teste
 * e os Travamentos (D-03, D-06); na da direita, a saúde do pack (D-08) e o desempenho do último
 * teste.
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
import { orderedProblemsBlocks, type ProblemsColumn } from './problems-blocks';
import './diagnostics.css';

/** As duas colunas do esboço: a larga e a da direita. */
const COLUMNS = ['main', 'side'] as const satisfies readonly ProblemsColumn[];

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

  if (report.isPending) {
    return <LoadingState label={t('secao.verificando')} />;
  }
  if (report.isError) {
    // O título da página fica no erro: é nele que o foco cai ao abrir a seção (HANDOFF §7).
    return (
      <div className="stack diag-page">
        <PageHead title={t('secao.titulo')} />
        <ErrorPanel
          title={t('secao.erroLer')}
          error={report.error}
          onRetry={() => {
            void report.refetch();
          }}
        />
      </div>
    );
  }

  return (
    <div className="stack diag-page">
      <PageHead
        title={t('secao.titulo')}
        sub={t('secao.descricao')}
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
      <div className="diag-cols">
        {COLUMNS.map((column) => (
          <div key={column} className="diag-cols__col">
            {orderedProblemsBlocks(undefined, column).map((block) => (
              <block.component
                key={block.id}
                packId={packId}
                report={data}
                checking={check.isPending}
              />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
