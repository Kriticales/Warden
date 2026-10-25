/**
 * Chamadas da conversa sem pack (ROADMAP D-19; SPEC T14 item 8; comandos `ai_standalone_*`).
 *
 * Chaves (dados sempre atuais, QUALITY §4): as conversas sem pack ficam no app, fora de qualquer
 * pack (`['app', 'ai', 'sem-pack', …]`). O `ai-standalone-updated` (cada pergunta, consulta,
 * resposta, erro ou espera) e cada mudança feita aqui (começar, perguntar, apagar) invalidam
 * **tudo** sob essa chave: a lista de conversas sem pack e a conversa aberta. O provedor, os
 * modelos e a chave vêm das chaves do Sensor Sculk com pack (`aiKeys.settings`, `aiKeys.models`).
 *
 * Como na conversa com pack, o laço demora: `ai_standalone_run` é chamado sem esperar, e a tela
 * acompanha pelos eventos.
 */
import { useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query';

import {
  commands,
  events,
  type AiPermissions,
  type AiStandalonePreviewRequest,
  type SearchMode,
} from '../../../lib/ipc/bindings';
import { useTauriEvent } from '../../../lib/ipc/events';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { unwrap } from '../../../lib/ipc/result';
import { log } from '../../../lib/log';

export const standaloneKeys = {
  /** Tudo das conversas sem pack. */
  all: ['app', 'ai', 'sem-pack'] as const,
  /** A lista "Conversas sem pack". */
  list: ['app', 'ai', 'sem-pack', 'list'] as const,
  /** Uma conversa. */
  conversation: (id: string) => ['app', 'ai', 'sem-pack', 'conversation', id] as const,
};

export function useStandaloneConversations() {
  return useQuery(commandQuery(standaloneKeys.list, commands.aiStandaloneList));
}

export function useStandaloneConversation(id: string) {
  return useQuery(
    commandQuery(standaloneKeys.conversation(id), () => commands.aiStandaloneGet(id)),
  );
}

/** Mantém a lista e as conversas sem pack atuais (eventos do backend). */
export function useStandaloneLiveUpdates(): void {
  const queryClient = useQueryClient();
  useTauriEvent(events.aiStandaloneUpdated, () => {
    void queryClient.invalidateQueries({ queryKey: standaloneKeys.all });
  });
}

/** "Escolher arquivo…" (o diálogo nativo). */
export function useChooseStandaloneLog() {
  return useCommandMutation(() => commands.aiStandaloneLogChoose());
}

/** O consentimento: baixa o link (se for o caso) e devolve o texto inicial exato. */
export function useStandalonePreview() {
  return useCommandMutation((request: AiStandalonePreviewRequest) =>
    commands.aiStandalonePreview(request),
  );
}

/** Roda a pergunta pendente sem esperar: o resultado chega pelos eventos. */
export function runStandaloneInBackground(queryClient: QueryClient, id: string): void {
  const run = commands.aiStandaloneRun(id);
  void run
    .then(unwrap)
    .catch((error: unknown) => {
      log.info('o Sculk parou com erro (gravado na conversa sem pack)', error);
    })
    .finally(() => {
      void queryClient.invalidateQueries({ queryKey: standaloneKeys.all });
    });
}

/** "Enviar e começar": grava a conversa e começa a rodar. Devolve o id. */
export function useStartStandalone() {
  const queryClient = useQueryClient();
  return useCommandMutation(
    ({ previewId, permissions }: { previewId: string; permissions: AiPermissions }) =>
      commands.aiStandaloneStart(previewId, permissions),
    {
      invalidates: [standaloneKeys.all],
      onSuccess: (id) => {
        runStandaloneInBackground(queryClient, id);
      },
    },
  );
}

/** Uma pergunta nova na conversa sem pack, com o modo dela (`null` no Gemini). */
export function useSendStandalone(id: string) {
  const queryClient = useQueryClient();
  return useCommandMutation(
    ({ text, mode }: { text: string; mode: SearchMode | null }) =>
      commands.aiStandaloneSend(id, text, mode),
    {
      invalidates: [standaloneKeys.all],
      onSuccess: () => {
        runStandaloneInBackground(queryClient, id);
      },
    },
  );
}

/** "Tentar de novo" depois de um erro ou de uma resposta interrompida. */
export function useRetryStandalone(id: string) {
  const queryClient = useQueryClient();
  return () => {
    runStandaloneInBackground(queryClient, id);
    void queryClient.invalidateQueries({ queryKey: standaloneKeys.all });
  };
}

export function useDeleteStandalone() {
  return useCommandMutation((id: string) => commands.aiStandaloneDelete(id), {
    invalidates: [standaloneKeys.all],
  });
}
