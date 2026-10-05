import { mockWindows } from '@tauri-apps/api/mocks';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { axePage } from '../../test/axe';
import { mockBackend, type Handler } from '../../test/backend';
import { TitleBar, usesCustomTitleBar } from './TitleBar';

const WINDOWS_UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0';
const LINUX_UA = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)';

function windowBackend(handlers: Record<string, Handler> = {}) {
  return mockBackend({
    'plugin:window|is_maximized': () => false,
    'plugin:window|minimize': () => null,
    'plugin:window|toggle_maximize': () => null,
    'plugin:window|close': () => null,
    window_set_maximize_area: () => null,
    ...handlers,
  });
}

beforeEach(() => {
  mockWindows('main');
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('usesCustomTitleBar', () => {
  it('só no Windows', () => {
    expect(usesCustomTitleBar(WINDOWS_UA)).toBe(true);
    expect(usesCustomTitleBar(LINUX_UA)).toBe(false);
  });
});

describe('TitleBar', () => {
  it('mostra a marca e os botões com rótulos em português, sem problemas de acessibilidade', async () => {
    windowBackend();
    const { container } = render(<TitleBar />);
    expect(screen.getByText('Warden')).toBeTruthy();
    expect(screen.getByRole('group', { name: 'Janela' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Minimizar' })).toBeTruthy();
    expect(await screen.findByRole('button', { name: 'Maximizar' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Fechar' })).toBeTruthy();
    expect(await axePage(container)).toHaveNoViolations();
  });

  it('janela maximizada: o botão vira Restaurar', async () => {
    windowBackend({ 'plugin:window|is_maximized': () => true });
    render(<TitleBar />);
    expect(await screen.findByRole('button', { name: 'Restaurar' })).toBeTruthy();
  });

  it('os botões minimizam, maximizam e fecham, também pelo teclado', async () => {
    const backend = windowBackend();
    const user = userEvent.setup();
    render(<TitleBar />);

    await user.tab();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Minimizar' }));
    await user.tab();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Maximizar' }));
    await user.keyboard('{Enter}');
    await user.click(screen.getByRole('button', { name: 'Minimizar' }));
    await user.click(screen.getByRole('button', { name: 'Fechar' }));

    await waitFor(() => {
      expect(backend.callsOf('plugin:window|toggle_maximize')).toHaveLength(1);
      expect(backend.callsOf('plugin:window|minimize')).toHaveLength(1);
      expect(backend.callsOf('plugin:window|close')).toHaveLength(1);
    });
  });

  it('informa ao Rust onde está o maximizar, a partir da borda direita', async () => {
    const backend = windowBackend();
    vi.spyOn(document.documentElement, 'clientWidth', 'get').mockReturnValue(1280);
    vi.spyOn(HTMLButtonElement.prototype, 'getBoundingClientRect').mockReturnValue(
      DOMRect.fromRect({ x: 1184, y: 0, width: 48, height: 36 }),
    );
    render(<TitleBar />);
    await waitFor(() => {
      expect(backend.callsOf('window_set_maximize_area')).toEqual([
        {
          command: 'window_set_maximize_area',
          args: { area: { right: 48, top: 0, width: 48, height: 36 } },
        },
      ]);
    });
  });

  it('sem medida (botão invisível), não informa nada', () => {
    const backend = windowBackend();
    render(<TitleBar />);
    expect(backend.callsOf('window_set_maximize_area')).toHaveLength(0);
  });

  it('desenha o hover e o aperto do maximizar que chegam do Rust', async () => {
    const backend = windowBackend();
    render(<TitleBar />);
    const maximize = await screen.findByRole('button', { name: 'Maximizar' });
    // A assinatura é assíncrona: emite até ela existir.
    await waitFor(async () => {
      await act(() => backend.emit('title-bar-maximize', { hovered: true, pressed: false }));
      expect(maximize.classList.contains('is-hover')).toBe(true);
    });
    expect(maximize.classList.contains('is-press')).toBe(false);
    await act(() => backend.emit('title-bar-maximize', { hovered: true, pressed: true }));
    expect(maximize.classList.contains('is-press')).toBe(true);
    await act(() => backend.emit('title-bar-maximize', { hovered: false, pressed: false }));
    expect(maximize.classList.contains('is-hover')).toBe(false);
    expect(maximize.classList.contains('is-press')).toBe(false);
  });

  it('janela sem foco fica apagada', () => {
    windowBackend();
    render(<TitleBar />);
    const bar = screen.getByTestId('titlebar');
    act(() => {
      window.dispatchEvent(new FocusEvent('focus'));
    });
    expect(bar.classList.contains('titlebar--inactive')).toBe(false);
    act(() => {
      window.dispatchEvent(new FocusEvent('blur'));
    });
    expect(bar.classList.contains('titlebar--inactive')).toBe(true);
    act(() => {
      window.dispatchEvent(new FocusEvent('focus'));
    });
    expect(bar.classList.contains('titlebar--inactive')).toBe(false);
  });
});
