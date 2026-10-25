/**
 * Analisar um log, a conversa com o Sculk sem pack (SPEC T14 item 8; ROADMAP D-19; ADR-0064), no
 * app inteiro com o backend simulado: a entrada por Meus packs, a falta de chave, o link e o
 * arquivo, as frases de recusa dos links, o consentimento (texto exato, cancelar sem mandar
 * nada, foco de volta, "Enviar e começar"), o modo por pergunta (Claude e OpenAI; no Gemini
 * `mode` vai nulo), a conversa sem propostas nem busca do culpado, a gaveta Detalhes só com o que
 * existe sem pack, os dados sempre atuais pelos eventos e apagar. Os critérios estão no nome de
 * cada teste.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AiPreview, AiProvider, ErrorCode, SecretsStatus } from '../../../lib/ipc/bindings';
import { axePage } from '../../../test/axe';
import { ipcError, mockBackend, type Handler } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import {
  CONVERSATION_ID,
  makeAnswer,
  makeChecked,
  makeSummary,
  makeTurn,
  makeView,
} from '../ai/ai.fixtures';

const LINK_TEXT =
  'Sintoma: o jogo trava ao entrar no mundo.\n\nSem pack: a pessoa quer entender um log avulso.\n\nAnalisar: um log aberto por um link do mclo.gs (4 linhas).\nL4: java.lang.NoSuchMethodError em C:\\Users\\<usuário>\\mods';

function makeLinkPreview(overrides: Partial<AiPreview> = {}): AiPreview {
  return {
    previewId: '01K7PREVIEWSEMPACKAAAAAAAA',
    text: LINK_TEXT,
    bytes: new TextEncoder().encode(LINK_TEXT).length,
    estimatedTokens: 2_100,
    provider: 'gemini',
    model: 'gemini-9.1-flash-lite',
    mode: null,
    githubAvailable: true,
    ...overrides,
  };
}

const LINK_SUMMARY = makeSummary({
  title: 'O jogo travou num NoSuchMethodError.',
  origin: { kind: 'pasteLink', site: 'mclogs', url: 'https://mclo.gs/abc123' },
  tokens: 4_000,
});

const LINK_STEP = {
  callId: 'call_1',
  name: 'search_log',
  args: '{"query":"NoSuchMethodError","session_id":"link"}',
  sent: '{"functionResponse":{"id":"call_1","name":"search_log","response":{"itens":[{"id":"log:link/link#L4","texto":"java.lang.NoSuchMethodError"}]}}}',
  ok: true,
  denied: false,
};

function linkView() {
  return makeView({
    summary: LINK_SUMMARY,
    permissions: { configs: false, github: true },
    turns: [
      makeTurn({
        question: LINK_TEXT,
        steps: [LINK_STEP],
        // Mesmo que a resposta traga propostas e "sem conclusão", sem pack nada disso vira ação.
        answer: makeAnswer({ checked: makeChecked({ noConclusion: true }) }),
      }),
    ],
  });
}

const KEYS: SecretsStatus = {
  curseforge: false,
  gemini: true,
  openai: true,
  anthropic: true,
  github: false,
};

function setup(
  handlers: Record<string, Handler> = {},
  keyConfigured = true,
  provider: AiProvider = 'gemini',
) {
  return mockBackend({
    secrets_status: () => ({ ...KEYS, [provider]: keyConfigured }),
    ai_settings_get: () => ({
      provider,
      keyConfigured,
      models: null,
      providers: [],
    }),
    ai_models_get: () => ({ economical: 'claude-sonnet-9-9', careful: 'claude-opus-9-9' }),
    ai_standalone_list: () => [LINK_SUMMARY],
    ai_standalone_preview: () => makeLinkPreview(),
    ai_standalone_start: () => CONVERSATION_ID,
    ai_standalone_run: () => null,
    ai_standalone_get: () => linkView(),
    ai_standalone_delete: () => null,
    ai_standalone_send: () => null,
    ai_standalone_log_choose: () => ({
      token: '01K7TOKENAAAAAAAAAAAAAAAAA',
      fileName: 'crash-2026.txt',
      bytes: 2048,
    }),
    ...handlers,
  });
}

async function openPage() {
  renderApp('/analisar-log');
  return screen.findByRole('heading', { level: 2, name: 'Nova análise' }, { timeout: 10_000 });
}

async function openConversation() {
  renderApp(`/analisar-log/${CONVERSATION_ID}`);
  return screen.findByRole(
    'heading',
    { level: 2, name: 'O jogo travou num NoSuchMethodError.' },
    { timeout: 10_000 },
  );
}

/** Escolhe o sintoma e cola o link: o botão Começar conversa fica pronto. */
async function fillLink(
  user: ReturnType<typeof userEvent.setup>,
  url = 'https://mclo.gs/abc123',
  symptom = /Trava ao abrir o jogo/,
) {
  await user.click(screen.getByRole('radio', { name: symptom }));
  await user.type(screen.getByRole('textbox', { name: 'Link do log' }), url);
  return screen.getByRole('button', { name: 'Começar conversa' });
}

function linkError(code: string) {
  return () =>
    ipcError(makeAppError({ domain: 'app', code } as ErrorCode, { params: { site: 'mclo.gs' } }));
}

describe('Analisar um log: sem pack (T14 item 8)', () => {
  it('CA-T14-26: Meus packs tem o botão Analisar um log, que abre a página sem pack', async () => {
    setup();
    renderApp('/');
    const user = userEvent.setup();
    const entry = await screen.findByRole('link', { name: 'Analisar um log' }, { timeout: 10_000 });
    expect(entry.getAttribute('href')).toBe('/analisar-log');
    await user.click(entry);
    expect(await screen.findByRole('heading', { level: 2, name: 'Nova análise' })).toBeDefined();
    // A lista de conversas sem pack é a mesma coluna do Sensor Sculk.
    const aside = await screen.findByRole('complementary', { name: 'Conversas' });
    expect(
      await within(aside).findByRole('link', { name: /^O jogo travou num NoSuchMethodError\./ }),
    ).toBeDefined();
  });

  it('sem a chave do provedor: o caminho para Configurações, sem pedir a prévia; as conversas continuam à vista', async () => {
    const backend = setup({}, false);
    await openPage();
    const link = await screen.findByRole('link', { name: 'Abrir Configurações' });
    expect(link.getAttribute('href')).toBe('/configuracoes/chaves?chave=gemini');
    expect(screen.getByText('Falta a chave do Gemini')).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Começar conversa' })).toBeNull();
    expect(backend.callsOf('ai_standalone_preview')).toHaveLength(0);
    expect(backend.callsOf('ai_models_get')).toHaveLength(0);
    // Antes a lista nem era pedida sem a chave; agora ela fica na coluna da esquerda, com o
    // rodapé dizendo que o provedor está sem chave.
    const aside = await screen.findByRole('complementary', { name: 'Conversas' });
    expect(await within(aside).findByRole('link', { name: /^O jogo travou/ })).toBeDefined();
    expect(within(aside).getByText(/sem chave salva\./)).toBeDefined();
  });

  it('CA-T14-26: link de outro site, http:// e hastebin mostram a frase própria', async () => {
    let code = 'AI_LINK_UNSUPPORTED';
    const backend = setup({ ai_standalone_preview: () => linkError(code)() });
    await openPage();
    const user = userEvent.setup();
    const field = screen.getByRole('textbox', { name: 'Link do log' });
    // Sem sintoma e sem link, Começar conversa espera.
    expect(
      screen.getByRole('button', { name: 'Começar conversa' }).getAttribute('aria-disabled'),
    ).toBe('true');
    await user.click(screen.getByRole('radio', { name: /Trava ao abrir o jogo/ }));
    expect(
      screen.getByRole('button', { name: 'Começar conversa' }).getAttribute('aria-disabled'),
    ).toBe('true');
    await user.type(field, 'https://example.com/abc');
    const begin = screen.getByRole('button', { name: 'Começar conversa' });
    expect(begin.getAttribute('aria-disabled')).toBe('false');
    await user.click(begin);
    expect(
      await screen.findByText(
        'O Warden só abre links do mclo.gs e do pastebin. Escolha o arquivo do log ou cole um link de um desses sites.',
      ),
    ).toBeDefined();
    expect(backend.callsOf('ai_standalone_preview')[0]?.args).toMatchObject({
      request: { source: { kind: 'link', url: 'https://example.com/abc' } },
    });
    expect(screen.queryByRole('dialog')).toBeNull();

    code = 'AI_LINK_NEEDS_KEY';
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    expect(await screen.findByText(/O hastebin exige uma chave para ler os textos/)).toBeDefined();
    code = 'AI_LINK_NOT_HTTPS';
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    expect(await screen.findByText('Use o link do mclo.gs que começa com https://.')).toBeDefined();
    expect(backend.callsOf('ai_standalone_start')).toHaveLength(0);
  });

  it('CA-T14-28: o consentimento mostra o texto exato; Cancelar não manda nada e devolve o foco; axe', async () => {
    const backend = setup();
    await openPage();
    const user = userEvent.setup();
    // Axe na Nova análise.
    expect(await axePage(document.body)).toHaveNoViolations();
    const begin = await fillLink(user, ' https://mclo.gs/abc123 ');
    await user.click(begin);
    const dialog = await screen.findByRole('dialog', {
      name: /Começar uma conversa com o Sculk\?/,
    });
    expect(backend.callsOf('ai_standalone_preview')[0]?.args).toMatchObject({
      request: {
        symptom: 'crashOnStart',
        source: { kind: 'link', url: 'https://mclo.gs/abc123' },
        note: null,
        mode: null,
      },
    });
    expect(within(dialog).getByLabelText('Texto inicial que será enviado').textContent).toBe(
      LINK_TEXT,
    );
    // Sem pack, só o log vale "Sempre": nada de pack, achados nem medidas de desempenho.
    const log = within(dialog).getByText('O log que você escolheu');
    expect(within(closestListItem(log)).getByText('Sempre')).toBeDefined();
    expect(within(dialog).getAllByText('Sempre')).toHaveLength(1);
    expect(within(dialog).queryByText('Medidas de desempenho dos testes')).toBeNull();
    expect(within(dialog).getByText(/não faz propostas de mudança/)).toBeDefined();
    // Sem pack não há caixa de configs; a busca no GitHub continua desmarcável.
    expect(within(dialog).queryByRole('checkbox', { name: /Configs do pack/ })).toBeNull();
    expect(
      within(dialog).getByRole('checkbox', { name: /Procurar problemas parecidos/ }),
    ).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Enviar e começar' })).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(document.activeElement).toBe(begin);
    expect(backend.callsOf('ai_standalone_start')).toHaveLength(0);
    expect(backend.callsOf('ai_standalone_run')).toHaveLength(0);
  });

  it('arquivo do computador: escolher, começar com o token e abrir a conversa', async () => {
    const backend = setup();
    await openPage();
    const user = userEvent.setup();
    await user.click(screen.getByRole('radio', { name: /Trava ao entrar no mundo/ }));
    await user.click(screen.getByRole('radio', { name: /Um arquivo do computador/ }));
    await user.click(screen.getByRole('button', { name: 'Escolher arquivo…' }));
    expect(await screen.findByText('crash-2026.txt')).toBeDefined();
    expect(
      screen.getByText(/KB\. Antes de ir para o Sculk, o texto passa pela mesma limpeza/),
    ).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    const dialog = await screen.findByRole('dialog');
    expect(backend.callsOf('ai_standalone_preview')[0]?.args).toMatchObject({
      request: { source: { kind: 'localFile', token: '01K7TOKENAAAAAAAAAAAAAAAAA' } },
    });
    await user.click(
      within(dialog).getByRole('checkbox', { name: /Procurar problemas parecidos/ }),
    );
    await user.click(within(dialog).getByRole('button', { name: 'Enviar e começar' }));
    expect(
      await screen.findByRole('heading', {
        level: 2,
        name: 'O jogo travou num NoSuchMethodError.',
      }),
    ).toBeDefined();
    expect(backend.callsOf('ai_standalone_start')[0]?.args).toMatchObject({
      previewId: '01K7PREVIEWSEMPACKAAAAAAAA',
      permissions: { configs: false, github: false },
    });
    await waitFor(() => {
      expect(backend.callsOf('ai_standalone_run')[0]?.args).toMatchObject({
        conversationId: CONVERSATION_ID,
      });
    });
  });

  it('CA-T14-29: a conversa sem pack mostra o que foi enviado, sem propostas nem busca do culpado; axe', async () => {
    setup();
    await openConversation();
    expect(screen.getByText(/Conversa sobre o log do link do mclo\.gs/)).toBeDefined();
    expect(
      screen.getByText(/busca por "NoSuchMethodError" no log do link/, { selector: '.tc__title' })
        .parentElement?.textContent,
    ).toContain('Enviado ao Sculk: busca por "NoSuchMethodError" no log do link');
    expect(screen.getByText(/O Sculk não encontrou a causa com o que consultou\./)).toBeDefined();
    expect(screen.getByText(/Conte mais ao Sculk ou analise o log dentro do pack/)).toBeDefined();
    expect(screen.queryByRole('link', { name: 'Encontrar o mod culpado' })).toBeNull();
    expect(screen.queryByText(/^\d+ propostas?$/)).toBeNull();
    expect(screen.queryByText(/Atualizar para a 1\.6\.7/)).toBeNull();
    // Sem pack, nenhum mod vira link para a lista de Mods.
    expect(screen.queryByRole('link', { name: "Create: Steam 'n' Rails" })).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('Detalhes sem pack: só o log e o GitHub no que o Sculk pode ler; o total do que foi enviado; axe', async () => {
    setup();
    await openConversation();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /^Detalhes/ }));
    const aside = screen.getByRole('complementary', { name: 'Detalhes' });
    const reads = closestListSection(within(aside).getByText('O Sculk pode ler'));
    expect(within(reads).getByText('Logs e crash reports')).toBeDefined();
    expect(within(reads).getByText('Páginas dos mods no GitHub')).toBeDefined();
    expect(within(reads).queryByText('Informações do pack e os achados do Warden')).toBeNull();
    expect(within(reads).queryByText('Medidas de desempenho dos testes')).toBeNull();
    expect(within(reads).queryByText('Configs do pack')).toBeNull();
    const github = closestListItem(within(reads).getByText('Páginas dos mods no GitHub'));
    expect(github.className).not.toContain('off');
    expect(within(github).getByText('Só o nome do mod e palavras do erro.')).toBeDefined();
    expect(within(aside).getByText('Tudo o que foi enviado')).toBeDefined();
    expect(within(aside).getByText('Total')).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('dados atuais: o ai-standalone-updated relê a lista (Lendo vira Respondida)', async () => {
    let state: 'running' | 'answered' = 'running';
    const backend = setup({
      ai_standalone_list: () => [{ ...LINK_SUMMARY, state }],
    });
    await openPage();
    const aside = await screen.findByRole('complementary', { name: 'Conversas' });
    expect(await within(aside).findByText('Lendo')).toBeDefined();
    state = 'answered';
    await backend.emit('ai-standalone-updated', { conversationId: CONVERSATION_ID });
    expect(await within(aside).findByText('Respondida')).toBeDefined();
  });

  it('dados atuais: a resposta que chega pelo evento aparece na conversa aberta', async () => {
    let answered = false;
    const backend = setup({
      ai_standalone_get: () => {
        const view = linkView();
        return answered
          ? view
          : {
              ...view,
              live: { kind: 'thinking' },
              turns: [{ ...view.turns[0], answer: null }],
            };
      },
    });
    await openConversation();
    expect(await screen.findByText('Sculk lendo…')).toBeDefined();
    answered = true;
    await backend.emit('ai-standalone-updated', { conversationId: CONVERSATION_ID });
    expect(
      await screen.findByText('O Steam n Rails 1.6.4 não funciona com o Create 6.'),
    ).toBeDefined();
    await waitFor(() => {
      expect(screen.queryByText('Sculk lendo…')).toBeNull();
    });
  });

  it.each([
    ['openai', 'gpt-9-sol', 'OpenAI', 'O uso é cobrado na sua chave da OpenAI'],
    ['anthropic', 'claude-opus-9-9', 'Claude', 'O uso é cobrado na sua chave da Anthropic'],
  ] as const)(
    'junção D-18 × D-19: o consentimento sem pack com %s diz o provedor e o modelo, sem plano gratuito',
    async (provider, model, name, charge) => {
      setup(
        {
          ai_standalone_preview: ({ request }) =>
            makeLinkPreview({
              provider,
              model,
              mode: (request as { mode?: 'quick' | 'deep' | null }).mode ?? null,
            }),
        },
        true,
        provider,
      );
      await openPage();
      const user = userEvent.setup();
      await user.click(await screen.findByRole('radio', { name: /Trava ao abrir o jogo/ }));
      await user.type(screen.getByRole('textbox', { name: 'Link do log' }), 'https://mclo.gs/abc');
      await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
      const dialog = await screen.findByRole('dialog', {
        name: /Começar uma conversa com o Sculk\?/,
      });
      expect(within(dialog).getByText(`${name} (${model}), com a sua chave`)).toBeDefined();
      expect(within(dialog).getByText(new RegExp(charge))).toBeDefined();
      expect(within(dialog).queryByText(/plano gratuito/)).toBeNull();
    },
  );

  it('modo por pergunta sem pack: no Claude, "Como procurar" vai em `mode` no pedido de prévia; no Gemini o seletor não aparece e `mode` vai nulo', async () => {
    // Claude: Procurar a fundo é o proposto; as setas trocam para Procurar rápido.
    const claude = setup({}, true, 'anthropic');
    const { unmount } = renderApp('/analisar-log');
    await screen.findByRole('heading', { level: 2, name: 'Nova análise' }, { timeout: 10_000 });
    const user = userEvent.setup();
    const group = await screen.findByRole('radiogroup', { name: 'Como procurar' });
    const [quick, deep] = within(group).getAllByRole('radio');
    expect(deep?.getAttribute('aria-checked')).toBe('true');
    expect(await screen.findByText('Provedor: Claude. Modelo: claude-opus-9-9.')).toBeDefined();
    deep?.focus();
    await user.keyboard('{ArrowLeft}');
    expect(quick?.getAttribute('aria-checked')).toBe('true');
    expect(screen.getByText('Provedor: Claude. Modelo: claude-sonnet-9-9.')).toBeDefined();
    const begin = await fillLink(user);
    await user.click(begin);
    await screen.findByRole('dialog');
    expect(claude.callsOf('ai_standalone_preview')[0]?.args).toMatchObject({
      request: { symptom: 'crashOnStart', mode: 'quick' },
    });
    unmount();

    // Gemini: nada para escolher.
    const gemini = setup();
    renderApp('/analisar-log');
    await screen.findByRole('heading', { level: 2, name: 'Nova análise' }, { timeout: 10_000 });
    expect(await screen.findByText(/No Gemini o Sculk usa sempre o mesmo modelo/)).toBeDefined();
    expect(screen.queryByRole('radiogroup', { name: 'Como procurar' })).toBeNull();
    await user.click(await fillLink(user));
    await screen.findByRole('dialog');
    expect(gemini.callsOf('ai_standalone_preview')[0]?.args).toMatchObject({
      request: { symptom: 'crashOnStart', mode: null },
    });
  });

  it('modo por pergunta sem pack, na conversa: Claude manda `mode` no envio e Gemini manda nulo', async () => {
    const claude = setup(
      { ai_standalone_get: () => ({ ...linkView(), provider: 'anthropic' as const }) },
      true,
      'anthropic',
    );
    const { unmount } = renderApp(`/analisar-log/${CONVERSATION_ID}`);
    await screen.findByRole(
      'heading',
      { level: 2, name: 'O jogo travou num NoSuchMethodError.' },
      { timeout: 10_000 },
    );
    const user = userEvent.setup();
    const group = screen.getByRole('radiogroup', { name: 'Como o Sculk procura' });
    await user.click(within(group).getByRole('radio', { name: 'Procurar rápido' }));
    await user.type(
      screen.getByRole('textbox', { name: 'Pergunte mais ao Sculk' }),
      'qual mod aparece primeiro?',
    );
    await user.click(screen.getByRole('button', { name: 'Enviar' }));
    await waitFor(() => {
      expect(claude.callsOf('ai_standalone_send')[0]?.args).toMatchObject({
        conversationId: CONVERSATION_ID,
        text: 'qual mod aparece primeiro?',
        mode: 'quick',
      });
    });
    unmount();

    const gemini = setup();
    renderApp(`/analisar-log/${CONVERSATION_ID}`);
    await screen.findByRole(
      'heading',
      { level: 2, name: 'O jogo travou num NoSuchMethodError.' },
      { timeout: 10_000 },
    );
    expect(screen.queryByRole('radiogroup', { name: 'Como o Sculk procura' })).toBeNull();
    await user.type(screen.getByRole('textbox', { name: 'Pergunte mais ao Sculk' }), 'e agora?');
    await user.click(screen.getByRole('button', { name: 'Enviar' }));
    await waitFor(() => {
      expect(gemini.callsOf('ai_standalone_send')[0]?.args).toMatchObject({
        text: 'e agora?',
        mode: null,
      });
    });
  });

  it('junção D-18 × D-19: a conversa sem pack mostra o provedor dela, o aviso de troca do Gemini e a chave que falta', async () => {
    setup({
      secrets_status: () => ({ ...KEYS, gemini: false }),
      ai_standalone_get: () => ({
        ...linkView(),
        model: 'gemini-9.1-flash',
        turns: [
          makeTurn({
            question: LINK_TEXT,
            steps: [LINK_STEP],
            fallbackModel: 'gemini-9.1-flash-lite',
            answer: makeAnswer({ checked: makeChecked({ noConclusion: true }) }),
          }),
        ],
      }),
    });
    await openConversation();
    expect(screen.getByText(/^Gemini · começou /)).toBeDefined();
    const notice = screen.getByText(
      /O limite diário do modelo mais forte acabou; esta resposta usou o mais leve\./,
    );
    expect(notice.closest('article')?.textContent).toContain('gemini-9.1-flash-lite');
    expect(await screen.findByText('Falta a chave do Gemini')).toBeDefined();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Detalhes, 2 avisos' }));
    expect(
      within(screen.getByRole('complementary', { name: 'Detalhes' })).getByText(
        /1 resposta usou o mais leve \(gemini-9\.1-flash-lite\)/,
      ),
    ).toBeDefined();
  });

  it('apagar uma conversa sem pack pelo Detalhes relê a lista e volta para a nova análise', async () => {
    let gone = false;
    const backend = setup({
      ai_standalone_list: () => (gone ? [] : [LINK_SUMMARY]),
      ai_standalone_delete: () => {
        gone = true;
        return null;
      },
    });
    await openConversation();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /^Detalhes/ }));
    await user.click(
      within(screen.getByRole('complementary', { name: 'Detalhes' })).getByRole('button', {
        name: 'Apagar esta conversa',
      }),
    );
    const confirm = await screen.findByRole('group', { name: 'Apagar esta conversa?' });
    await user.click(within(confirm).getByRole('button', { name: 'Apagar conversa' }));
    expect(await screen.findByRole('heading', { level: 2, name: 'Nova análise' })).toBeDefined();
    expect(await screen.findByText('Nenhuma conversa sem pack ainda.')).toBeDefined();
    expect(backend.callsOf('ai_standalone_delete')[0]?.args).toMatchObject({
      conversationId: CONVERSATION_ID,
    });
  });
});

function closestListItem(element: HTMLElement): HTMLElement {
  const found = element.closest<HTMLElement>('li');
  if (!found) throw new Error('sem li em volta');
  return found;
}

function closestListSection(element: HTMLElement): HTMLElement {
  const found = element.closest<HTMLElement>('section');
  if (!found) throw new Error('sem section em volta');
  return found;
}
