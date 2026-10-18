/**
 * Chamadas das propostas da IA (ARCHITECTURE §4.1, domínio `ai`; ROADMAP D-14).
 *
 * Dados sempre atuais (QUALITY §4): Aplicar pode mudar a lista de Mods, as configs, o histórico
 * (o ponto de segurança), a linha do pack em Meus packs, o cabeçalho e as contagens, e os
 * ajustes do teste; por isso invalida **tudo do pack** (`['pack', id]`, que inclui a conversa)
 * e a lista de Meus packs, além do `pack-changed` que o backend manda. Descartar só muda a
 * conversa (e a contagem de propostas pendentes na lista de conversas).
 */
import { useQuery } from '@tanstack/react-query';

import { commands, type PackId } from '../../../../lib/ipc/bindings';
import { queryKeys } from '../../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../../lib/ipc/query';
import { packsKeys } from '../../../packs/api';
import { aiKeys } from '../api';

export const proposalKeys = {
  /** A diferença de uma proposta (relida a cada abertura do diálogo). */
  review: (packId: PackId, conversationId: string, proposalId: string) =>
    [...aiKeys.conversation(packId, conversationId), 'proposal', proposalId] as const,
};

/** A diferença exata (o diálogo de Aplicar). Nada é gravado. */
export function useProposalReview(
  packId: PackId,
  conversationId: string,
  proposalId: string,
  enabled: boolean,
) {
  return useQuery({
    ...commandQuery(proposalKeys.review(packId, conversationId, proposalId), () =>
      commands.aiProposalReview(packId, conversationId, proposalId),
    ),
    enabled,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

/** Tudo o que Aplicar pode mudar. */
export function appliedInvalidates(packId: PackId) {
  return [queryKeys.pack(packId), packsKeys.list] as const;
}

export function useApplyProposal(packId: PackId, conversationId: string) {
  return useCommandMutation(
    ({ proposalId, token }: { proposalId: string; token: string }) =>
      commands.aiProposalApply(packId, conversationId, proposalId, token),
    { invalidates: appliedInvalidates(packId) },
  );
}

export function useDiscardProposal(packId: PackId, conversationId: string) {
  return useCommandMutation(
    (proposalId: string) => commands.aiProposalDiscard(packId, conversationId, proposalId),
    { invalidates: [aiKeys.conversation(packId, conversationId), aiKeys.list(packId)] },
  );
}
