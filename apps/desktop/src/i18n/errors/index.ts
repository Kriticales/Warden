/**
 * Frases dos erros por domínio (ARCHITECTURE §5). Cada domínio tem o seu arquivo, tipado como
 * `Record<CódigoDoDomínio, string>`: o TypeScript falha se faltar uma tradução.
 *
 * Registro acréscimo-apenas (ROADMAP §1): uma linha de import e uma linha em `errorMessages`
 * por domínio.
 */
import type { ErrorCode } from '../../lib/ipc/bindings';

import { app } from './app';

type ErrorMessages = {
  [Domain in ErrorCode['domain']]: Record<Extract<ErrorCode, { domain: Domain }>['code'], string>;
};

export const errorMessages: ErrorMessages = {
  app,
};

/** Frase pt-BR de um código de erro; código desconhecido cai na frase de `app.INTERNAL`. */
export function errorMessage(code: ErrorCode): string {
  const byCode: Partial<Record<string, string>> = errorMessages[code.domain];
  return byCode[code.code] ?? errorMessages.app.INTERNAL;
}
