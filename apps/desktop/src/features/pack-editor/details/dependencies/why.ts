/** "Por que está no pack": a resposta do grafo em linhas prontas para virar texto (D-07). */
import type { WhyReport } from '../../../../lib/ipc/bindings';

export type WhyLine =
  | { key: string; kind: 'user'; at: number | null }
  | { key: string; kind: 'noDependents' }
  | { key: string; kind: 'chain'; names: string[] }
  | { key: string; kind: 'unused'; optionalUsers: string[] }
  | { key: string; kind: 'cycle'; names: string[] };

/** Uma linha por cadeia (já da mais curta para a mais longa) e, se houver, a do ciclo. */
export function describeWhy(report: WhyReport): WhyLine[] {
  const lines: WhyLine[] = [];
  const { why } = report;
  switch (why.kind) {
    case 'userAdded': {
      const at = why.at === null ? Number.NaN : Date.parse(why.at);
      lines.push({ key: 'user', kind: 'user', at: Number.isFinite(at) ? at : null });
      break;
    }
    case 'noDependents':
      lines.push({ key: 'noDependents', kind: 'noDependents' });
      break;
    case 'requiredBy':
      for (const chain of why.chains) {
        // A cadeia começa no próprio item: o que importa é quem vem depois.
        const names = chain.slice(1).map((link) => link.item.name);
        lines.push({ key: `chain:${names.join('>')}`, kind: 'chain', names });
      }
      break;
    case 'unused':
      lines.push({
        key: 'unused',
        kind: 'unused',
        optionalUsers: why.optionalUsers.map((user) => user.name),
      });
      break;
  }
  if (report.cycle.length > 0) {
    lines.push({ key: 'cycle', kind: 'cycle', names: report.cycle.map((node) => node.name) });
  }
  return lines;
}
