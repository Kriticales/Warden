import { createMemoryHistory } from '@tanstack/react-router';
import { mockIPC } from '@tauri-apps/api/mocks';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { App } from '../app/App';
import type { AppError, AppInfo } from '../lib/ipc/bindings';

function renderHome() {
  render(<App history={createMemoryHistory({ initialEntries: ['/'] })} />);
}

describe('página inicial', () => {
  it('F0-01 critério 2: mostra o texto do catálogo e a versão de app_info', async () => {
    const info: AppInfo = { version: '0.1.0', commit: 'abc123def456' };
    mockIPC((command) => (command === 'app_info' ? info : undefined));

    renderHome();

    expect(await screen.findByRole('heading', { level: 1, name: 'Warden' })).toBeDefined();
    expect(
      screen.getByText('Crie, teste e diagnostique modpacks no formato packwiz.'),
    ).toBeDefined();
    expect(await screen.findByText('Versão 0.1.0 (abc123def456)')).toBeDefined();
  });

  it('mostra só a versão quando o commit é desconhecido', async () => {
    const info: AppInfo = { version: '0.1.0', commit: null };
    mockIPC(() => info);

    renderHome();

    expect(await screen.findByText('Versão 0.1.0')).toBeDefined();
  });

  it('mostra o carregamento e depois o erro quando app_info falha', async () => {
    const error: AppError = {
      code: { domain: 'app', code: 'INTERNAL' },
      params: {},
      detail: 'falhou',
      retryable: false,
    };
    // O IPC do Tauri rejeita com o AppError serializado, não com um Error. A rejeição só
    // acontece quando o teste manda, para o estado de carregamento ser visto antes.
    let fail: () => void = () => undefined;
    const pending = new Promise<never>((_, reject) => {
      fail = () => {
        // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors
        reject(error);
      };
    });
    mockIPC(() => pending);

    renderHome();

    expect((await screen.findByRole('status')).textContent).toBe('Lendo a versão do app…');
    fail();
    expect(await screen.findByText('Não foi possível ler a versão do app.')).toBeDefined();
  });
});
