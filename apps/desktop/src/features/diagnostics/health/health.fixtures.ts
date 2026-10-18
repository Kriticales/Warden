/**
 * Notas de saúde como o backend manda (D-08), para os testes de componente. `CA_T14_07` é o pack
 * do critério: 2 erros, 1 aviso, o último teste travado, 2 causas de travamento, 1 sobreposição
 * alta de mixin e 2 mods da CurseForge bloqueados (nota 33, Crítico).
 */
import type { HealthLine, HealthReason, HealthScore } from '../../../lib/ipc/bindings';
import { BALM_GROUP, makeGroup } from '../crashes/crashes.fixtures';
import { MISSING_DEP, WRONG_SIDE } from '../diagnostics.fixtures';

export function finding(
  findingId: string,
  rule: string,
  severity: 'error' | 'warning' | 'info',
  points: number,
  item: string | null,
  risk: 'high' | 'medium' | null = null,
): HealthReason {
  return { kind: 'finding', findingId, rule, severity, risk, item, points };
}

export function line(
  category: HealthLine['category'],
  reasons: HealthReason[],
  overrides: Partial<HealthLine> = {},
): HealthLine {
  const points = reasons.reduce((sum, reason) => sum + reason.points, 0);
  return { category, points, rawPoints: points, ignored: false, reasons, ...overrides };
}

export function makeHealth(overrides: Partial<HealthScore> = {}): HealthScore {
  return {
    score: 100,
    band: 'great',
    lines: [],
    bands: { otimo: 90, bom: 75, atencao: 50 },
    recentSessions: 10,
    ...overrides,
  };
}

export const CA_T14_07 = makeHealth({
  score: 33,
  band: 'critical',
  lines: [
    line('problems', [
      finding(MISSING_DEP.id, 'E_MISSING_DEP', 'error', 15, 'mods/meu-mod.jar'),
      finding('E_EXCLUSIVE|mods/a.pw.toml', 'E_EXCLUSIVE', 'error', 15, null),
      finding(WRONG_SIDE.id, 'W_SIDE', 'warning', 3, 'mods/sodium.pw.toml'),
    ]),
    line('lastTest', [
      { kind: 'lastTest', state: 'crashed', session: '2026-10-07_15-00-00', points: 20 },
    ]),
    line('crashes', [
      {
        kind: 'crash',
        signature: BALM_GROUP.signature.hash,
        session: '2026-10-07_15-00-00',
        points: 3,
      },
      {
        kind: 'crash',
        signature: makeGroup().signature.hash,
        session: '2026-10-07_14-20-00',
        points: 3,
      },
    ]),
    line('mixins', [
      finding('W_MIXIN_OVERLAP|mods/c.pw.toml', 'W_MIXIN_OVERLAP', 'warning', 4, null, 'high'),
    ]),
    line('download', [
      finding(
        'W_DOWNLOAD_BLOCKED|mods/e.pw.toml',
        'W_DOWNLOAD_BLOCKED',
        'warning',
        2,
        'mods/e.pw.toml',
      ),
      finding(
        'W_DOWNLOAD_BLOCKED|mods/f.pw.toml',
        'W_DOWNLOAD_BLOCKED',
        'warning',
        2,
        'mods/f.pw.toml',
      ),
    ]),
  ],
});
