import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { useToastStore } from '../../../components/ui/toast';
import type { GithubLoginStatus, GithubLoginUpdate } from '../../../lib/ipc/bindings';
import { axeComponent } from '../../../test/axe';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { mockSettingsBackend } from '../testing';

const SIGNED_OUT: GithubLoginStatus = { available: true, login: null, waiting: false };

/** Configurações com o login do GitHub simulado, com estado como no Rust. */
function loginBackend(initial: GithubLoginStatus = SIGNED_OUT) {
  let status = initial;
  const backend = mockSettingsBackend(
    {},
    {
      github_login_status: () => status,
      github_login_start: () => {
        status = { ...status, waiting: true };
        return {
          userCode: 'WDJB-MJHT',
          verificationUri: 'https://github.com/login/device',
          expiresInSeconds: 900,
        };
      },
      github_login_cancel: () => {
        status = { ...status, waiting: false };
        return null;
      },
      github_login_open_page: () => null,
      github_logout: () => {
        status = { ...status, login: null, waiting: false };
        return status;
      },
    },
  );
  return {
    backend,
    connect: async (login: string) => {
      status = { ...status, login, waiting: false };
      await backend.emit('github-login-updated', {
        state: 'connected',
        login,
      } satisfies GithubLoginUpdate);
    },
    update: (update: GithubLoginUpdate) => backend.emit('github-login-updated', update),
  };
}

async function section() {
  renderApp('/configuracoes');
  return screen.findByTestId('github-login');
}

afterEach(() => {
  useToastStore.getState().clear();
  vi.restoreAllMocks();
});

describe('Entrar com o GitHub (UI-03)', () => {
  it('CA-UI03-10: sem o identificador do GitHub App, o botão fica desabilitado com a explicação', async () => {
    loginBackend({ available: false, login: null, waiting: false });
    const root = await section();
    const button = await within(root).findByRole('button', { name: 'Entrar com o GitHub' });
    expect(button).toHaveProperty('disabled', true);
    const hint = within(root).getByText(
      'O login com o GitHub ainda não foi configurado nesta versão do Warden. Use Abrir no GitHub no relato.',
    );
    expect(button.getAttribute('aria-describedby')).toBe(hint.id);
    expect(await axeComponent(root)).toHaveNoViolations();
  });

  it('CA-UI03-02: mostra o código grande e copiável, abre a página e conecta', async () => {
    const { backend, connect, update } = loginBackend();
    const user = userEvent.setup();
    const root = await section();
    await user.click(await within(root).findByRole('button', { name: 'Entrar com o GitHub' }));
    const code = await within(root).findByTestId('github-login-code');
    expect(code.textContent).toBe('WDJB-MJHT');
    expect(within(root).getByRole('status', { name: 'Código para digitar no GitHub' })).toBe(code);
    expect(
      within(root).getByText(
        'Clique em Abrir o GitHub, digite este código na página e aprove o acesso. O código vale 15 minutos.',
      ),
    ).toBeDefined();
    expect(within(root).getByText('Aguardando você aprovar no GitHub…')).toBeDefined();
    expect(await axeComponent(root)).toHaveNoViolations();

    const writeText = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
    await user.click(within(root).getByRole('button', { name: 'Copiar código' }));
    expect(writeText).toHaveBeenCalledWith('WDJB-MJHT');
    expect(await within(root).findByText('Código copiado')).toBeDefined();

    await user.click(within(root).getByRole('button', { name: 'Abrir o GitHub' }));
    await waitFor(() => {
      expect(backend.callsOf('github_login_open_page')).toHaveLength(1);
    });

    await update({ state: 'slowDown', intervalSeconds: 10 });
    expect(
      await within(root).findByText(
        'O GitHub pediu para conferir com menos frequência. Continua esperando.',
      ),
    ).toBeDefined();

    await connect('Kriticales');
    expect(await within(root).findByText('Conectado como Kriticales')).toBeDefined();
    expect(await screen.findByText('Conectado ao GitHub como Kriticales')).toBeDefined();

    await user.click(within(root).getByRole('button', { name: 'Sair do GitHub' }));
    expect(await within(root).findByRole('button', { name: 'Entrar com o GitHub' })).toBeDefined();
    expect(backend.callsOf('github_logout')).toHaveLength(1);
    expect(
      await screen.findByText(
        'Você saiu do GitHub. O token e o token de renovação foram apagados.',
      ),
    ).toBeDefined();
  });

  it('CA-UI03-03: recusado no GitHub mostra o motivo; Cancelar para a espera', async () => {
    const { backend, update } = loginBackend();
    const user = userEvent.setup();
    const root = await section();
    await user.click(await within(root).findByRole('button', { name: 'Entrar com o GitHub' }));
    await within(root).findByTestId('github-login-code');
    await update({
      state: 'failed',
      error: makeAppError({ domain: 'github', code: 'LOGIN_DENIED' }),
    });
    expect(
      await within(root).findByText(
        'O acesso foi recusado na página do GitHub. Clique em Entrar com o GitHub para tentar de novo.',
      ),
    ).toBeDefined();

    await user.click(within(root).getByRole('button', { name: 'Entrar com o GitHub' }));
    await within(root).findByTestId('github-login-code');
    await user.click(within(root).getByRole('button', { name: 'Cancelar' }));
    expect(await within(root).findByText('Entrada cancelada.')).toBeDefined();
    expect(backend.callsOf('github_login_cancel')).toHaveLength(1);
    expect(within(root).queryByTestId('github-login-code')).toBeNull();
  });

  it('CA-UI03-03: código vencido', async () => {
    const { update } = loginBackend();
    const user = userEvent.setup();
    const root = await section();
    await user.click(await within(root).findByRole('button', { name: 'Entrar com o GitHub' }));
    await within(root).findByTestId('github-login-code');
    await update({
      state: 'failed',
      error: makeAppError({ domain: 'github', code: 'LOGIN_CODE_EXPIRED' }),
    });
    expect(
      await within(root).findByText(
        'O código venceu antes da aprovação. Clique em Entrar com o GitHub para gerar outro.',
      ),
    ).toBeDefined();
  });

  it('já conectado: mostra a conta', async () => {
    loginBackend({ available: true, login: 'Kriticales', waiting: false });
    const root = await section();
    expect(await within(root).findByText('Conectado como Kriticales')).toBeDefined();
    expect(within(root).getByRole('button', { name: 'Sair do GitHub' })).toBeDefined();
  });
});
