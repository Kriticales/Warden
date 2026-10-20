/**
 * Linhas `.drow` do design system: a grade tem um número fixo de colunas por modificador
 * (`design/system/components.css`), e cada filho direto ocupa uma, na ordem. Se faltar um filho
 * antes do texto, o texto cai na coluna estreita do começo e o botão do fim fica por cima dele
 * (P16: "+ Adicionar kit" em cima do nome do kit).
 */

import { DROW_GRID, type DrowGrid } from './drow-grid';

function gridFor(row: Element): DrowGrid {
  if (row.classList.contains('drow--nobox')) return DROW_GRID['drow--nobox'];
  if (row.classList.contains('drow--compact')) return DROW_GRID['drow--compact'];
  return DROW_GRID.drow;
}

/**
 * Linhas `.drow` dentro de `root` que não combinam com a grade do modificador: mais filhos que
 * colunas, ou o `.drow__main` fora da coluna flexível (caindo na estreita do começo, como no
 * P16). Colunas vazias no fim (uma linha sem `.drow__end`) não atrapalham.
 */
export function drowShapeErrors(root: ParentNode): string[] {
  return Array.from(root.querySelectorAll('.drow')).flatMap((row) => {
    const grid = gridFor(row);
    const children = Array.from(row.children);
    const main = children.findIndex((child) => child.classList.contains('drow__main'));
    const problems: string[] = [];
    if (children.length > grid.columns) {
      problems.push(
        `"${row.className}" tem ${String(children.length)} filhos e ${String(grid.columns)} colunas`,
      );
    }
    if (main !== grid.main) {
      problems.push(
        `"${row.className}": o texto (.drow__main) está na coluna ${String(main + 1)}, e não na ${String(grid.main + 1)}`,
      );
    }
    return problems;
  });
}
