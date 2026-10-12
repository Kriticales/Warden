import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import i18n from '../../i18n';
import type {
  GithubLoginStatus,
  OperationEvent,
  ProblemReportInput,
  ProblemReportPreview,
  ReportImage,
} from '../../lib/ipc/bindings';
import { showToast, useToastStore } from '../../components/ui/toast';
import { appErrorMessage } from '../../lib/ipc/errors';
import { forgetShownError } from '../../lib/ipc/last-error';
import { axeComponent } from '../../test/axe';
import { ipcError, mockBackend, type Handler } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { sendOnChannel } from '../test/test.fixtures';
import { isReportShortcut } from './ReportProblem';
import { screenOf } from './screen';
import { closeReportDialog } from './store';

const URL_PREFIX = 'https://github.com/Kriticales/Warden/issues/new?title=';

/** A pré-visualização que o Rust devolveria para a entrada (corpo com o título e a tela). */
function fakePreview(
  input: ProblemReportInput,
  overrides: Partial<ProblemReportPreview> = {},
): ProblemReportPreview {
  const body = `**Categoria:** ${input.category}\n\n### Descrição\n\n${input.description}\n\n### Ambiente\n\n- **Tela:** ${input.screen}\n`;
  const prints = input.attachments
    .map((token, index) => `![Print ${String(index + 1)}: ${token}.png](…)`)
    .join('\n');
  return {
    title: input.title,
    labels: ['relato-do-dono', `categoria: ${input.category}`],
    directBody: `${body}${prints === '' ? '' : `\n### Prints\n\n${prints}\n`}(direto)`,
    directLogLines: input.includeLog ? 30 : 0,
    body,
    url: `${URL_PREFIX}${encodeURIComponent(input.title)}&labels=relato-do-dono&body=${encodeURIComponent(body)}`,
    fullBody: `${body}\n(completo)`,
    cut: 'none',
    environment: [
      { label: 'Warden', value: '0.1.0 (commit 1a2b3c4d5e6f)' },
      { label: 'Sistema', value: 'Ubuntu 24.04 LTS (Linux 6.8.0)' },
      { label: 'Tela', value: input.screen },
      { label: 'Pack aberto', value: 'nenhum' },
      {
        label: 'Último erro mostrado',
        value: input.lastError
          ? input.lastError.code === null
            ? input.lastError.message
            : `${input.lastError.code}: ${input.lastError.message}`
          : 'nenhum',
      },
    ],
    logLines: input.includeLog ? 12 : 0,
    ...overrides,
  };
}

function reportBackend(handlers: Record<string, Handler> = {}) {
  return mockBackend({
    problem_report_preview: (args) => fakePreview(args.input as ProblemReportInput),
    problem_report_open: () => null,
    ...handlers,
  });
}

function lastInput(backend: ReturnType<typeof mockBackend>): ProblemReportInput {
  const calls = backend.callsOf('problem_report_preview');
  return calls.at(-1)?.args.input as ProblemReportInput;
}

async function openFromButton() {
  const user = userEvent.setup();
  renderApp('/packs');
  await user.click(await screen.findByRole('button', { name: 'Relatar problema' }));
  return { user, dialog: await screen.findByRole('dialog', { name: 'Relatar problema' }) };
}

/** Aperta o atalho Ctrl+Shift+E. */
function pressShortcut() {
  fireEvent.keyDown(window, { key: 'E', code: 'KeyE', ctrlKey: true, shiftKey: true });
}

afterEach(() => {
  closeReportDialog();
  forgetShownError();
  useToastStore.getState().clear();
  vi.restoreAllMocks();
});

describe('Relatar problema (UI-02)', () => {
  it('CA-UI02-01: o botão do rodapé abre o diálogo; sem título não abre o GitHub; passa no axe', async () => {
    const backend = reportBackend();
    const { dialog } = await openFromButton();
    expect(
      within(dialog).getByText(
        'Este repositório é público: o relato fica visível para qualquer pessoa.',
      ),
    ).toBeDefined();
    expect(
      within(dialog).getByRole('checkbox', { name: /Incluir os registros do app/ }),
    ).toHaveProperty('checked', true);
    // A pré-visualização sai mesmo sem título, mas o botão fica desabilitado.
    expect(await within(dialog).findByTestId('report-preview-body')).toBeDefined();
    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    expect(open).toHaveProperty('disabled', true);
    expect(lastInput(backend)).toMatchObject({
      title: '',
      includeLog: true,
      screen: 'Meus packs',
      packId: null,
      lastError: null,
    });
    expect(within(dialog).getByText('Ubuntu 24.04 LTS (Linux 6.8.0)')).toBeDefined();
    expect(within(dialog).getByText('Registros: 12 linhas')).toBeDefined();
    expect(
      within(dialog).getByText(
        'Para mandar um print, cole a imagem (Ctrl+V) na caixa de texto da issue, no GitHub.',
      ),
    ).toBeDefined();
    expect(await axeComponent(dialog)).toHaveNoViolations();
  });

  it('CA-UI02-02: a pré-visualização segue os campos e o GitHub recebe o endereço mostrado', async () => {
    const backend = reportBackend();
    const { user, dialog } = await openFromButton();
    await user.type(within(dialog).getByLabelText(/^Título/), 'O Testar travou');
    await user.type(within(dialog).getByTestId('report-description'), 'Nada abriu');
    await user.click(within(dialog).getByRole('checkbox', { name: /Incluir os registros do app/ }));

    const body = within(dialog).getByTestId('report-preview-body');
    await waitFor(() => {
      expect(body.textContent).toContain('Nada abriu');
    });
    expect(within(dialog).getByTestId('report-preview-title').textContent).toBe('O Testar travou');
    expect(lastInput(backend)).toMatchObject({
      category: 'error',
      title: 'O Testar travou',
      description: 'Nada abriu',
      attachments: [],
      includeLog: false,
    });
    expect(within(dialog).getByText('Registros: não incluídos')).toBeDefined();

    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    await waitFor(() => {
      expect(open).toHaveProperty('disabled', false);
    });
    await user.click(open);
    await waitFor(() => {
      expect(backend.callsOf('problem_report_open')).toHaveLength(1);
    });
    const url = backend.callsOf('problem_report_open')[0]?.args.url as string;
    expect(url).toBe(fakePreview(lastInput(backend)).url);
    expect(url.startsWith(URL_PREFIX)).toBe(true);
    expect(await screen.findByText('Relato aberto no navegador')).toBeDefined();
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Relatar problema' })).toBeNull();
    });
  });

  it('título apagado mostra o erro e desabilita de novo', async () => {
    reportBackend();
    const { user, dialog } = await openFromButton();
    const title = within(dialog).getByLabelText(/^Título/);
    await user.type(title, 'x');
    await user.clear(title);
    await user.tab();
    expect(within(dialog).getByText('Escreva um título para enviar o relato.')).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Abrir no GitHub' })).toHaveProperty(
      'disabled',
      true,
    );
  });

  it('CA-UI02-03: com corte, copia o relato completo e avisa', async () => {
    const backend = reportBackend({
      problem_report_preview: (args) =>
        fakePreview(args.input as ProblemReportInput, { cut: 'log', logLines: 40 }),
    });
    const { user, dialog } = await openFromButton();
    // O `userEvent.setup` instala a área de transferência simulada; aqui só se observa.
    const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
    await user.type(within(dialog).getByLabelText(/^Título/), 'Registro grande');
    expect(
      await within(dialog).findByText(/Os registros são longos demais para o endereço da issue/),
    ).toBeDefined();
    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    await waitFor(() => {
      expect(open).toHaveProperty('disabled', false);
    });
    await user.click(open);
    await waitFor(() => {
      expect(backend.callsOf('problem_report_open')).toHaveLength(1);
    });
    expect(writeText).toHaveBeenCalledWith(fakePreview(lastInput(backend)).fullBody);
    expect(
      await screen.findByText(
        'O relato completo está na área de transferência: cole na issue (Ctrl+V) se quiser mandar tudo.',
      ),
    ).toBeDefined();
  });

  it('o navegador que não abre mostra o erro e mantém o diálogo', async () => {
    reportBackend({
      problem_report_open: () => ipcError(makeAppError({ domain: 'app', code: 'INTERNAL' })),
    });
    const { user, dialog } = await openFromButton();
    await user.type(within(dialog).getByLabelText(/^Título/), 'Algo');
    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    await waitFor(() => {
      expect(open).toHaveProperty('disabled', false);
    });
    await user.click(open);
    expect(await within(dialog).findByText('O navegador não abriu')).toBeDefined();
  });

  it('o atalho Ctrl+Shift+E abre o diálogo com o último erro mostrado; Esc fecha', async () => {
    const backend = reportBackend();
    const user = userEvent.setup();
    renderApp('/packs');
    await screen.findByRole('button', { name: 'Relatar problema' });
    // Um erro mostrado num toast (como o TestPage e o WhyCrashed fazem).
    showToast({
      kind: 'danger',
      title: appErrorMessage(makeAppError({ domain: 'core', code: 'TIMEOUT' })),
    });
    await screen.findByText('A operação demorou demais e foi interrompida. Tente de novo.');
    pressShortcut();
    const dialog = await screen.findByRole('dialog', { name: 'Relatar problema' });
    await waitFor(() => {
      expect(lastInput(backend).lastError).toEqual({
        code: 'core.TIMEOUT',
        message: 'A operação demorou demais e foi interrompida. Tente de novo.',
      });
    });
    expect(dialog).toBeDefined();
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Relatar problema' })).toBeNull();
    });
  });

  it('CA-UI02-01: fechado com Esc ou Cancelar, o foco volta ao botão "Relatar problema"', async () => {
    reportBackend();
    const { user } = await openFromButton();
    const button = screen.getByRole('button', { name: 'Relatar problema', hidden: true });
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Relatar problema' })).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(button);
    });

    await user.click(button);
    const dialog = await screen.findByRole('dialog', { name: 'Relatar problema' });
    await user.click(within(dialog).getByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Relatar problema' })).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(button);
    });
  });

  it('aberto pelo atalho, o foco volta ao que estava focado; sem nada focado, ao botão', async () => {
    reportBackend();
    const user = userEvent.setup();
    renderApp('/packs');
    const button = await screen.findByRole('button', { name: 'Relatar problema' });
    const tasks = screen.getByTestId('tasks-indicator');

    tasks.focus();
    pressShortcut();
    await screen.findByRole('dialog', { name: 'Relatar problema' });
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Relatar problema' })).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(tasks);
    });

    tasks.blur();
    expect(document.activeElement).toBe(document.body);
    pressShortcut();
    await screen.findByRole('dialog', { name: 'Relatar problema' });
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Relatar problema' })).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(button);
    });
  });
});

const CONNECTED: GithubLoginStatus = { available: true, login: 'Kriticales', waiting: false };
const ISSUE_URL = 'https://github.com/Kriticales/Warden/issues/42';

function image(token: string, name: string): ReportImage {
  return { token, name, size: 2048, preview: 'data:image/png;base64,iVBORw0KGgo=' };
}

/** O backend com o login do GitHub feito. */
function directBackend(handlers: Record<string, Handler> = {}) {
  return reportBackend({
    github_login_status: () => CONNECTED,
    problem_report_send: () => ({ number: 42, url: ISSUE_URL }),
    problem_report_detach: () => null,
    problem_report_choose_images: () => ({ added: [image('a1', 'tela.png')], refused: [] }),
    ...handlers,
  });
}

describe('Relatar problema direto no GitHub (UI-03)', () => {
  it('CA-UI03-01: com login, categoria, título, descrição, prints e Enviar; passa no axe', async () => {
    const backend = directBackend();
    const { dialog } = await openFromButton();
    const category = await within(dialog).findByLabelText('Categoria');
    expect((category as HTMLSelectElement).value).toBe('error');
    expect(
      Array.from((category as HTMLSelectElement).options).map((option) => option.text),
    ).toEqual(['Erro', 'Sugestão', 'Visual ou texto', 'Lentidão', 'Outro']);
    expect(within(dialog).getByLabelText(/^Descrição/)).toBeDefined();
    expect(within(dialog).getByRole('region', { name: /^Prints/ })).toBeDefined();
    const send = await within(dialog).findByRole('button', { name: 'Enviar' });
    expect(send).toHaveProperty('disabled', true);
    expect(within(dialog).queryByRole('button', { name: 'Abrir no GitHub' })).toBeNull();
    expect(
      within(dialog).queryByText(
        'Entre com o GitHub nas Configurações para enviar direto, com prints.',
      ),
    ).toBeNull();
    // A pré-visualização mostra o corpo do envio direto.
    await waitFor(() => {
      expect(within(dialog).getByTestId('report-preview-body').textContent).toContain('(direto)');
    });
    // O rascunho de prints abriu com o diálogo.
    expect(backend.callsOf('problem_report_draft')[0]?.args).toEqual({ open: true });
    expect(await axeComponent(dialog)).toHaveNoViolations();
  });

  it('CA-UI03-05: envia com categoria e print e mostra "Relato enviado (#42)" com o link', async () => {
    const backend = directBackend();
    const { user, dialog } = await openFromButton();
    await user.selectOptions(await within(dialog).findByLabelText('Categoria'), 'suggestion');
    await user.type(within(dialog).getByLabelText(/^Título/), 'Botão novo');
    await user.type(within(dialog).getByTestId('report-description'), 'Seria bom');
    await user.click(within(dialog).getByRole('button', { name: 'Escolher imagens…' }));
    expect(await within(dialog).findByRole('img', { name: 'Print 1: tela.png' })).toBeDefined();
    expect(
      within(dialog).getByText(
        'Os prints vão para o repositório do Warden e ficam visíveis como o relato. Confira se não mostram dados pessoais.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByText('1 print acrescentado.')).toBeDefined();
    const send = within(dialog).getByRole('button', { name: 'Enviar' });
    await waitFor(() => {
      expect(send).toHaveProperty('disabled', false);
    });
    await user.click(send);
    await waitFor(() => {
      expect(backend.callsOf('problem_report_send')).toHaveLength(1);
    });
    expect(backend.callsOf('problem_report_send')[0]?.args.input).toMatchObject({
      category: 'suggestion',
      title: 'Botão novo',
      description: 'Seria bom',
      attachments: ['a1'],
      includeLog: true,
    });
    const sent = await screen.findByTestId('report-sent');
    expect(within(sent).getByText('Relato enviado (#42)')).toBeDefined();
    expect(await axeComponent(screen.getByRole('dialog'))).toHaveNoViolations();
    await user.click(screen.getByRole('button', { name: 'Ver no GitHub' }));
    await waitFor(() => {
      expect(backend.callsOf('problem_report_open').at(-1)?.args.url).toBe(ISSUE_URL);
    });
    await user.click(screen.getByRole('button', { name: 'Concluir' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Relatar problema' })).toBeNull();
    });
    expect(backend.callsOf('problem_report_draft').at(-1)?.args).toEqual({ open: false });
  });

  it('o progresso do envio é anunciado', async () => {
    let channel: unknown;
    const finished = deferredSent();
    directBackend({
      problem_report_send: (args) => {
        channel = args.onEvent;
        return finished.promise;
      },
    });
    const { user, dialog } = await openFromButton();
    await user.type(await within(dialog).findByLabelText(/^Título/), 'Lento');
    const send = within(dialog).getByRole('button', { name: 'Enviar' });
    await waitFor(() => {
      expect(send).toHaveProperty('disabled', false);
    });
    await user.click(send);
    const progress = within(dialog).getByTestId('report-progress');
    expect(progress.getAttribute('aria-live')).toBe('polite');
    const events: OperationEvent[] = [
      { type: 'started', operationId: '01J9ZQ0000000000000000000A', kind: 'report.send' },
      { type: 'stage', stage: 'prints', labelKey: 'relato:etapas.prints' },
      { type: 'progress', current: 1, total: 2, unit: 'items' },
    ];
    await waitFor(() => {
      expect(channel).toBeDefined();
    });
    act(() => {
      sendOnChannel(channel, events);
    });
    expect(await within(progress).findByText('Enviando print 2 de 2…')).toBeDefined();
    act(() => {
      sendOnChannel(channel, [{ type: 'stage', stage: 'issue', labelKey: 'relato:etapas.issue' }]);
    });
    expect(await within(progress).findByText('Criando a issue…')).toBeDefined();
    finished.resolve({ number: 7, url: ISSUE_URL });
    expect(await screen.findByText('Relato enviado (#7)')).toBeDefined();
  });

  it('CA-UI03-07: imagem recusada mostra o motivo; Remover tira o print; Ctrl+V cola', async () => {
    const backend = directBackend({
      problem_report_choose_images: () => ({
        added: [image('a1', 'tela.png')],
        refused: [
          {
            name: 'enorme.png',
            error: makeAppError(
              { domain: 'app', code: 'REPORT_IMAGE_TOO_LARGE' },
              { params: { name: 'enorme.png', maxMb: '10' } },
            ),
          },
        ],
      }),
      problem_report_attach: (args) => image('b2', args.name as string),
    });
    const { user, dialog } = await openFromButton();
    await user.click(await within(dialog).findByRole('button', { name: 'Escolher imagens…' }));
    expect(
      await within(dialog).findByText(
        'enorme.png passa de 10 MB. Corte a imagem ou salve em JPEG e tente de novo.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByRole('img', { name: 'Print 1: tela.png' })).toBeDefined();

    // Colar uma imagem (Ctrl+V) em qualquer ponto do diálogo.
    const file = new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], 'colado.png', {
      type: 'image/png',
    });
    fireEvent.paste(within(dialog).getByLabelText(/^Título/), {
      clipboardData: { files: [file], items: [], types: ['Files'] },
    });
    expect(await within(dialog).findByRole('img', { name: 'Print 2: colado.png' })).toBeDefined();
    expect(backend.callsOf('problem_report_attach')[0]?.args).toEqual({
      name: 'colado.png',
      data: 'iVBORw==',
    });

    await user.click(within(dialog).getByRole('button', { name: 'Remover tela.png' }));
    expect(within(dialog).queryByRole('img', { name: /tela\.png/ })).toBeNull();
    expect(backend.callsOf('problem_report_detach')[0]?.args).toEqual({ token: 'a1' });
    expect(within(dialog).getByText('tela.png removido.')).toBeDefined();
  });

  it('CA-UI03-07: arrastar imagens sobre a janela acrescenta prints', async () => {
    const backend = directBackend();
    const { dialog } = await openFromButton();
    await within(dialog).findByRole('region', { name: /^Prints/ });
    await backend.emit('report-images-dropped', { added: [image('c3', 'solta.jpg')], refused: [] });
    expect(await within(dialog).findByRole('img', { name: 'Print 1: solta.jpg' })).toBeDefined();
  });

  it('CA-UI03-08: falha no envio mantém o rascunho, com Tentar de novo e Abrir no GitHub', async () => {
    let attempts = 0;
    const backend = directBackend({
      problem_report_send: () => {
        attempts += 1;
        if (attempts === 1) {
          ipcError(makeAppError({ domain: 'core', code: 'NETWORK_UNAVAILABLE' }));
        }
        return { number: 43, url: ISSUE_URL.replace('42', '43') };
      },
    });
    const { user, dialog } = await openFromButton();
    await user.type(await within(dialog).findByLabelText(/^Título/), 'Sem internet');
    await user.type(within(dialog).getByTestId('report-description'), 'texto guardado');
    await user.click(within(dialog).getByRole('button', { name: 'Escolher imagens…' }));
    await within(dialog).findByRole('img', { name: 'Print 1: tela.png' });
    const send = within(dialog).getByRole('button', { name: 'Enviar' });
    await waitFor(() => {
      expect(send).toHaveProperty('disabled', false);
    });
    await user.click(send);
    expect(await within(dialog).findByText('O relato não foi enviado')).toBeDefined();
    expect(
      within(dialog).getByText('O rascunho continua aqui: tente de novo ou use Abrir no GitHub.'),
    ).toBeDefined();
    // Nada se perdeu.
    expect(within(dialog).getByLabelText<HTMLInputElement>(/^Título/).value).toBe('Sem internet');
    expect(within(dialog).getByTestId<HTMLTextAreaElement>('report-description').value).toBe(
      'texto guardado',
    );
    expect(within(dialog).getByRole('img', { name: 'Print 1: tela.png' })).toBeDefined();
    // "Abrir no GitHub" continua à mão.
    await user.click(within(dialog).getByRole('button', { name: 'Abrir no GitHub' }));
    await waitFor(() => {
      expect(backend.callsOf('problem_report_open')).toHaveLength(1);
    });
    expect(backend.callsOf('problem_report_open')[0]?.args.url).toBe(
      fakePreview(lastInput(backend)).url,
    );
  });

  it('CA-UI03-08: Tentar de novo envia o mesmo rascunho', async () => {
    let attempts = 0;
    const backend = directBackend({
      problem_report_send: () => {
        attempts += 1;
        if (attempts === 1) {
          ipcError(makeAppError({ domain: 'github', code: 'RATE_LIMITED' }));
        }
        return { number: 43, url: ISSUE_URL.replace('42', '43') };
      },
    });
    const { user, dialog } = await openFromButton();
    await user.type(await within(dialog).findByLabelText(/^Título/), 'Limite');
    const send = within(dialog).getByRole('button', { name: 'Enviar' });
    await waitFor(() => {
      expect(send).toHaveProperty('disabled', false);
    });
    await user.click(send);
    expect(
      await within(dialog).findByText(
        'O GitHub limitou os pedidos do Warden por um tempo. Espere alguns minutos e clique em Tentar de novo.',
      ),
    ).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Tentar de novo' }));
    expect(await screen.findByText('Relato enviado (#43)')).toBeDefined();
    const inputs = backend.callsOf('problem_report_send').map((call) => call.args.input);
    expect(inputs).toHaveLength(2);
    expect(inputs[1]).toEqual(inputs[0]);
  });

  it('CA-UI03-10: sem login (ou sem o GitHub App), o aviso e o Abrir no GitHub de hoje', async () => {
    reportBackend({
      github_login_status: () => ({ available: false, login: null, waiting: false }),
    });
    const { dialog } = await openFromButton();
    expect(
      await within(dialog).findByText(
        'Entre com o GitHub nas Configurações para enviar direto, com prints.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Ir para Configurações' })).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Abrir no GitHub' })).toBeDefined();
    expect(within(dialog).queryByRole('button', { name: 'Enviar' })).toBeNull();
    expect(within(dialog).queryByRole('region', { name: /^Prints/ })).toBeNull();
    expect(await axeComponent(dialog)).toHaveNoViolations();
  });
});

/** Resposta do envio que só chega quando o teste manda. */
function deferredSent() {
  let resolve: (value: { number: number; url: string }) => void = () => undefined;
  const promise = new Promise<{ number: number; url: string }>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe('peças do relato', () => {
  it('o atalho é Ctrl+Shift+E e mais nada', () => {
    const key = (init: KeyboardEventInit) => new KeyboardEvent('keydown', init);
    expect(isReportShortcut(key({ code: 'KeyE', ctrlKey: true, shiftKey: true }))).toBe(true);
    expect(isReportShortcut(key({ code: 'KeyE', ctrlKey: true }))).toBe(false);
    expect(
      isReportShortcut(key({ code: 'KeyE', ctrlKey: true, shiftKey: true, altKey: true })),
    ).toBe(false);
    expect(isReportShortcut(key({ code: 'F1' }))).toBe(false);
  });

  it('a tela vem da rota mais funda, nos termos da interface', () => {
    const t = i18n.getFixedT('pt-BR', 'relato');
    expect(
      screenOf(
        [
          { routeId: '__root__', params: {} },
          { routeId: '/configuracoes', params: {} },
        ],
        t,
      ),
    ).toEqual({
      screen: 'Configurações',
      packId: null,
    });
    expect(screenOf([{ routeId: '/packs/$packId/mods', params: { packId: 'abc' } }], t)).toEqual({
      screen: 'Pack › Mods',
      packId: 'abc',
    });
    expect(screenOf([{ routeId: '/outra', params: {} }], t).screen).toBe('Tela não identificada');
    expect(screenOf([], t).screen).toBe('Tela não identificada');
  });
});
