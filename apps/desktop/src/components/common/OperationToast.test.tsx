import { fireEvent, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { axeComponent } from '../../test/axe';
import { makeAppError, makeOperation } from '../../test/factories';
import { renderWithProviders } from '../../test/render';
import { useToastStore } from '../ui/toast';
import { operationToast, showOperationToast } from './OperationToast';

describe('OperationToast', () => {
  it('concluída, cancelada e em andamento', () => {
    expect(operationToast(makeOperation({ state: 'succeeded' }), { name: 'Exportar' })).toEqual({
      kind: 'ok',
      title: 'Concluída: Exportar',
    });
    expect(operationToast(makeOperation({ state: 'cancelled' }), { name: 'Exportar' })).toEqual({
      kind: 'info',
      title: 'Cancelada: Exportar',
    });
    for (const state of ['running', 'waitingForLock', 'cancelling'] as const) {
      expect(operationToast(makeOperation({ state }), { name: 'x' })).toBeNull();
      expect(showOperationToast(makeOperation({ state }), { name: 'x' })).toBe(false);
    }
  });

  it('falhou: frase do erro e "Ver detalhes", que chama quem abre a gaveta', async () => {
    const onShowDetails = vi.fn();
    const { container } = renderWithProviders(<div />);
    const shown = showOperationToast(
      makeOperation({
        state: 'failed',
        error: makeAppError({ domain: 'core', code: 'NETWORK_UNAVAILABLE' }),
      }),
      { name: 'Publicar versão', onShowDetails },
    );
    expect(shown).toBe(true);
    expect(await screen.findByText('Falhou: Publicar versão')).toBeDefined();
    expect(
      screen.getByText('Sem conexão com a internet. Confira a conexão e tente de novo.'),
    ).toBeDefined();
    expect(useToastStore.getState().toasts[0]?.kind).toBe('danger');
    expect(await axeComponent(container.ownerDocument.body)).toHaveNoViolations();
    fireEvent.click(screen.getByRole('button', { name: 'Ver detalhes' }));
    expect(onShowDetails).toHaveBeenCalledTimes(1);
  });

  it('falhou sem erro nem quem abra detalhes: só o título', () => {
    expect(operationToast(makeOperation({ state: 'failed' }), { name: 'x' })).toEqual({
      kind: 'danger',
      title: 'Falhou: x',
    });
  });
});
