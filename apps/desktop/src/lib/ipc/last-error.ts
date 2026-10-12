/**
 * O último erro mostrado ao usuário (UI-02: entra no "Relatar problema").
 *
 * `appErrorMessage` (`errors.ts`) é por onde passa toda frase de erro que aparece na tela, e
 * guarda aqui o código e a frase. Fica numa variável simples, sem estado do React: quem lê é o
 * diálogo do relato, no momento em que abre.
 */

/** Código `domínio.CÓDIGO` e a frase mostrada. */
export interface ShownErrorText {
  code: string;
  message: string;
}

let last: ShownErrorText | null = null;

/** Guarda o erro que acabou de ser mostrado. */
export function rememberShownError(code: string, message: string): void {
  last = { code, message };
}

/** O último erro mostrado desde que o app abriu, ou `null`. */
export function lastShownError(): ShownErrorText | null {
  return last;
}

/** Esquece o último erro (testes). */
export function forgetShownError(): void {
  last = null;
}
