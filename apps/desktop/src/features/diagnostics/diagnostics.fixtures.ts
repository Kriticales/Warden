/**
 * Dados dos testes de componente do diagnóstico (D-03): achados como o backend manda (já com
 * os itens ligados ao pack), relatórios e análises de travamento.
 */
import type {
  CrashDiagnosis,
  DiagnosticsReport,
  Evidence,
  Finding,
  ReportFinding,
  ReportItem,
} from '../../lib/ipc/bindings';

export function makeEntry(
  finding: Partial<Finding> & Pick<Finding, 'rule'>,
  items: ReportItem[] = [],
  overrides: Partial<ReportFinding> = {},
): ReportFinding {
  const evidence: Evidence[] = finding.evidence ?? [
    { kind: 'packFile', path: 'pack.toml', excerpt: 'trecho' },
  ];
  return {
    id: `${finding.rule}|${items.map((item) => item.path ?? item.name).join('|')}`,
    finding: {
      severity: 'error',
      titleKey: `diagnostics.pretest.${finding.rule.toLowerCase()}`,
      params: {},
      items: items.map((item) =>
        item.path ? { kind: 'metafile', path: item.path } : { kind: 'modId', id: item.name },
      ),
      fixes: [],
      ...finding,
      evidence,
    },
    items,
    pattern: null,
    labels: {},
    ignored: false,
    ...overrides,
  };
}

const MEU_MOD: ReportItem = { path: 'mods/meu-mod.jar', name: 'Meu Mod' };
const SODIUM: ReportItem = { path: 'mods/sodium.pw.toml', name: 'Sodium' };

/** Erro: Meu Mod precisa do Sodium (evidência do `fabric.mod.json`). */
export const MISSING_DEP = makeEntry(
  {
    rule: 'E_MISSING_DEP',
    severity: 'error',
    evidence: [
      {
        kind: 'jarMetadata',
        item: { kind: 'metafile', path: 'mods/meu-mod.jar' },
        fileInJar: 'fabric.mod.json',
        excerpt: 'sodium: Any',
      },
    ],
    fixes: [{ kind: 'addDependency', modId: 'sodium', versionRange: 'Any' }],
  },
  [MEU_MOD],
  { labels: { sodium: 'Sodium' } },
);

/** Aviso: Sodium nos dois lados, mas só funciona no cliente (Modrinth). */
export const WRONG_SIDE = makeEntry(
  {
    rule: 'W_SIDE',
    severity: 'warning',
    evidence: [
      {
        kind: 'api',
        source: 'modrinth',
        url: 'https://api.modrinth.com/v2/version/SMxNOGZ6',
        field: 'server_side',
      },
    ],
    fixes: [
      { kind: 'setSide', item: { kind: 'metafile', path: SODIUM.path ?? '' }, side: 'client' },
    ],
  },
  [SODIUM],
);

/** Informação: versão beta. */
export const PRERELEASE = makeEntry({ rule: 'I_PRERELEASE', severity: 'info' }, [SODIUM]);

export function makeReport(overrides: Partial<DiagnosticsReport> = {}): DiagnosticsReport {
  return {
    pass: 'complete',
    checkedAtMs: Date.now() - 3 * 60_000,
    itemCount: 2,
    checkedMods: true,
    findings: [MISSING_DEP, WRONG_SIDE, PRERELEASE],
    packHash: 'sha256:abc',
    stale: false,
    ...overrides,
  };
}

/** Travamento com causa: falta o Balm, exigido pelo Waystones (log do Forge). */
export const CRASH_CAUSE = makeEntry(
  {
    rule: 'E_MISSING_DEP',
    severity: 'error',
    titleKey: 'diagnostico.achados.dependenciaFaltando',
    params: { mod_name: 'Waystones', mod: 'waystones', dependency: 'balm', range: '[7.3.0,)' },
    evidence: [
      {
        kind: 'log',
        file: 'logs/latest.log',
        line: 1204,
        excerpt: "Mod ID: 'balm', Requested by: 'waystones', Expected range: '[7.3.0,)'",
      },
    ],
    fixes: [{ kind: 'addDependency', modId: 'balm', versionRange: '[7.3.0,)' }],
  },
  [{ path: 'mods/waystones.pw.toml', name: 'Waystones' }],
  { pattern: 'forge.mandatory_dependency_missing' },
);

export function makeDiagnosis(overrides: Partial<CrashDiagnosis> = {}): CrashDiagnosis {
  return {
    sessionId: '2026-10-07_14-20-00',
    foundCause: true,
    findings: [CRASH_CAUSE],
    sources: [
      { name: 'output.log', kind: 'output', lines: 120 },
      { name: 'logs/latest.log', kind: 'latestLog', lines: 1300 },
    ],
    ...overrides,
  };
}
