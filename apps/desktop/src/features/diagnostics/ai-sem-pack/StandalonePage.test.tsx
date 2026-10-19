/**
 * ✦ Analisar um log, a conversa com a IA sem pack (SPEC T14 item 8; ROADMAP D-19), no app inteiro
 * com o backend simulado: a entrada por Meus packs, a falta de chave, o link e o arquivo, as
 * frases de recusa dos links, o consentimento (texto exato, cancelar sem mandar nada, foco de
 * volta), a conversa sem propostas nem busca do culpado, os dados sempre atuais pelos eventos e
 * apagar. Os critérios estão no nome de cada teste.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AiPreview, ErrorCode } from '../../../lib/ipc/bindings';
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
    tier: 'economical',
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

function setup(handlers: Record<string, Handler> = {}, keyConfigured = true) {
  return mockBackend({
    ai_settings_get: () => ({
      provider: 'gemini',
      tier: 'economical',
      keyConfigured,
      models: null,
      providers: [],
    }),
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
  return screen.findByRole('heading', { level: 1, name: /Analisar um log/ }, { timeout: 10_000 });
}

async function openConversation() {
  renderApp(`/analisar-log/${CONVERSATION_ID}`);
  return screen.findByRole(
    'heading',
    { level: 1, name: 'O jogo travou num NoSuchMethodError.' },
    { timeout: 10_000 },
  );
}

function linkError(code: string) {
  return () =>
    ipcError(makeAppError({ domain: 'app', code } as ErrorCode, { params: { site: 'mclo.gs' } }));
}

describe('✦ Analisar um log: sem pack (T14 item 8)', () => {
  it('CA-T14-25: Meus packs tem o botão ✦ Analisar um log, que abre a página sem pack', async () => {
    setup();
    renderApp('/');
    const user = userEvent.setup();
    const entry = await screen.findByRole('link', { name: 'Analisar um log' }, { timeout: 10_000 });
    expect(entry.getAttribute('href')).toBe('/analisar-log');
    await user.click(entry);
    expect(await screen.findByRole('heading', { level: 1, name: /Analisar um log/ })).toBeDefined();
    expect(await screen.findByRole('heading', { name: 'Conversas sem pack' })).toBeDefined();
  });

  it('sem a chave do provedor: o caminho para Configurações, sem nenhuma chamada à IA', async () => {
    const backend = setup({}, false);
    await openPage();
    const link = await screen.findByRole('link', { name: 'Abrir Configurações' });
    expect(link.getAttribute('href')).toBe('/configuracoes');
    expect(backend.callsOf('ai_standalone_preview')).toHaveLength(0);
    expect(backend.callsOf('ai_standalone_list')).toHaveLength(0);
  });

  it('CA-T14-25: link de outro site, http:// e hastebin mostram a frase própria', async () => {
    let code = 'AI_LINK_UNSUPPORTED';
    const backend = setup({ ai_standalone_preview: () => linkError(code)() });
    await openPage();
    const user = userEvent.setup();
    const field = screen.getByRole('textbox', { name: 'Link do log' });
    const begin = screen.getByRole('button', { name: 'Começar conversa' });
    expect((begin as HTMLButtonElement).disabled).toBe(true);
    await user.type(field, 'https://example.com/abc');
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
    await user.click(begin);
    expect(await screen.findByText(/O hastebin exige uma chave para ler os textos/)).toBeDefined();
    code = 'AI_LINK_NOT_HTTPS';
    await user.click(begin);
    expect(await screen.findByText('Use o link do mclo.gs que começa com https://.')).toBeDefined();
    expect(backend.callsOf('ai_standalone_start')).toHaveLength(0);
  });

  it('CA-T14-27: o consentimento mostra o texto exato; Cancelar não manda nada e devolve o foco; axe', async () => {
    const backend = setup();
    await openPage();
    const user = userEvent.setup();
    await user.click(screen.getByRole('radio', { name: /Trava ao abrir o jogo/ }));
    await user.type(
      screen.getByRole('textbox', { name: 'Link do log' }),
      ' https://mclo.gs/abc123 ',
    );
    const begin = screen.getByRole('button', { name: 'Começar conversa' });
    await user.click(begin);
    const dialog = await screen.findByRole('dialog', { name: 'Começar uma conversa com a IA?' });
    expect(backend.callsOf('ai_standalone_preview')[0]?.args).toMatchObject({
      request: {
        symptom: 'crashOnStart',
        source: { kind: 'link', url: 'https://mclo.gs/abc123' },
        note: null,
      },
    });
    expect(within(dialog).getByLabelText('Texto inicial que será enviado').textContent).toBe(
      LINK_TEXT,
    );
    expect(within(dialog).getByText('O log que você escolheu')).toBeDefined();
    expect(within(dialog).getByText(/não faz propostas de mudança/)).toBeDefined();
    // Sem pack não há caixa de configs; a busca no GitHub continua desmarcável.
    expect(within(dialog).queryByRole('checkbox', { name: /Configs do pack/ })).toBeNull();
    expect(
      within(dialog).getByRole('checkbox', { name: /Procurar problemas parecidos/ }),
    ).toBeDefined();
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
    await user.click(screen.getByRole('radio', { name: /Um arquivo do computador/ }));
    await user.click(screen.getByRole('button', { name: 'Escolher arquivo…' }));
    expect(await screen.findByText('crash-2026.txt')).toBeDefined();
    expect(
      screen.getByText(/KB\. Antes de ir para a IA, o texto passa pela mesma limpeza/),
    ).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    const dialog = await screen.findByRole('dialog');
    expect(backend.callsOf('ai_standalone_preview')[0]?.args).toMatchObject({
      request: { source: { kind: 'localFile', token: '01K7TOKENAAAAAAAAAAAAAAAAA' } },
    });
    await user.click(
      within(dialog).getByRole('checkbox', { name: /Procurar problemas parecidos/ }),
    );
    await user.click(within(dialog).getByRole('button', { name: 'Começar conversa' }));
    expect(
      await screen.findByRole('heading', {
        level: 1,
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

  it('CA-T14-28: a conversa sem pack mostra o que foi enviado, sem propostas nem busca do culpado; axe', async () => {
    setup();
    await openConversation();
    expect(screen.getByText(/Conversa sobre Link do mclo\.gs/)).toBeDefined();
    expect(
      screen.getByText('Enviado à IA: busca por "NoSuchMethodError" no log do link'),
    ).toBeDefined();
    expect(screen.getByText('A IA não encontrou a causa com o que consultou.')).toBeDefined();
    expect(screen.queryByRole('link', { name: 'Encontrar o mod culpado' })).toBeNull();
    expect(screen.queryByText('Propostas')).toBeNull();
    expect(screen.queryByText(/Atualizar para a 1\.6\.7/)).toBeNull();
    // Sem pack, nenhum mod vira link para a lista de Mods.
    expect(screen.queryByRole('link', { name: "Create: Steam 'n' Rails" })).toBeNull();
    expect(screen.getByText('o log escolhido, issues no GitHub')).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('dados atuais: o ai-standalone-updated relê a lista e a conversa aberta', async () => {
    let state: 'running' | 'answered' = 'running';
    const backend = setup({
      ai_standalone_list: () => [{ ...LINK_SUMMARY, state }],
    });
    await openPage();
    expect(await screen.findByText('A IA está respondendo')).toBeDefined();
    state = 'answered';
    await backend.emit('ai-standalone-updated', { conversationId: CONVERSATION_ID });
    expect(await screen.findByText('Respondida')).toBeDefined();
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
    expect(await screen.findByText(/A IA está lendo/)).toBeDefined();
    answered = true;
    await backend.emit('ai-standalone-updated', { conversationId: CONVERSATION_ID });
    expect(
      await screen.findByText('O Steam n Rails 1.6.4 não funciona com o Create 6.'),
    ).toBeDefined();
    await waitFor(() => {
      expect(screen.queryByText(/A IA está lendo/)).toBeNull();
    });
  });

  it('apagar uma conversa sem pack relê a lista', async () => {
    let gone = false;
    const backend = setup({
      ai_standalone_list: () => (gone ? [] : [LINK_SUMMARY]),
      ai_standalone_delete: () => {
        gone = true;
        return null;
      },
    });
    await openPage();
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole('button', {
        name: 'Apagar a conversa "O jogo travou num NoSuchMethodError."',
      }),
    );
    const confirm = await screen.findByRole('alertdialog');
    await user.click(within(confirm).getByRole('button', { name: 'Apagar conversa' }));
    expect(await screen.findByText('Nenhuma conversa sem pack ainda.')).toBeDefined();
    expect(backend.callsOf('ai_standalone_delete')[0]?.args).toMatchObject({
      conversationId: CONVERSATION_ID,
    });
  });
});
