/**
 * "Memória no último teste" na seção Problemas (SPEC T13 "Achados automáticos" e T14; L-11): os
 * achados de memória (`W_LOW_HEAP`, `I_HEAP_OVERSIZED`) do teste mais novo do
 * pack, pelo mecanismo da D-03 (registro `problems-blocks.ts`, `FindingRow` e as correções de
 * sempre: "Aumentar a memória do teste" abre Ajustes do teste). Valem até o próximo teste: o
 * fim de um teste relê a lista de sessões, e o bloco acompanha.
 */
import { MemoryStick } from 'lucide-react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

import type { Finding, ReportFinding, TestSessionSummary } from '../../../../lib/ipc/bindings';
import { Icon } from '../../../../components/ui/icon';
import { FindingRow } from '../../../diagnostics/components/FindingRow';
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
    <section className="panel panel--parts" aria-labelledby={titleId}>
      <div className="panel__bar">
        <h2 className="panel__bar-title" id={titleId}>
          <Icon icon={MemoryStick} />
          {t('problemas.titulo')}
        </h2>
        <span className="t-xs t-3">
          {t('problemas.sub', { quando: formatWhen(latest.startedAtMs) })}
        </span>
      </div>
      <ul className="rowlist">
        {findings.map((finding) => {
          const entry = memoryEntry(latest, finding);
          return (
            <FindingRow
              key={entry.id}
              packId={packId}
              entry={entry}
              headingLevel={3}
              showSeverity
              onFix={fixes.run}
              busyFix={fixes.busy}
            />
          );
        })}
      </ul>
      {fixes.dialogs}
    </section>
  );
}
