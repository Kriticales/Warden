/**
 * ✦ Diagnóstico com IA com vários provedores (ADR-0060; D-18), no app inteiro com o backend
 * simulado:
 *
 * - CA-T14-24: o aviso de plano gratuito (no consentimento e na espera do limite por minuto)
 *   aparece só com o Gemini; com a OpenAI e o Claude, o consentimento diz que o uso é cobrado na
 *   chave, e as frases de limite não falam em plano gratuito;
 * - CA-T14-20 (tela): o consentimento mostra o nome do provedor e o modelo;
 * - CA-T14-21 (telas): a conversa mostra o provedor em que começou; sem a chave dele, pede essa
 *   chave e leva a Configurações, e o erro `AI_PROVIDER_KEY_MISSING` diz qual chave falta;
 * - a seção com o provedor ativo sem chave pede a chave dele, citando os três provedores;
 * - os erros na frase do provedor da conversa.
 */
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AiProvider, SecretsStatus, TurnFailure } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import type { Handler } from '../../../test/backend';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { fakeAiHandlers } from '../../settings/ai/testing';
import {
  CONVERSATION_ID,
  CRASH_STEP,
  makePreview,
  makeSources,
  makeSummary,
  makeTurn,
  makeView,
} from './ai.fixtures';

const ALL_KEYS: SecretsStatus = {
  backend: 'keyring',
  curseforge: false,
  gemini: true,
  openai: true,
  anthropic: true,
  github: false,
};

const MODEL: Record<AiProvider, string> = {
  gemini: 'gemini-9.1-flash-lite',
  openai: 'gpt-9-luna',
  anthropic: 'claude-sonnet-9-9',
};

function setup({
  provider = 'gemini',
  keys = {},
  handlers = {},
}: {
  provider?: AiProvider;
  keys?: Partial<SecretsStatus>;
  handlers?: Record<string, Handler>;
} = {}) {
  const secrets: SecretsStatus = { ...ALL_KEYS, ...keys };
  const fake = fakeAiHandlers(() => secrets, { provider });
  return editorBackend({
    handlers: {
      ...fake.handlers,
      secrets_status: () => secrets,
      ai_sources_list: () => makeSources(),
      ai_conversations_list: () => [makeSummary()],
      ai_conversation_preview: () =>
        makePreview({ provider, model: MODEL[provider], tier: 'economical' }),
      ai_conversation_start: () => CONVERSATION_ID,
      ai_conversation_run: () => null,
      ai_conversation_get: () => makeView(),
      ...handlers,
    },
  });
}

async function openConsent(url: string) {
  renderApp(`${url}/ia`);
  await screen.findByRole('heading', { level: 1, name: /Diagnóstico com IA/ }, { timeout: 10_000 });
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: 'Começar conversa' }));
  return screen.findByRole('dialog', { name: 'Começar uma conversa com a IA?' });
}

async function openConversation(url: string) {
  renderApp(`${url}/ia/${CONVERSATION_ID}`);
  return screen.findByRole(
    'heading',
    { level: 1, name: makeSummary().title ?? '' },
    { timeout: 10_000 },
  );
}

function failure(code: string, params: Record<string, string> = {}): TurnFailure {
  return { domain: 'ai', code, params, detail: null, retryable: false };
}

describe('consentimento por provedor (CA-T14-20, CA-T14-24)', () => {
  it('Gemini: o provedor e o modelo, o custo típico e o aviso do plano gratuito', async () => {
    const { url } = setup();
    const dialog = await openConsent(url);
    expect(
      within(dialog).getByText('Google Gemini (gemini-9.1-flash-lite), com a sua chave'),
    ).toBeDefined();
    expect(within(dialog).getByText(/Entre US\$ 0,005 e US\$ 0,04/)).toBeDefined();
    expect(within(dialog).getByText(/No plano gratuito não há custo/)).toBeDefined();
  });

  it.each([
    ['openai', 'OpenAI (gpt-9-luna), com a sua chave', 'O uso é cobrado na sua chave da OpenAI'],
    [
      'anthropic',
      'Claude (Anthropic) (claude-sonnet-9-9), com a sua chave',
      'O uso é cobrado na sua chave da Anthropic',
    ],
  ] as const)(
    '%s: o provedor e o modelo, a cobrança na chave e nada de plano gratuito nem números',
    async (provider, to, charged) => {
      const { url } = setup({ provider });
      const dialog = await openConsent(url);
      expect(within(dialog).getByText(to)).toBeDefined();
      expect(within(dialog).getByText(new RegExp(charged))).toBeDefined();
      expect(dialog.textContent).not.toMatch(/plano gratuito/);
      expect(dialog.textContent).not.toMatch(/US\$/);
      expect(dialog.textContent).not.toMatch(/Google/);
      expect(await axePage(document.body)).toHaveNoViolations();
    },
  );
});

describe('a conversa e o provedor em que começou (CA-T14-21, CA-T14-24)', () => {
  it('mostra o provedor no subtítulo e na lateral', async () => {
    const { url } = setup({
      handlers: { ai_conversation_get: () => makeView({ provider: 'anthropic' }) },
    });
    await openConversation(url);
    expect(screen.getByText(/começou .* com Claude/)).toBeDefined();
    const side = screen.getByRole('complementary', { name: 'Sobre esta conversa' });
    expect(within(side).getByText('Provedor').nextElementSibling?.textContent).toBe('Claude');
  });

  it('sem a chave do provedor da conversa: pede essa chave e leva a Configurações', async () => {
    const { url } = setup({
      provider: 'gemini',
      keys: { openai: false },
      handlers: {
        ai_conversation_get: () =>
          makeView({
            provider: 'openai',
            summary: makeSummary({ state: 'failed' }),
            turns: [
              makeTurn(),
              makeTurn({
                question: 'E agora?',
                answer: null,
                failure: failure('PROVIDER_KEY_MISSING', {
                  provider: 'openai',
                  providerName: 'OpenAI',
                }),
              }),
            ],
          }),
      },
    });
    await openConversation(url);
    const ask = await screen.findByText(
      'Esta conversa começou com OpenAI e continua nesse provedor. Para perguntar de novo, salve a chave dele em Configurações → Chaves e contas.',
    );
    const notice = ask.closest<HTMLElement>('.alert');
    if (!notice) throw new Error('sem o aviso');
    expect(within(notice).getByText('Falta a chave da OpenAI')).toBeDefined();
    expect(
      within(notice).getByRole('link', { name: 'Abrir Configurações' }).getAttribute('href'),
    ).toBe('/configuracoes');
    const alert = screen.getByRole('alert');
    expect(
      within(alert).getByText(/Esta conversa começou com OpenAI e continua nesse provedor\./),
    ).toBeDefined();
    expect(within(alert).getByRole('link', { name: 'Abrir Configurações' })).toBeDefined();
  });

  it('com a chave do provedor da conversa: nenhum pedido de chave', async () => {
    const { url } = setup({
      keys: { gemini: false },
      handlers: { ai_conversation_get: () => makeView({ provider: 'openai' }) },
    });
    await openConversation(url);
    expect(screen.queryByText(/Falta a chave/)).toBeNull();
  });

  it.each([
    ['gemini', /Aguardando o limite por minuto do plano gratuito…/],
    [
      'openai',
      /Aguardando o limite por minuto da sua chave… A conversa continua sozinha em cerca de 12 s\./,
    ],
    ['anthropic', /Aguardando o limite por minuto da sua chave…/],
  ] as const)('%s: a espera do limite por minuto', async (provider, text) => {
    const { url } = setup({
      handlers: {
        ai_conversation_get: () =>
          makeView({
            provider,
            summary: makeSummary({ state: 'running' }),
            turns: [makeTurn({ answer: null, steps: [CRASH_STEP] })],
            live: { kind: 'waitingPerMinute', seconds: 12 },
          }),
      },
    });
    await openConversation(url);
    const status = screen.getByText(text);
    if (provider !== 'gemini') expect(status.textContent).not.toMatch(/plano gratuito/);
  });

  it.each([
    ['gemini', 'O Google está sobrecarregado agora. Tentando de novo sozinho (2 de 3)…'],
    ['openai', 'A OpenAI está sobrecarregada agora. Tentando de novo sozinho (2 de 3)…'],
    ['anthropic', 'A Anthropic está sobrecarregada agora. Tentando de novo sozinho (2 de 3)…'],
  ] as const)('%s: a nova tentativa fala do provedor da conversa', async (provider, text) => {
    const { url } = setup({
      handlers: {
        ai_conversation_get: () =>
          makeView({
            provider,
            summary: makeSummary({ state: 'running' }),
            turns: [makeTurn({ answer: null })],
            live: { kind: 'retrying', attempt: 2, of: 3 },
          }),
      },
    });
    await openConversation(url);
    expect(screen.getByText(text)).toBeDefined();
  });

  it.each([
    ['gemini', 'QUOTA_EXCEEDED', /O limite diário do plano gratuito deste modelo acabou\./],
    ['openai', 'QUOTA_EXCEEDED', /A cota da sua conta da OpenAI acabou/],
    ['anthropic', 'RATE_LIMITED', /O limite por minuto da sua chave da Anthropic continuou/],
    ['anthropic', 'KEY_INVALID', /A Anthropic recusou a chave do Claude\./],
  ] as const)('%s: o erro %s na frase do provedor', async (provider, code, text) => {
    const { url } = setup({
      handlers: {
        ai_conversation_get: () =>
          makeView({
            provider,
            summary: makeSummary({ state: 'failed' }),
            turns: [makeTurn({ answer: null, failure: failure(code, { status: '401' }) })],
          }),
      },
    });
    await openConversation(url);
    const alert = screen.getByRole('alert');
    expect(within(alert).getByText(text)).toBeDefined();
    if (provider !== 'gemini') expect(alert.textContent).not.toMatch(/plano gratuito|Google/);
  });
});

describe('a seção com o provedor ativo', () => {
  it('OpenAI ativa sem a chave dela: pede a chave da OpenAI, cita os três provedores e não consulta a IA', async () => {
    const { backend, url } = setup({ provider: 'openai', keys: { openai: false } });
    renderApp(`${url}/ia`);
    expect(
      await screen.findByText('Falta a chave da OpenAI', {}, { timeout: 10_000 }),
    ).toBeDefined();
    expect(screen.getByText(/o sugerido é o Gemini, que tem plano gratuito/)).toBeDefined();
    expect(backend.callsOf('ai_models_get')).toHaveLength(0);
    expect(backend.callsOf('ai_conversation_preview')).toHaveLength(0);
  });

  it('Claude ativo com a chave: o provedor e o modelo em uso', async () => {
    const { url } = setup({ provider: 'anthropic' });
    renderApp(`${url}/ia`);
    expect(
      await screen.findByText(
        'Provedor: Claude. Modelo: claude-sonnet-9-9 (mais econômico).',
        {},
        { timeout: 10_000 },
      ),
    ).toBeDefined();
    expect(
      screen.getByText(/Converse com a IA \(Claude\) sobre um problema do pack/),
    ).toBeDefined();
  });
});
