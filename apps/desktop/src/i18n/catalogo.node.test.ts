// @vitest-environment node
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

const DIR = join(import.meta.dirname, 'pt-BR');
const files = readdirSync(DIR).filter((name) => name.endsWith('.ts') && !name.endsWith('.test.ts'));

/** `boas-vindas.ts` → `boasVindas`. */
function namespaceOf(file: string): string {
  return file
    .replace(/\.ts$/, '')
    .replace(/-([a-z0-9])/g, (_, letter: string) => letter.toUpperCase());
}

describe('catálogo de textos', () => {
  it.each(files)('%s exporta o namespace e o registra nos tipos (catalogo.ts)', (file: string) => {
    const name = namespaceOf(file);
    const source = readFileSync(join(DIR, file), 'utf8');
    expect(source, 'export const').toMatch(new RegExp(`^export const ${name}\\b`, 'm'));
    expect(source, "declare module '../catalogo'").toContain("declare module '../catalogo'");
    expect(source, 'interface Catalogo').toMatch(
      new RegExp(`interface Catalogo \\{[^}]*\\b${name}: typeof ${name};`),
    );
  });
});
