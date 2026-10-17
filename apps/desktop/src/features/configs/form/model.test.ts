/** Modelo do formulário de configs (C-04): faixa, validação de números, valores e modo. */
import { renderHook, act } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { ConfigField, FieldRange } from '../../../lib/ipc/bindings';
import {
  checkNumber,
  fieldName,
  rangeText,
  sameValue,
  setEditorMode,
  useEditorMode,
  valueFromText,
  valueText,
} from './model';

const range = (min: number | null, max: number | null, inferred = false): FieldRange => ({
  min,
  max,
  text: 'texto',
  inferred,
});

function field(value: ConfigField['value']): ConfigField {
  return {
    path: ['a'],
    key: 'a',
    line: 1,
    value,
    control: { type: 'text' },
    confidence: 'high',
    description: null,
    range: null,
    default: null,
    defaultState: 'unknown',
    outOfRange: false,
    label: null,
  };
}

describe('modelo do formulário', () => {
  it('escreve a faixa no jeito brasileiro e ignora os limites "sem limite" do Java', () => {
    expect(rangeText(range(0, 10.5))).toEqual({ kind: 'both', min: '0', max: '10,5' });
    expect(rangeText(range(5, 2147483647))).toEqual({ kind: 'min', min: '5' });
    expect(rangeText(range(-2147483648, 4097))).toEqual({ kind: 'max', max: '4.097' });
    expect(rangeText(range(null, null))).toEqual({ kind: 'raw', text: 'texto' });
  });

  it('confere números: inteiro, decimal e faixa declarada (a deduzida não bloqueia)', () => {
    expect(checkNumber('12', true, null)).toBeNull();
    expect(checkNumber(' -3 ', true, null)).toBeNull();
    expect(checkNumber('1.5', true, null)).toBe('invalidoInteiro');
    expect(checkNumber('1,5', false, null)).toBe('invalidoNumero');
    expect(checkNumber('.5', false, null)).toBeNull();
    expect(checkNumber('1e3', false, null)).toBeNull();
    expect(checkNumber('abc', false, null)).toBe('invalidoNumero');
    expect(checkNumber('11', true, range(0, 10))).toBe('foraDaFaixa');
    expect(checkNumber('-1', true, range(0, null))).toBe('foraDaFaixa');
    expect(checkNumber('11', true, range(0, 10, true))).toBeNull();
  });

  it('converte o texto digitado no tipo atual do campo', () => {
    expect(valueFromText(field({ type: 'bool', value: true }), 'false')).toEqual({
      type: 'bool',
      value: false,
    });
    expect(valueFromText(field({ type: 'list', value: [] }), ' a \n\n b\r\n')).toEqual({
      type: 'list',
      value: ['a', 'b'],
    });
    expect(valueFromText(field({ type: 'integer', value: '1' }), ' 7 ')).toEqual({
      type: 'integer',
      value: '7',
    });
    expect(valueFromText(field({ type: 'string', value: '' }), ' x ')).toEqual({
      type: 'string',
      value: ' x ',
    });
    expect(valueText({ type: 'list', value: ['a', 'b'] })).toBe('a\nb');
    expect(valueText({ type: 'bool', value: false })).toBe('false');
    expect(valueText({ type: 'datetime', value: '1979-05-27' })).toBe('1979-05-27');
    expect(sameValue({ type: 'float', value: '1.0' }, { type: 'float', value: '1.0' })).toBe(true);
    expect(sameValue({ type: 'float', value: '1.0' }, { type: 'string', value: '1.0' })).toBe(
      false,
    );
  });

  it('o nome do campo é o rótulo do mod ou a chave', () => {
    const plain = field({ type: 'bool', value: true });
    expect(fieldName(plain)).toBe('a');
    expect(
      fieldName({
        ...plain,
        label: { text: 'Rótulo', tooltip: null, translationKey: 'x', locale: 'enUs' },
      }),
    ).toBe('Rótulo');
  });

  it('o modo escolhido vale para todos que o usam', () => {
    setEditorMode('text');
    const first = renderHook(() => useEditorMode());
    const second = renderHook(() => useEditorMode());
    act(() => {
      first.result.current[1]('form');
    });
    expect(second.result.current[0]).toBe('form');
    act(() => {
      setEditorMode('form');
    });
    expect(first.result.current[0]).toBe('form');
    setEditorMode('text');
  });
});
