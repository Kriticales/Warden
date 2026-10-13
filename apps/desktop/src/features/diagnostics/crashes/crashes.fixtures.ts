/**
 * Dados dos testes de componente dos Travamentos (D-06): grupos, a lista e as vezes de um grupo
 * como o backend manda.
 */
import type {
  CrashDetail,
  CrashGroupView,
  CrashList,
  CrashSignature,
} from '../../../lib/ipc/bindings';
import { makeDiagnosis } from '../diagnostics.fixtures';

const NOW = Date.now();
const MIB = 1024 * 1024;

export function makeSignature(overrides: Partial<CrashSignature> = {}): CrashSignature {
  return {
    version: 2,
    hash: '0a1b2c3d4e5f6071',
    rule: 'crash_report.ticking_entity',
    exception: 'java.lang.NullPointerException',
    frame: 'com.example.meumod.EntityTicker.tick',
    mods: ['jar:meumod'],
    subject: [],
    ...overrides,
  };
}

export function makeGroup(overrides: Partial<CrashGroupView> = {}): CrashGroupView {
  return {
    signature: makeSignature(),
    count: 2,
    firstAtMs: NOW - 3 * 60 * 60_000,
    lastAtMs: NOW - 60 * 60_000,
    versions: [
      { version: '1.4.1', unsaved: false },
      { version: '1.4.2', unsaved: false },
    ],
    origins: [{ origin: 'test', count: 2 }],
    situation: { kind: 'causeUnknown' },
    latestSessionId: '2026-10-07_14-20-00',
    diagnosis: null,
    links: { culpritSearches: [], conversations: [] },
    ...overrides,
  };
}

/** Falta o Balm: uma vez, com a causa encontrada. */
export const BALM_GROUP = makeGroup({
  signature: makeSignature({
    hash: 'ffeeddccbbaa9988',
    rule: 'forge.mandatory_dependency_missing',
    exception: null,
    frame: null,
    mods: ['mod:balm'],
  }),
  count: 1,
  versions: [{ version: '1.4.2', unsaved: true }],
  origins: [{ origin: 'test', count: 1 }],
  situation: { kind: 'causeFound' },
  latestSessionId: '2026-10-07_15-00-00',
  diagnosis: makeDiagnosis({ sessionId: '2026-10-07_15-00-00' }),
});

export function makeCrashList(overrides: Partial<CrashList> = {}): CrashList {
  return {
    groups: [makeGroup()],
    retention: { budgetBytes: 500 * MIB, nearLimit: false, overLimit: false, prunable: [] },
    ...overrides,
  };
}

export function makeDetail(group: CrashGroupView = makeGroup()): CrashDetail {
  return {
    group,
    occurrences: [
      {
        sessionId: '2026-10-07_14-20-00',
        startedAtMs: group.lastAtMs,
        version: { version: '1.4.2', unsaved: false },
        exitCode: -1,
        origin: 'test',
      },
      {
        sessionId: '2026-10-07_12-20-00',
        startedAtMs: group.firstAtMs,
        version: { version: '1.4.1', unsaved: false },
        exitCode: -1,
        origin: 'test',
      },
    ],
  };
}
