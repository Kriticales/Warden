/**
 * Tipo de um catálogo de erros (`i18n/errors/<domínio>.ts`): uma frase por código e, para as
 * frases com número, as formas `CÓDIGO_one`/`CÓDIGO_other` opcionais, como no i18next
 * (`pt-BR/exportar.ts`). A frase do código vale quando o erro não traz `count`.
 */
export type ErrorPhrases<Code extends string> = Record<Code, string> &
  Partial<Record<`${Code}_${'one' | 'other'}`, string>>;
