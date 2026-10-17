/**
 * "Memória no último teste" na seção Problemas (SPEC T13 "Achados automáticos" e T14; L-11): os
 * achados de memória (`W_LOW_HEAP`, `I_HEAP_OVERSIZED`) do teste mais novo do
 * pack, pelo mecanismo da D-03 (registro `problems-blocks.ts`, `FindingCard` e as correções de
 * sempre: "Aumentar a memória do teste" abre Ajustes do teste). Valem até o próximo teste: o
 * fim de um teste relê a lista de sessões, e o bloco acompanha.
 */
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import type { Finding, ReportFinding, TestSessionSummary } from '../../../../lib/ipc/bindings';
import { FindingCard } from '../../../diagnostics/components/FindingCard';
import { useFixes } from '../../../diagnostics/fixes';
import type { ProblemsBlockProps } from '../../../diagnostics/problems-blocks';
import { useTestSessions } from '../../api';
import { formatWhen } from '../../text';

/** Um achado de memória como a seção Problemas mostra (sem itens do pack). */
export function memoryEntry(session: TestSessionSummary, finding: Finding): ReportFinding {
  return {
    id: `${finding.rule}|${session.id}`,
    finding,
    items: [],
    pattern: null,
    labels: {},
    ignored: false,
  };
}

export function HeapProblemsBlock({ packId }: ProblemsBlockProps) {
  const { t } = useTranslation('memoria');
  const titleId = useId();
  const sessions = useTestSessions(packId);
  const fixes = useFixes(packId);
  const latest = sessions.data?.[0];
  const findings = latest?.perf?.findings ?? [];
  if (!latest || findings.length === 0) {
    return null;
  }
  return (
    <section className="panel stack-3" aria-labelledby={titleId}>
      <div>
        <h2 className="panel__title" id={titleId}>
          {t('problemas.titulo')}
        </h2>
        <p className="t-xs t-3">{t('problemas.sub', { quando: formatWhen(latest.startedAtMs) })}</p>
      </div>
      {findings.map((finding) => {
        const entry = memoryEntry(latest, finding);
        return (
          <FindingCard
            key={entry.id}
            packId={packId}
            entry={entry}
            onFix={fixes.run}
            busyFix={fixes.busy}
          />
        );
      })}
      {fixes.dialogs}
    </section>
  );
}
