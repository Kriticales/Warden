/**
 * Backend simulado com estado para a IA em Configurações (D-18; ADR-0070): a ordem de
 * preferência dos provedores e o provedor das conversas novas (o primeiro da ordem com a chave
 * funcionando, como no Rust). O modo é escolhido a cada pergunta (ADR-0064), não aqui. Junto
 * com `mockSettingsBackend`, que guarda o estado das chaves e o último teste.
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
  order: AiProvider[];
}

export function fakeAiHandlers(
  secrets: () => SecretsStatus,
  /** `provider`: esse provedor em primeiro na ordem (o jeito antigo de escolher um só). */
  { provider, ...initial }: Partial<FakeAiState> & { provider?: AiProvider } = {},
  /** Se a chave do provedor foi recusada no último teste. */
  refused: (provider: AiProvider) => boolean = () => false,
): { state: FakeAiState; handlers: Record<string, Handler> } {
  const state: FakeAiState = {
    order: provider
      ? [provider, ...AI_PROVIDERS.filter((item) => item !== provider)]
      : [...AI_PROVIDERS],
    ...initial,
  };
  const usable = (provider: AiProvider) => secrets()[provider] && !refused(provider);
  const current = () => state.order.find(usable) ?? state.order[0] ?? 'gemini';
  const view = (): AiSettingsView => {
    const providers = AI_PROVIDERS.map((provider) => ({
      provider,
      keyConfigured: secrets()[provider],
      keyRefused: refused(provider),
      models: null,
    }));
    const active = providers.find((item) => item.provider === current()) ?? providers[0];
    if (!active) throw new Error('sem provedor');
    return {
      provider: active.provider,
      keyConfigured: active.keyConfigured,
      models: active.models,
      providers,
      order: [...state.order],
    };
  };
  return {
    state,
    handlers: {
      ai_settings_get: view,
      ai_provider_set: ({ provider }) => {
        const chosen = provider as AiProvider;
        state.order = [chosen, ...state.order.filter((item) => item !== chosen)];
        return view();
      },
      ai_order_set: ({ order }) => {
        const asked = order as AiProvider[];
        state.order = [...asked, ...AI_PROVIDERS.filter((item) => !asked.includes(item))];
        return view();
      },
      ai_models_get: () => TEST_MODELS[current()],
      ai_models_of: ({ provider }) => TEST_MODELS[provider as AiProvider],
    },
  };
}
