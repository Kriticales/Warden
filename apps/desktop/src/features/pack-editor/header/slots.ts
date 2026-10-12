/**
 * Botões e indicadores do cabeçalho do pack (SPEC T05). **Registro acréscimo-apenas**
 * (ROADMAP §1): cada tarefa acrescenta (ou, se for a dona, troca) a sua linha.
 *
 * Lugares (`placement`):
 * - `actions`: à direita do nome, em ordem crescente de `order`. A P1-08 registra o
 *   "Salvar versão · N alterações" (`save`, que a V-02 trocou pelo botão que abre o diálogo
 *   Salvar versão) e o "▶ Testar ▾" provisório (`test`, que a L-04 troca pelo botão real com
 *   os estados "Testando… ver progresso" e "● Jogo aberto: ver teste").
 * - `alerts`: avisos passageiros abaixo do nome ("Mudanças do teste para revisar", da C-03;
 *   "O pack mudou desde o início do teste", da L-04). Cada componente devolve `null` quando
 *   não tem nada a avisar. A P1-08 registra o aviso de arquivos que não devem ir para os
 *   jogadores (higiene do T04), até a seção Exportar existir.
 *
 * Também registram aqui: V-02 (salvar), D-03 (indicador de problemas) e A-05.
 */
import type { ComponentType } from 'react';

import type { PackId, PackRow } from '../../../lib/ipc/bindings';
import { SaveVersionButton } from '../../versioning/components/SaveVersionButton';
import { HygieneAlert } from '../hygiene/HygieneAlert';
import { TestButton } from './TestButton';

/** O que todo componente do cabeçalho recebe. */
export interface HeaderSlotProps {
  packId: PackId;
  /** A linha do pack (nome, versões, alterações não salvas), lida do disco. */
  pack: PackRow;
}

export type HeaderSlotPlacement = 'actions' | 'alerts';

export interface HeaderSlot {
  /** Identificador estável (a tarefa dona troca a linha com o mesmo id). */
  id: string;
  placement: HeaderSlotPlacement;
  /** Posição dentro do lugar (crescente). */
  order: number;
  component: ComponentType<HeaderSlotProps>;
}

/** Registro acréscimo-apenas: uma linha por botão ou indicador. */
export const headerSlots: readonly HeaderSlot[] = [
  { id: 'save', placement: 'actions', order: 100, component: SaveVersionButton },
  { id: 'test', placement: 'actions', order: 200, component: TestButton },
  { id: 'hygiene', placement: 'alerts', order: 100, component: HygieneAlert },
];

/** Os componentes de um lugar, na ordem. */
export function slotsFor(
  placement: HeaderSlotPlacement,
  slots: readonly HeaderSlot[] = headerSlots,
): HeaderSlot[] {
  return slots.filter((slot) => slot.placement === placement).sort((a, b) => a.order - b.order);
}
