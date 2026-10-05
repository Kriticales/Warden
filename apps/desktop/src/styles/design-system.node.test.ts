// @vitest-environment node
/**
 * O app usa os arquivos do design system direto de `design/system/` (sem cópia; HANDOFF §1).
 * Aqui se confere que os três gerados (`tokens.css`, `tailwind-theme.css`, `shadcn-theme.css`)
 * estão em dia com `tokens.json`: o gerador roda numa cópia temporária e a saída tem de ser
 * idêntica à versionada. Também confere que o `app.css` importa o que o HANDOFF manda.
 */
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterAll, describe, expect, it } from 'vitest';

const SYSTEM = join(import.meta.dirname, '..', '..', '..', '..', 'design', 'system');
const GENERATED = ['tokens.css', 'tailwind-theme.css', 'shadcn-theme.css'];

describe('design system', () => {
  const work = mkdtempSync(join(tmpdir(), 'warden-tokens-'));
  afterAll(() => {
    rmSync(work, { recursive: true, force: true });
  });

  it('os temas gerados batem com tokens.json (rode build-tokens.mjs se falhar)', () => {
    mkdirSync(join(work, 'tools'));
    copyFileSync(join(SYSTEM, 'tokens.json'), join(work, 'tokens.json'));
    copyFileSync(
      join(SYSTEM, 'tools', 'build-tokens.mjs'),
      join(work, 'tools', 'build-tokens.mjs'),
    );
    execFileSync(process.execPath, [join(work, 'tools', 'build-tokens.mjs')], { stdio: 'pipe' });
    for (const file of GENERATED) {
      expect(readFileSync(join(work, file), 'utf8'), file).toBe(
        readFileSync(join(SYSTEM, file), 'utf8'),
      );
    }
  });

  it('app.css importa Tailwind, fontes embutidas, os dois temas e os componentes como camada', () => {
    const css = readFileSync(join(import.meta.dirname, 'app.css'), 'utf8');
    const imports = [...css.matchAll(/@import '([^']+)'(?: layer\((\w+)\))?;/g)].map(
      ([, path, layer]) => `${path ?? ''}${layer ? ` [${layer}]` : ''}`,
    );
    expect(imports).toEqual([
      'tailwindcss',
      '../../../../design/system/fonts/fonts.css',
      '../../../../design/system/tailwind-theme.css',
      '../../../../design/system/shadcn-theme.css',
      '../../../../design/system/components.css [components]',
      './warden.css [components]',
    ]);
  });

  it('fontes só locais (nada baixado da internet)', () => {
    const fonts = readFileSync(join(SYSTEM, 'fonts', 'fonts.css'), 'utf8');
    expect(fonts).not.toMatch(/https?:/);
    expect(fonts.match(/@font-face/g)?.length).toBeGreaterThanOrEqual(12);
  });

  it('o CSS do app não usa cor, raio ou sombra fora dos tokens', () => {
    const css = readFileSync(join(import.meta.dirname, 'warden.css'), 'utf8');
    expect(css).not.toMatch(/#[0-9a-f]{3,8}\b/i);
    expect(css).not.toMatch(/rgba?\(/);
    expect(css).not.toMatch(/border-radius/);
    expect(css).not.toMatch(/box-shadow:[^;]*\d+px/);
  });
});
