/**
 * Saúde do pack na coluna da direita da seção Problemas (SPEC T14 item 1; ROADMAP D-08;
 * ADR-0034; esboço aprovado do bug-01, "Separadas"): a nota de 0 a 100 em fonte pixel, a faixa,
 * a barra de 20 blocos, "O que tirou pontos" com os links e o texto fixo, calculada
 * com o último relatório, os testes gravados e o último "Verificar atualizações". Bloco
 * registrado em `problems-blocks.ts` (ordem 100, coluna `side`).
 *
 * A nota acompanha o relatório e os travamentos que a tela já lê (veja `api.ts`): "Verificar
 * agora", "Ignorar neste pack" e o fim de um teste a recalculam sem clique, e a mesma nota vai
 * para a linha do pack em Meus packs.
 */
import { ShieldCheck } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import { ErrorPanel } from '../../../components/common/ErrorPanel';
import { LoadingState } from '../../../components/common/LoadingState';
import { Icon } from '../../../components/ui/icon';
import type { CrashGroupView, ReportFinding } from '../../../lib/ipc/bindings';
import { useCrashes } from '../crashes/api';
import type { ProblemsBlockProps } from '../problems-blocks';
import { formatAgo } from '../text';
import { usePackHealth } from './api';
import { HealthLosses } from './HealthLosses';
import { HealthMeter } from './HealthMeter';
import './health.css';

export function HealthBlock({ packId, report }: ProblemsBlockProps) {
  const { t } = useTranslation('saude');
  const titleId = useId();
  const crashes = useCrashes(packId);
  const health = usePackHealth(
    packId,
    report,
    crashes.isPending ? undefined : crashes.dataUpdatedAt,
  );

  let content;
  if (health.isError) {
    content = (
      <ErrorPanel
        title={t('erro')}
        error={health.error}
        compact
        onRetry={() => {
          void health.refetch();
        }}
      />
    );
  } else if (report === undefined || health.isPending) {
    content = <LoadingState label={t('calculando')} inline />;
  } else if (health.data === null) {
    content = <HealthMeter value={null} size="lg" note={t('semDados')} />;
  } else {
    const data = health.data;
    const findings = new Map<string, ReportFinding>(
      (report?.findings ?? []).map((entry) => [entry.id, entry]),
    );
    const groups = new Map<string, CrashGroupView>(
      (crashes.data?.groups ?? []).map((group) => [group.signature.hash, group]),
    );
    const { otimo, bom, atencao } = data.bands;
    content = (
      <>
        <HealthMeter value={data} size="lg" note={null} />
        <div className="health-panel__losses">
          <h3 className="panel__subtitle">{t('tirouPontos')}</h3>
          {data.lines.length > 0 ? (
            <HealthLosses
              lines={data.lines}
              context={{
                packId,
                findings,
                crashes: groups,
                recentSessions: data.recentSessions,
              }}
            />
          ) : (
            <p className="t-sm t-3">{t('nadaTirou')}</p>
          )}
        </div>
        <p className="t-xs t-3">{t('aviso')}</p>
        <details className="disclosure">
          <summary>{t('comoCalcula')}</summary>
          <p className="t-xs t-3 health-panel__bands">
            {t('faixas', {
              otimo,
              bom,
              bomAte: otimo - 1,
              atencao,
              atencaoAte: bom - 1,
            })}
          </p>
        </details>
      </>
    );
  }

  return (
    <section className="panel panel--parts health-panel" aria-labelledby={titleId}>
      <div className="panel__bar">
        <h2 className="panel__bar-title" id={titleId}>
          <Icon icon={ShieldCheck} />
          {t('titulo')}
        </h2>
        <span className="tag tag--plain">
          {report ? t('verificado', { quando: formatAgo(report.checkedAtMs) }) : t('naoVerificado')}
        </span>
      </div>
      <div className="panel__body">{content}</div>
    </section>
  );
}
