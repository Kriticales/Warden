import { describe, expect, it } from 'vitest';

import { dayRelation, formatBytes, formatDate, formatInteger, formatTime } from './format';

describe('formatação pt-BR', () => {
  it('números com ponto de milhar', () => {
    expect(formatInteger(1204)).toBe('1.204');
    expect(formatInteger(0)).toBe('0');
  });

  it('tamanhos com vírgula decimal', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1_258_291)).toBe('1,2 MB');
    expect(formatBytes(420 * 1024 * 1024)).toBe('420,0 MB');
    expect(formatBytes(3 * 1024 ** 4)).toBe('3,0 TB');
    expect(formatBytes(-5)).toBe('0 B');
  });

  it('hora e data', () => {
    const ms = new Date(2026, 9, 1, 14, 1).getTime();
    expect(formatTime(ms)).toBe('14:01');
    expect(formatDate(ms)).toBe('01/10/2026');
  });

  it('hoje, ontem e outro dia pelo relógio local', () => {
    const now = new Date(2026, 9, 4, 0, 30).getTime();
    expect(dayRelation(new Date(2026, 9, 4, 0, 1).getTime(), now)).toBe('today');
    expect(dayRelation(new Date(2026, 9, 3, 23, 59).getTime(), now)).toBe('yesterday');
    expect(dayRelation(new Date(2026, 9, 2, 12, 0).getTime(), now)).toBe('other');
    // Virada de mês.
    expect(
      dayRelation(new Date(2026, 8, 30, 8, 0).getTime(), new Date(2026, 9, 1, 8).getTime()),
    ).toBe('yesterday');
  });
});
