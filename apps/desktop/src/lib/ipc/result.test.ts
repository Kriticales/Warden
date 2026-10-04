import { describe, expect, it } from 'vitest';

import { errorMessage } from '../../i18n/errors';
import type { AppError } from './bindings';
import { CommandError, unwrap } from './result';

const appError: AppError = {
  code: { domain: 'app', code: 'INTERNAL' },
  params: { nome: 'x' },
  detail: 'detalhe',
  retryable: false,
};

describe('unwrap', () => {
  it('devolve o dado de um resultado ok', () => {
    expect(unwrap({ status: 'ok', data: 42 })).toBe(42);
  });

  it('lança CommandError com o AppError original', () => {
    let caught: unknown;
    try {
      unwrap({ status: 'error', error: appError });
    } catch (error) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(CommandError);
    expect((caught as CommandError).appError).toBe(appError);
    expect((caught as CommandError).message).toBe('app.INTERNAL');
  });
});

describe('errorMessage', () => {
  it('traduz o código do domínio', () => {
    expect(errorMessage(appError.code)).toMatch(/^Algo deu errado dentro do Warden\./);
  });

  it('código desconhecido cai na frase de INTERNAL', () => {
    const unknown = { domain: 'app', code: 'NAO_EXISTE' } as unknown as AppError['code'];
    expect(errorMessage(unknown)).toBe(errorMessage(appError.code));
  });
});
