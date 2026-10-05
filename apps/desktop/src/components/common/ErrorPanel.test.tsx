import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { errorMessages } from '../../i18n/errors';
import type { ErrorCode } from '../../lib/ipc/bindings';
import { CommandError } from '../../lib/ipc/result';
import { axeComponent } from '../../test/axe';
import { mockBackend } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderWithProviders } from '../../test/render';
import { ErrorPanel } from './ErrorPanel';

/** Um código de cada domínio (o último declarado, para não ser sempre o INTERNAL). */
function oneCodePerDomain(): ErrorCode[] {
  return Object.entries(errorMessages).map(([domain, byCode]) => {
    const codes = Object.keys(byCode);
    return { domain, code: codes[codes.length - 1] } as ErrorCode;
  });
}

describe('ErrorPanel', () => {
  it('F0-06 critério 2: frase traduzida de um AppError de cada domínio e o código nos detalhes', () => {
    const codes = oneCodePerDomain();
    expect(codes).toHaveLength(26);
    for (const code of codes) {
      const phrase = (errorMessages[code.domain] as Record<string, string>)[code.code] ?? '';
      const { unmount } = renderWithProviders(
        <ErrorPanel error={new CommandError(makeAppError(code, { detail: 'saída do packwiz' }))} />,
      );
      const panel = screen.getByRole('alert');
      // A frase pode ter parâmetros ({{path}}): confere o começo, antes do primeiro.
      const fixed = phrase.split('{{')[0] ?? phrase;
      expect(panel.textContent).toContain(fixed);
      expect(within(panel).getByText(`${code.domain}.${code.code}`)).toBeDefined();
      expect(within(panel).getByText('saída do packwiz')).toBeDefined();
      unmount();
    }
  });

  it('detalhes técnicos recolhidos com código, tarefa e detail; Copiar leva tudo', async () => {
    mockBackend();
    const writeText = vi.fn(() => Promise.resolve());
    Object.assign(navigator, { clipboard: { writeText } });
    const error = makeAppError(
      { domain: 'core', code: 'IO' },
      {
        params: { path: 'mods/x.pw.toml' },
        detail: 'Acesso negado (os error 5)',
        operationId: '01J9ZQ0000000000000000000A',
      },
    );
    renderWithProviders(<ErrorPanel error={error} />);
    const details = screen.getByText('Detalhes técnicos').closest('details');
    expect(details?.open).toBe(false);
    fireEvent.click(screen.getByText('Detalhes técnicos'));
    expect(screen.getByText('core.IO')).toBeDefined();
    expect(screen.getByText('01J9ZQ0000000000000000000A')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Copiar' }));
    await screen.findByText('Detalhes copiados.');
    expect(writeText).toHaveBeenCalledWith(
      [
        'Código: core.IO',
        'Tarefa: 01J9ZQ0000000000000000000A',
        'path: mods/x.pw.toml',
        '',
        'Acesso negado (os error 5)',
      ].join('\n'),
    );
  });

  it('se copiar falhar, diz como copiar à mão', async () => {
    mockBackend();
    Object.assign(navigator, {
      clipboard: { writeText: () => Promise.reject(new Error('negado')) },
    });
    renderWithProviders(<ErrorPanel error={makeAppError()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Copiar' }));
    expect(
      await screen.findByText('Não foi possível copiar. Selecione o texto e copie com Ctrl+C.'),
    ).toBeDefined();
  });

  it('com título, a frase vai embaixo; "Tentar de novo" chama onRetry', () => {
    const onRetry = vi.fn();
    renderWithProviders(
      <ErrorPanel
        title="Esta tela não abriu"
        error={makeAppError({ domain: 'core', code: 'NETWORK_UNAVAILABLE' })}
        onRetry={onRetry}
      />,
    );
    expect(screen.getByText('Esta tela não abriu')).toBeDefined();
    expect(
      screen.getByText('Sem conexão com a internet. Confira a conexão e tente de novo.'),
    ).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('erro inesperado: frase genérica e a mensagem só nos detalhes', () => {
    renderWithProviders(<ErrorPanel error={new TypeError('cannot read x')} />);
    expect(screen.getByRole('alert').textContent).toContain(errorMessages.app.INTERNAL);
    expect(screen.getByText(/cannot read x/).closest('pre')).not.toBeNull();
  });

  it('passa no axe (fechado e aberto)', async () => {
    const { container } = renderWithProviders(
      <ErrorPanel
        error={makeAppError({ domain: 'core', code: 'TIMEOUT' }, { detail: 'x' })}
        onRetry={vi.fn()}
      />,
    );
    expect(await axeComponent(container)).toHaveNoViolations();
    fireEvent.click(screen.getByText('Detalhes técnicos'));
    await waitFor(async () => {
      expect(await axeComponent(container)).toHaveNoViolations();
    });
  });
});
