/**
 * A grade das linhas `.drow` em `design/system/components.css`, sem DOM, para o `drow.ts` (que
 * confere as linhas no jsdom) e o `drow.node.test.ts` (que confere estes números contra o CSS).
 */

/** Quantas colunas a grade tem e em qual delas está o texto (a única flexível, `1fr`). */
export interface DrowGrid {
  columns: number;
  main: number;
}

/** A grade de cada modificador (o jsdom não lê CSS; por isso os números ficam aqui). */
export const DROW_GRID = {
  drow: { columns: 4, main: 2 },
  'drow--compact': { columns: 3, main: 1 },
  'drow--nobox': { columns: 3, main: 1 },
} as const satisfies Record<string, DrowGrid>;
