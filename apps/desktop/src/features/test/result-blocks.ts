/**
 * Partes do resultado do teste, abaixo do título (SPEC T13 passo 7). **Registro
 * acréscimo-apenas** (ROADMAP §1): a C-03 acrescenta "O que mudou durante o teste" (ordem 100);
 * a D-03 troca a linha `why-crashed` pela análise do travamento ("Por que travou" com a causa e
 * a correção); a L-12, "Desempenho"; a 1.1, "Esta versão está mais pesada". Cada componente
 * devolve `null` quando não tem nada a mostrar.
 */
import type { ComponentType } from 'react';

import type { ConsoleLine, PackId, TestSessionSummary } from '../../lib/ipc/bindings';
import { WhyCrashed } from './views/WhyCrashed';

export interface ResultBlockProps {
  packId: PackId;
  session: TestSessionSummary;
  /** O console da sessão (ao vivo ou lido do log gravado). */
  lines: readonly ConsoleLine[];
}

export interface ResultBlock {
  id: string;
  order: number;
  component: ComponentType<ResultBlockProps>;
}

/** Registro acréscimo-apenas: uma linha por parte. */
export const resultBlocks: readonly ResultBlock[] = [
  { id: 'why-crashed', order: 200, component: WhyCrashed },
];

export function orderedBlocks(blocks: readonly ResultBlock[] = resultBlocks): ResultBlock[] {
  return [...blocks].sort((a, b) => a.order - b.order);
}
