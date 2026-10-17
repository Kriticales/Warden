/**
 * Blocos da seção Problemas (SPEC T14 "Seção Problemas"; ROADMAP D-03, D4). **Registro
 * acréscimo-apenas** (ROADMAP §1): a D-08 acrescenta "Saúde do pack" (ordem 100); a D-06 troca
 * a linha `last-crash` pela lista de Travamentos (ordem 300); a 1.1, "Verificações do pack"
 * (ordem 150); a P1-19, "N itens para revisar" do pack importado (ordem 50, o primeiro). Cada
 * componente devolve `null` quando não tem nada a mostrar.
 */
import type { ComponentType } from 'react';

import type { DiagnosticsReport, PackId } from '../../lib/ipc/bindings';
import { ImportReviewBlock } from '../packs/import/ImportReviewBlock';
import { FindingsBlock } from './components/FindingsBlock';
import { LastCrashBlock } from './crash/LastCrashBlock';

export interface ProblemsBlockProps {
  packId: PackId;
  /** O último relatório (`null`: nunca verificado; `undefined`: lendo). */
  report: DiagnosticsReport | null | undefined;
  /** "Verificar agora" em andamento. */
  checking: boolean;
}

export interface ProblemsBlock {
  id: string;
  order: number;
  component: ComponentType<ProblemsBlockProps>;
}

/** Registro acréscimo-apenas: uma linha por bloco. */
export const problemsBlocks: readonly ProblemsBlock[] = [
  { id: 'import-review', order: 50, component: ImportReviewBlock },
  { id: 'findings', order: 200, component: FindingsBlock },
  { id: 'last-crash', order: 300, component: LastCrashBlock },
];

export function orderedProblemsBlocks(
  blocks: readonly ProblemsBlock[] = problemsBlocks,
): ProblemsBlock[] {
  return [...blocks].sort((a, b) => a.order - b.order);
}
