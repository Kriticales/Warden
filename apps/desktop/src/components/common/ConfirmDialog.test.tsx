import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { axeComponent } from '../../test/axe';
import { mockBackend } from '../../test/backend';
import { makeAppError } from '../../test/factories';
import { renderWithProviders } from '../../test/render';
import { Button } from '../ui/button';
import { ConfirmDialog } from './ConfirmDialog';

function renderDialog(props: Partial<Parameters<typeof ConfirmDialog>[0]> = {}) {
  const onConfirm = props.onConfirm ?? vi.fn();
  const result = renderWithProviders(
    <ConfirmDialog
      trigger={<Button variant="danger-ghost">Apagar pack…</Button>}
      title="Apagar o pack Vale Sereno?"
      description="A pasta vai para a Lixeira. Dá para recuperar de lá."
      confirmLabel="Apagar Vale Sereno"
      confirmingLabel="Apagando…"
      {...props}
      onConfirm={onConfirm}
    />,
  );
  return { ...result, onConfirm };
}

describe('ConfirmDialog', () => {
  it('abre pelo gatilho com o foco no título, é alertdialog e passa no axe', async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole('button', { name: 'Apagar pack…' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Apagar o pack Vale Sereno?' });
    expect(document.activeElement).toBe(
      screen.getByRole('heading', { name: 'Apagar o pack Vale Sereno?' }),
    );
    expect(dialog.getAttribute('aria-describedby')).not.toBeNull();
    expect(await axeComponent(document.body)).toHaveNoViolations();
  });

  it('Cancelar e Esc fecham sem confirmar e devolvem o foco ao gatilho', async () => {
    const user = userEvent.setup();
    const { onConfirm } = renderDialog();
    const trigger = screen.getByRole('button', { name: 'Apagar pack…' });
    await user.click(trigger);
    await user.click(await screen.findByRole('button', { name: 'Cancelar' }));
    await waitFor(() => {
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });
    expect(document.activeElement).toBe(trigger);
    await user.click(trigger);
    await screen.findByRole('alertdialog');
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('com confirmação por digitação, só habilita com o texto exato', async () => {
    const user = userEvent.setup();
    const { onConfirm } = renderDialog({ confirmText: 'Vale Sereno' });
    await user.click(screen.getByRole('button', { name: 'Apagar pack…' }));
    const confirm = await screen.findByRole<HTMLButtonElement>('button', {
      name: 'Apagar Vale Sereno',
    });
    expect(confirm.disabled).toBe(true);
    const field = screen.getByLabelText('Para confirmar, digite Vale Sereno');
    await user.type(field, 'vale sereno');
    expect(screen.getByText('O texto digitado não confere.')).toBeDefined();
    expect(field.getAttribute('aria-invalid')).toBe('true');
    expect(confirm.disabled).toBe(true);
    await user.clear(field);
    await user.type(field, 'Vale Sereno');
    expect(confirm.disabled).toBe(false);
    await user.keyboard('{Enter}');
    await waitFor(() => {
      expect(onConfirm).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });
  });

  it('enquanto confirma fica ocupado; se falhar, mostra o erro e continua aberto', async () => {
    mockBackend();
    const user = userEvent.setup();
    let fail: (error: unknown) => void = () => undefined;
    const onConfirm = vi.fn(
      () =>
        new Promise<void>((_, reject) => {
          fail = reject;
        }),
    );
    renderDialog({ onConfirm });
    await user.click(screen.getByRole('button', { name: 'Apagar pack…' }));
    await user.click(await screen.findByRole('button', { name: 'Apagar Vale Sereno' }));
    const busy = await screen.findByRole('button', { name: 'Apagando…' });
    expect(busy.getAttribute('aria-busy')).toBe('true');
    expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Cancelar' }).disabled).toBe(true);
    fail(makeAppError({ domain: 'core', code: 'IO' }, { params: { path: 'C:\\Packs\\vale' } }));
    expect(
      await screen.findByText(/Não foi possível ler ou gravar um arquivo em C:\\Packs\\vale/),
    ).toBeDefined();
    expect(screen.getByRole('alertdialog')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Apagar Vale Sereno' })).toBeDefined();
  });

  it('controlado por open e não destrutivo usa o botão primário', async () => {
    const onOpenChange = vi.fn();
    renderWithProviders(
      <ConfirmDialog
        open
        onOpenChange={onOpenChange}
        destructive={false}
        title="Salvar a versão 1.4.3?"
        description="As 5 alterações entram no histórico."
        confirmLabel="Salvar versão"
        onConfirm={() => undefined}
      />,
    );
    const confirm = await screen.findByRole('button', { name: 'Salvar versão' });
    expect(confirm.className).toContain('btn--primary');
    await userEvent.setup().click(confirm);
    await waitFor(() => {
      expect(onOpenChange).toHaveBeenCalledWith(false);
    });
  });
});
