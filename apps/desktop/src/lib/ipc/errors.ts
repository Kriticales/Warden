/**
 * Tradução de `AppError` para a interface (ARCHITECTURE §5; QUALITY §3).
 *
 * - `toAppError` reconhece um `AppError` em qualquer erro que chegue a um componente: o
 *   `CommandError` de `unwrap`, a rejeição crua do IPC ou o próprio objeto.
 * - `appErrorMessage` devolve a frase do catálogo `i18n/errors/<domínio>.ts` com os `params`
 *   no lugar de `{{nome}}`.
 * - `technicalDetails` monta o texto de "Detalhes técnicos" (código, detalhe e tarefa), que é o
 *   que o botão Copiar copia.
 */
import i18n from '../../i18n';
import { errorMessage } from '../../i18n/errors';
import { describeError } from '../log';
import type { AppError, ErrorCode } from './bindings';
import { CommandError } from './result';

const PLACEHOLDER = /\{\{\s*([\w.]+)\s*\}\}/g;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isErrorCode(value: unknown): value is ErrorCode {
  return isRecord(value) && typeof value.domain === 'string' && typeof value.code === 'string';
}

/** Se `value` tem a forma de um `AppError` serializado. */
export function isAppError(value: unknown): value is AppError {
  return (
    isRecord(value) &&
    isErrorCode(value.code) &&
    isRecord(value.params) &&
    typeof value.retryable === 'boolean' &&
    (value.detail === null || typeof value.detail === 'string') &&
    (value.operationId === null || typeof value.operationId === 'string')
  );
}

/**
 * O `AppError` contido num erro, ou um `app.INTERNAL` com a mensagem do erro inesperado no
 * `detail` (erro de JavaScript, exceção do React). Nunca devolve a mensagem crua como frase.
 */
export function toAppError(error: unknown): AppError {
  if (error instanceof CommandError) {
    return error.appError;
  }
  if (isAppError(error)) {
    return error;
  }
  return {
    code: { domain: 'app', code: 'INTERNAL' },
    params: {},
    detail: describeError(error),
    retryable: false,
    operationId: null,
  };
}

/** Frase pt-BR de um erro, com os parâmetros preenchidos. */
export function appErrorMessage(error: AppError): string {
  const missing = i18n.t('erro.parametroAusente');
  return errorMessage(error.code).replace(
    PLACEHOLDER,
    (_, name: string) => error.params[name] ?? missing,
  );
}

/** `domínio.CÓDIGO`, como aparece em "Detalhes técnicos". */
export function errorCodeText(code: ErrorCode): string {
  return `${code.domain}.${code.code}`;
}

/** Texto de "Detalhes técnicos": o que o botão Copiar leva para a área de transferência. */
export function technicalDetails(error: AppError): string {
  const lines = [`${i18n.t('erro.codigo')}: ${errorCodeText(error.code)}`];
  if (error.operationId) {
    lines.push(`${i18n.t('erro.operacao')}: ${error.operationId}`);
  }
  const params = Object.entries(error.params);
  if (params.length > 0) {
    lines.push(...params.map(([key, value]) => `${key}: ${value}`));
  }
  if (error.detail) {
    lines.push('', error.detail);
  }
  return lines.join('\n');
}
