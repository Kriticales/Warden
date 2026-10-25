/**
 * A ordem de preferência dos provedores do Sensor Sculk (ADR-0070) e os modelos de cada
 * provedor para a tabela informativa de Configurações → Sensor Sculk. O provedor, a ordem e o
 * estado das chaves vêm de `ai_settings_get` (`aiKeys.settings`).
 */
import { useQuery, useQueryClient } from '@tanstack/react-query';

import { commands, type AiProvider } from '../../../lib/ipc/bindings';
import { commandQuery, useCommandMutation } from '../../../lib/ipc/query';
import { aiKeys } from '../../diagnostics/ai/api';

/** Os modelos de um provedor qualquer (sob `['app', 'ai']`: salvar uma chave invalida). */
export const modelsOfKey = (provider: AiProvider) => ['app', 'ai', 'models-of', provider] as const;

/** Os modelos que a chave de `provider` alcança (rede uma vez por execução e por chave). */
export function useModelsOf(provider: AiProvider, enabled: boolean) {
  return useQuery({
    ...commandQuery(modelsOfKey(provider), () => commands.aiModelsOf(provider)),
    enabled,
    staleTime: Infinity,
  });
}

/** Grava a ordem nova; a tela já mostra a ordem pedida enquanto o backend grava. */
export function useSetAiOrder() {
  const queryClient = useQueryClient();
  return useCommandMutation((order: AiProvider[]) => commands.aiOrderSet(order), {
    onSuccess: (view) => {
      queryClient.setQueryData(aiKeys.settings, view);
    },
    invalidates: [aiKeys.settings],
  });
}
