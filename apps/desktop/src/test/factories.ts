/**
 * Fábricas de dados dos testes, tipadas pelos tipos gerados (`bindings.ts`): se o contrato do
 * Rust mudar, os testes deixam de compilar em vez de testar uma forma velha.
 */
import type {
  AppError,
  AppInfo,
  ErrorCode,
  OperationSnapshot,
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
