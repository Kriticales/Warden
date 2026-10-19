/**
 * Chamadas do ✦ Diagnóstico com IA (ARCHITECTURE §4.1, domínio `ai`; ROADMAP D-04).
 *
 * Chaves (dados sempre atuais, QUALITY §4):
 * - as conversas ficam sob o pack (`['pack', id, 'ai', …]`): o `ai-conversation-updated` (cada
 *   pergunta, consulta, resposta, erro ou espera) invalida a lista e a conversa; o `pack-changed`
 *   de qualquer área também, porque a conversa mostra "o pack mudou desde esta conversa";
 * - as origens ("Começar pelo") ficam sob as sessões do Testar (`testKeys.sessions`): o fim de um
 *   teste, que invalida as sessões, invalida também as origens;
 * - o provedor, a escolha do modelo e os modelos ficam no app (`['app', 'ai', …]`), os modelos
 *   por provedor (D-18: trocar de provedor nunca mostra os modelos do outro); as chaves vêm do
 *   estado das chaves de Configurações (`settingsKeys.secrets`). Salvar, substituir ou remover
 *   uma chave invalida também `['app', 'ai']` (`settings/api.ts`): o "tem chave" e os modelos.
 *
 * O laço demora (de segundos a minutos): `ai_conversation_run` é chamado sem esperar, e a tela
 * acompanha pelos eventos. A conversa continua mesmo se a pessoa sair da tela.
 */
import { useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query';

import {
  commands,
  events,
  type AiPermissions,
  type AiProvider,
  type AiPreviewRequest,
  type ModelTier,
  type PackId,
} from '../../../lib/ipc/bindings';
import { useTauriEvent } from '../../../lib/ipc/events';
import { queryKeys } from '../../../lib/ipc/keys';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { unwrap } from '../../../lib/ipc/result';
import { log } from '../../../lib/log';
import { AI_APP_KEY } from '../../settings/api';
import { testKeys } from '../../test/api';

export const aiKeys = {
  /** Tudo da IA de um pack. */
  all: (packId: PackId) => [...queryKeys.pack(packId), 'ai'] as const,
  /** A lista de conversas. */
  list: (packId: PackId) => [...queryKeys.pack(packId), 'ai', 'list'] as const,
  /** Uma conversa. */
  conversation: (packId: PackId, id: string) =>
    [...queryKeys.pack(packId), 'ai', 'conversation', id] as const,
  /** As origens de "Começar pelo" (sob as sessões do Testar). */
  sources: (packId: PackId) => [...testKeys.sessions(packId), 'ai-sources'] as const,
  /** Tudo da IA no app (provedor, escolhas e modelos). */
  app: AI_APP_KEY,
  /** O provedor e a escolha do modelo de cada provedor. */
  settings: ['app', 'ai', 'settings'] as const,
  /** Os modelos que a chave de um provedor alcança. */
  models: (provider: AiProvider) => ['app', 'ai', 'models', provider] as const,
};

export function useAiSettings() {
  return useQuery(commandQuery(aiKeys.settings, commands.aiSettingsGet));
}

export function useSetAiTier() {
  const queryClient = useQueryClient();
  return useCommandMutation((tier: ModelTier) => commands.aiSettingsSet(tier), {
    onSuccess: (view) => {
      queryClient.setQueryData(aiKeys.settings, view);
    },
    invalidates: [aiKeys.settings],
  });
}

/**
 * Troca o provedor das conversas novas (D-18). As conversas já começadas continuam no delas.
 * A seção da IA e Configurações leem o provedor de `aiKeys.settings`, gravado aqui.
 */
export function useSetAiProvider() {
  const queryClient = useQueryClient();
  return useCommandMutation((provider: AiProvider) => commands.aiProviderSet(provider), {
    onSuccess: (view) => {
      queryClient.setQueryData(aiKeys.settings, view);
    },
    invalidates: [aiKeys.settings],
  });
}

/**
 * Os modelos do provedor ativo (rede: a lista de modelos da conta, uma vez por execução do app
 * e por chave). `provider` é o ativo: o backend consulta sempre o provedor ativo.
 */
export function useAiModels(provider: AiProvider | undefined, enabled: boolean) {
  return useQuery({
    ...commandQuery(aiKeys.models(provider ?? 'gemini'), () => commands.aiModelsGet(false)),
    enabled: enabled && provider !== undefined,
    staleTime: Infinity,
  });
}

export function useAiSources(packId: PackId) {
  return useQuery(commandQuery(aiKeys.sources(packId), () => commands.aiSourcesList(packId)));
}

export function useAiConversations(packId: PackId) {
  return useQuery(commandQuery(aiKeys.list(packId), () => commands.aiConversationsList(packId)));
}

export function useAiConversation(packId: PackId, id: string) {
  return useQuery(
    commandQuery(aiKeys.conversation(packId, id), () => commands.aiConversationGet(packId, id)),
  );
}

/** Mantém a lista e as conversas do pack atuais (eventos do backend). */
export function useAiLiveUpdates(packId: PackId): void {
  const queryClient = useQueryClient();
  useTauriEvent(events.aiConversationUpdated, (change) => {
    if (change.packId === packId) {
      void queryClient.invalidateQueries({ queryKey: aiKeys.list(packId) });
      void queryClient.invalidateQueries({
        queryKey: aiKeys.conversation(packId, change.conversationId),
      });
    }
  });
  useTauriEvent(events.packChanged, (change) => {
    if (change.packId === packId) {
      void queryClient.invalidateQueries({ queryKey: aiKeys.all(packId) });
    }
  });
}

/** "Um log ou crash report do computador…" (o diálogo nativo). */
export function useChooseLog(packId: PackId) {
  return useCommandMutation(() => commands.aiLogChoose(packId));
}

/** O consentimento: o texto inicial exato e o tamanho. */
export function useAiPreview(packId: PackId) {
  return useCommandMutation((request: AiPreviewRequest) =>
    commands.aiConversationPreview(packId, request),
  );
}

/**
 * Roda a pergunta pendente sem esperar: o resultado chega pelos eventos. Um erro fica gravado
 * na própria conversa; aqui só se registra e se relê.
 */
export function runInBackground(queryClient: QueryClient, packId: PackId, id: string): void {
  const run = commands.aiConversationRun(packId, id);
  void run
    .then(unwrap)
    .catch((error: unknown) => {
      log.info('a IA parou com erro (gravado na conversa)', error);
    })
    .finally(() => {
      void queryClient.invalidateQueries({ queryKey: aiKeys.all(packId) });
    });
}

/** "✦ Começar conversa": grava a conversa e começa a rodar. Devolve o id. */
export function useStartConversation(packId: PackId) {
  const queryClient = useQueryClient();
  return useCommandMutation(
    ({ previewId, permissions }: { previewId: string; permissions: AiPermissions }) =>
      commands.aiConversationStart(packId, previewId, permissions),
    {
      invalidates: [aiKeys.list(packId)],
      onSuccess: (id) => {
        runInBackground(queryClient, packId, id);
      },
    },
  );
}

/** Uma pergunta nova na conversa. */
export function useSendQuestion(packId: PackId, id: string) {
  const queryClient = useQueryClient();
  return useCommandMutation((text: string) => commands.aiConversationSend(packId, id, text), {
    invalidates: [aiKeys.conversation(packId, id), aiKeys.list(packId)],
    onSuccess: () => {
      runInBackground(queryClient, packId, id);
    },
  });
}

/** "Tentar de novo" depois de um erro ou de uma resposta interrompida. */
export function useRetryConversation(packId: PackId, id: string) {
  const queryClient = useQueryClient();
  return {
    retry: () => {
      runInBackground(queryClient, packId, id);
      void queryClient.invalidateQueries({ queryKey: aiKeys.conversation(packId, id) });
    },
  };
}

export function useDeleteConversation(packId: PackId) {
  return useCommandMutation((id: string) => commands.aiConversationDelete(packId, id), {
    invalidates: [aiKeys.all(packId)],
  });
}
