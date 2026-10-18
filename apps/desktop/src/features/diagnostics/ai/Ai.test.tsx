/**
 * ✦ Diagnóstico com IA (SPEC T14 ✦; ROADMAP D-04) no app inteiro, com o backend simulado: a seção
 * sem chave, a nova conversa e o consentimento (texto exato, cancelar sem mandar nada, foco de
 * volta), a conversa (blocos "Enviado à IA" byte a byte, selos, "não verificado", "baseado em
 * mensagem de jogador", medidor do Warden, propostas sem evidência fora), as esperas e os erros,
 * o pack mudado, apagar, os dados sempre atuais pelos eventos e a escolha do modelo em
 * Configurações. Os critérios estão no nome de cada teste.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { axePage } from '../../../test/axe';
import { ipcError, type Handler } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { makeSession, makeSessionView } from '../../test/test.fixtures';
import { makeDiagnosis } from '../diagnostics.fixtures';
import {
  CONVERSATION_ID,
  CRASH_STEP,
  INITIAL_TEXT,
  SESSION_ID,
  makeAnswer,
  makeChecked,
  makePreview,
  makeSources,
  makeSummary,
  makeTurn,
  makeView,
} from './ai.fixtures';

function setup(handlers: Record<string, Handler> = {}, gemini = true) {
  return editorBackend({
    handlers: {
      secrets_status: () => ({ backend: 'keyring', curseforge: false, gemini, github: false }),
      ai_sources_list: () => makeSources(),
      ai_conversations_list: () => [makeSummary()],
      ai_conversation_preview: () => makePreview(),
      ai_conversation_start: () => CONVERSATION_ID,
      ai_conversation_run: () => null,
      ai_conversation_get: () => makeView(),
      ai_conversation_delete: () => null,
      ai_conversation_send: () => null,
      ...handlers,
    },
  });
}

function closestOf(element: HTMLElement, selector: string): HTMLElement {
  const found = element.closest<HTMLElement>(selector);
  if (!found) throw new Error(`sem ${selector} em volta`);
  return found;
}

async function openSection(url: string) {
  renderApp(`${url}/ia`);
  return screen.findByRole(
    'heading',
    { level: 1, name: /Diagnóstico com IA/ },
    { timeout: 10_000 },
  );
}

async function openConversation(url: string) {
  renderApp(`${url}/ia/${CONVERSATION_ID}`);
  return screen.findByRole(
    'heading',
    { level: 1, name: makeSummary().title ?? '' },
    { timeout: 10_000 },
  );
}

describe('✦ Diagnóstico com IA: seção (T14)', () => {
  it('sem chave do Gemini: o caminho para Configurações, sem nenhuma chamada à IA', async () => {
    const { backend, url } = setup({}, false);
    await openSection(url);
    expect(await screen.findByText('Falta a chave do Gemini')).toBeDefined();
    expect(
      screen.getByText(/Para usar a IA, informe sua chave do Gemini em Configurações\./),
    ).toBeDefined();
    const link = screen.getByRole('link', { name: 'Abrir Configurações' });
    expect(link.getAttribute('href')).toBe('/configuracoes');
    expect(backend.callsOf('ai_conversation_preview')).toHaveLength(0);
    expect(backend.callsOf('ai_conversations_list')).toHaveLength(0);
  });

  it('CA-T14-05: o consentimento mostra o texto inicial exato; Cancelar não manda nada e devolve o foco; axe', async () => {
    const { backend, url } = setup();
    await openSection(url);
    const user = userEvent.setup();
    // Começar pelo: o último travamento, já escolhido.
    const select = await screen.findByLabelText('Começar pelo');
    await waitFor(() => {
      expect((select as HTMLSelectElement).value).toBe('ultimo');
    });
    await user.click(screen.getByRole('radio', { name: /Está lento/ }));
    await user.type(screen.getByRole('textbox', { name: /Contar algo à IA/ }), 'trava no Nether');
    const begin = screen.getByRole('button', { name: 'Começar conversa' });
    await user.click(begin);

    const dialog = await screen.findByRole('dialog', { name: 'Começar uma conversa com a IA?' });
    expect(
      within(dialog).getByText(
        /Cada coisa que a IA consultar aparece na conversa, exatamente como foi enviada\. Nada muda no pack sem você clicar em Aplicar\./,
      ),
    ).toBeDefined();
    const text = within(dialog).getByLabelText('Texto inicial que será enviado');
    expect(text.textContent).toBe(INITIAL_TEXT);
    expect(
      within(dialog).getByText('Google Gemini (gemini-9.1-flash-lite), com a sua chave'),
    ).toBeDefined();
    expect(within(dialog).getByText(/cerca de ~31 mil tokens/)).toBeDefined();
    expect(within(dialog).getByText(/Entre US\$ 0,005 e US\$ 0,04/)).toBeDefined();
    expect(within(dialog).getByText(/pessoas podem lê-lo/)).toBeDefined();
    // A busca no GitHub (D-14): marcada, desmarcável, com o que vai e o que nunca vai.
    const github = within(dialog).getByRole('checkbox', {
      name: /Procurar problemas parecidos nas páginas dos mods no GitHub/,
    });
    expect((github as HTMLInputElement).checked).toBe(true);
    expect(
      within(dialog).getByText(
        'Envia ao GitHub só o nome do mod e algumas palavras do erro. Nunca o log.',
      ),
    ).toBeDefined();
    expect(backend.callsOf('ai_conversation_preview')[0]?.args).toEqual({
      packId: expect.any(String) as string,
      request: {
        symptom: 'slow',
        source: { kind: 'lastCrash' },
        note: 'trava no Nether',
      },
    });
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(document.activeElement).toBe(begin);
    expect(backend.callsOf('ai_conversation_start')).toHaveLength(0);
    expect(backend.callsOf('ai_conversation_run')).toHaveLength(0);
  });

  it('Começar conversa grava as permissões escolhidas, roda e abre a conversa', async () => {
    const { backend, url } = setup();
    await openSection(url);
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Começar conversa' }));
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('checkbox', { name: /Configs do pack/ }));
    // CA-T14-13: com a caixa do GitHub desmarcada, a conversa grava a permissão negada.
    await user.click(within(dialog).getByRole('checkbox', { name: /no GitHub/ }));
    await user.click(within(dialog).getByRole('button', { name: 'Começar conversa' }));
    await openConversationHeading();
    expect(backend.callsOf('ai_conversation_start')[0]?.args).toEqual({
      packId: expect.any(String) as string,
      previewId: makePreview().previewId,
      permissions: { configs: false, github: false },
    });
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_run')).toHaveLength(1);
    });
  });

  it('vindo de "✦ Conversar com a IA": a sessão vem escolhida e o consentimento abre sozinho', async () => {
    const { backend, url } = setup();
    renderApp(`${url}/ia?sessao=2026-10-02_19-40-00`);
    await screen.findByRole(
      'dialog',
      { name: 'Começar uma conversa com a IA?' },
      { timeout: 10_000 },
    );
    expect(backend.callsOf('ai_conversation_preview')[0]?.args.request).toMatchObject({
      source: { kind: 'session', sessionId: '2026-10-02_19-40-00' },
    });
  });

  it('um log do computador: o diálogo nativo e só o nome do arquivo na tela', async () => {
    const { backend, url } = setup({
      ai_log_choose: () => ({ token: 'tok', fileName: 'crash-x.txt', bytes: 2048 }),
    });
    await openSection(url);
    const user = userEvent.setup();
    const select = await screen.findByLabelText('Começar pelo');
    await user.selectOptions(select, 'arquivo');
    expect(await screen.findByText(/Arquivo do computador: crash-x\.txt/)).toBeDefined();
    expect(screen.getByText(/2,0 KB\. Antes de ir para a IA/)).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    await screen.findByRole('dialog');
    expect(backend.callsOf('ai_conversation_preview')[0]?.args.request).toMatchObject({
      source: { kind: 'localFile', token: 'tok' },
    });
  });

  it('Conversas: título, o que foi analisado, situação, Continuar e Apagar com confirmação', async () => {
    let list = [
      makeSummary(),
      makeSummary({
        id: '01K7BBBBBBBBBBBBBBBBBBBBBB',
        title: null,
        state: 'failed',
        origin: { kind: 'packOnly' },
        tokens: 0,
      }),
    ];
    const { backend, url } = setup({
      ai_conversations_list: () => list,
      ai_conversation_delete: () => {
        list = list.slice(0, 1);
        return null;
      },
    });
    await openSection(url);
    const table = await screen.findByRole('table');
    const rows = within(table).getAllByRole('row');
    expect(rows).toHaveLength(3);
    const [, answered, failed] = rows;
    if (!answered || !failed) throw new Error('linhas da tabela');
    expect(
      within(answered).getByText('Travamento de 03/10 às 21:10 · ~182 mil tokens'),
    ).toBeDefined();
    expect(within(answered).getByText('Respondida')).toBeDefined();
    expect(within(failed).getByText('Trava ao entrar no mundo')).toBeDefined();
    expect(within(failed).getByText('Parou com erro')).toBeDefined();
    const continuar = within(answered).getByRole('link', {
      name: /Continuar a conversa/,
    });
    expect(continuar.getAttribute('href')).toContain(`/ia/${CONVERSATION_ID}`);

    const user = userEvent.setup();
    await user.click(within(failed).getByRole('button', { name: /Apagar a conversa/ }));
    const confirm = await screen.findByRole('alertdialog', { name: 'Apagar esta conversa?' });
    await user.click(within(confirm).getByRole('button', { name: 'Apagar conversa' }));
    await waitFor(() => {
      expect(within(screen.getByRole('table')).getAllByRole('row')).toHaveLength(2);
    });
    expect(backend.callsOf('ai_conversation_delete')[0]?.args.conversationId).toBe(
      '01K7BBBBBBBBBBBBBBBBBBBBBB',
    );
  });

  it('dados atuais: o evento ai-conversation-updated relê a lista', async () => {
    let state: 'running' | 'answered' = 'running';
    const { backend, url, row } = setup({
      ai_conversations_list: () => [makeSummary({ state })],
    });
    await openSection(url);
    expect(await screen.findByText('A IA está respondendo')).toBeDefined();
    state = 'answered';
    await backend.emit('ai-conversation-updated', {
      packId: row.id,
      conversationId: CONVERSATION_ID,
    });
    expect(await screen.findByText('Respondida')).toBeDefined();
  });
});

async function openConversationHeading() {
  return screen.findByRole(
    'heading',
    { level: 1, name: makeSummary().title ?? '' },
    { timeout: 10_000 },
  );
}

describe('✦ Diagnóstico com IA: a conversa (T14)', () => {
  it('CA-T14-03 (tela): cada bloco "Enviado à IA" mostra byte a byte o que saiu; a consulta negada diz isso', async () => {
    const { url } = setup();
    await openConversation(url);
    const user = userEvent.setup();
    const initial = screen.getByText('Enviado à IA: texto inicial');
    await user.click(initial);
    const block = closestOf(initial, 'details');
    expect(within(block).getByLabelText('Bytes enviados à IA').textContent).toBe(INITIAL_TEXT);

    const log = screen.getByText(
      /Enviado à IA: trecho do log do teste de 03\/10 às 21:10, linhas 1\.200–1\.260/,
    );
    await user.click(log);
    const logBlock = closestOf(log, 'details');
    expect(within(logBlock).getByLabelText('Bytes enviados à IA').textContent).toBe(
      CRASH_STEP.sent,
    );
    expect(within(logBlock).getByLabelText('Consulta pedida pela IA').textContent).toBe(
      `read_log ${CRASH_STEP.args}`,
    );
    expect(
      screen.getByText(
        /Enviado à IA: config config\/create-common\.toml · não permitido nesta conversa/,
      ),
    ).toBeDefined();
  });

  it('CA-T14-11 e CA-T14-19 (tela): selos, não verificado, mensagem de jogador, medidor do Warden, propostas sem evidência fora; axe', async () => {
    const { url } = setup();
    await openConversation(url);
    const claims = closestOf(screen.getByText('O que a IA afirma'), 'section');
    const verified = closestOf(
      within(claims).getByText('O mixin do Rails chama um método que o Create 6 tirou.'),
      'li',
    );
    expect(verified.className).not.toContain('claim--no');
    const badge = within(verified).getByRole('button', {
      name: /Evidência conferida: log, linha 1\.234/,
    });
    const user = userEvent.setup();
    await user.click(badge);
    expect(
      await within(verified).findByText('Caused by: java.lang.NoSuchMethodError: getPositionVec'),
    ).toBeDefined();
    // O mod citado que existe vira link para a lista de Mods.
    expect(
      within(verified).getByRole('link', { name: "Create: Steam 'n' Rails" }).getAttribute('href'),
    ).toContain('item=mods%2Frailways.pw.toml');

    const unverified = closestOf(
      within(claims).getByText('O problema aparece em todos os servidores.'),
      'li',
    );
    expect(unverified.className).toContain('claim--no');
    expect(within(unverified).getAllByText('não verificado').length).toBeGreaterThan(0);
    expect(
      within(unverified).getByText('Citados pela IA, mas não estão no pack: turbofix.'),
    ).toBeDefined();
    expect(within(unverified).queryByRole('link')).toBeNull();

    const chat = closestOf(within(claims).getByText('Falta memória.'), 'li');
    expect(within(chat).getByText('baseado em mensagem de jogador')).toBeDefined();
    expect(chat.className).not.toContain('claim--no');
    expect(
      within(chat).getByRole('button', { name: /Mensagem de jogador: log, linha 789/ }),
    ).toBeDefined();

    // O medidor é o do Warden (média), não a confiança "alta" que a IA declarou.
    expect(screen.getByText('Confiança média')).toBeDefined();
    expect(screen.queryByText('Confiança alta')).toBeNull();
    expect(screen.getByText('A IA pode errar. Confira antes de mudar o pack.')).toBeDefined();
    expect(screen.getByText(/não que a conclusão está certa/)).toBeDefined();

    // Só a proposta com evidência aparece; a do TurboFix (descartada) não.
    expect(
      screen.getByText('Atualizar para a 1.6.7, que corrige o travamento com o Create 6.'),
    ).toBeDefined();
    expect(screen.queryByText(/TurboFix/)).toBeNull();
    expect(await axePage(document.body)).toHaveNoViolations();
  });

  it('CA-T14-19: com o medidor baixo, a tela diz baixa mesmo se a IA declarou alta', async () => {
    const view = makeView({
      turns: [
        makeTurn({
          answer: { ...makeAnswer(), checked: makeChecked({ confidence: 'low' }) },
        }),
      ],
    });
    const { url } = setup({ ai_conversation_get: () => view });
    await openConversation(url);
    expect(screen.getByText('Confiança baixa')).toBeDefined();
  });

  it('"Não sei" é resposta válida: sugere encontrar o mod culpado', async () => {
    const view = makeView({
      turns: [
        makeTurn({
          answer: {
            ...makeAnswer(),
            checked: makeChecked({
              noConclusion: true,
              claims: [],
              proposals: [],
              mods: [],
              confidence: 'low',
            }),
          },
        }),
      ],
    });
    const { url } = setup({ ai_conversation_get: () => view });
    await openConversation(url);
    expect(screen.getByText('A IA não encontrou a causa com o que consultou.')).toBeDefined();
    expect(
      screen.getByRole('link', { name: 'Encontrar o mod culpado' }).getAttribute('href'),
    ).toContain('/problemas');
  });

  it('plano gratuito: a espera do limite por minuto aparece, e o evento troca pela resposta', async () => {
    let view = makeView({
      summary: makeSummary({ state: 'running' }),
      turns: [makeTurn({ answer: null, steps: [CRASH_STEP] })],
      live: { kind: 'waitingPerMinute', seconds: 39 },
    });
    const { backend, url, row } = setup({ ai_conversation_get: () => view });
    await openConversation(url);
    expect(
      screen.getByText(
        /Aguardando o limite por minuto do plano gratuito… A conversa continua sozinha em cerca de 39 s\./,
      ),
    ).toBeDefined();
    expect(screen.getByRole('textbox', { name: 'Perguntar mais à IA' })).toHaveProperty(
      'disabled',
      true,
    );
    view = makeView();
    await backend.emit('ai-conversation-updated', {
      packId: row.id,
      conversationId: CONVERSATION_ID,
    });
    expect(await screen.findByText('O que a IA afirma')).toBeDefined();
    expect(screen.queryByText(/Aguardando o limite/)).toBeNull();
    expect(screen.getByRole('textbox', { name: 'Perguntar mais à IA' })).toHaveProperty(
      'disabled',
      false,
    );
  });

  it('limite diário: a frase sem horário, Tentar de novo e o caminho para trocar de modelo', async () => {
    const view = makeView({
      summary: makeSummary({ state: 'failed' }),
      turns: [
        makeTurn({
          answer: null,
          failure: {
            domain: 'ai',
            code: 'QUOTA_EXCEEDED',
            params: { model: 'gemini-9.1-flash-lite', freeTier: 'true' },
            detail: null,
            retryable: false,
          },
        }),
      ],
    });
    const { backend, url } = setup({ ai_conversation_get: () => view });
    await openConversation(url);
    const alert = screen.getByRole('alert');
    expect(
      within(alert).getByText(/O limite diário do plano gratuito deste modelo acabou\./),
    ).toBeDefined();
    expect(alert.textContent).not.toMatch(/\d{1,2}:\d{2}|\bh\b|horas/);
    expect(
      within(alert).getByText('Nada foi alterado no pack. A conversa fica guardada.'),
    ).toBeDefined();
    expect(within(alert).getByRole('link', { name: 'Abrir Configurações' })).toBeDefined();
    const user = userEvent.setup();
    await user.click(within(alert).getByRole('button', { name: 'Tentar de novo' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_run')).toHaveLength(1);
    });
  });

  it('pergunta nova: só depois da resposta, e roda em seguida', async () => {
    const { backend, url } = setup();
    await openConversation(url);
    const user = userEvent.setup();
    await user.type(
      screen.getByRole('textbox', { name: 'Perguntar mais à IA' }),
      'E se eu voltar o Create?',
    );
    await user.click(screen.getByRole('button', { name: 'Enviar' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_send')[0]?.args.text).toBe(
        'E se eu voltar o Create?',
      );
    });
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_run')).toHaveLength(1);
    });
  });

  it('CA-T14-15 (tela): o pack mudou desde a conversa: o aviso com os 3 mods e as duas saídas; o pack-changed relê', async () => {
    let changes = { updated: 0, added: 0, removed: 0, other: false };
    const { backend, url, row } = setup({
      ai_conversation_get: () => makeView({ packChanges: changes }),
    });
    await openConversation(url);
    expect(screen.queryByText('O pack mudou desde esta conversa.')).toBeNull();
    changes = { updated: 3, added: 0, removed: 0, other: false };
    await backend.emit('pack-changed', { packId: row.id, areas: ['inventory'], external: false });
    expect(await screen.findByText('O pack mudou desde esta conversa.')).toBeDefined();
    expect(screen.getByText(/3 mods foram atualizados depois de/)).toBeDefined();
    expect(
      screen.getByRole('link', { name: 'Começar nova conversa' }).getAttribute('href'),
    ).toMatch(/\/ia$/);
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Continuar mesmo assim' }));
    expect(screen.queryByText('O pack mudou desde esta conversa.')).toBeNull();
  });

  it('apagar a conversa pela lateral volta para Conversas', async () => {
    const { backend, url } = setup();
    await openConversation(url);
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Apagar esta conversa' }));
    const confirm = await screen.findByRole('alertdialog');
    await user.click(within(confirm).getByRole('button', { name: 'Apagar conversa' }));
    await openSection_heading();
    expect(backend.callsOf('ai_conversation_delete')).toHaveLength(1);
  });

  it('conversa que não existe mais: diz isso, com o caminho de volta', async () => {
    const { url } = setup({
      ai_conversation_get: () =>
        ipcError(makeAppError({ domain: 'ai', code: 'CONVERSATION_NOT_FOUND' })),
    });
    renderApp(`${url}/ia/${CONVERSATION_ID}`);
    expect(
      await screen.findByText(
        'Essa conversa não existe mais. Ela pode ter sido apagada.',
        {},
        { timeout: 10_000 },
      ),
    ).toBeDefined();
    expect(screen.getByRole('link', { name: 'Conversas' })).toBeDefined();
  });

  it('a lateral: modelo, tamanho, requisições e o que a IA pode ler (sem configs, que foram desmarcadas)', async () => {
    const { url } = setup();
    await openConversation(url);
    const side = screen.getByRole('complementary', { name: 'Sobre esta conversa' });
    expect(within(side).getByText('gemini-9.1-flash-lite')).toBeDefined();
    expect(within(side).getByText('~182 mil tokens')).toBeDefined();
    expect(within(side).getByText('4')).toBeDefined();
    expect(within(side).getByText('pack e achados, logs e crash reports')).toBeDefined();
    expect(within(side).getByText('~2 mil tokens, contados pelo Google')).toBeDefined();
  });
});

async function openSection_heading() {
  return screen.findByRole(
    'heading',
    { level: 1, name: /Diagnóstico com IA/ },
    { timeout: 10_000 },
  );
}

describe('entradas da IA (T14 ✦ "Onde")', () => {
  it('Travamentos: "✦ Conversar com a IA" leva à seção com a sessão mais nova do grupo', async () => {
    const { url } = editorBackend({
      handlers: {
        diagnostics_report_get: () => null,
        diagnostics_run: () => null,
        crashes_list: () => ({
          groups: [
            {
              signature: {
                version: 6,
                hash: 'abc',
                rule: null,
                exception: 'java.lang.NullPointerException',
                frame: null,
                mods: [],
                parts: [],
              },
              count: 1,
              firstAtMs: 1,
              lastAtMs: 2,
              versions: [{ version: '1.4.0', unsaved: false }],
              origins: [{ origin: 'test', count: 1 }],
              situation: { kind: 'causeUnknown' },
              latestSessionId: SESSION_ID,
              diagnosis: null,
              links: { culpritSearches: [], conversations: [] },
            },
          ],
          retention: { budgetBytes: 1, nearLimit: false, overLimit: false, prunable: [] },
        }),
      },
    });
    renderApp(`${url}/problemas`);
    const link = await screen.findByRole(
      'link',
      { name: /Conversar com a IA sobre o travamento de/ },
      { timeout: 10_000 },
    );
    expect(link.getAttribute('href')).toContain(`/ia?sessao=${SESSION_ID}`);
  });

  it('resultado do teste que travou: "✦ Pedir ajuda à IA" leva à conversa sobre aquela sessão', async () => {
    const crashed = makeSession({ id: SESSION_ID, outcome: 'crashed', exitCode: 1 });
    const { url } = editorBackend({
      handlers: {
        test_sessions_list: () => [crashed],
        test_session_get: () => makeSessionView(crashed, []),
        diagnostics_crash_get: () => makeDiagnosis({ foundCause: false, findings: [] }),
      },
    });
    renderApp(`${url}/teste?sessao=${SESSION_ID}`);
    const link = await screen.findByRole('link', { name: 'Pedir ajuda à IA' }, { timeout: 10_000 });
    expect(link.getAttribute('href')).toContain(`/ia?sessao=${SESSION_ID}`);
    expect(screen.getByText('Ou converse com a IA sobre este travamento.')).toBeDefined();
  });

  it('resultado de um teste que não travou: nada da IA', async () => {
    const closed = makeSession({ id: SESSION_ID, outcome: 'closedNormally', exitCode: 0 });
    const { url } = editorBackend({
      handlers: {
        test_sessions_list: () => [closed],
        test_session_get: () => makeSessionView(closed, []),
      },
    });
    renderApp(`${url}/teste?sessao=${SESSION_ID}`);
    await screen.findByRole('heading', { level: 1 }, { timeout: 10_000 });
    expect(screen.queryByRole('link', { name: 'Pedir ajuda à IA' })).toBeNull();
  });
});
