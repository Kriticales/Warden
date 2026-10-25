/**
 * Blocos da seção Problemas (SPEC T14 "Seção Problemas"; ROADMAP D-03, D4). **Registro
 * acréscimo-apenas** (ROADMAP §1): a D-08 acrescenta "Saúde do pack" (ordem 100); a D-06 troca
 * a linha `last-crash` pela lista de Travamentos (ordem 300); a 1.1, "Verificações do pack"
 * (ordem 150); a L-11, "Memória no último teste" (ordem 250); a P1-19, "N itens para revisar"
 * do pack importado (ordem 50, o primeiro); o bug-01, "Desempenho do último teste" (ordem
 * 150, coluna da direita). Cada componente devolve `null` quando não tem nada a mostrar.
 *
 * `column` diz em qual das duas colunas do esboço aprovado do bug-01 o bloco fica: `main` (a
 * larga: achados e travamentos; o padrão) ou `side` (a da direita, ~340 px: saúde e
 * desempenho).
 */
import type { ComponentType } from 'react';

import type { DiagnosticsReport, PackId } from '../../lib/ipc/bindings';
import { ImportReviewBlock } from '../packs/import/ImportReviewBlock';
import { HeapProblemsBlock } from '../test/perf-strip/heap/HeapProblemsBlock';
import { FindingsBlock } from './components/FindingsBlock';
import { CrashesBlock } from './crashes/CrashesBlock';
import { HealthBlock } from './health/HealthBlock';
import { PerfBlock } from './perf/PerfBlock';

export interface ProblemsBlockProps {
  packId: PackId;
  /** O último relatório (`null`: nunca verificado; `undefined`: lendo). */
  report: DiagnosticsReport | null | undefined;
  /** "Verificar agora" em andamento. */
  checking: boolean;
}

/** As colunas da seção. */
export type ProblemsColumn = 'main' | 'side';

export interface ProblemsBlock {
  id: string;
  order: number;
  /** A coluna (padrão: `main`). */
  column?: ProblemsColumn;
  component: ComponentType<ProblemsBlockProps>;
}

/** Registro acréscimo-apenas: uma linha por bloco. */
export const problemsBlocks: readonly ProblemsBlock[] = [
  { id: 'import-review', order: 50, component: ImportReviewBlock },
  // D-08: a nota de saúde do pack, no topo.
  { id: 'health', order: 100, column: 'side', component: HealthBlock },
  { id: 'findings', order: 200, component: FindingsBlock },
  // D-06: a lista de Travamentos, no lugar do "Último travamento" da D-03.
  { id: 'crashes', order: 300, component: CrashesBlock },
  // L-11: os achados de memória do último teste.
  { id: 'memory', order: 250, component: HeapProblemsBlock },
  // bug-01: o desempenho do último teste, abaixo da saúde.
  { id: 'performance', order: 150, column: 'side', component: PerfBlock },
];

export function orderedProblemsBlocks(
  blocks: readonly ProblemsBlock[] = problemsBlocks,
  column?: ProblemsColumn,
): ProblemsBlock[] {
  return [...blocks]
    .filter((block) => column === undefined || (block.column ?? 'main') === column)
    .sort((a, b) => a.order - b.order);
}
