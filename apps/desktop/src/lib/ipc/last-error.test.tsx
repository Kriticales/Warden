import { act, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { ErrorPanel } from '../../components/common/ErrorPanel';
import { Alert } from '../../components/ui/alert';
import { showToast, useToastStore } from '../../components/ui/toast';
import { makeAppError } from '../../test/factories';
import { renderWithProviders } from '../../test/render';
import { appErrorMessage } from './errors';
import { forgetShownError, lastShownError } from './last-error';

const TIMEOUT = 'A operação demorou demais e foi interrompida. Tente de novo.';

afterEach(() => {
  act(() => {
    useToastStore.getState().clear();
  });
  forgetShownError();
});

describe('último erro mostrado (UI-02)', () => {
  it('a frase traduzida sozinha, sem aparecer na tela, não conta', () => {
    appErrorMessage(makeAppError({ domain: 'core', code: 'TIMEOUT' }));
    expect(lastShownError()).toBeNull();
  });

  it('toast de erro com a frase de um AppError: guarda a frase e o código', async () => {
    renderWithProviders(null);
    act(() => {
      showToast({
        kind: 'danger',
        title: appErrorMessage(makeAppError({ domain: 'core', code: 'TIMEOUT' })),
      });
    });
    await screen.findByText(TIMEOUT);
    expect(lastShownError()).toEqual({ code: 'core.TIMEOUT', message: TIMEOUT });
  });

  it('toast de erro da própria interface (sem AppError): guarda o texto, sem código', async () => {
    renderWithProviders(null);
    act(() => {
      showToast({ kind: 'danger', title: 'Não deu para copiar.', text: 'Tente de novo.' });
    });
    await screen.findByText('Não deu para copiar.');
    expect(lastShownError()).toEqual({
      code: null,
      message: 'Não deu para copiar. Tente de novo.',
    });
  });

  it('toast que não é de erro não conta', async () => {
    renderWithProviders(null);
    act(() => {
      showToast({ kind: 'ok', title: 'Salvo.' });
    });
    await screen.findByText('Salvo.');
    expect(lastShownError()).toBeNull();
  });

  it('ErrorPanel: título e frase, sem os botões nem os detalhes técnicos', () => {
    render(
      <ErrorPanel
        error={makeAppError({ domain: 'core', code: 'TIMEOUT' }, { detail: 'C:\\segredo' })}
        title="Esta tela não abriu"
        onRetry={() => undefined}
      />,
    );
    expect(lastShownError()).toEqual({
      code: 'core.TIMEOUT',
      message: `Esta tela não abriu ${TIMEOUT}`,
    });
  });

  it('aviso de perigo com texto próprio (sem frase de AppError) também conta', () => {
    render(<Alert kind="danger" title="Este pack usa Quilt" />);
    expect(lastShownError()).toEqual({ code: null, message: 'Este pack usa Quilt' });
  });

  it('aviso que não é de perigo não conta', () => {
    render(<Alert kind="warn" title="Cuidado" />);
    expect(lastShownError()).toBeNull();
  });

  it('um erro antigo que só se redesenha não toma o lugar do mais novo', async () => {
    const error = makeAppError({ domain: 'core', code: 'TIMEOUT' });
    // O painel numa raiz própria: `rerender` só redesenha, não monta de novo.
    const view = render(<ErrorPanel error={error} />);
    renderWithProviders(null);
    expect(lastShownError()?.code).toBe('core.TIMEOUT');
    act(() => {
      showToast({ kind: 'danger', title: 'Não deu para abrir a pasta.' });
    });
    await screen.findByText('Não deu para abrir a pasta.');
    view.rerender(<ErrorPanel error={{ ...error }} />);
    expect(lastShownError()).toEqual({ code: null, message: 'Não deu para abrir a pasta.' });
  });
});
