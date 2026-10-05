import { describe, expect, it } from 'vitest';

import { makeAppError } from '../../test/factories';
import { appErrorMessage, errorCodeText, isAppError, technicalDetails, toAppError } from './errors';
import { CommandError } from './result';

describe('toAppError', () => {
  it('tira o AppError de um CommandError', () => {
    const error = makeAppError({ domain: 'core', code: 'IO' });
    expect(toAppError(new CommandError(error))).toBe(error);
  });

  it('aceita a rejeição crua do IPC (o AppError serializado)', () => {
    const error = makeAppError({ domain: 'secrets', code: 'NOT_CONFIGURED' });
    expect(toAppError(JSON.parse(JSON.stringify(error)))).toEqual(error);
  });

  it('erro de JavaScript vira app.INTERNAL com a mensagem só nos detalhes', () => {
    const converted = toAppError(new TypeError('x is undefined'));
    expect(converted.code).toEqual({ domain: 'app', code: 'INTERNAL' });
    expect(converted.detail).toContain('x is undefined');
    expect(appErrorMessage(converted)).not.toContain('x is undefined');
  });

  it('texto, objeto qualquer, undefined e objeto circular também viram INTERNAL', () => {
    expect(toAppError('falhou').detail).toBe('falhou');
    expect(toAppError({ a: 1 }).detail).toBe('{"a":1}');
    expect(toAppError(undefined).detail).toBe('undefined');
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(toAppError(circular).detail).toBe('[object Object]');
  });
});

describe('isAppError', () => {
  it('reconhece só a forma completa', () => {
    expect(isAppError(makeAppError())).toBe(true);
    expect(isAppError({ code: { domain: 'app', code: 'INTERNAL' } })).toBe(false);
    expect(isAppError({ ...makeAppError(), detail: 3 })).toBe(false);
    expect(isAppError({ ...makeAppError(), operationId: 3 })).toBe(false);
    expect(isAppError(null)).toBe(false);
    expect(isAppError('app.INTERNAL')).toBe(false);
  });
});

describe('appErrorMessage', () => {
  it('preenche os parâmetros da frase', () => {
    const error = makeAppError(
      { domain: 'core', code: 'IO' },
      { params: { path: 'mods/jei.pw.toml' } },
    );
    expect(appErrorMessage(error)).toContain('em mods/jei.pw.toml.');
  });

  it('parâmetro que faltou vira "valor desconhecido", nunca a chave crua', () => {
    const message = appErrorMessage(makeAppError({ domain: 'core', code: 'IO' }));
    expect(message).toContain('em valor desconhecido.');
    expect(message).not.toContain('{{');
  });
});

describe('detalhes técnicos', () => {
  it('código, tarefa, parâmetros e detail, nessa ordem', () => {
    const error = makeAppError(
      { domain: 'packwiz', code: 'INVALID_TOML' },
      {
        params: { path: 'pack.toml' },
        detail: 'linha 3: esperado =',
        operationId: '01J9ZQ0000000000000000000A',
      },
    );
    expect(errorCodeText(error.code)).toBe('packwiz.INVALID_TOML');
    expect(technicalDetails(error)).toBe(
      [
        'Código: packwiz.INVALID_TOML',
        'Tarefa: 01J9ZQ0000000000000000000A',
        'path: pack.toml',
        '',
        'linha 3: esperado =',
      ].join('\n'),
    );
  });

  it('sem tarefa nem detail, só o código', () => {
    expect(technicalDetails(makeAppError())).toBe('Código: app.INTERNAL');
  });
});
