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
});
