/**
 * Modelo do formulário de configs (C-04): validação do que a pessoa digita, o texto da faixa e
 * o modo do editor (formulário ou texto) lembrado entre arquivos. Funções puras, sem IPC.
 */
import { useSyncExternalStore } from 'react';

import type { ConfigField, FieldRange, FormValue } from '../../../lib/ipc/bindings';

/** Modo do editor: o mesmo arquivo visto como formulário ou como texto. */
export type EditorMode = 'form' | 'text';

let preferred: EditorMode = 'text';
const listeners = new Set<() => void>();

/**
 * O modo escolhido por último, que vale para o próximo arquivo aberto (SPEC T12: abrir "no
 * modo em que você estava"). Dura a sessão do app.
 */
export function useEditorMode(): [EditorMode, (mode: EditorMode) => void] {
  const mode = useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    () => preferred,
  );
  return [mode, setEditorMode];
}

export function setEditorMode(mode: EditorMode): void {
  if (mode !== preferred) {
    preferred = mode;
    for (const listener of listeners) listener();
  }
}

/** Número para mostrar, no jeito brasileiro ("0,5", "1.024"). */
export function formatNumber(value: number): string {
  return value.toLocaleString('pt-BR', { maximumFractionDigits: 20 });
}

/** As partes da frase da faixa: os dois limites, só um, ou o texto do arquivo. */
export type RangeText =
  | { kind: 'both'; min: string; max: string }
  | { kind: 'min'; min: string }
  | { kind: 'max'; max: string }
  | { kind: 'raw'; text: string };

/** Limites "sem limite" do Java (`Integer.MAX_VALUE`, `Double.MAX_VALUE`…) não são faixa. */
function bound(value: number | null): number | null {
  if (value === null || !Number.isFinite(value)) return null;
  return Math.abs(value) >= 2_147_483_647 ? null : value;
}

export function rangeText(range: FieldRange): RangeText {
  const min = bound(range.min);
  const max = bound(range.max);
  if (min !== null && max !== null) {
    return { kind: 'both', min: formatNumber(min), max: formatNumber(max) };
  }
  if (min !== null) return { kind: 'min', min: formatNumber(min) };
  if (max !== null) return { kind: 'max', max: formatNumber(max) };
  return { kind: 'raw', text: range.text };
}

/** O valor do campo como texto, para os controles de texto. */
export function valueText(value: FormValue): string {
  switch (value.type) {
    case 'bool':
      return value.value ? 'true' : 'false';
    case 'list':
      return value.value.join('\n');
    case 'integer':
    case 'float':
    case 'string':
    case 'datetime':
      return value.value;
  }
}

/** O problema do que foi digitado num campo de número, ou `null` se serve. */
export type NumberProblem = 'invalidoInteiro' | 'invalidoNumero' | 'foraDaFaixa';

export function checkNumber(
  text: string,
  integer: boolean,
  range: FieldRange | null,
): NumberProblem | null {
  const trimmed = text.trim();
  if (
    integer ? !/^[+-]?\d+$/.test(trimmed) : !/^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/.test(trimmed)
  ) {
    return integer ? 'invalidoInteiro' : 'invalidoNumero';
  }
  // Só a faixa que o próprio arquivo declara bloqueia; a deduzida é só uma pista.
  if (range && !range.inferred) {
    const number = Number(trimmed);
    if ((range.min !== null && number < range.min) || (range.max !== null && number > range.max)) {
      return 'foraDaFaixa';
    }
  }
  return null;
}

/** O valor novo de um campo a partir do texto digitado, no mesmo tipo do atual. */
export function valueFromText(field: ConfigField, text: string): FormValue {
  switch (field.value.type) {
    case 'bool':
      return { type: 'bool', value: text.trim() === 'true' };
    case 'list':
      return {
        type: 'list',
        value: text
          .split(/\r?\n/)
          .map((item) => item.trim())
          .filter((item) => item !== ''),
      };
    case 'integer':
    case 'float':
      return { type: field.value.type, value: text.trim() };
    case 'string':
    case 'datetime':
      return { type: field.value.type, value: text };
  }
}

/** Se dois valores são iguais (a edição que não muda nada nem vai ao Rust). */
export function sameValue(left: FormValue, right: FormValue): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

/** O nome do campo para leitores de tela e títulos: o rótulo do mod ou a chave real. */
export function fieldName(field: ConfigField): string {
  return field.label?.text ?? field.key;
}
