/**
 * Dados de exemplo de Salvar versão e do Histórico para os testes de componente (reflexo dos
 * tipos gerados em `bindings.ts`).
 */
import type {
  ChangeSet,
  HistoryView,
  ItemChange,
  SafetyPoint,
  SavePreview,
  SavedVersion,
} from '../../lib/ipc/bindings';

export function makeItemChange(overrides: Partial<ItemChange> = {}): ItemChange {
  return {
    kind: 'added',
    category: 'mod',
    source: 'modrinth',
    name: 'Sophisticated Backpacks',
    path: 'mods/sophisticated-backpacks.pw.toml',
    projectId: 'backpacks',
    side: 'both',
    old: null,
    new: {
      reference: { kind: 'modrinth', projectId: 'backpacks', versionId: 'v1' },
      filename: 'sophisticatedbackpacks-3.20.17.jar',
      label: '3.20.17',
    },
    ...overrides,
  };
}

export function makeUpdated(name: string, from: string, to: string): ItemChange {
  return makeItemChange({
    kind: 'updated',
    name,
    path: `mods/${name.toLowerCase().replaceAll(' ', '-')}.pw.toml`,
    projectId: name,
    old: {
      reference: { kind: 'modrinth', projectId: name, versionId: `${name}-${from}` },
      filename: `${name}-${from}.jar`,
      label: from,
    },
    new: {
      reference: { kind: 'modrinth', projectId: name, versionId: `${name}-${to}` },
      filename: `${name}-${to}.jar`,
      label: to,
    },
  });
}

export function makeChangeSet(overrides: Partial<ChangeSet> = {}): ChangeSet {
  const items = overrides.items ?? [];
  const configs = overrides.configs ?? [];
  return {
    minecraft: null,
    loaders: [],
    items,
    configs,
    files: [
      ...items.map((item) => ({ path: item.path, kind: 'modified' as const })),
      ...configs.map((path) => ({ path, kind: 'modified' as const })),
    ],
    ...overrides,
  };
}

export function makeSavedVersion(
  version: string,
  overrides: Partial<SavedVersion> = {},
): SavedVersion {
  return {
    version,
    commit: 'a'.repeat(40),
    date: '2026-09-28T15:02:00-03:00',
    message: `### Mods atualizados\n- Create 0.5.1.i → 0.5.1.j\n`,
    isFinal: false,
    isPublished: false,
    reachable: true,
    ...overrides,
  };
}

export function makeHistory(overrides: Partial<HistoryView> = {}): HistoryView {
  return {
    unsaved: makeChangeSet(),
    lastVersion: null,
    versions: [],
    ...overrides,
  };
}

export function makeSavePreview(overrides: Partial<SavePreview> = {}): SavePreview {
  const changes = makeChangeSet({
    items: [
      makeItemChange(),
      makeItemChange({ name: 'Sophisticated Core', path: 'mods/sophisticated-core.pw.toml' }),
      makeUpdated('Just Enough Items', '15.20.0.106', '15.20.0.112'),
    ],
    configs: ['config/create-common.toml'],
  });
  return {
    changes,
    suggestion: {
      version: '1.5.0',
      bump: 'minor',
      reasons: [{ kind: 'addedItems', count: 2 }],
    },
    body: '### Mods adicionados\n- Sophisticated Backpacks 3.20.17\n- Sophisticated Core 3.20.17\n\n### Mods atualizados\n- Just Enough Items 15.20.0.106 → 15.20.0.112\n\n### Configs alteradas\n- config/create-common.toml\n',
    lastVersion: '1.4.2',
    highestVersion: '1.4.2',
    worldWarning: false,
    ...overrides,
  };
}

export function makeSafetyPoint(overrides: Partial<SafetyPoint> = {}): SafetyPoint {
  return {
    name: '20260928T180200Z-voltar-1.2.0',
    reason: 'antes de voltar para 1.2.0',
    createdAt: '2026-09-28T15:02:00-03:00',
    commit: 'b'.repeat(40),
    ...overrides,
  };
}
