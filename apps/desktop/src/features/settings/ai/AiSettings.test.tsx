/**
 * Chaves e contas › Sensor Sculk (SPEC T21 e T14; ADR-0060 e ADR-0064; D-18), com o backend
 * simulado com estado:
 *
 * - CA-T21-06: escolher o provedor (Gemini, o padrão, OpenAI ou Claude) e a chave de cada um
 *   com Testar, Substituir e Remover; o modelo deixou de ser escolhido aqui (o modo é escolhido
 *   a cada pergunta, no Sensor Sculk), então a seção só informa os modelos de "Procurar rápido"
 *   e "Procurar a fundo"; trocar de provedor e voltar mantém o provedor escolhido;
 * - no Gemini, a linha única "Usa sempre o modelo mais forte do plano gratuito", com o modelo e
 *   o mais leve da troca na cota diária (ou sem o mais leve, a frase sem nome);
 * - OpenAI e Claude: os dois modelos e a frase da cobrança por uso, sem números;
 * - dados atuais: salvar ou remover a chave relê o "tem chave" e os modelos da IA;
 * - o erro do Testar na frase do provedor; axe sem violações sérias.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AiProvider, SecretsStatus } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError, type Handler } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { mockSettingsBackend } from '../testing';
import { fakeAiHandlers, type FakeAiState } from './testing';

const NO_KEYS: SecretsStatus = {
  backend: 'keyring',
  curseforge: false,
  gemini: false,
  openai: false,
  anthropic: false,
  github: false,
};

function setup(
  keys: Partial<Record<AiProvider, boolean>> = {},
  ai: Partial<FakeAiState> = {},
  handlers: Record<string, Handler> = {},
) {
  let secrets = (): SecretsStatus => ({ ...NO_KEYS, ...keys });
  const fake = fakeAiHandlers(() => secrets(), ai);
  const backend = mockSettingsBackend(
    { secrets: { ...NO_KEYS, ...keys } },
    { ...fake.handlers, ...handlers },
  );
  secrets = () => backend.state.secrets;
  return { backend, fake };
}

async function openAi() {
  renderApp('/configuracoes');
  await screen.findByRole('heading', { level: 2, name: 'Geral' }, { timeout: 5000 });
  const keys = screen.getByRole('region', { name: 'Chaves e contas' });
  return within(keys).findByRole('region', { name: 'Sensor Sculk' });
}

describe('Chaves e contas › Sensor Sculk (D-18)', () => {
  it('Gemini padrão: os três provedores, a chave de cada um e a linha única do modelo mais forte do plano gratuito', async () => {
    const { backend } = setup({ gemini: true });
    const ai = await openAi();
    const providers = within(ai).getByRole('group', { name: 'Provedor do Sculk' });
    const gemini = within(providers).getByRole('radio', { name: /Gemini/ });
    await waitFor(() => {
      expect(gemini).toHaveProperty('checked', true);
    });
    expect(within(providers).getByRole('radio', { name: /OpenAI/ })).toHaveProperty(
      'checked',
      false,
    );
    expect(within(providers).getByRole('radio', { name: /Claude/ })).toBeDefined();
    expect(within(providers).getByText('Sugerido')).toBeDefined();
    expect(within(providers).getByText(/Tem plano gratuito pela API/)).toBeDefined();

    // A chave de cada provedor, na mesma linha das outras chaves.
    expect(within(ai).getByTestId('chave-gemini').textContent).toContain('Configurada');
    expect(within(ai).getByLabelText('Chave da OpenAI')).toBeDefined();
    expect(within(ai).getByLabelText('Chave do Claude')).toBeDefined();

    const line = within(ai).getByTestId('ia-modelo-gemini');
    expect(
      within(line).getByText('Usa sempre o modelo mais forte do plano gratuito'),
    ).toBeDefined();
    expect(await within(line).findByText(/Hoje: gemini-9\.1-flash\./)).toBeDefined();
    // O mais leve da troca na cota diária, pelo nome que veio do backend.
    expect(
      within(line).getByText(
        /troca sozinho para o mais leve \(gemini-9\.1-flash-lite\), só naquela pergunta, e avisa/,
      ),
    ).toBeDefined();
    // No Gemini não há modo para escolher, nem aqui nem como "duas opções" de modelo.
    expect(
      within(ai).queryByRole('radio', { name: /Procurar a fundo|Procurar rápido/ }),
    ).toBeNull();
    expect(within(ai).queryByText(/Procurar rápido/)).toBeNull();
    expect(backend.callsOf('ai_models_get')).toHaveLength(1);
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('Gemini sem o Flash na conta: só o Flash-Lite em uso, sem nome de modelo mais leve', async () => {
    setup(
      { gemini: true },
      {},
      {
        ai_models_get: () => ({
          economical: 'gemini-9.1-flash-lite',
          careful: 'gemini-9.1-flash-lite',
          lighter: 'gemini-9.1-flash-lite',
        }),
      },
    );
    const ai = await openAi();
    const line = await within(ai).findByTestId('ia-modelo-gemini');
    expect(await within(line).findByText(/Hoje: gemini-9\.1-flash-lite\./)).toBeDefined();
    expect(within(line).getByText(/troca sozinho para um modelo mais leve e avisa/)).toBeDefined();
    expect(line.textContent).not.toMatch(/só naquela pergunta/);
  });

  it('CA-T21-06: trocar para a OpenAI, ir ao Claude e voltar mantém o provedor; os dois modelos aparecem só como informação, sem escolha de modelo', async () => {
    const { backend, fake } = setup({ gemini: true, openai: true, anthropic: true });
    const ai = await openAi();
    const user = userEvent.setup();
    const providers = within(ai).getByRole('group', { name: 'Provedor do Sculk' });

    await user.click(within(providers).getByRole('radio', { name: /OpenAI/ }));
    expect(backend.callsOf('ai_provider_set')[0]?.args).toEqual({ provider: 'openai' });
    expect(await screen.findByText('Conversas novas vão usar OpenAI.')).toBeDefined();
    const openai = await within(ai).findByTestId('ia-modelos');
    expect(
      await within(openai).findByText(
        'Procurar rápido usa o gpt-9-luna; Procurar a fundo usa o gpt-9-sol. Você escolhe a cada pergunta, no Sensor Sculk.',
      ),
    ).toBeDefined();
    expect(
      within(openai).getByText(
        'A OpenAI cobra por uso, na sua conta, conforme os preços dela. Não há plano gratuito pela API.',
      ),
    ).toBeDefined();
    // O modelo não é mais escolhido aqui: nenhum rádio de modo ou de "Mais econômico".
    expect(within(openai).queryAllByRole('radio')).toHaveLength(0);

    await user.click(within(providers).getByRole('radio', { name: /Claude/ }));
    await waitFor(() => {
      expect(within(ai).getByTestId('ia-modelos').textContent).toMatch(/A Anthropic cobra por uso/);
    });
    const claude = within(ai).getByTestId('ia-modelos');
    expect(
      await within(claude).findByText(
        'Procurar rápido usa o claude-sonnet-9-9; Procurar a fundo usa o claude-opus-9-9. Você escolhe a cada pergunta, no Sensor Sculk.',
      ),
    ).toBeDefined();
    // Nenhum número de preço inventado.
    expect(claude.textContent).not.toMatch(/US\$|\d+,\d+/);

    await user.click(within(providers).getByRole('radio', { name: /OpenAI/ }));
    await waitFor(() => {
      expect(within(providers).getByRole('radio', { name: /OpenAI/ })).toHaveProperty(
        'checked',
        true,
      );
    });
    expect(
      await within(within(ai).getByTestId('ia-modelos')).findByText(
        /Procurar rápido usa o gpt-9-luna/,
      ),
    ).toBeDefined();
    expect(fake.state).toEqual({ provider: 'openai' });
    // Só o provedor é gravado em Configurações: a escolha de modelo saiu (ADR-0064).
    expect(backend.callsOf('ai_provider_set').map((call) => call.args)).toEqual([
      { provider: 'openai' },
      { provider: 'anthropic' },
      { provider: 'openai' },
    ]);
    expect(backend.callsOf('ai_settings_set')).toHaveLength(0);
  });

  it('sem a chave do provedor ativo: explica, não consulta os modelos; salvar a chave relê o estado e traz os modelos', async () => {
    const { backend } = setup({}, { provider: 'openai' });
    const ai = await openAi();
    const group = await within(ai).findByTestId('ia-modelos');
    expect(
      within(group).getByText('Os nomes dos modelos aparecem depois de salvar a chave da OpenAI.'),
    ).toBeDefined();
    expect(backend.callsOf('ai_models_get')).toHaveLength(0);

    const user = userEvent.setup();
    const row = within(ai).getByTestId('chave-openai');
    await user.type(within(row).getByLabelText('Chave da OpenAI'), 'sk-teste-falsa');
    await user.click(within(row).getByRole('button', { name: 'Salvar' }));
    expect(backend.callsOf('secrets_set')[0]?.args).toEqual({
      kind: 'openai',
      value: 'sk-teste-falsa',
    });
    expect(await within(group).findByText(/Procurar rápido usa o gpt-9-luna;/)).toBeDefined();
    expect(within(group).queryByText(/aparecem depois de salvar/)).toBeNull();
    // O "tem chave" da IA foi relido junto (dados atuais).
    expect(backend.callsOf('ai_settings_get').length).toBeGreaterThan(1);
  });

  it('remover a chave do provedor ativo volta a explicar que falta a chave', async () => {
    const { backend } = setup({ anthropic: true }, { provider: 'anthropic' });
    const ai = await openAi();
    const group = await within(ai).findByTestId('ia-modelos');
    expect(
      await within(group).findByText(/Procurar a fundo usa o claude-opus-9-9\./),
    ).toBeDefined();
    const user = userEvent.setup();
    await user.click(
      within(within(ai).getByTestId('chave-anthropic')).getByRole('button', { name: 'Remover' }),
    );
    const confirm = await screen.findByRole('alertdialog', { name: 'Remover a chave do Claude?' });
    await user.click(within(confirm).getByRole('button', { name: 'Remover chave' }));
    expect(
      await within(group).findByText(
        'Os nomes dos modelos aparecem depois de salvar a chave do Claude.',
      ),
    ).toBeDefined();
    expect(backend.callsOf('secrets_remove')[0]?.args).toEqual({ kind: 'anthropic' });
  });

  it('Testar com a chave recusada: a frase do provedor daquela chave', async () => {
    setup(
      { gemini: true, openai: true },
      {},
      {
        secrets_test: ({ kind }) =>
          kind === 'openai'
            ? ipcError(
                makeAppError({ domain: 'ai', code: 'KEY_INVALID' }, { params: { status: '401' } }),
              )
            : { status: 'valid' },
      },
    );
    const ai = await openAi();
    const user = userEvent.setup();
    await user.click(
      within(within(ai).getByTestId('chave-openai')).getByRole('button', { name: 'Testar' }),
    );
    expect(
      await within(ai).findByText(
        'A OpenAI recusou a chave. Confira a chave em Configurações → Chaves e contas.',
      ),
    ).toBeDefined();
    await user.click(
      within(within(ai).getByTestId('chave-gemini')).getByRole('button', { name: 'Testar' }),
    );
    expect(
      await within(within(ai).getByTestId('chave-gemini')).findByText('Funcionando'),
    ).toBeDefined();
  });
});
