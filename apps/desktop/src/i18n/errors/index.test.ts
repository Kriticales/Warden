import { describe, expect, it } from 'vitest';

import type { ErrorCode } from '../../lib/ipc/bindings';

import { errorMessage, errorMessages } from './index';

describe('errorMessages', () => {
  it('tem os 26 domínios da ARCHITECTURE §5, cada um com INTERNAL', () => {
    const domains = Object.keys(errorMessages);
    expect(domains).toHaveLength(26);
    for (const domain of domains) {
      const code = { domain, code: 'INTERNAL' } as ErrorCode;
      expect(errorMessage(code)).toMatch(/\.$/);
    }
  });

  it('frases dizem o que fazer e não ficam vazias', () => {
    for (const byCode of Object.values(errorMessages)) {
      for (const phrase of Object.values(byCode as Record<string, string>)) {
        expect(phrase.trim().length).toBeGreaterThan(10);
        expect(phrase).not.toContain('!');
      }
    }
  });

  it('usa a frase do domínio e cai no INTERNAL do app para código desconhecido', () => {
    expect(errorMessage({ domain: 'core', code: 'CANCELLED' })).toBe('Operação cancelada.');
    expect(errorMessage({ domain: 'secrets', code: 'NOT_CONFIGURED' })).toContain('Configurações');
    const unknown = { domain: 'core', code: 'NAO_EXISTE' } as unknown as ErrorCode;
    expect(errorMessage(unknown)).toBe(errorMessages.app.INTERNAL);
  });

  it('com count, escolhe CÓDIGO_one/CÓDIGO_other; sem as formas, a frase do código', () => {
    const code: ErrorCode = { domain: 'app', code: 'TEST_GAME_FILES_MISSING' };
    expect(errorMessage(code, 1)).toBe(errorMessages.app.TEST_GAME_FILES_MISSING_one);
    expect(errorMessage(code, 3)).toBe(errorMessages.app.TEST_GAME_FILES_MISSING_other);
    expect(errorMessage(code)).toBe(errorMessages.app.TEST_GAME_FILES_MISSING);
    const semPlural: ErrorCode = { domain: 'core', code: 'CANCELLED' };
    expect(errorMessage(semPlural, 2)).toBe('Operação cancelada.');
  });

  it('cada CÓDIGO_one/CÓDIGO_other tem o par e o código base', () => {
    for (const byCode of Object.values(errorMessages)) {
      const keys = Object.keys(byCode);
      for (const key of keys.filter((k) => /_(one|other)$/.test(k))) {
        const base = key.replace(/_(one|other)$/, '');
        expect(keys).toContain(base);
        expect(keys).toContain(`${base}_one`);
        expect(keys).toContain(`${base}_other`);
        expect(byCode[key as keyof typeof byCode]).toContain('{{count}}');
      }
    }
  });
});
