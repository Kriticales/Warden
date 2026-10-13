/**
 * Marca de problema na lista de Mods (SPEC T14: "Itens com problema também ganham a marca na
 * lista de Mods"; T06): o pior problema do item no último relatório (erro ou aviso, com ícone e
 * palavra), com link para a seção Problemas. Informações não marcam o item.
 */
import { Link, useParams } from '@tanstack/react-router';
import { CircleX, TriangleAlert } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { Icon } from '../../../components/ui/icon';
import type { PackId, ReportFinding } from '../../../lib/ipc/bindings';
import { useDiagnosticsReport } from '../api';

export interface ItemProblem {
  severity: 'error' | 'warning';
  count: number;
}

/** O pior problema de cada item (pelo caminho no pack). */
export function problemsByPath(findings: readonly ReportFinding[]): Map<string, ItemProblem> {
  const byPath = new Map<string, ItemProblem>();
  for (const entry of findings) {
    const severity = entry.finding.severity;
    if (entry.ignored || severity === 'info') continue;
    for (const item of entry.items) {
      if (!item.path) continue;
      const current = byPath.get(item.path);
      byPath.set(item.path, {
        severity: current?.severity === 'error' || severity === 'error' ? 'error' : 'warning',
        count: (current?.count ?? 0) + 1,
      });
    }
  }
  return byPath;
}

function ProblemBadge({ packId, problem }: { packId: PackId; problem: ItemProblem }) {
  const { t } = useTranslation('diagnostico');
  const kind = problem.severity === 'error' ? 'danger' : 'warn';
  return (
    <Link
      to="/packs/$packId/problemas"
      params={{ packId }}
      className={`badge badge--${kind} badge--link`}
      title={t('marca.dica', { count: problem.count })}
      aria-label={`${t(`marca.${problem.severity}`)}: ${t('marca.dica', { count: problem.count })}`}
    >
      <Icon icon={problem.severity === 'error' ? CircleX : TriangleAlert} />
      {t(`marca.${problem.severity}`)}
    </Link>
  );
}

function Mark({ packId, path }: { packId: PackId; path: string }) {
  const report = useDiagnosticsReport(packId);
  const problem = report.data ? problemsByPath(report.data.findings).get(path) : undefined;
  return problem ? <ProblemBadge packId={packId} problem={problem} /> : null;
}

/** A marca do item, na tela de um pack. */
export function ProblemMark({ path }: { path: string }) {
  const params = useParams({ strict: false });
  return params.packId ? <Mark packId={params.packId} path={path} /> : null;
}
