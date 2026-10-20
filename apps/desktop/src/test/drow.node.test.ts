// @vitest-environment node
/**
 * As colunas de `.drow` que `src/test/drow-grid.ts` guarda batem com `design/system/components.css`: se a
 * grade mudar no design system, este teste avisa antes que a checagem das linhas fique errada.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { DROW_GRID, type DrowGrid } from './drow-grid';

const CSS = readFileSync(
  join(import.meta.dirname, '..', '..', '..', '..', 'design', 'system', 'components.css'),
  'utf8',
);

/** As colunas de `grid-template-columns`, sem partir o que está dentro de parênteses. */
function columnsOf(template: string): string[] {
  const columns: string[] = [];
  let depth = 0;
  let current = '';
  for (const char of template.trim()) {
    if (char === '(') depth += 1;
    if (char === ')') depth -= 1;
    if (/\s/.test(char) && depth === 0) {
      if (current) columns.push(current);
      current = '';
    } else {
      current += char;
    }
  }
  if (current) columns.push(current);
  return columns;
}

function gridOf(selector: string): DrowGrid {
  const escaped = selector.replace(/[.-]/g, (c) => `\\${c}`);
  const match = new RegExp(`(?:^|\\n)${escaped} \\{[^}]*grid-template-columns:\\s*([^;]+);`).exec(
    CSS,
  );
  const columns = columnsOf(match?.[1] ?? '');
  return { columns: columns.length, main: columns.findIndex((column) => column.includes('1fr')) };
}

describe('grade das linhas .drow', () => {
  it('as colunas de cada modificador batem com o design system', () => {
    for (const [modifier, grid] of Object.entries(DROW_GRID)) {
      expect(gridOf(`.${modifier}`), modifier).toEqual(grid);
    }
  });
});
