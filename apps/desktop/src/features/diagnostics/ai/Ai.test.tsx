/**
 * Sensor Sculk (SPEC T14; ROADMAP D-04; ADR-0064) no app inteiro, com o backend simulado: a seção
 * sem chave, a Nova conversa e o consentimento (texto exato, cancelar sem mandar nada, foco de
 * volta, "Sempre", "Enviar e começar"), a lista de conversas (busca sem acentos, grupos por data,
 * selos, conversa aberta), a conversa (blocos "Enviado ao Sculk" byte a byte, selos de evidência
 * que abrem a consulta, "não verificado", "baseado em mensagem de jogador", medidor do Warden,
 * propostas sem evidência fora), as esperas e os erros, a gaveta Detalhes (avisos, serviço e uso,
 * o que o Sculk pode ler, tudo o que foi enviado, apagar), o pack mudado, os dados sempre atuais
 * pelos eventos e o nome da seção no menu. Os critérios estão no nome de cada teste.
 */
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { AiConversationSummary, SecretsStatus } from '../../../lib/ipc/bindings';
import { formatBytes } from '../../../lib/format';
import { axePage } from '../../../test/axe';
import { ipcError, type Handler } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { editorBackend } from '../../pack-editor/testing';
import { fakeAiHandlers } from '../../settings/ai/testing';
import { makeSession, makeSessionView } from '../../test/test.fixtures';
import { makeDiagnosis } from '../diagnostics.fixtures';
import {
  CONVERSATION_ID,
  CRASH_STEP,
  DENIED_STEP,
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
import { byteLength } from './text';

const KEYS: SecretsStatus = {
  backend: 'keyring',
  curseforge: false,
  gemini: true,
  openai: false,
  anthropic: false,
  github: false,
};

function setup(
  handlers: Record<string, Handler> = {},
  keys: Partial<SecretsStatus> = {},
  provider: 'gemini' | 'openai' | 'anthropic' = 'gemini',
) {
  const secrets: SecretsStatus = { ...KEYS, ...keys };
  const fake = fakeAiHandlers(() => secrets, { provider });
  return editorBackend({
    handlers: {
      ...fake.handlers,
      secrets_status: () => secrets,
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

/** O `<details>` em volta (consultas e anexos abrem e fecham). */
function closestDetails(element: HTMLElement): HTMLDetailsElement {
  const found = element.closest('details');
  if (!found) throw new Error('sem details em volta');
  return found;
}

/** A seção aberta em Nova conversa (o título da coluna do meio). */
async function openSection(url: string) {
  renderApp(`${url}/ia`);
  return screen.findByRole('heading', { level: 2, name: 'Nova conversa' }, { timeout: 10_000 });
}

/** A conversa aberta (o título dela no topo da coluna do meio). */
async function openConversation(url: string, title = makeSummary().title ?? '') {
  renderApp(`${url}/ia/${CONVERSATION_ID}`);
  return screen.findByRole('heading', { level: 2, name: title }, { timeout: 10_000 });
}

/** Escolhe o sintoma e clica em Começar conversa: o consentimento abre. */
async function beginConversation(user: ReturnType<typeof userEvent.setup>, symptom = /Está lento/) {
  await user.click(await screen.findByRole('radio', { name: symptom }));
  const begin = screen.getByRole('button', { name: 'Começar conversa' });
  await user.click(begin);
  return {
    begin,
    dialog: await screen.findByRole('dialog', { name: /Começar uma conversa com o Sculk\?/ }),
  };
}

/** O botão Detalhes (o nome traz o número de avisos, quando há). */
function detailsButton(name: string | RegExp = /^Detalhes/) {
  return screen.getByRole('button', { name });
}

/** A gaveta Detalhes. */
function drawer() {
  return screen.getByRole('complementary', { name: 'Detalhes' });
}

/** O valor de uma linha "nome / valor" da gaveta. */
function valueOf(name: string): HTMLElement {
  const label = within(drawer()).getByText(name, { selector: 'dt' });
  const value = label.nextElementSibling;
  if (!(value instanceof HTMLElement)) throw new Error(`sem valor para ${name}`);
  return value;
}

describe('Sensor Sculk: seção (T14)', () => {
  it('sem chave do Gemini: o caminho para Configurações, os três provedores e as conversas, sem nenhuma chamada ao Sculk', async () => {
    const { backend, url } = setup({}, { gemini: false });
    renderApp(`${url}/ia`);
    expect(
      await screen.findByText('Falta a chave do Gemini', {}, { timeout: 10_000 }),
    ).toBeDefined();
    expect(
      screen.getByText(
        /O Sculk funciona com o Gemini \(Google\), a OpenAI e o Claude \(Anthropic\)\. Para começar, o sugerido é o Gemini, que tem plano gratuito\./,
      ),
    ).toBeDefined();
    const link = screen.getByRole('link', { name: 'Abrir Configurações' });
    expect(link.getAttribute('href')).toBe('/configuracoes');
    // As conversas continuam à vista: cada uma fica no provedor em que começou (ADR-0060), e o
    // rodapé da lista diz que o provedor das conversas novas está sem chave.
    const list = await screen.findByRole('complementary', { name: 'Conversas' });
    expect(await within(list).findByRole('link', { name: /^O mixin do Rails/ })).toBeDefined();
    expect(within(list).getByText(/sem chave salva\./)).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Começar conversa' })).toBeNull();
    expect(backend.callsOf('ai_conversation_preview')).toHaveLength(0);
    expect(backend.callsOf('ai_models_get')).toHaveLength(0);
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
    await user.type(
      screen.getByRole('textbox', { name: /Conte o que aconteceu/ }),
      'trava no Nether',
    );
    // Axe na Nova conversa (o estado principal da seção).
    expect(await axePage(document.body)).toHaveNoViolations();
    const { begin, dialog } = await beginConversation(user);
    expect(
      within(dialog).getByText(
        /Cada coisa que o Sculk consultar aparece na conversa\. Nada muda no pack sem você clicar em Aplicar\./,
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
    // No Gemini não há modo: o pedido de prévia leva `mode: null`.
    expect(backend.callsOf('ai_conversation_preview')[0]?.args).toEqual({
      packId: expect.any(String) as string,
      request: {
        symptom: 'slow',
        source: { kind: 'lastCrash' },
        note: 'trava no Nether',
        mode: null,
      },
    });
    // Axe no consentimento.
    expect(await axePage(document.body)).toHaveNoViolations();

    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(document.activeElement).toBe(begin);
    expect(backend.callsOf('ai_conversation_start')).toHaveLength(0);
    expect(backend.callsOf('ai_conversation_run')).toHaveLength(0);
  });

  it('o consentimento lista "Medidas de desempenho dos testes" como "Sempre"; o botão é "Enviar e começar" e Cancelar não começa nada', async () => {
    const { backend, url } = setup();
    await openSection(url);
    const user = userEvent.setup();
    const { dialog } = await beginConversation(user);
    const fixed = within(dialog).getByText('Medidas de desempenho dos testes');
    const row = closestOf(fixed, 'li');
    expect(within(row).getByText('Sempre')).toBeDefined();
    expect(within(row).getByText(/Tempo para abrir e para entrar no mundo/)).toBeDefined();
    // As quatro coisas que valem sempre, cada uma com a marca; configs e GitHub são caixas.
    expect(within(dialog).getAllByText('Sempre')).toHaveLength(4);
    for (const name of [
      'Informações do pack e a lista de mods',
      'Os problemas que o Warden já encontrou',
      'Logs e crash reports dos seus testes',
    ]) {
      expect(
        within(closestOf(within(dialog).getByText(name), 'li')).getByText('Sempre'),
      ).toBeDefined();
    }
    expect(within(dialog).getByRole('checkbox', { name: /Configs do pack/ })).toBeDefined();
    // O botão que começa é "Enviar e começar"; o rótulo antigo não existe mais no diálogo.
    expect(within(dialog).getByRole('button', { name: 'Enviar e começar' })).toBeDefined();
    expect(within(dialog).queryByRole('button', { name: 'Começar conversa' })).toBeNull();

    // Cancelar fecha sem gravar nada e sem rodar.
    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(backend.callsOf('ai_conversation_start')).toHaveLength(0);
    expect(backend.callsOf('ai_conversation_run')).toHaveLength(0);

    // Só "Enviar e começar" grava.
    await user.click(screen.getByRole('button', { name: 'Começar conversa' }));
    const again = await screen.findByRole('dialog');
    await user.click(within(again).getByRole('button', { name: 'Enviar e começar' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_start')).toHaveLength(1);
    });
  });

  it('sem escolher o que está acontecendo, Começar conversa espera e não pede a prévia', async () => {
    const { backend, url } = setup();
    await openSection(url);
    const user = userEvent.setup();
    const begin = await screen.findByRole('button', { name: 'Começar conversa' });
    expect(begin.getAttribute('aria-disabled')).toBe('true');
    await user.click(begin);
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(backend.callsOf('ai_conversation_preview')).toHaveLength(0);
    // Escolher o sintoma libera o botão (e a dica do porquê some).
    await user.click(screen.getByRole('radio', { name: /Está lento/ }));
    expect(
      screen.getByRole('button', { name: 'Começar conversa' }).getAttribute('aria-disabled'),
    ).toBe('false');
  });

  it('Enviar e começar grava as permissões escolhidas, roda e abre a conversa', async () => {
    const { backend, url } = setup();
    await openSection(url);
    const user = userEvent.setup();
    const { dialog } = await beginConversation(user);
    await user.click(within(dialog).getByRole('checkbox', { name: /Configs do pack/ }));
    // CA-T14-13: com a caixa do GitHub desmarcada, a conversa grava a permissão negada.
    await user.click(within(dialog).getByRole('checkbox', { name: /no GitHub/ }));
    await user.click(within(dialog).getByRole('button', { name: 'Enviar e começar' }));
    await screen.findByRole(
      'heading',
      { level: 2, name: makeSummary().title ?? '' },
      { timeout: 10_000 },
    );
    expect(backend.callsOf('ai_conversation_start')[0]?.args).toEqual({
      packId: expect.any(String) as string,
      previewId: makePreview().previewId,
      permissions: { configs: false, github: false },
    });
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_run')).toHaveLength(1);
    });
  });

  it('vindo de "Conversar com o Sculk": a sessão vem escolhida e o consentimento abre sozinho', async () => {
    const { backend, url } = setup();
    renderApp(`${url}/ia?sessao=2026-10-02_19-40-00`);
    await screen.findByRole(
      'dialog',
      { name: /Começar uma conversa com o Sculk\?/ },
      { timeout: 10_000 },
    );
    expect(backend.callsOf('ai_conversation_preview')[0]?.args.request).toMatchObject({
      source: { kind: 'session', sessionId: '2026-10-02_19-40-00' },
      mode: null,
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
    expect(screen.getByText(/2,0 KB\. O texto passa pela limpeza/)).toBeDefined();
    await beginConversation(user);
    expect(backend.callsOf('ai_conversation_preview')[0]?.args.request).toMatchObject({
      source: { kind: 'localFile', token: 'tok' },
    });
  });
});

describe('Sensor Sculk: a lista de conversas (T14)', () => {
  it('cada conversa mostra título, o que foi analisado e a situação; abrir leva à conversa (no lugar do "Continuar"); a sem título usa o sintoma', async () => {
    const list = [
      makeSummary(),
      makeSummary({
        id: '01K7BBBBBBBBBBBBBBBBBBBBBB',
        title: null,
        state: 'failed',
        origin: { kind: 'packOnly' },
        tokens: 0,
      }),
    ];
    const { url } = setup({ ai_conversations_list: () => list });
    await openSection(url);
    const aside = await screen.findByRole('complementary', { name: 'Conversas' });
    const answered = await within(aside).findByRole('link', {
      name: 'O mixin do Rails chama um método que o Create 6 tirou., Travamento de 03/10 às 21:10, Respondida',
    });
    expect(within(answered).getByText('Respondida')).toBeDefined();
    expect(answered.getAttribute('href')).toContain(`/ia/${CONVERSATION_ID}`);
    const failed = within(aside).getByRole('link', {
      name: 'Trava ao entrar no mundo, Só o pack, Parou com erro',
    });
    expect(within(failed).getByText('Parou com erro')).toBeDefined();

    // Nenhuma conversa aberta: Nova conversa fica na tela e nenhuma conversa é a atual.
    expect(answered.getAttribute('aria-current')).toBeNull();
    const user = userEvent.setup();
    await user.click(answered);
    expect(
      await screen.findByRole('heading', { level: 2, name: list[0]?.title ?? '' }),
    ).toBeDefined();
  });

  it('a busca acha pelo título e pela origem sem diferenciar acentos; sem resultado diz isso, com "Limpar busca"', async () => {
    const list = [
      makeSummary({ id: '01K7AAAAAAAAAAAAAAAAAAAAA1', title: 'Falta de memória no Nether' }),
      makeSummary({
        id: '01K7AAAAAAAAAAAAAAAAAAAAA2',
        title: 'O mixin do Rails travou',
        origin: { kind: 'packOnly' },
      }),
      makeSummary({ id: '01K7AAAAAAAAAAAAAAAAAAAAA3', title: 'Jogo lento na vila' }),
    ];
    const { url } = setup({ ai_conversations_list: () => list });
    await openSection(url);
    const aside = await screen.findByRole('complementary', { name: 'Conversas' });
    await within(aside).findByRole('link', { name: /^Falta de memória/ });
    expect(within(aside).getAllByRole('link', { name: /,.*,/ })).toHaveLength(3);
    const user = userEvent.setup();
    const search = within(aside).getByRole('searchbox', { name: 'Buscar nas conversas' });

    // "memoria" (sem acento) acha "memória".
    await user.type(search, 'MEMORIA');
    expect(within(aside).getAllByRole('link', { name: /,.*,/ })).toHaveLength(1);
    expect(within(aside).getByRole('link', { name: /^Falta de memória/ })).toBeDefined();

    // A origem também conta: "so o pack" acha "Só o pack".
    await user.clear(search);
    await user.type(search, 'so o pack');
    expect(within(aside).getAllByRole('link', { name: /,.*,/ })).toHaveLength(1);
    expect(within(aside).getByRole('link', { name: /^O mixin do Rails travou/ })).toBeDefined();

    // Sem resultado.
    await user.clear(search);
    await user.type(search, 'xyz');
    const empty = within(aside).getByRole('status');
    expect(within(empty).getByText('Nenhuma conversa com “xyz”.')).toBeDefined();
    expect(within(aside).queryAllByRole('link', { name: /,.*,/ })).toHaveLength(0);
    await user.click(within(empty).getByRole('button', { name: 'Limpar busca' }));
    expect((search as HTMLInputElement).value).toBe('');
    expect(document.activeElement).toBe(search);
    expect(within(aside).getAllByRole('link', { name: /,.*,/ })).toHaveLength(3);
  });

  it('os selos: Esperando você (proposta pendente), Aplicada (aplicada e nada pendente), Lendo, Respondida, Parou com erro e Sem resposta', async () => {
    const make = (id: string, title: string, overrides: Partial<AiConversationSummary>) =>
      makeSummary({ id: `01K7AAAAAAAAAAAAAAAAAAAAA${id}`, title, ...overrides });
    const list = [
      make('1', 'Com pendente', { pendingProposals: 2 }),
      make('2', 'Com pendente e aplicada', { pendingProposals: 1, appliedProposals: 1 }),
      make('3', 'Só aplicada', { pendingProposals: 0, appliedProposals: 3 }),
      make('4', 'Lendo agora', { state: 'running', pendingProposals: 1 }),
      make('5', 'Respondida pura', {}),
      make('6', 'Quebrou', { state: 'failed' }),
      make('7', 'Fechou antes', { state: 'interrupted' }),
    ];
    const { url } = setup({ ai_conversations_list: () => list });
    await openSection(url);
    const aside = await screen.findByRole('complementary', { name: 'Conversas' });
    await within(aside).findByRole('link', { name: /^Com pendente,/ });
    const tagOf = (title: string) =>
      closestOf(within(aside).getByText(title, { selector: '.conv__title' }), 'a').querySelector(
        '.tag',
      )?.textContent;
    expect(tagOf('Com pendente')).toBe('Esperando você');
    // Pendente vence aplicada: ainda há o que decidir.
    expect(tagOf('Com pendente e aplicada')).toBe('Esperando você');
    expect(tagOf('Só aplicada')).toBe('Aplicada');
    // Lendo vence tudo enquanto o Sculk responde.
    expect(tagOf('Lendo agora')).toBe('Lendo');
    expect(tagOf('Respondida pura')).toBe('Respondida');
    expect(tagOf('Quebrou')).toBe('Parou com erro');
    expect(tagOf('Fechou antes')).toBe('Sem resposta');
  });

  it('a conversa aberta tem aria-current="page"; os grupos por data (Hoje, Ontem, Esta semana, Antes) e as setas andam entre as conversas', async () => {
    const DAY = 24 * 60 * 60 * 1000;
    const now = Date.now();
    const make = (n: number, title: string, updatedAtMs: number) =>
      makeSummary({
        id: n === 1 ? CONVERSATION_ID : `01K7AAAAAAAAAAAAAAAAAAAAA${String(n)}`,
        title,
        updatedAtMs,
      });
    const list = [
      make(1, 'Conversa de hoje', now),
      make(2, 'Conversa de ontem', now - DAY),
      make(3, 'Conversa da semana', now - 3 * DAY),
      make(4, 'Conversa antiga', now - 30 * DAY),
    ];
    const { url } = setup({
      ai_conversations_list: () => list,
      ai_conversation_get: () => makeView({ summary: list[0] ?? makeSummary() }),
    });
    await openConversation(url, 'Conversa de hoje');
    const aside = screen.getByRole('complementary', { name: 'Conversas' });
    const open = await within(aside).findByRole('link', { name: /^Conversa de hoje,/ });
    expect(open.getAttribute('aria-current')).toBe('page');
    expect(
      within(aside)
        .getAllByRole('link', { name: /,.*,/ })
        .filter((link) => link.getAttribute('aria-current') === 'page'),
    ).toHaveLength(1);
    expect(
      within(aside)
        .getByRole('link', { name: /^Conversa antiga,/ })
        .getAttribute('aria-current'),
    ).toBeNull();

    for (const [group, title] of [
      ['Hoje', 'Conversa de hoje'],
      ['Ontem', 'Conversa de ontem'],
      ['Esta semana', 'Conversa da semana'],
      ['Antes', 'Conversa antiga'],
    ] as const) {
      const section = within(aside).getByRole('group', { name: group });
      expect(within(section).getByText(title, { selector: '.conv__title' })).toBeDefined();
      expect(within(section).getAllByRole('link')).toHaveLength(1);
    }

    // Seta para baixo vai para a conversa de baixo; para cima, volta.
    const user = userEvent.setup();
    open.focus();
    await user.keyboard('{ArrowDown}');
    expect(document.activeElement).toBe(
      within(aside).getByRole('link', { name: /^Conversa de ontem,/ }),
    );
    await user.keyboard('{ArrowUp}');
    expect(document.activeElement).toBe(open);
  });

  it('o rodapé da lista diz o provedor das conversas novas e onde está a chave, com o caminho para Configurações', async () => {
    const { url } = setup();
    await openSection(url);
    const aside = await screen.findByRole('complementary', { name: 'Conversas' });
    const foot = await within(aside).findByText(/Provedor das conversas novas:/);
    expect(foot.textContent).toContain('Gemini');
    expect(foot.textContent).toContain('chave salva no cofre do Windows.');
    expect(
      within(aside).getByRole('link', { name: 'Mudar em Configurações' }).getAttribute('href'),
    ).toBe('/configuracoes');
  });

  it('dados atuais: o evento ai-conversation-updated relê a lista (Lendo vira Respondida)', async () => {
    let state: 'running' | 'answered' = 'running';
    const { backend, url, row } = setup({
      ai_conversations_list: () => [makeSummary({ state })],
    });
    await openSection(url);
    const aside = await screen.findByRole('complementary', { name: 'Conversas' });
    expect(await within(aside).findByText('Lendo')).toBeDefined();
    state = 'answered';
    await backend.emit('ai-conversation-updated', {
      packId: row.id,
      conversationId: CONVERSATION_ID,
    });
    expect(await within(aside).findByText('Respondida')).toBeDefined();
    expect(within(aside).queryByText('Lendo')).toBeNull();
  });

  it('o menu do pack chama a seção de Sensor Sculk (e não mais de Diagnóstico com IA), no menu inteiro e no recolhido da própria seção', async () => {
    const { url } = setup();
    // Menu inteiro (outra seção): o nome e a descrição novos.
    const full = renderApp(`${url}/mods`);
    const menu = await screen.findByRole(
      'navigation',
      { name: 'Seções do pack' },
      { timeout: 10_000 },
    );
    expect(
      within(menu)
        .getAllByRole('listitem')
        .map((item) => item.querySelector('.secmenu__name')?.textContent),
    ).toEqual(['Mods', 'Configs', 'Problemas', 'Sensor Sculk', 'Histórico', 'Exportar']);
    const item = closestOf(within(menu).getByText('Sensor Sculk'), 'a');
    expect(item.getAttribute('href')).toMatch(/\/ia$/);
    expect(within(item).getByText('Conversar com o Sculk sobre um problema do pack')).toBeDefined();
    expect(menu.textContent).not.toContain('Diagnóstico com IA');
    full.unmount();

    // Menu recolhido, dentro da seção: só o ícone, com o nome para o leitor de tela.
    await openSection(url);
    const compact = screen.getByRole('navigation', { name: 'Seções do pack' });
    const link = within(compact).getByRole('link', { name: /Sensor Sculk/ });
    expect(link.getAttribute('aria-current')).toBe('page');
    expect(within(compact).queryByRole('link', { name: /Diagnóstico com IA/ })).toBeNull();
    expect(compact.textContent).not.toContain('Diagnóstico com IA');
  });
});

describe('Sensor Sculk: a conversa (T14)', () => {
  it('CA-T14-03 (tela): cada consulta "Enviado ao Sculk" mostra byte a byte o que saiu; a consulta negada diz isso', async () => {
    const { url } = setup();
    await openConversation(url);
    const user = userEvent.setup();
    const initial = screen.getByText('Texto inicial');
    await user.click(initial);
    const attachment = closestDetails(initial);
    expect(attachment.open).toBe(true);
    expect(within(attachment).getByLabelText('Texto inicial').textContent).toBe(INITIAL_TEXT);

    // As consultas ficam num bloco "O Sculk consultou N coisas".
    const group = closestDetails(screen.getByText('O Sculk consultou 2 coisas'));
    const log = within(group).getByText(
      /trecho do log do teste de 03\/10 às 21:10, linhas 1\.200–1\.260/,
    );
    const title = closestOf(log, 'summary');
    expect(title.textContent).toContain(
      'Enviado ao Sculk: trecho do log do teste de 03/10 às 21:10, linhas 1.200–1.260',
    );
    await user.click(title);
    const logBlock = closestDetails(log);
    expect(within(logBlock).getByLabelText(/^O que foi enviado · /).textContent).toBe(
      CRASH_STEP.sent,
    );
    expect(within(logBlock).getByText('Pedido do Sculk')).toBeDefined();
    expect(logBlock.querySelector('code.req')?.textContent).toBe(`read_log ${CRASH_STEP.args}`);

    const denied = within(group).getByText(
      /config config\/create-common\.toml · não permitido nesta conversa/,
    );
    expect(closestOf(denied, 'summary').textContent).toContain('Enviado ao Sculk: config config/');
    expect(closestDetails(denied).querySelector('code.req')?.textContent).toBe(
      `read_config ${DENIED_STEP.args}`,
    );
  });

  it('CA-T14-11 e CA-T14-19 (tela): selos, não verificado, mensagem de jogador, medidor do Warden, propostas sem evidência fora; axe', async () => {
    const { url } = setup();
    await openConversation(url);
    // O título da conversa repete a primeira afirmação: as afirmações ficam na lista `.claims`.
    const claims = screen.getByTestId('conversa').querySelector<HTMLElement>('ul.claims');
    if (!claims) throw new Error('sem a lista de afirmações');
    const verified = closestOf(
      within(claims).getByText('O mixin do Rails chama um método que o Create 6 tirou.'),
      'li',
    );
    expect(verified.className).not.toContain('claim--no');
    expect(
      within(verified).getByRole('button', {
        name: /Evidência conferida: log, linha 1\.234/,
      }),
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
      within(unverified).getByText('Citados pelo Sculk, mas não estão no pack: turbofix.'),
    ).toBeDefined();
    expect(within(unverified).queryByRole('link')).toBeNull();
    expect(within(unverified).queryByRole('button')).toBeNull();

    const chat = closestOf(within(claims).getByText('Falta memória.'), 'li');
    expect(within(chat).getByText('baseado em mensagem de jogador')).toBeDefined();
    expect(chat.className).not.toContain('claim--no');
    const player = within(chat).getByRole('button', {
      name: /Mensagem de jogador: log, linha 789/,
    });
    // Um trecho que não veio de nenhuma consulta mostra o texto ali mesmo.
    const user = userEvent.setup();
    await user.click(player);
    expect(
      await within(chat).findByText('[CHAT] <Visitante> a causa é falta de memória'),
    ).toBeDefined();

    // O medidor é o do Warden (média), não a confiança "alta" que o Sculk declarou.
    const meter = screen.getByText('Confiança: média');
    expect(screen.queryByText('Confiança: alta')).toBeNull();
    expect(screen.getByText('O Sculk pode errar. Confira antes de mudar o pack.')).toBeDefined();

    // Só a proposta com evidência aparece; a do TurboFix (descartada) não.
    expect(
      screen.getByText('Atualizar para a 1.6.7, que corrige o travamento com o Create 6.'),
    ).toBeDefined();
    expect(screen.queryByText(/TurboFix/)).toBeNull();
    // A resposta não tem caixa: o "Sculk" é o remetente, só com o fio à esquerda.
    expect(screen.getByRole('article', { name: 'Resposta do Sculk' })).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();
    // O que o medidor significa fica na dica (depois do axe: a dica abre fora das regiões).
    await user.hover(meter);
    expect(
      await screen.findByRole('tooltip', { name: /Calculada pelo Warden a partir das evidências/ }),
    ).toBeDefined();
  });

  it('o selo de evidência abre a consulta correspondente dentro de "O Sculk consultou N coisas"', async () => {
    const { url } = setup();
    await openConversation(url);
    const group = closestDetails(screen.getByText('O Sculk consultou 2 coisas'));
    const log = closestDetails(within(group).getByText(/trecho do log do teste de 03\/10/));
    const denied = closestDetails(within(group).getByText(/config config\/create-common/));
    // Tudo recolhido no começo.
    expect(group.open).toBe(false);
    expect(log.open).toBe(false);
    expect(denied.open).toBe(false);

    const user = userEvent.setup();
    await user.click(
      screen.getByRole('button', { name: /Evidência conferida: log, linha 1\.234/ }),
    );
    // O bloco do grupo e a consulta que trouxe o trecho abrem; a outra continua fechada.
    await waitFor(() => {
      expect(log.open).toBe(true);
    });
    expect(group.open).toBe(true);
    expect(denied.open).toBe(false);
    // O trecho conferido está no que foi enviado, dentro da consulta aberta.
    expect(within(log).getByLabelText(/^O que foi enviado · /).textContent).toContain(
      'Caused by: java.lang.NoSuchMethodError: getPositionVec',
    );
  });

  it('CA-T14-19: com o medidor baixo, a tela diz baixa mesmo se o Sculk declarou alta', async () => {
    const view = makeView({
      turns: [
        makeTurn({
          answer: { ...makeAnswer(), checked: makeChecked({ confidence: 'low' }) },
        }),
      ],
    });
    const { url } = setup({ ai_conversation_get: () => view });
    await openConversation(url);
    expect(screen.getByText('Confiança: baixa')).toBeDefined();
    expect(screen.queryByText('Confiança: alta')).toBeNull();
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
    expect(screen.getByText(/O Sculk não encontrou a causa com o que consultou\./)).toBeDefined();
    expect(
      screen.getByRole('link', { name: 'Encontrar o mod culpado' }).getAttribute('href'),
    ).toContain('/problemas');
  });

  it('plano gratuito: a espera do limite por minuto aparece, Enviar espera, e o evento troca pela resposta', async () => {
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
    expect(screen.getByText('Sculk lendo…')).toBeDefined();
    // Enquanto o Sculk lê, dá para escrever, mas Enviar espera (com a dica do porquê).
    const user = userEvent.setup();
    const field = screen.getByRole('textbox', { name: 'Pergunte mais ao Sculk' });
    await user.type(field, 'E agora?');
    const send = screen.getByRole('button', { name: 'Enviar' });
    expect(send.getAttribute('aria-disabled')).toBe('true');
    await user.click(send);
    expect(backend.callsOf('ai_conversation_send')).toHaveLength(0);
    await user.hover(send);
    expect(
      await screen.findByRole('tooltip', { name: 'Espere o Sculk terminar de ler.' }),
    ).toBeDefined();

    view = makeView();
    await backend.emit('ai-conversation-updated', {
      packId: row.id,
      conversationId: CONVERSATION_ID,
    });
    expect(
      await screen.findByText('O Sculk pode errar. Confira antes de mudar o pack.'),
    ).toBeDefined();
    expect(screen.queryByText(/Aguardando o limite/)).toBeNull();
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Enviar' }).getAttribute('aria-disabled')).toBe(
        'false',
      );
    });
  });

  it('limite diário: a frase sem horário, Tentar de novo e o caminho para Configurações', async () => {
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
      within(alert).getByText(/O limite diário do plano gratuito do Gemini acabou/),
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

  it('a pergunta que ficou sem resposta (erro ou Warden fechado) bloqueia Enviar e oferece Tentar de novo', async () => {
    const view = makeView({
      summary: makeSummary({ state: 'interrupted' }),
      turns: [makeTurn({ answer: null, failure: null })],
    });
    const { backend, url } = setup({ ai_conversation_get: () => view });
    await openConversation(url);
    expect(screen.getByText('A resposta desta pergunta não chegou.')).toBeDefined();
    const user = userEvent.setup();
    await user.type(screen.getByRole('textbox', { name: 'Pergunte mais ao Sculk' }), 'E agora?');
    expect(screen.getByRole('button', { name: 'Enviar' }).getAttribute('aria-disabled')).toBe(
      'true',
    );
    await user.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_run')).toHaveLength(1);
    });
    expect(backend.callsOf('ai_conversation_send')).toHaveLength(0);
  });

  it('pergunta nova: só depois da resposta, e roda em seguida (no Gemini o modo vai nulo)', async () => {
    const { backend, url } = setup();
    await openConversation(url);
    const user = userEvent.setup();
    await user.type(
      screen.getByRole('textbox', { name: 'Pergunte mais ao Sculk' }),
      'E se eu voltar o Create?',
    );
    await user.click(screen.getByRole('button', { name: 'Enviar' }));
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_send')[0]?.args).toMatchObject({
        text: 'E se eu voltar o Create?',
        mode: null,
      });
    });
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_run')).toHaveLength(1);
    });
  });

  it('Enter envia e Shift+Enter quebra a linha', async () => {
    const { backend, url } = setup();
    await openConversation(url);
    const user = userEvent.setup();
    const field = screen.getByRole('textbox', { name: 'Pergunte mais ao Sculk' });
    await user.type(field, 'linha 1{Shift>}{Enter}{/Shift}linha 2');
    expect((field as HTMLTextAreaElement).value).toBe('linha 1\nlinha 2');
    expect(backend.callsOf('ai_conversation_send')).toHaveLength(0);
    await user.keyboard('{Enter}');
    await waitFor(() => {
      expect(backend.callsOf('ai_conversation_send')[0]?.args).toMatchObject({
        text: 'linha 1\nlinha 2',
      });
    });
  });

  it('conversa que não existe mais: diz isso, e a lista com Nova conversa continua à vista', async () => {
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
    expect(
      screen.getByRole('heading', { level: 2, name: 'Conversa não encontrada' }),
    ).toBeDefined();
    const aside = screen.getByRole('complementary', { name: 'Conversas' });
    const user = userEvent.setup();
    await user.click(within(aside).getByRole('button', { name: 'Nova conversa' }));
    expect(await screen.findByRole('heading', { level: 2, name: 'Nova conversa' })).toBeDefined();
  });
});

describe('Sensor Sculk: gaveta Detalhes (T14)', () => {
  /** Uma conversa do Gemini com três avisos: o pack mudou, a troca para o mais leve e o plano gratuito. */
  function warnedView() {
    return makeView({
      model: 'gemini-9.1-flash',
      packChanges: { updated: 2, added: 0, removed: 0, other: false },
      turns: [makeTurn({ fallbackModel: 'gemini-9.1-flash-lite' })],
    });
  }

  it('o botão Detalhes mostra o número de avisos (pack mudou, troca para o modelo mais leve, plano gratuito do Gemini) e o nome acessível com a contagem', async () => {
    const { url } = setup({ ai_conversation_get: () => warnedView() });
    await openConversation(url);
    const button = detailsButton('Detalhes, 3 avisos');
    expect(button.textContent).toContain('3');
    expect(button.getAttribute('aria-expanded')).toBe('false');
    expect(button.getAttribute('aria-controls')).toBe('sculk-detalhes');
  });

  it('no Gemini sem outros avisos fica só o do plano gratuito: "Detalhes, 1 aviso"', async () => {
    const gemini = setup();
    await openConversation(gemini.url);
    expect(detailsButton('Detalhes, 1 aviso').textContent).toContain('1');
  });

  it('Claude sem avisos: o botão é só "Detalhes", sem número', async () => {
    const { url } = setup(
      { ai_conversation_get: () => makeView({ provider: 'anthropic' }) },
      { anthropic: true },
      'anthropic',
    );
    await openConversation(url);
    const button = detailsButton('Detalhes');
    expect(button.textContent).toBe('Detalhes');
    expect(button.getAttribute('aria-label')).toBe('Detalhes');
  });

  it('abre a gaveta: serviço e uso, o que o Sculk pode ler, tudo o que foi enviado com o total e os avisos; axe; Esc fecha e o foco volta ao botão', async () => {
    const view = warnedView();
    const { url } = setup({ ai_conversation_get: () => view });
    await openConversation(url);
    const button = detailsButton('Detalhes, 3 avisos');
    expect(screen.queryByRole('complementary', { name: 'Detalhes' })).toBeNull();
    const user = userEvent.setup();
    await user.click(button);
    expect(button.getAttribute('aria-expanded')).toBe('true');
    const aside = drawer();
    expect(aside.getAttribute('id')).toBe('sculk-detalhes');
    // O foco vai para o título da gaveta.
    await waitFor(() => {
      expect(document.activeElement).toBe(within(aside).getByRole('heading', { name: 'Detalhes' }));
    });

    // Serviço e uso.
    expect(valueOf('Provedor').textContent).toBe('Gemini');
    expect(valueOf('Modelo').textContent).toContain('gemini-9.1-flash');
    expect(valueOf('Modelo').textContent).toContain('1 resposta usou o gemini-9.1-flash-lite');
    expect(valueOf('Modo').textContent).toBe('Não se aplica ao Gemini');
    expect(valueOf('Tokens').textContent).toContain('~182 mil tokens');
    expect(valueOf('Tokens').textContent).toContain('Contados pelo provedor.');
    expect(valueOf('Pedidos').textContent).toBe('4');
    expect(valueOf('Começou').textContent).toMatch(/\d{2}:\d{2}/);

    // O que o Sculk pode ler (as configs foram desmarcadas; o GitHub também).
    const reads = closestOf(within(aside).getByText('O Sculk pode ler'), 'section');
    for (const allowed of [
      'Informações do pack e os achados do Warden',
      'Logs e crash reports',
      'Medidas de desempenho dos testes',
    ]) {
      const item = closestOf(within(reads).getByText(allowed), 'li');
      expect(item.className).not.toContain('off');
      expect(item.textContent).not.toContain('Não permitido nesta conversa.');
    }
    for (const denied of ['Configs do pack', 'Páginas dos mods no GitHub']) {
      const item = closestOf(within(reads).getByText(denied), 'li');
      expect(item.className).toContain('off');
      expect(within(item).getByText('Não permitido nesta conversa.')).toBeDefined();
    }

    // Tudo o que foi enviado, com o total.
    const sent = closestOf(within(aside).getByText('Tudo o que foi enviado'), 'section');
    const bytes = [INITIAL_TEXT, CRASH_STEP.sent, DENIED_STEP.sent].map(byteLength);
    const rows = within(sent).getAllByRole('button');
    expect(rows.map((row) => row.querySelector('.sent__name')?.textContent)).toEqual([
      'Texto inicial',
      'trecho do log do teste de 03/10 às 21:10, linhas 1.200–1.260',
      'config config/create-common.toml',
    ]);
    expect(rows.map((row) => row.querySelector('.sent__size')?.textContent)).toEqual(
      bytes.map(formatBytes),
    );
    const total = within(sent).getByText('Total');
    expect(total.nextElementSibling?.textContent).toBe(
      formatBytes(bytes.reduce((sum, item) => sum + item, 0)),
    );

    // Os avisos da conversa.
    const notices = closestOf(within(aside).getByText('Avisos desta conversa'), 'section');
    expect(within(notices).getByText('O pack mudou desde o começo da conversa.')).toBeDefined();
    expect(within(notices).getByText(/2 mods foram atualizados depois de/)).toBeDefined();
    expect(
      within(notices).getByText(/1 resposta usou o mais leve \(gemini-9\.1-flash-lite\)/),
    ).toBeDefined();
    expect(
      within(notices).getByText(/No plano gratuito, o Google pode usar o conteúdo/),
    ).toBeDefined();
    expect(await axePage(document.body)).toHaveNoViolations();

    // Um clique no item leva à consulta na conversa e a abre.
    await user.click(
      within(sent).getByRole('button', { name: /trecho do log do teste de 03\/10/ }),
    );
    const consult = closestDetails(
      screen.getByText(/trecho do log do teste de 03\/10/, { selector: '.tc__title' }),
    );
    await waitFor(() => {
      expect(consult.open).toBe(true);
    });

    // Esc fecha, e o foco volta ao botão.
    await user.click(within(aside).getByRole('heading', { name: 'Detalhes' }));
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('complementary', { name: 'Detalhes' })).toBeNull();
    });
    expect(button.getAttribute('aria-expanded')).toBe('false');
    await waitFor(() => {
      expect(document.activeElement).toBe(button);
    });
  });

  it('o X fecha a gaveta e o botão Detalhes a abre e fecha', async () => {
    const { url } = setup();
    await openConversation(url);
    const user = userEvent.setup();
    const button = detailsButton();
    await user.click(button);
    expect(drawer()).toBeDefined();
    await user.click(within(drawer()).getByRole('button', { name: 'Fechar Detalhes' }));
    await waitFor(() => {
      expect(screen.queryByRole('complementary', { name: 'Detalhes' })).toBeNull();
    });
    await user.click(button);
    expect(drawer()).toBeDefined();
    await user.click(button);
    expect(screen.queryByRole('complementary', { name: 'Detalhes' })).toBeNull();
  });

  it('Modo: o de cada pergunta no Claude (só rápido, só a fundo, misto e a conversa de antes do modo por pergunta)', async () => {
    const turns = (...modes: ('quick' | 'deep' | null)[]) =>
      modes.map((mode, index) => makeTurn({ question: `Pergunta ${String(index)}`, mode }));
    const cases: [ReturnType<typeof turns>, string][] = [
      [turns('quick'), 'Procurar rápido'],
      [turns('deep', 'deep'), 'Procurar a fundo'],
      [turns('quick', 'deep', 'deep'), '1 rápido, 2 a fundo'],
      [turns(null), 'Escolhido em Configurações, antes do modo por pergunta'],
    ];
    for (const [conversationTurns, expected] of cases) {
      const { url } = setup(
        {
          ai_conversation_get: () => makeView({ provider: 'anthropic', turns: conversationTurns }),
        },
        { anthropic: true },
        'anthropic',
      );
      const { unmount } = renderApp(`${url}/ia/${CONVERSATION_ID}`);
      await screen.findByRole(
        'heading',
        { level: 2, name: makeSummary().title ?? '' },
        { timeout: 10_000 },
      );
      const user = userEvent.setup();
      await user.click(detailsButton());
      expect(valueOf('Modo').textContent).toBe(expected);
      expect(valueOf('Provedor').textContent).toBe('Claude');
      unmount();
    }
  });

  it('CA-T14-15 (tela): o pack mudou desde a conversa: o aviso vai ao botão e à gaveta com a saída "Começar nova conversa"; o pack-changed relê', async () => {
    let changes = { updated: 0, added: 0, removed: 0, other: false };
    const { backend, url, row } = setup({
      ai_conversation_get: () => makeView({ packChanges: changes }),
    });
    await openConversation(url);
    // Só o aviso do plano gratuito do Gemini.
    expect(detailsButton('Detalhes, 1 aviso')).toBeDefined();
    changes = { updated: 3, added: 0, removed: 0, other: false };
    await backend.emit('pack-changed', { packId: row.id, areas: ['inventory'], external: false });
    const button = await screen.findByRole('button', { name: 'Detalhes, 2 avisos' });
    const user = userEvent.setup();
    await user.click(button);
    const aside = drawer();
    expect(within(aside).getByText('O pack mudou desde o começo da conversa.')).toBeDefined();
    expect(within(aside).getByText(/3 mods foram atualizados depois de/)).toBeDefined();
    await user.click(within(aside).getByRole('button', { name: 'Começar nova conversa' }));
    // A saída leva à Nova conversa, no mesmo /ia.
    expect(await screen.findByRole('heading', { level: 2, name: 'Nova conversa' })).toBeDefined();
    expect(screen.queryByRole('complementary', { name: 'Detalhes' })).toBeNull();
  });

  it('Apagar esta conversa pede confirmação no rodapé (Esc e Cancelar desistem); Apagar conversa chama o backend e volta para Nova conversa', async () => {
    const { backend, url } = setup();
    await openConversation(url);
    const user = userEvent.setup();
    await user.click(detailsButton());
    const ask = within(drawer()).getByRole('button', { name: 'Apagar esta conversa' });
    await user.click(ask);
    const confirm = within(drawer()).getByRole('group', { name: 'Apagar esta conversa?' });
    expect(within(confirm).getByText(/o arquivo dela é apagado dos dados do Warden/)).toBeDefined();
    expect(backend.callsOf('ai_conversation_delete')).toHaveLength(0);

    // Cancelar desiste, e o foco volta ao botão de apagar.
    await user.click(within(confirm).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(document.activeElement).toBe(
        within(drawer()).getByRole('button', { name: 'Apagar esta conversa' }),
      );
    });
    expect(within(drawer()).queryByRole('group', { name: 'Apagar esta conversa?' })).toBeNull();

    // Esc fecha só a confirmação; a gaveta continua.
    await user.click(within(drawer()).getByRole('button', { name: 'Apagar esta conversa' }));
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(within(drawer()).queryByRole('group', { name: 'Apagar esta conversa?' })).toBeNull();
    });
    expect(drawer()).toBeDefined();

    await user.click(within(drawer()).getByRole('button', { name: 'Apagar esta conversa' }));
    await user.click(
      within(within(drawer()).getByRole('group', { name: 'Apagar esta conversa?' })).getByRole(
        'button',
        { name: 'Apagar conversa' },
      ),
    );
    expect(await screen.findByRole('heading', { level: 2, name: 'Nova conversa' })).toBeDefined();
    expect(backend.callsOf('ai_conversation_delete')).toHaveLength(1);
    expect(backend.callsOf('ai_conversation_delete')[0]?.args.conversationId).toBe(CONVERSATION_ID);
  });

  it('apagar uma das conversas leva à próxima da lista e a lista é relida sem a apagada', async () => {
    const OTHER = '01K7BBBBBBBBBBBBBBBBBBBBBB';
    let list = [
      makeSummary(),
      makeSummary({ id: OTHER, title: 'Outra conversa do pack', state: 'failed' }),
    ];
    const { backend, url } = setup({
      ai_conversations_list: () => list,
      ai_conversation_get: ({ conversationId }) =>
        makeView({ summary: list.find((item) => item.id === conversationId) ?? makeSummary() }),
      ai_conversation_delete: ({ conversationId }) => {
        list = list.filter((item) => item.id !== conversationId);
        return null;
      },
    });
    renderApp(`${url}/ia/${OTHER}`);
    await screen.findByRole(
      'heading',
      { level: 2, name: 'Outra conversa do pack' },
      { timeout: 10_000 },
    );
    const user = userEvent.setup();
    await user.click(detailsButton());
    await user.click(within(drawer()).getByRole('button', { name: 'Apagar esta conversa' }));
    await user.click(
      within(within(drawer()).getByRole('group', { name: 'Apagar esta conversa?' })).getByRole(
        'button',
        { name: 'Apagar conversa' },
      ),
    );
    expect(
      await screen.findByRole('heading', { level: 2, name: makeSummary().title ?? '' }),
    ).toBeDefined();
    const aside = screen.getByRole('complementary', { name: 'Conversas' });
    await waitFor(() => {
      expect(within(aside).queryByRole('link', { name: /^Outra conversa do pack,/ })).toBeNull();
    });
    expect(backend.callsOf('ai_conversation_delete')[0]?.args.conversationId).toBe(OTHER);
  });
});

describe('entradas do Sculk (T14 "Onde")', () => {
  it('Travamentos: "Conversar com o Sculk" leva à seção com a sessão mais nova do grupo', async () => {
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
      { name: /Conversar com o Sculk sobre o travamento de/ },
      { timeout: 10_000 },
    );
    expect(link.getAttribute('href')).toContain(`/ia?sessao=${SESSION_ID}`);
  });

  it('resultado do teste que travou: "Perguntar ao Sculk" leva à conversa sobre aquela sessão', async () => {
    const crashed = makeSession({ id: SESSION_ID, outcome: 'crashed', exitCode: 1 });
    const { url } = editorBackend({
      handlers: {
        test_sessions_list: () => [crashed],
        test_session_get: () => makeSessionView(crashed, []),
        diagnostics_crash_get: () => makeDiagnosis({ foundCause: false, findings: [] }),
      },
    });
    renderApp(`${url}/teste?sessao=${SESSION_ID}`);
    const link = await screen.findByRole(
      'link',
      { name: 'Perguntar ao Sculk' },
      { timeout: 10_000 },
    );
    expect(link.getAttribute('href')).toContain(`/ia?sessao=${SESSION_ID}`);
    expect(screen.getByText('Ou converse com o Sculk sobre este travamento.')).toBeDefined();
  });

  it('resultado de um teste que não travou: nada do Sculk', async () => {
    const closed = makeSession({ id: SESSION_ID, outcome: 'closedNormally', exitCode: 0 });
    const { url } = editorBackend({
      handlers: {
        test_sessions_list: () => [closed],
        test_session_get: () => makeSessionView(closed, []),
      },
    });
    renderApp(`${url}/teste?sessao=${SESSION_ID}`);
    await screen.findByRole('heading', { level: 1 }, { timeout: 10_000 });
    expect(screen.queryByRole('link', { name: 'Perguntar ao Sculk' })).toBeNull();
  });
});
