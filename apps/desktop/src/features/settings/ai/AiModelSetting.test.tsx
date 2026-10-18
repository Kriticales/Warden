/**
 * O modelo da IA em Configurações → Chaves e contas (SPEC T14 ✦; ADR-0055): "Mais econômico"
 * (padrão) e "Análise mais cuidadosa", com os nomes vindos do `models.list` da chave e o aviso
 * de poucas conversas por dia; a escolha é gravada e a tela mostra o valor gravado.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AiSettingsView } from '../../../lib/ipc/bindings';
import { renderApp } from '../../../test/render';
import { mockSettingsBackend } from '../testing';

async function openKeys() {
  renderApp('/configuracoes');
  await screen.findByRole('heading', { level: 2, name: 'Geral' }, { timeout: 5000 });
  return screen.getByRole('region', { name: 'Chaves e contas' });
}

describe('Modelo da IA em Configurações', () => {
  it('com a chave: os modelos da conta e a troca para "Análise mais cuidadosa"', async () => {
    let view: AiSettingsView = {
      provider: 'gemini',
      tier: 'economical',
      keyConfigured: true,
      models: null,
      providers: [],
    };
    const backend = mockSettingsBackend(
      {
        secrets: {
          backend: 'keyring',
          curseforge: false,
          gemini: true,
          openai: false,
          anthropic: false,
          github: false,
        },
      },
      {
        ai_settings_get: () => view,
        ai_settings_set: ({ tier }) => {
          view = { ...view, tier: tier as AiSettingsView['tier'] };
          return view;
        },
        ai_models_get: () => ({ economical: 'gemini-9.1-flash-lite', careful: 'gemini-9.2-flash' }),
      },
    );
    const keys = await openKeys();
    const group = await within(keys).findByRole('group', { name: 'Modelo da IA (Gemini)' });
    const economical = await within(group).findByRole('radio', {
      name: /Mais econômico \(padrão\)/,
    });
    await waitFor(() => {
      expect(economical).toHaveProperty('checked', true);
    });
    expect(await within(group).findByText(/Hoje: gemini-9\.1-flash-lite\./)).toBeDefined();
    expect(within(group).getByText(/permite poucas conversas por dia/)).toBeDefined();
    const user = userEvent.setup();
    await user.click(within(group).getByRole('radio', { name: /Análise mais cuidadosa/ }));
    await waitFor(() => {
      expect(within(group).getByRole('radio', { name: /Análise mais cuidadosa/ })).toHaveProperty(
        'checked',
        true,
      );
    });
    expect(backend.callsOf('ai_settings_set')[0]?.args).toEqual({ tier: 'careful' });
  });

  it('sem a chave: não consulta os modelos e explica', async () => {
    const backend = mockSettingsBackend();
    const keys = await openKeys();
    const group = await within(keys).findByRole('group', { name: 'Modelo da IA (Gemini)' });
    expect(
      await within(group).findByText(
        'Os nomes dos modelos aparecem depois de salvar a chave do Gemini.',
      ),
    ).toBeDefined();
    expect(backend.callsOf('ai_models_get')).toHaveLength(0);
  });
});
