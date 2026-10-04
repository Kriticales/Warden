/**
 * Resultado dos comandos gerados pelo `tauri-specta` (`bindings.ts`): todo comando devolve
 * `{ status: 'ok', data }` ou `{ status: 'error', error: AppError }` (ARCHITECTURE §4.1).
 */
import type { AppError } from './bindings';

export type CommandResult<T> = { status: 'ok'; data: T } | { status: 'error'; error: AppError };

/** Erro lançado por `unwrap`, com o `AppError` original para o painel de erro. */
export class CommandError extends Error {
  readonly appError: AppError;

  constructor(appError: AppError) {
    super(`${appError.code.domain}.${appError.code.code}`);
    this.name = 'CommandError';
    this.appError = appError;
  }
}

/** Devolve o dado de um resultado ok ou lança `CommandError` (para o TanStack Query). */
export function unwrap<T>(result: CommandResult<T>): T {
  if (result.status === 'ok') {
    return result.data;
  }
  throw new CommandError(result.error);
}
