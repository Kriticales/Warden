/**
 * Backend simulado com estado para Chaves e contas › Sensor Sculk (D-18): o provedor ativo muda
 * como no Rust (`ai/settings.json`), e os modelos vêm do provedor ativo. O modo é escolhido a
 * cada pergunta (ADR-0064), não aqui. Junto com `mockSettingsBackend`, que guarda o estado das chaves.
 */
import type {
  AiProvider,
  AiSettingsView,
  ModelChoices,
  SecretsStatus,
} from '../../../lib/ipc/bindings';
import type { Handler } from '../../../test/backend';
import { AI_PROVIDERS } from './providers';

/** Os modelos de cada provedor nos testes (nomes visivelmente de teste). */
export const TEST_MODELS: Record<AiProvider, ModelChoices> = {
  gemini: {
    economical: 'gemini-9.1-flash',
    careful: 'gemini-9.1-flash',
    lighter: 'gemini-9.1-flash-lite',
  },
  openai: { economical: 'gpt-9-luna', careful: 'gpt-9-sol' },
  anthropic: { economical: 'claude-sonnet-9-9', careful: 'claude-opus-9-9' },
};

export interface FakeAiState {
  provider: AiProvider;
}

export function fakeAiHandlers(
  secrets: () => SecretsStatus,
  initial: Partial<FakeAiState> = {},
): { state: FakeAiState; handlers: Record<string, Handler> } {
  const state: FakeAiState = {
    provider: 'gemini',
    ...initial,
  };
  const view = (): AiSettingsView => {
    const providers = AI_PROVIDERS.map((provider) => ({
      provider,
      keyConfigured: secrets()[provider],
      models: null,
    }));
    const active = providers.find((item) => item.provider === state.provider) ?? providers[0];
    if (!active) throw new Error('sem provedor');
    return { ...active, providers };
  };
  return {
    state,
    handlers: {
      ai_settings_get: view,
      ai_provider_set: ({ provider }) => {
        state.provider = provider as AiProvider;
        return view();
      },
      ai_models_get: () => TEST_MODELS[state.provider],
    },
  };
}
