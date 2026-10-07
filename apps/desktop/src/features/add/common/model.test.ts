import { describe, expect, it } from 'vitest';

import { makePage, makeResult, ref } from '../add.fixtures';
import {
  ago,
  formatCount,
  isInPack,
  knownSources,
  mergePages,
  pageWarnings,
  refKey,
  sourceLabel,
} from './model';

describe('página Adicionar: regras puras', () => {
  it('juntar páginas descarta o projeto que já apareceu por qualquer fonte', () => {
    const jei = makeResult({
      title: 'JEI',
      sources: [ref('u6dRKJwZ'), ref('238222', { source: 'curseforge' })],
    });
    const again = makeResult({ title: 'JEI', sources: [ref('238222', { source: 'curseforge' })] });
    const other = makeResult({ title: 'Outro', sources: [ref('XYZ')] });
    const merged = mergePages([makePage([jei]), makePage([again, other])]);
    expect(merged.map((item) => item.title)).toEqual(['JEI', 'Outro']);
    expect(refKey(ref('A'))).toBe('modrinth:A');
  });

  it('"Já no pack" pela busca ou pelo inventário atual', () => {
    const sodium = makeResult();
    expect(isInPack(sodium, new Set())).toBe(false);
    expect(isInPack(sodium, new Set(['modrinth:AANobbMI']))).toBe(true);
    expect(isInPack({ ...sodium, inPack: true }, new Set())).toBe(true);
  });

  it('rótulo de fonte e fontes conhecidas', () => {
    expect(sourceLabel([ref('A')])).toBe('modrinth');
    expect(sourceLabel([ref('A', { source: 'curseforge' })])).toBe('curseforge');
    expect(sourceLabel([ref('A'), ref('B', { source: 'curseforge' })])).toBe('ambas');
    const pages = [
      makePage([], {
        warnings: [{ source: 'curseforge', reason: 'unavailable', detail: null }],
      }),
      makePage([], {
        warnings: [{ source: 'curseforge', reason: 'unavailable', detail: 'outra' }],
      }),
    ];
    expect(knownSources(pages)).toEqual(['modrinth', 'curseforge']);
    expect(pageWarnings(pages)).toHaveLength(1);
    expect(knownSources([makePage([])])).toEqual(['modrinth']);
  });

  it('contagem curta e "há quanto tempo"', () => {
    // O Intl separa número e unidade com espaço sem quebra.
    expect(formatCount(48_000_000)).toBe('48 mi');
    expect(formatCount(840_000)).toBe('840 mil');
    expect(formatCount(null)).toBe('0');
    const now = new Date(2026, 9, 7, 15).getTime();
    expect(ago(new Date(2026, 9, 7, 1).toISOString(), now)).toEqual({ unit: 'hoje' });
    expect(ago(new Date(2026, 9, 6, 23).toISOString(), now)).toEqual({ unit: 'ontem' });
    expect(ago(new Date(2026, 9, 1).toISOString(), now)).toEqual({ unit: 'dias', count: 6 });
    expect(ago(new Date(2026, 6, 1).toISOString(), now)).toEqual({ unit: 'meses', count: 3 });
    expect(ago(new Date(2024, 9, 1).toISOString(), now)).toEqual({ unit: 'anos', count: 2 });
    expect(ago('não é data', now)).toBeNull();
  });
});
