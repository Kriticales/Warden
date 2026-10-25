/**
 * Sensor Sculk com vários provedores (ADR-0060 e ADR-0064; D-18), no app inteiro com o backend
 * simulado:
 *
 * - CA-T14-24: o aviso de plano gratuito (no consentimento e na espera do limite por minuto)
 *   aparece só com o Gemini; com a OpenAI e o Claude, o consentimento diz que o uso é cobrado na
 *   chave, e as frases de limite não falam em plano gratuito;
 * - CA-T14-20 (tela): o consentimento mostra o nome do provedor e o modelo do modo escolhido;
 * - CA-T14-21 (telas): a conversa mostra o provedor em que começou; sem a chave dele, pede essa
 *   chave e leva a Configurações, e o erro `AI_PROVIDER_KEY_MISSING` diz qual chave falta;
 * - a seção com o provedor ativo sem chave pede a chave dele, citando os três provedores;
 * - o modo por pergunta (ADR-0064): "Procurar rápido | Procurar a fundo" na Nova conversa e no
 *   campo de pergunta, só no Claude e na OpenAI, indo em `mode` para o backend; no Gemini não
 *   aparece e `mode` vai nulo;
 * - os erros na frase do provedor da conversa.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AiProvider, SearchMode, SecretsStatus, TurnFailure } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import type { Handler } from '../../../test/backend';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { fakeAiHandlers, TEST_MODELS } from '../../settings/ai/testing';
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
  curseforge: false,
  gemini: true,
  openai: true,
  anthropic: true,
  github: false,
};

/** O modelo que o backend usa em cada modo (nos testes, os de `TEST_MODELS`). */
function modelOf(provider: AiProvider, mode: SearchMode | null | undefined): string {
  const models = TEST_MODELS[provider];
  return (mode === 'quick' ? models.economical : models.careful) ?? '';
}

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
      // Como o backend: o modelo vem do modo da pergunta (no Gemini, `null`, o mais forte).
      ai_conversation_preview: ({ request }) => {
        const mode = (request as { mode?: SearchMode | null }).mode;
        return makePreview({ provider, model: modelOf(provider, mode), mode: mode ?? null });
      },
      ai_conversation_start: () => CONVERSATION_ID,
      ai_conversation_run: () => null,
      ai_conversation_get: () => makeView(),
      ai_conversation_send: () => null,
      ...handlers,
    },
  });
}

async function openNew(url: string) {
  renderApp(`${url}/ia`);
  await screen.findByRole('heading', { level: 2, name: 'Nova conversa' }, { timeout: 10_000 });
}

async function openConsent(url: string) {
  await openNew(url);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('radio', { name: /Trava ao entrar no mundo/ }));
  await user.click(await screen.findByRole('button', { name: 'Começar conversa' }));
  return screen.findByRole('dialog', { name: /Começar uma conversa com o Sculk\?/ });
}

async function openConversation(url: string) {
  renderApp(`${url}/ia/${CONVERSATION_ID}`);
  return screen.findByRole(
    'heading',
    { level: 2, name: makeSummary().title ?? '' },
    { timeout: 10_000 },
  );
}

function closestOf(element: HTMLElement, selector: string): HTMLElement {
  const found = element.closest<HTMLElement>(selector);
  if (!found) throw new Error(`sem ${selector} em volta`);
  return found;
}

function failure(code: string, params: Record<string, string> = {}): TurnFailure {
  return { domain: 'ai', code, params, detail: null, retryable: false };
}

describe('consentimento por provedor (CA-T14-20, CA-T14-24)', () => {
  it('Gemini: o provedor e o modelo mais forte (sempre, sem escolha de modo), o custo dele, o aviso do plano gratuito e o de poucas conversas por dia', async () => {
    const { url } = setup();
    const dialog = await openConsent(url);
    expect(
      within(dialog).getByText('Google Gemini (gemini-9.1-flash), com a sua chave'),
    ).toBeDefined();
    expect(within(dialog).getByText(/Entre US\$ 0,02 e US\$ 0,09/)).toBeDefined();
    expect(within(dialog).getByText(/No plano gratuito não há custo/)).toBeDefined();
    expect(within(dialog).getByText(/poucas conversas por dia/).textContent).toMatch(
      /troca sozinho para um modelo mais leve e avisa/,
    );
  });

  it('Gemini sem o Flash na conta: o custo do Flash-Lite, sem o aviso de poucas conversas', async () => {
    const lite = {
      economical: 'gemini-9.1-flash-lite',
      careful: 'gemini-9.1-flash-lite',
      lighter: 'gemini-9.1-flash-lite',
    };
    const { url } = setup({
      handlers: {
        ai_models_get: () => lite,
        ai_conversation_preview: () =>
          makePreview({ provider: 'gemini', model: lite.economical, mode: null }),
      },
    });
    const dialog = await openConsent(url);
    expect(
      within(dialog).getByText('Google Gemini (gemini-9.1-flash-lite), com a sua chave'),
    ).toBeDefined();
    expect(within(dialog).getByText(/Entre US\$ 0,005 e US\$ 0,04/)).toBeDefined();
    expect(dialog.textContent).not.toMatch(/poucas conversas por dia/);
  });

  it.each([
    ['openai', 'OpenAI (gpt-9-sol), com a sua chave', 'O uso é cobrado na sua chave da OpenAI'],
    [
      'anthropic',
      'Claude (claude-opus-9-9), com a sua chave',
      'O uso é cobrado na sua chave da Anthropic',
    ],
  ] as const)(
    '%s: o provedor e o modelo do modo "Procurar a fundo" (o proposto), a cobrança na chave e nada de plano gratuito nem números',
    async (provider, to, charged) => {
      const { url } = setup({ provider });
      const dialog = await openConsent(url);
      expect(within(dialog).getByText(to)).toBeDefined();
      expect(within(dialog).getByText(new RegExp(charged))).toBeDefined();
      expect(dialog.textContent).not.toMatch(/plano gratuito/);
      expect(dialog.textContent).not.toMatch(/US\$/);
      expect(dialog.textContent).not.toMatch(/Google|Anthropic\)/);
      expect(await axePage(document.body)).toHaveNoViolations();
    },
  );
});

describe('a conversa e o provedor em que começou (CA-T14-21, CA-T14-24)', () => {
  it('mostra o provedor no topo da conversa e em Detalhes', async () => {
    const { url } = setup({
      handlers: { ai_conversation_get: () => makeView({ provider: 'anthropic' }) },
    });
    await openConversation(url);
    expect(screen.getByText(/^Claude · começou /)).toBeDefined();
    await userEvent.setup().click(screen.getByRole('button', { name: /^Detalhes/ }));
    const side = screen.getByRole('complementary', { name: 'Detalhes' });
    expect(within(side).getByText('Provedor').nextElementSibling?.textContent).toBe('Claude');
  });

  it('CA-T14-25 (tela): a pergunta que usou o modelo mais leve avisa a troca e mostra o modelo; a que não usou, não', async () => {
    const { url } = setup({
      handlers: {
        ai_conversation_get: () =>
          makeView({
            model: 'gemini-9.1-flash',
            turns: [
              makeTurn({ fallbackModel: 'gemini-9.1-flash-lite' }),
              makeTurn({ question: 'E agora?' }),
            ],
          }),
      },
    });
    await openConversation(url);
    const notices = screen.getAllByText(
      /O limite diário do modelo mais forte acabou; esta resposta usou o mais leve\./,
    );
    expect(notices).toHaveLength(1);
    const answers = screen.getAllByRole('article', { name: 'Resposta do Sculk' });
    expect(answers[0]?.textContent).toContain('gemini-9.1-flash-lite');
    expect(answers[0]?.textContent).toContain(notices[0]?.textContent ?? '?');
    expect(answers[1]?.textContent).not.toContain('gemini-9.1-flash-lite');
    expect(answers[1]?.textContent).toContain('· gemini-9.1-flash');
    // O aviso também conta em Detalhes (mais o do plano gratuito do Gemini).
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Detalhes, 2 avisos' }));
    expect(
      within(screen.getByRole('complementary', { name: 'Detalhes' })).getByText(
        'O limite diário do modelo mais forte acabou; 1 resposta usou o mais leve (gemini-9.1-flash-lite).',
      ),
    ).toBeDefined();
  });

  it('D-15b (tela): a troca pelo tempo-limite diz o motivo dela, não o da cota', async () => {
    const { url } = setup({
      handlers: {
        ai_conversation_get: () =>
          makeView({
            model: 'gemini-9.1-flash',
            turns: [
              makeTurn({ fallbackModel: 'gemini-9.1-flash-lite', fallbackReason: 'timeout' }),
            ],
          }),
      },
    });
    await openConversation(url);
    const notice = screen.getByText(
      /O modelo mais forte demorou demais para responder; esta resposta usou o mais leve\./,
    );
    expect(closestOf(notice, 'article').textContent).toContain('gemini-9.1-flash-lite');
    expect(screen.queryByText(/O limite diário do modelo mais forte acabou/)).toBeNull();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /^Detalhes/ }));
    const side = screen.getByRole('complementary', { name: 'Detalhes' });
    expect(
      within(side).getByText(
        'O modelo mais forte demorou demais para responder; 1 resposta usou o mais leve (gemini-9.1-flash-lite).',
      ),
    ).toBeDefined();
    expect(within(side).queryByText(/O limite diário do modelo mais forte acabou/)).toBeNull();
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
    ).toBe('/configuracoes/chaves?chave=openai');
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
    ['gemini', 'QUOTA_EXCEEDED', /O limite diário do plano gratuito do Gemini acabou/],
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
  it('OpenAI ativa sem a chave dela: pede a chave da OpenAI, cita os três provedores e não consulta o Sculk', async () => {
    // ADR-0070: com outro provedor com chave, o Sculk usaria esse; aqui nenhum tem chave.
    const { backend, url } = setup({
      provider: 'openai',
      keys: { openai: false, gemini: false, anthropic: false },
    });
    renderApp(`${url}/ia`);
    expect(
      await screen.findByText('Falta a chave da OpenAI', {}, { timeout: 10_000 }),
    ).toBeDefined();
    expect(screen.getByText(/o sugerido é o Gemini, que tem plano gratuito/)).toBeDefined();
    expect(backend.callsOf('ai_models_get')).toHaveLength(0);
    expect(backend.callsOf('ai_conversation_preview')).toHaveLength(0);
  });

  it('Claude ativo com a chave: o provedor e o modelo de "Procurar a fundo" em uso, e a explicação da seção', async () => {
    const { url } = setup({ provider: 'anthropic' });
    renderApp(`${url}/ia`);
    expect(
      await screen.findByText(
        'Provedor: Claude. Modelo: claude-opus-9-9.',
        {},
        { timeout: 10_000 },
      ),
    ).toBeDefined();
    expect(screen.getByText(/O Sculk lê o travamento, o pack e os testes/)).toBeDefined();
  });
});

describe('o modo por pergunta: Procurar rápido | Procurar a fundo (ADR-0064)', () => {
  /** Os dois rádios do campo de pergunta, na ordem da tela. */
  function modeRadios(name = 'Como o Sculk procura') {
    const group = screen.getByRole('radiogroup', { name });
    const [quick, deep] = within(group).getAllByRole('radio');
    if (!quick || !deep) throw new Error('faltam os dois modos');
    return { group, quick, deep };
  }

  function claudeConversation(handlers: Record<string, Handler> = {}) {
    return setup({
      provider: 'anthropic',
      handlers: {
        ai_conversation_get: () => makeView({ provider: 'anthropic' }),
        ...handlers,
      },
    });
  }

  it('Claude, no campo de pergunta: os dois modos, "a fundo" proposto, as setas trocam e Enviar manda `mode` quick ou deep', async () => {
    const { backend, url } = claudeConversation();
    await openConversation(url);
    const { group, quick, deep } = modeRadios();
    expect(within(group).getByRole('radio', { name: 'Procurar rápido' })).toBe(quick);
    expect(within(group).getByRole('radio', { name: 'Procurar a fundo' })).toBe(deep);
    expect(quick.getAttribute('aria-checked')).toBe('false');
    expect(deep.getAttribute('aria-checked')).toBe('true');

    const user = userEvent.setup();
    const field = screen.getByRole('textbox', { name: 'Pergunte mais ao Sculk' });
    await user.type(field, 'primeira');
    // Seta para a esquerda: Procurar rápido, com o foco nele.
    deep.focus();
    await user.keyboard('{ArrowLeft}');
    expect(quick.getAttribute('aria-checked')).toBe('true');
    expect(deep.getAttribute('aria-checked')).toBe('false');
    expect(document.activeElement).toBe(quick);
    await user.click(screen.getByRole('button', { name: 'Enviar' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_send')).toHaveLength(1);
    });
    expect(backend.callsOf('ai_conversation_send')[0]?.args).toEqual({
      packId: expect.any(String) as string,
      conversationId: CONVERSATION_ID,
      text: 'primeira',
      mode: 'quick',
    });

    // A escolha fica para a pergunta seguinte; seta para a direita volta para "a fundo".
    await waitFor(() => {
      expect((field as HTMLTextAreaElement).value).toBe('');
    });
    expect(quick.getAttribute('aria-checked')).toBe('true');
    await user.type(field, 'segunda');
    quick.focus();
    await user.keyboard('{ArrowRight}');
    expect(deep.getAttribute('aria-checked')).toBe('true');
    await user.click(screen.getByRole('button', { name: 'Enviar' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_send')).toHaveLength(2);
    });
    expect(backend.callsOf('ai_conversation_send')[1]?.args).toMatchObject({
      text: 'segunda',
      mode: 'deep',
    });
  });

  it('Claude: o campo propõe o modo da última pergunta da conversa', async () => {
    const { url } = claudeConversation({
      ai_conversation_get: () =>
        makeView({
          provider: 'anthropic',
          turns: [makeTurn({ mode: 'deep' }), makeTurn({ question: 'E agora?', mode: 'quick' })],
        }),
    });
    await openConversation(url);
    const { quick, deep } = modeRadios();
    expect(quick.getAttribute('aria-checked')).toBe('true');
    expect(deep.getAttribute('aria-checked')).toBe('false');
  });

  it('OpenAI: o mesmo seletor e o mesmo `mode` no envio', async () => {
    const { backend, url } = setup({
      provider: 'openai',
      handlers: { ai_conversation_get: () => makeView({ provider: 'openai' }) },
    });
    await openConversation(url);
    const user = userEvent.setup();
    await user.type(screen.getByRole('textbox', { name: 'Pergunte mais ao Sculk' }), 'E agora?');
    await user.click(screen.getByRole('button', { name: 'Enviar' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_send')[0]?.args).toMatchObject({
        text: 'E agora?',
        mode: 'deep',
      });
    });
  });

  it('Gemini: o seletor não aparece (nem na conversa) e `mode` vai nulo', async () => {
    const { backend, url } = setup();
    await openConversation(url);
    expect(screen.queryByRole('radiogroup')).toBeNull();
    expect(screen.queryByRole('radio', { name: /Procurar/ })).toBeNull();
    const user = userEvent.setup();
    await user.type(screen.getByRole('textbox', { name: 'Pergunte mais ao Sculk' }), 'E agora?');
    await user.click(screen.getByRole('button', { name: 'Enviar' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_send')).toHaveLength(1);
    });
    expect(backend.callsOf('ai_conversation_send')[0]?.args.mode).toBeNull();
  });

  it('Claude, na Nova conversa: "Como procurar" com os dois modos; o modo vai no pedido de prévia e o consentimento mostra o modelo dele', async () => {
    const { backend, url } = setup({ provider: 'anthropic' });
    await openNew(url);
    const { quick, deep } = modeRadios('Como procurar');
    expect(deep.getAttribute('aria-checked')).toBe('true');
    expect(await screen.findByText('Provedor: Claude. Modelo: claude-opus-9-9.')).toBeDefined();
    expect(
      screen.getByText(
        'Modelo mais forte e mais consultas. Demora mais e custa mais na sua chave.',
      ),
    ).toBeDefined();

    const user = userEvent.setup();
    await user.click(quick);
    expect(quick.getAttribute('aria-checked')).toBe('true');
    expect(screen.getByText('Provedor: Claude. Modelo: claude-sonnet-9-9.')).toBeDefined();
    expect(
      screen.getByText('Modelo mais econômico e menos consultas. Bom para dúvidas simples.'),
    ).toBeDefined();

    await user.click(screen.getByRole('radio', { name: /Está lento/ }));
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    const dialog = await screen.findByRole('dialog', {
      name: /Começar uma conversa com o Sculk\?/,
    });
    expect(backend.callsOf('ai_conversation_preview')[0]?.args.request).toMatchObject({
      symptom: 'slow',
      mode: 'quick',
    });
    expect(within(dialog).getByText('Claude (claude-sonnet-9-9), com a sua chave')).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });

    // As setas trocam de modo também aqui; "a fundo" vai como `deep`.
    quick.focus();
    await user.keyboard('{ArrowRight}');
    expect(modeRadios('Como procurar').deep.getAttribute('aria-checked')).toBe('true');
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    await screen.findByRole('dialog');
    expect(backend.callsOf('ai_conversation_preview')[1]?.args.request).toMatchObject({
      mode: 'deep',
    });
  });

  it('OpenAI, na Nova conversa: o modo proposto é "a fundo" e vai no pedido de prévia', async () => {
    const { backend, url } = setup({ provider: 'openai' });
    await openNew(url);
    expect(modeRadios('Como procurar').deep.getAttribute('aria-checked')).toBe('true');
    expect(await screen.findByText('Provedor: OpenAI. Modelo: gpt-9-sol.')).toBeDefined();
    const user = userEvent.setup();
    await user.click(screen.getByRole('radio', { name: /Está lento/ }));
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    await screen.findByRole('dialog');
    expect(backend.callsOf('ai_conversation_preview')[0]?.args.request).toMatchObject({
      mode: 'deep',
    });
  });

  it('Gemini, na Nova conversa: o seletor some, a frase diz que o modelo é sempre o mesmo e `mode` vai nulo', async () => {
    const { backend, url } = setup();
    await openNew(url);
    expect(await screen.findByText(/No Gemini o Sculk usa sempre o mesmo modelo/)).toBeDefined();
    expect(screen.queryByRole('radiogroup', { name: 'Como procurar' })).toBeNull();
    expect(screen.queryByText('Como procurar')).toBeNull();
    expect(screen.queryByRole('radio', { name: /Procurar/ })).toBeNull();
    const user = userEvent.setup();
    await user.click(screen.getByRole('radio', { name: /Está lento/ }));
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    await screen.findByRole('dialog');
    const request = backend.callsOf('ai_conversation_preview')[0]?.args.request;
    expect(request).toMatchObject({ symptom: 'slow', mode: null });
  });
});
