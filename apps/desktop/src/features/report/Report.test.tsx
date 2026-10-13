import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import i18n from '../../i18n';
import type {
  ProblemReportInput,
  ProblemReportPreview,
  ReportCategory,
} from '../../lib/ipc/bindings';
import { showToast, useToastStore } from '../../components/ui/toast';
import { appErrorMessage } from '../../lib/ipc/errors';
import { forgetShownError } from '../../lib/ipc/last-error';
import { axeComponent } from '../../test/axe';
import { ipcError, mockBackend, type Handler } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { CAPTURING_CLASS } from './ReportDialog';
import { isReportShortcut } from './ReportProblem';
import { screenOf } from './screen';
import { closeReportDialog } from './store';

const URL_PREFIX = 'https://github.com/Kriticales/Warden/issues/new?title=';

/** Nome de cada categoria, como o Rust escreve no corpo. */
const CATEGORY_NAMES: Record<ReportCategory, string> = {
  error: 'Erro',
  visual: 'Visual ou texto',
  slowness: 'Lentidão',
  suggestion: 'Sugestão',
  other: 'Outro',
};

/** A pré-visualização que o Rust devolveria para a entrada (corpo com o título e a tela). */
function fakePreview(
  input: ProblemReportInput,
  overrides: Partial<ProblemReportPreview> = {},
): ProblemReportPreview {
  const name = CATEGORY_NAMES[input.category];
  const label = `categoria: ${name.toLowerCase()}`;
  const body = `## ${name}\n\n${input.description}\n\n### Ambiente\n\n- **Tela:** ${input.screen}\n`;
  return {
    title: input.title,
    body,
    url: `${URL_PREFIX}${encodeURIComponent(input.title)}&labels=${encodeURIComponent(`relato-do-dono,${label}`)}&body=${encodeURIComponent(body)}`,
    labels: ['relato-do-dono', label],
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
    problem_report_capture: () => ({ width: 1280, height: 800 }),
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

describe('Relatar problema (UI-02 e UI-03)', () => {
  it('CA-UI02-01 e CA-UI03-08: o botão abre o diálogo; sem título não abre o GitHub; passa no axe', async () => {
    const backend = reportBackend();
    const { dialog } = await openFromButton();
    expect(
      within(dialog).getByText(
        'Vira uma issue no GitHub já preenchida. Lá você cola os prints e envia.',
      ),
    ).toBeDefined();
    expect(
      within(dialog).getByText(
        'O repositório é público: o relato fica visível para qualquer pessoa.',
      ),
    ).toBeDefined();
    expect(within(dialog).getByText('Ctrl+Shift+E')).toBeDefined();
    expect(
      within(dialog).getByRole('checkbox', { name: /Incluir os registros do app/ }),
    ).toHaveProperty('checked', true);
    expect(
      within(dialog).getByRole('checkbox', { name: /Copiar um print desta tela/ }),
    ).toHaveProperty('checked', false);
    // A pré-visualização sai mesmo sem título, mas o botão fica desabilitado.
    expect(await within(dialog).findByTestId('report-preview-body')).toBeDefined();
    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    expect(open).toHaveProperty('disabled', true);
    expect(lastInput(backend)).toEqual({
      category: 'error',
      title: '',
      description: '',
      includeLog: true,
      screen: 'Meus packs',
      packId: null,
      lastError: null,
    });
    expect(await axeComponent(dialog)).toHaveNoViolations();
  });

  it('CA-UI03-01: a categoria é um grupo de rádio, começa em Erro e vai para o relato', async () => {
    const backend = reportBackend();
    const { user, dialog } = await openFromButton();
    const group = within(dialog).getByRole('group', { name: 'Categoria' });
    const radios = within(group).getAllByRole('radio');
    const names = radios.map(
      (radio) => document.getElementById(radio.getAttribute('aria-labelledby') ?? '')?.textContent,
    );
    expect(names).toEqual(['Erro', 'Visual ou texto', 'Lentidão', 'Sugestão', 'Outro']);
    expect(within(group).getByRole('radio', { name: 'Erro' })).toHaveProperty('checked', true);
    const slow = within(group).getByRole('radio', { name: 'Lentidão' });
    expect(slow.getAttribute('aria-describedby')).toBeTruthy();
    expect(document.getElementById(slow.getAttribute('aria-describedby') ?? '')?.textContent).toBe(
      'Demora ou trava',
    );
    await user.click(within(group).getByText('Lentidão'));
    expect(slow).toHaveProperty('checked', true);
    await user.type(within(dialog).getByLabelText(/^Título/), 'Abrir demora');
    await waitFor(() => {
      expect(lastInput(backend)).toMatchObject({ category: 'slowness', title: 'Abrir demora' });
    });
    await user.click(within(dialog).getByText('Ver como a issue vai ficar'));
    await waitFor(() => {
      expect(within(dialog).getByTestId('report-preview-body').textContent).toMatch(/^## Lentidão/);
    });
    expect(
      within(within(dialog).getByTestId('report-preview-labels'))
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['relato-do-dono', 'categoria: lentidão']);
  });

  it('CA-UI03-02: a dica do título e o exemplo da descrição mudam com a categoria', async () => {
    reportBackend();
    const { user, dialog } = await openFromButton();
    const title = within(dialog).getByLabelText(/^Título/);
    const description = within(dialog).getByLabelText(/^Descrição/);
    const expected: [string, string, string][] = [
      [
        'Erro',
        'Uma frase com o problema. Exemplo: "O Testar fecha sem abrir o jogo".',
        'O que você fez, o que apareceu na tela e o que esperava que acontecesse.',
      ],
      [
        'Visual ou texto',
        'Onde está e o que está estranho. Exemplo: "Botão cortado na tela de Exportar".',
        'Em que tela está, o que aparece e como deveria aparecer.',
      ],
      [
        'Lentidão',
        'O que ficou lento. Exemplo: "Abrir um pack com 300 mods demora 1 minuto".',
        'O que você fez, quanto tempo levou e o tamanho do pack.',
      ],
      [
        'Sugestão',
        'A ideia numa frase. Exemplo: "Filtrar os mods por lado (cliente ou servidor)".',
        'O que você gostaria que o Warden fizesse e por quê.',
      ],
      ['Outro', 'Uma frase com o assunto.', 'Conte o que quiser.'],
    ];
    for (const [name, hint, example] of expected) {
      await user.click(within(dialog).getByRole('radio', { name }));
      const hintId = title.getAttribute('aria-describedby') ?? '';
      expect(document.getElementById(hintId)?.textContent).toBe(hint);
      expect(description.getAttribute('placeholder')).toBe(example);
    }
  });

  it('CA-UI03-03: uma descrição só, com contador; os três campos antigos não existem', async () => {
    const backend = reportBackend();
    const { user, dialog } = await openFromButton();
    for (const old of ['O que você fez', 'O que aconteceu', 'O que você esperava']) {
      expect(within(dialog).queryByLabelText(new RegExp(old))).toBeNull();
    }
    const description = within(dialog).getByLabelText(/^Descrição/);
    expect(description.getAttribute('maxLength')).toBe('4000');
    expect(within(dialog).getByTestId('report-count').textContent).toContain('0 / 4000');
    await user.type(description, 'Nada abriu');
    expect(within(dialog).getByTestId('report-count').textContent).toContain('10 / 4000');
    expect(within(dialog).getByTestId('report-count').textContent).toContain(
      '10 de 4000 caracteres',
    );
    await waitFor(() => {
      expect(lastInput(backend).description).toBe('Nada abriu');
    });
  });

  it('CA-UI02-02 e CA-UI03-07: a pré-visualização segue os campos e o GitHub recebe o endereço mostrado', async () => {
    const backend = reportBackend();
    const { user, dialog } = await openFromButton();
    await user.type(within(dialog).getByLabelText(/^Título/), 'O Testar travou');
    await user.type(within(dialog).getByLabelText(/^Descrição/), 'Nada abriu');
    await user.click(within(dialog).getByRole('checkbox', { name: /Incluir os registros do app/ }));

    // Recolhidos: o coletado e a pré-visualização começam fechados (o teclado, no teste abaixo).
    const collected = within(dialog).getByText('O que o Warden coleta sozinho').closest('details');
    const issue = within(dialog).getByText('Ver como a issue vai ficar').closest('details');
    expect(collected?.open).toBe(false);
    expect(issue?.open).toBe(false);
    await user.click(within(dialog).getByText('O que o Warden coleta sozinho'));
    await user.click(within(dialog).getByText('Ver como a issue vai ficar'));
    expect(collected?.open).toBe(true);
    expect(issue?.open).toBe(true);

    const body = within(dialog).getByTestId('report-preview-body');
    await waitFor(() => {
      expect(body.textContent).toContain('Nada abriu');
    });
    expect(within(dialog).getByTestId('report-preview-title').textContent).toBe('O Testar travou');
    expect(lastInput(backend)).toMatchObject({
      title: 'O Testar travou',
      description: 'Nada abriu',
      includeLog: false,
    });
    const kv = within(dialog).getByTestId('report-collected');
    expect(within(kv).getByText('Ubuntu 24.04 LTS (Linux 6.8.0)')).toBeDefined();
    expect(within(kv).getByText('não incluídos')).toBeDefined();

    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    await waitFor(() => {
      expect(open).toHaveProperty('disabled', false);
    });
    await user.click(open);
    await waitFor(() => {
      expect(backend.callsOf('problem_report_open')).toHaveLength(1);
    });
    // Sem o print marcado, nada de captura.
    expect(backend.callsOf('problem_report_capture')).toHaveLength(0);
    const url = backend.callsOf('problem_report_open')[0]?.args.url as string;
    expect(url).toBe(fakePreview(lastInput(backend)).url);
    expect(url.startsWith(URL_PREFIX)).toBe(true);
    expect(await screen.findByText('Relato aberto no navegador')).toBeDefined();
    expect(
      screen.getByText('Cole os prints (Ctrl+V) e clique em "Submit new issue".'),
    ).toBeDefined();
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Relatar problema' })).toBeNull();
    });
  });

  it('CA-UI03-07: os recolhidos são resumos nativos, alcançados pelo Tab', async () => {
    // Abrir com Enter e Espaço é do próprio `<summary>` no WebView, e o jsdom não simula isso:
    // a abertura pelo teclado é conferida no E2E (relatar-problema.e2e.ts).
    reportBackend();
    const { user, dialog } = await openFromButton();
    await within(dialog).findByTestId('report-preview-body');
    const summaries = ['O que o Warden coleta sozinho', 'Ver como a issue vai ficar'].map((text) =>
      within(dialog).getByText(text),
    );
    for (const summary of summaries) {
      expect(summary.tagName).toBe('SUMMARY');
      const details = summary.closest('details');
      expect(details?.open).toBe(false);
      expect(details?.firstElementChild).toBe(summary);
    }
    const [collected, issue] = summaries;
    collected?.focus();
    expect(document.activeElement).toBe(collected);
    await user.tab();
    expect(document.activeElement).toBe(issue);
  });

  it('CA-UI03-08: o atalho do cabeçalho tem texto para leitor de tela', async () => {
    reportBackend();
    const { dialog } = await openFromButton();
    const key = within(dialog).getByText('Ctrl+Shift+E');
    expect(key.tagName).toBe('KBD');
    const holder = key.parentElement;
    expect(holder?.textContent).toBe('Atalho: Ctrl+Shift+E');
    // "Atalho:" só para o leitor de tela; quem vê lê só a tecla.
    expect(holder?.querySelector('.sr-only')?.textContent).toBe('Atalho: ');
    expect(holder?.closest('[aria-hidden="true"]')).toBeNull();
    expect(
      screen
        .getByRole('button', { name: 'Relatar problema', hidden: true })
        .getAttribute('aria-keyshortcuts'),
    ).toBe('Control+Shift+E');
  });

  it('CA-UI03-06: os três passos do envio ficam à vista, numerados', async () => {
    reportBackend();
    const { dialog } = await openFromButton();
    const steps = within(dialog).getByRole('list', { name: 'Como fica o envio' });
    expect(steps.tagName).toBe('OL');
    expect(
      within(steps)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual([
      '1Abrir no GitHubA issue abre no navegador com tudo preenchido.',
      '2Colar os printsCtrl+V na caixa de texto da issue.',
      '3Submit new issueO relato chega para o orquestrador na hora.',
    ]);
  });

  it('título apagado mostra o erro e desabilita de novo', async () => {
    reportBackend();
    const { user, dialog } = await openFromButton();
    const title = within(dialog).getByLabelText(/^Título/);
    await user.type(title, 'x');
    await user.clear(title);
    await user.tab();
    expect(within(dialog).getByText('Escreva um título para abrir o relato.')).toBeDefined();
    expect(within(dialog).getByRole('button', { name: 'Abrir no GitHub' })).toHaveProperty(
      'disabled',
      true,
    );
  });

  it('CA-UI02-03: com corte e sem print, copia o relato completo e avisa', async () => {
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

  it('CA-UI03-04: com o print marcado, o diálogo some, o print é copiado e o diálogo volta antes de abrir', async () => {
    let hiddenDuringCapture: boolean | null = null;
    const backend = reportBackend({
      problem_report_capture: () => {
        // No momento da captura, o diálogo (e o fundo escurecido) está escondido.
        hiddenDuringCapture =
          document.querySelector(`.${CAPTURING_CLASS} [role="dialog"]`) !== null;
        return { width: 1280, height: 800 };
      },
      problem_report_open: () => {
        expect(document.querySelector(`.${CAPTURING_CLASS}`)).toBeNull();
        return null;
      },
    });
    const { user, dialog } = await openFromButton();
    const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
    const shot = within(dialog).getByRole('checkbox', { name: /Copiar um print desta tela/ });
    expect(within(within(dialog).getByTestId('report-screenshot')).getByText('novo')).toBeDefined();
    await user.click(shot);
    await user.type(within(dialog).getByLabelText(/^Título/), 'Botão cortado');
    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    await waitFor(() => {
      expect(open).toHaveProperty('disabled', false);
    });
    await user.click(open);
    await waitFor(() => {
      expect(backend.callsOf('problem_report_open')).toHaveLength(1);
    });
    expect(backend.callsOf('problem_report_capture')).toHaveLength(1);
    expect(hiddenDuringCapture).toBe(true);
    // A captura vem antes de abrir o navegador; sem corte, o texto não é copiado.
    const order = backend.calls
      .map((call) => call.command)
      .filter(
        (command) => command === 'problem_report_capture' || command === 'problem_report_open',
      );
    expect(order).toEqual(['problem_report_capture', 'problem_report_open']);
    expect(writeText).not.toHaveBeenCalled();
    expect(
      await screen.findByText('Cole o print (Ctrl+V) na issue e clique em "Submit new issue".'),
    ).toBeDefined();
  });

  it('CA-UI03-05: print e corte juntos: o aviso fica com "Copiar relato completo" e nada se perde', async () => {
    const backend = reportBackend({
      problem_report_preview: (args) =>
        fakePreview(args.input as ProblemReportInput, { cut: 'text', logLines: 0 }),
    });
    const { user, dialog } = await openFromButton();
    const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
    await user.click(within(dialog).getByRole('checkbox', { name: /Copiar um print desta tela/ }));
    await user.type(within(dialog).getByLabelText(/^Título/), 'Texto enorme');
    expect(
      await within(dialog).findByText(/o aviso tem o botão "Copiar relato completo"/),
    ).toBeDefined();
    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    await waitFor(() => {
      expect(open).toHaveProperty('disabled', false);
    });
    await user.click(open);
    await waitFor(() => {
      expect(backend.callsOf('problem_report_open')).toHaveLength(1);
    });
    // O print ocupa a área de transferência: o texto não a sobrescreve.
    expect(writeText).not.toHaveBeenCalled();
    expect(
      await screen.findByText(
        'Cole o print (Ctrl+V) na issue. Depois clique em "Copiar relato completo" e cole o texto no lugar do que foi cortado.',
      ),
    ).toBeDefined();
    // O aviso não some sozinho enquanto o relato completo só existe nele.
    expect(useToastStore.getState().toasts.at(-1)?.persistent).toBe(true);
    await user.click(screen.getByRole('button', { name: 'Copiar relato completo' }));
    expect(writeText).toHaveBeenCalledWith(fakePreview(lastInput(backend)).fullBody);
    expect(await screen.findByText('Relato completo copiado')).toBeDefined();
  });

  it('CA-UI03-04: se o print falha, a issue abre mesmo assim e o aviso diz em palavras simples', async () => {
    const backend = reportBackend({
      problem_report_capture: () => ipcError(makeAppError({ domain: 'app', code: 'INTERNAL' })),
      problem_report_preview: (args) =>
        fakePreview(args.input as ProblemReportInput, { cut: 'log', logLines: 40 }),
    });
    const { user, dialog } = await openFromButton();
    const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
    await user.click(within(dialog).getByRole('checkbox', { name: /Copiar um print desta tela/ }));
    await user.type(within(dialog).getByLabelText(/^Título/), 'Sem print');
    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    await waitFor(() => {
      expect(open).toHaveProperty('disabled', false);
    });
    await user.click(open);
    await waitFor(() => {
      expect(backend.callsOf('problem_report_open')).toHaveLength(1);
    });
    // Sem print, o relato completo vai para a área de transferência, como antes.
    expect(writeText).toHaveBeenCalledWith(fakePreview(lastInput(backend)).fullBody);
    expect(
      await screen.findByText(
        'O print não foi copiado. O relato completo está na área de transferência: cole na issue (Ctrl+V) se quiser mandar tudo.',
      ),
    ).toBeDefined();
    expect(document.querySelector(`.${CAPTURING_CLASS}`)).toBeNull();
  });

  it('a área de transferência que recusa o texto deixa o botão "Copiar relato completo" no aviso', async () => {
    reportBackend({
      problem_report_preview: (args) =>
        fakePreview(args.input as ProblemReportInput, { cut: 'log', logLines: 40 }),
    });
    const { user, dialog } = await openFromButton();
    const writeText = vi
      .spyOn(navigator.clipboard, 'writeText')
      .mockRejectedValueOnce(new Error('negado'))
      .mockResolvedValue();
    await user.type(within(dialog).getByLabelText(/^Título/), 'Recusado');
    const open = within(dialog).getByRole('button', { name: 'Abrir no GitHub' });
    await waitFor(() => {
      expect(open).toHaveProperty('disabled', false);
    });
    await user.click(open);
    expect(
      await screen.findByText(
        'Não deu para copiar o relato completo sozinho. Clique em "Copiar relato completo" para tentar de novo.',
      ),
    ).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Copiar relato completo' }));
    expect(writeText).toHaveBeenCalledTimes(2);
    expect(await screen.findByText('Relato completo copiado')).toBeDefined();
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
    // Criar pack e Abrir pack são diálogos de Meus packs, abertos pela busca (D38).
    expect(screenOf([{ routeId: '/packs/', params: {}, search: {} }], t).screen).toBe('Meus packs');
    expect(screenOf([{ routeId: '/packs/', params: {}, search: { criar: true } }], t).screen).toBe(
      'Meus packs › Criar pack',
    );
    expect(
      screenOf([{ routeId: '/packs/', params: {}, search: { pasta: 'D:/Packs/x' } }], t).screen,
    ).toBe('Meus packs › Abrir ou importar');
    expect(screenOf([{ routeId: '/outra', params: {} }], t).screen).toBe('Tela não identificada');
    expect(screenOf([], t).screen).toBe('Tela não identificada');
  });
});
