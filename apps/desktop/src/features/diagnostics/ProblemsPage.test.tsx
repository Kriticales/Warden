/**
 * Seção Problemas (T14): o erro ao ler o relatório (A-01).
 */
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { axePage } from '../../test/axe';
import { ipcError } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderApp } from '../../test/render';
import { editorBackend } from '../pack-editor/testing';
import { makeReport } from './diagnostics.fixtures';

/** Primeira tela do arquivo: roteador e consultas, numa máquina ocupada. */
const FIRST_PAINT = { timeout: 10_000 };

describe('Problemas: leitura do relatório (A-01)', () => {
  it('erro ao ler: título da página, o que falhou e "Tentar de novo", que lê de novo; axe', async () => {
    let fail = true;
    const { url } = editorBackend({
      handlers: {
        diagnostics_report_get: () =>
          fail
            ? ipcError(makeAppError({ domain: 'app', code: 'INTERNAL' }))
            : makeReport({ findings: [] }),
      },
    });
    const user = userEvent.setup();
    const { container } = renderApp(`${url}/problemas`);
    const retry = await screen.findByRole('button', { name: 'Tentar de novo' }, FIRST_PAINT);
    expect(screen.getByRole('heading', { level: 1, name: 'Problemas' })).toBeDefined();
    expect(
      within(screen.getByRole('alert')).getByText('Não foi possível ler os problemas do pack.'),
    ).toBeDefined();
    expect(await axePage(container)).toHaveNoViolations();
    fail = false;
    await user.click(retry);
    expect(await screen.findByRole('button', { name: 'Verificar agora' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Tentar de novo' })).toBeNull();
  });
});
