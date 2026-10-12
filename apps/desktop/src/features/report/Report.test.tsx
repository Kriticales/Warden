import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import i18n from '../../i18n';
import type { ProblemReportInput, ProblemReportPreview } from '../../lib/ipc/bindings';
import { showToast, useToastStore } from '../../components/ui/toast';
import { appErrorMessage } from '../../lib/ipc/errors';
import { forgetShownError } from '../../lib/ipc/last-error';
import { axeComponent } from '../../test/axe';
import { ipcError, mockBackend, type Handler } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { isReportShortcut } from './ReportProblem';
import { screenOf } from './screen';
import { closeReportDialog } from './store';

const URL_PREFIX = 'https://github.com/Kriticales/Warden/issues/new?title=';

/** A pré-visualização que o Rust devolveria para a entrada (corpo com o título e a tela). */
function fakePreview(
  input: ProblemReportInput,
  overrides: Partial<ProblemReportPreview> = {},
): ProblemReportPreview {
  const body = `### O que aconteceu\n\n${input.whatHappened}\n\n### Ambiente\n\n- **Tela:** ${input.screen}\n`;
  return {
    title: input.title,
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
        value: input.lastError ? `${input.lastError.code}: ${input.lastError.message}` : 'nenhum',
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
    await user.type(within(dialog).getByTestId('report-happened'), 'Nada abriu');
    await user.click(within(dialog).getByRole('checkbox', { name: /Incluir os registros do app/ }));

    const body = within(dialog).getByTestId('report-preview-body');
    await waitFor(() => {
      expect(body.textContent).toContain('Nada abriu');
    });
    expect(within(dialog).getByTestId('report-preview-title').textContent).toBe('O Testar travou');
    expect(lastInput(backend)).toMatchObject({
      title: 'O Testar travou',
      whatHappened: 'Nada abriu',
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
    expect(within(dialog).getByText('Escreva um título para abrir o relato.')).toBeDefined();
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
