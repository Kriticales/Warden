/**
 * O último erro mostrado ao usuário (UI-02: entra no "Relatar problema").
 *
 * O registro acontece num ponto só: quando um erro **aparece na tela**, pelas duas formas que
 * a interface tem de mostrar erro, o aviso de perigo (`Alert kind="danger"`, que o
 * `ErrorPanel` usa) e o toast de perigo. As duas chamam `useRememberShownError`, que guarda o
 * texto visível quando o aviso aparece ou muda; um aviso antigo que só se redesenha não toma
 * o lugar de um erro mais novo. Erros que a tela trata com texto próprio (por exemplo, "loader
 * não suportado") e falhas da própria interface (copiar, abrir pasta) entram do mesmo jeito.
 *
 * O código `domínio.CÓDIGO` vem de `appErrorMessage` (`errors.ts`), que anota a frase de cada
 * `AppError` que traduz (`notePhrase`): se o texto mostrado contém uma frase anotada, o
 * código dela vai junto. Fica tudo em variáveis simples, sem estado do React: quem lê é o
 * diálogo do relato, no momento em que abre.
 */
import { useEffect, useRef, type RefObject } from 'react';

/** Código `domínio.CÓDIGO` (quando o erro veio de um `AppError`) e o texto mostrado. */
export interface ShownErrorText {
  code: string | null;
  message: string;
}

/** Maior texto guardado (o relato só precisa do começo). */
const MAX_MESSAGE = 500;
/** Quantas frases de `AppError` ficam anotadas (as mais recentes). */
const MAX_PHRASES = 50;

let last: ShownErrorText | null = null;
const phrases = new Map<string, string>();

/** Anota a frase traduzida de um `AppError` e o código dele (`appErrorMessage`). */
export function notePhrase(code: string, message: string): void {
  const phrase = normalize(message);
  if (phrase === '') return;
  phrases.delete(phrase);
  phrases.set(phrase, code);
  if (phrases.size > MAX_PHRASES) {
    const oldest = phrases.keys().next().value;
    if (oldest !== undefined) phrases.delete(oldest);
  }
}

/** Guarda o texto de um erro que acabou de aparecer na tela. */
export function rememberShownError(text: string): void {
  const message = normalize(text);
  if (message === '') return;
  let code: string | null = null;
  let longest = 0;
  for (const [phrase, phraseCode] of phrases) {
    if (phrase.length > longest && message.includes(phrase)) {
      code = phraseCode;
      longest = phrase.length;
    }
  }
  last = { code, message: message.slice(0, MAX_MESSAGE) };
}

/** O último erro mostrado desde que o app abriu, ou `null`. */
export function lastShownError(): ShownErrorText | null {
  return last;
}

/** Esquece o último erro e as frases anotadas (testes). */
export function forgetShownError(): void {
  last = null;
  phrases.clear();
}

/**
 * Registra o erro mostrado por `ref` quando ele aparece e sempre que o texto visível muda
 * (`active` falso: não é um erro). Usado só pelo `Alert` e pelo toast.
 */
export function useRememberShownError(ref: RefObject<HTMLElement | null>, active: boolean): void {
  const recorded = useRef<string | null>(null);
  // Sem dependências: confere o texto a cada desenho, mas só registra quando ele muda.
  useEffect(() => {
    const element = ref.current;
    if (!active || !element) return;
    const text = visibleText(element);
    if (text !== recorded.current) {
      recorded.current = text;
      rememberShownError(text);
    }
  });
}

/** O que o erro diz, sem os botões nem os "Detalhes técnicos" recolhidos. */
function visibleText(element: HTMLElement): string {
  const copy = element.cloneNode(true) as HTMLElement;
  for (const extra of copy.querySelectorAll('button, details, svg, [aria-hidden="true"]')) {
    extra.remove();
  }
  // Cada parte (título, texto, parágrafos) fica separada por espaço: não grudam.
  for (const part of [...copy.children, ...copy.querySelectorAll('div, p, li')]) {
    part.append(' ');
  }
  return normalize(copy.textContent);
}

function normalize(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}
