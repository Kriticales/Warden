/**
 * Textos do editor de scripts: toda chave de `i18n/pt-BR/scripts.ts` é usada (literal no código
 * ou pelas chaves dinâmicas `trechos.<id>` e `ids.<tipo>`), e todo trecho pronto dos dados
 * (`crates/warden-scripts/data/snippets/*.toml`) tem nome.
 */
import { describe, expect, it } from 'vitest';

import { scripts } from '../../../i18n/pt-BR/scripts';

const SOURCES = {
  ...import.meta.glob<string>(['./*.ts', './*.tsx', '!./*.test.ts', '!./*.test.tsx'], {
    query: '?raw',
    import: 'default',
    eager: true,
  }),
  ...import.meta.glob<string>(['../../test/reload-scripts/*.tsx', '!**/*.test.tsx'], {
    query: '?raw',
    import: 'default',
    eager: true,
  }),
};

const SNIPPETS = import.meta.glob<string>(
  '../../../../../../crates/warden-scripts/data/snippets/*.toml',
  {
    query: '?raw',
    import: 'default',
    eager: true,
  },
);

function flatten(value: unknown, prefix = ''): string[] {
  if (typeof value === 'string') return [prefix];
  return Object.entries(value as Record<string, unknown>).flatMap(([key, inner]) =>
    flatten(inner, prefix ? `${prefix}.${key}` : key),
  );
}

describe('textos do editor de scripts', () => {
  it('nenhuma chave sem uso', () => {
    const source = Object.values(SOURCES).join('\n');
    expect(Object.keys(SOURCES).length).toBeGreaterThan(5);
    const dynamic = ['trechos.', 'ids.'];
    const unused = flatten(scripts).filter(
      (key) =>
        !dynamic.some((prefix) => key.startsWith(prefix) && key !== 'ids.jogoAberto') &&
        !source.includes(`'${key}'`),
    );
    expect(unused).toEqual([]);
    expect(source).toContain('t(`trechos.${');
    expect(source).toContain('t(`ids.${value}`)');
  });

  it('todo trecho pronto dos dados tem nome, e todo nome tem trecho', () => {
    expect(Object.keys(SNIPPETS)).toHaveLength(5);
    const ids = new Set(
      Object.values(SNIPPETS).flatMap((text) =>
        [...text.matchAll(/^id = "([^"]+)"$/gm)].map((match) => match[1]),
      ),
    );
    expect([...ids].sort()).toEqual(Object.keys(scripts.trechos).sort());
  });

  it('os tipos de ID têm nome', () => {
    expect(Object.keys(scripts.ids).sort()).toEqual(
      ['block', 'blockTag', 'fluidTag', 'item', 'itemTag', 'jogoAberto', 'recipe'].sort(),
    );
  });
});
