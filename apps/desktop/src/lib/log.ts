/**
 * Registros do frontend (QUALITY §2.2: `console` é proibido). Encaminha ao `tauri-plugin-log`,
 * que grava no mesmo arquivo dos registros do Rust (ARCHITECTURE §16), com o alvo `webview`.
 *
 * Nunca registre segredos, chaves ou o corpo de respostas de APIs (QUALITY §9). Fora do app
 * (testes, navegador), a chamada falha em silêncio: registro nunca derruba a interface.
 */
import { writeLog, type LogLevelName } from './ipc/log-bridge';

/** Texto de um erro qualquer, para registros e "Detalhes técnicos". */
export function describeError(cause: unknown): string {
  if (cause instanceof Error) {
    return cause.stack ?? `${cause.name}: ${cause.message}`;
  }
  if (typeof cause === 'string') {
    return cause;
  }
  try {
    // `JSON.stringify(undefined)` devolve `undefined`, apesar do tipo.
    const json = JSON.stringify(cause) as string | undefined;
    return json ?? String(cause);
  } catch {
    return String(cause);
  }
}

function write(level: LogLevelName, message: string, cause?: unknown): void {
  const text = cause === undefined ? message : `${message}: ${describeError(cause)}`;
  // Registro nunca derruba a interface: fora do app (testes, navegador) a chamada falha.
  writeLog(level, text).catch(() => undefined);
}

export const log = {
  info: (message: string, cause?: unknown) => {
    write('info', message, cause);
  },
  warn: (message: string, cause?: unknown) => {
    write('warn', message, cause);
  },
  error: (message: string, cause?: unknown) => {
    write('error', message, cause);
  },
};
