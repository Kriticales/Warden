/**
 * Encaixes da resposta da IA (registro acréscimo-apenas, ROADMAP §1). A D-04 mostra cada
 * proposta com evidência como cartão; a D-14 (propostas aplicáveis, `ai/proposals/`) acrescenta
 * aqui os botões **Aplicar** e **Descartar** de cada cartão, com a diferença exata.
 */
import type { ComponentType } from 'react';

import type { CheckedProposal, PackId } from '../../../lib/ipc/bindings';

export interface ProposalSlotProps {
  packId: PackId;
  conversationId: string;
  proposal: CheckedProposal;
  /** Posição da proposta na resposta. */
  index: number;
}

/** Ações de cada cartão de proposta. */
export const proposalSlots: readonly ComponentType<ProposalSlotProps>[] = [];
