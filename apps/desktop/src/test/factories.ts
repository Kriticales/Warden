/**
 * Fábricas de dados dos testes, tipadas pelos tipos gerados (`bindings.ts`): se o contrato do
 * Rust mudar, os testes deixam de compilar em vez de testar uma forma velha.
 */
import type {
  AppError,
  AppInfo,
  ErrorCode,
  HygieneFinding,
  ImportPreview,
  Loader,
  LoaderVersions,
  MinecraftVersions,
  OperationSnapshot,
  PackId,
  PackRow,
  Progress,
} from '../lib/ipc/bindings';

export function makeAppInfo(overrides: Partial<AppInfo> = {}): AppInfo {
  return {
    version: '0.1.0',
    commit: 'abc123def456',
    platform: 'windows',
    debugBuild: false,
    ...overrides,
  };
}

export function makeAppError(
  code: ErrorCode = { domain: 'app', code: 'INTERNAL' },
  overrides: Partial<Omit<AppError, 'code'>> = {},
): AppError {
  return {
    code,
    params: {},
    detail: null,
    retryable: false,
    operationId: null,
    ...overrides,
  };
}

let sequence = 0;

/** Id de operação no formato ULID (26 caracteres), diferente a cada chamada. */
export function nextOperationId(): string {
  sequence += 1;
  return `01J9ZQ${String(sequence).padStart(20, '0')}`;
}

/** Início padrão das operações dos testes: 04/10/2026, 14:00 no fuso local. */
export const BASE_TIME_MS = new Date(2026, 9, 4, 14, 0, 0).getTime();

export function makeOperation(overrides: Partial<OperationSnapshot> = {}): OperationSnapshot {
  return {
    id: nextOperationId(),
    kind: 'teste.simulado',
    packId: null,
    state: 'running',
    stage: null,
    progress: null,
    cancellable: true,
    startedAtMs: BASE_TIME_MS,
    finishedAtMs: null,
    error: null,
    ...overrides,
  };
}

export function makeProgress(
  current: number,
  total: number | null,
  unit: Progress['unit'] = 'items',
): Progress {
  return { current, total, unit };
}

// ---- Packs (P1-07) ----

let packSequence = 0;

/** Id de pack no formato ULID, diferente a cada chamada. */
export function nextPackId(): PackId {
  packSequence += 1;
  return `01JA00${String(packSequence).padStart(20, '0')}`;
}

export function makePackRow(overrides: Partial<PackRow> = {}): PackRow {
  return {
    id: nextPackId(),
    path: 'C:/Packs/vale-sereno',
    name: 'Vale Sereno',
    minecraft: '1.20.1',
    loader: 'forge',
    loaderVersion: '47.3.0',
    version: '0.1.0',
    modifiedAtMs: BASE_TIME_MS,
    status: 'ready',
    detail: null,
    readOnlyReason: null,
    unsavedFiles: 0,
    lastTest: null,
    ...overrides,
  };
}

/** Manifesto curto da Mojang, na ordem oficial (mais nova primeiro), com um snapshot. */
export function makeMinecraftVersions(
  overrides: Partial<MinecraftVersions> = {},
): MinecraftVersions {
  const release = (id: string, bestEffort = false) =>
    ({ id, kind: 'release', releaseTime: '2024-01-01T00:00:00Z', bestEffort }) as const;
  return {
    versions: [
      {
        id: '26.3-snapshot-1',
        kind: 'snapshot',
        releaseTime: '2026-09-01T00:00:00Z',
        bestEffort: false,
      },
      release('26.3'),
      release('1.21.1'),
      release('1.20.1'),
      release('1.19.2'),
      release('1.12.2'),
      release('1.7.10'),
      release('1.6.4', true),
    ],
    latestRelease: '26.3',
    latestSnapshot: '26.3-snapshot-1',
    freshness: { fetchedAtMs: BASE_TIME_MS, offline: false },
    ...overrides,
  };
}

/** Versões de um loader; `versions` vazio = o loader não existe para a versão. */
export function makeLoaderVersions(
  loader: Loader,
  minecraft: string,
  versions: string[],
  overrides: Partial<LoaderVersions> = {},
): LoaderVersions {
  return {
    loader,
    minecraft,
    versions: versions.map((version, index) => ({
      version,
      maven: `net.example:${loader}:${version}`,
      stable: true,
      recommended: loader === 'forge' && index === 0,
    })),
    preselected: versions[0] ?? null,
    freshness: { fetchedAtMs: BASE_TIME_MS, offline: false },
    ...overrides,
  };
}

export function makeHygieneFinding(overrides: Partial<HygieneFinding> = {}): HygieneFinding {
  return {
    path: 'config/x.toml.bak',
    isDir: false,
    onDisk: true,
    inIndex: true,
    bytes: 4096,
    reasons: [{ kind: 'pattern', pattern: '*.bak', group: 'anyDepth' }],
    ...overrides,
  };
}

export function makeImportPreview(overrides: Partial<ImportPreview> = {}): ImportPreview {
  return {
    path: 'D:/Packs/meu-pack-antigo',
    name: 'Meu pack antigo',
    minecraft: '1.20.1',
    loader: 'neoforge',
    hygiene: [],
    missingControls: [],
    controlDiffs: [],
    readOnlyReason: null,
    ...overrides,
  };
}
