/**
 * Blocos do painel de detalhes do item (SPEC T07). **Registro acréscimo-apenas** (ROADMAP §1,
 * D4): cada tarefa acrescenta uma linha com o seu bloco, na posição dada por `order`.
 *
 * Os blocos da P1-08, de cima para baixo: cabeçalho (100), aviso de origem (150), descrição
 * (200), versão instalada e arquivo (300), lado (400) e novidades da versão (800). Reservados:
 * 500 para "Depende de / Usado por / Por que está no pack" (D-07) e 600 para "O que este mod
 * altera no jogo" (raio-x, D-10). Os da 1.1 (nota e grupos, segurança, manutenção) usam o mesmo
 * registro.
 *
 * Um bloco devolve `null` quando não tem o que mostrar para aquele item.
 */
import type { ComponentType } from 'react';

import type { InventoryItem, ItemDetails, PackId } from '../../../lib/ipc/bindings';
import { ChangelogBlock } from './blocks/ChangelogBlock';
import { DescriptionBlock } from './blocks/DescriptionBlock';
import { HeadBlock } from './blocks/HeadBlock';
import { OriginBlock } from './blocks/OriginBlock';
import { SideBlock } from './blocks/SideBlock';
import { VersionBlock } from './blocks/VersionBlock';

/** O que todo bloco recebe. */
export interface DetailBlockProps {
  packId: PackId;
  /** O item, da lista (sempre presente, mesmo sem internet). */
  item: InventoryItem;
  /** Os detalhes da fonte (`undefined` enquanto carregam ou se falharem). */
  details: ItemDetails | undefined;
}

export interface DetailBlock {
  /** Identificador estável. */
  id: string;
  /** Posição no painel (crescente). */
  order: number;
  component: ComponentType<DetailBlockProps>;
}

/** Registro acréscimo-apenas: uma linha por bloco. */
export const detailBlocks: readonly DetailBlock[] = [
  { id: 'cabecalho', order: 100, component: HeadBlock },
  { id: 'origem', order: 150, component: OriginBlock },
  { id: 'descricao', order: 200, component: DescriptionBlock },
  { id: 'versao', order: 300, component: VersionBlock },
  { id: 'lado', order: 400, component: SideBlock },
  { id: 'novidades', order: 800, component: ChangelogBlock },
];

/** Os blocos na ordem do painel. */
export function orderedBlocks(blocks: readonly DetailBlock[] = detailBlocks): DetailBlock[] {
  return [...blocks].sort((a, b) => a.order - b.order);
}
