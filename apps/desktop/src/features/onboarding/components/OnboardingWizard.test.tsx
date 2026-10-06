import { screen, waitFor, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { axePage } from '../../../test/axe';
import { ipcError } from '../../../test/backend';
import { makeAppError } from '../../../test/factories';
import { renderApp } from '../../../test/render';
import { mockSettingsBackend } from '../../settings/testing';

/** Primeira tela de cada caso: roteador, redirecionamento e consultas, numa máquina ocupada. */
const FIRST_PAINT = { timeout: 5000 };

const LEGAL =
  'NÃO É UM PRODUTO OFICIAL DO MINECRAFT. NÃO É APROVADO PELA MOJANG OU PELA MICROSOFT NEM ASSOCIADO A ELAS.';

function firstRunBackend(handlers: Parameters<typeof mockSettingsBackend>[1] = {}) {
  return mockSettingsBackend({ exists: false }, handlers);
}

/** Vai do aviso até a etapa das chaves com o nome padrão. */
async function goToKeys(user: UserEvent) {
  await user.click(await screen.findByRole('button', { name: 'Entendi' }, FIRST_PAINT));
  await screen.findByRole('heading', { level: 1, name: 'Nome do jogador nos testes' });
  await user.click(screen.getByRole('button', { name: 'Próximo' }));
  await screen.findByRole('heading', { level: 1, name: 'Pasta dos packs' });
  await user.click(screen.getByRole('button', { name: 'Próximo' }));
  await screen.findByRole('heading', { level: 1, name: 'Chaves' });
}

describe('primeira execução', () => {
  it('CA-T01-01: sem settings.json o app abre nas boas-vindas; ao concluir, grava e mostra o início', async () => {
    const backend = firstRunBackend();
    const user = userEvent.setup();
    renderApp('/');

    const title = await screen.findByRole(
      'heading',
      { level: 1, name: 'Boas-vindas ao Warden' },
      FIRST_PAINT,
    );
    expect(title).toBeDefined();
    expect(screen.getByTestId('aviso-legal').textContent).toBe(LEGAL);
    // Sem menu: só o indicador de etapas.
    expect(within(screen.getByRole('banner')).queryByRole('link')).toBeNull();
    const steps = screen.getByRole('list', { name: 'Etapas da primeira execução' });
    expect(within(steps).getAllByRole('listitem')).toHaveLength(4);

    await goToKeys(user);
    await user.click(screen.getByRole('button', { name: 'Concluir' }));

    expect(await screen.findByRole('heading', { level: 1, name: 'Meus packs' })).toBeDefined();
    expect(screen.queryByRole('list', { name: 'Etapas da primeira execução' })).toBeNull();
    expect(backend.state.exists).toBe(true);
    expect(backend.callsOf('settings_update').map((call) => call.args)).toEqual([
      { patch: { playerName: 'Jogador' } },
    ]);
  });

  it('com settings.json o app abre direto no início', async () => {
    mockSettingsBackend({ exists: true });
    renderApp('/');
    expect(await screen.findByRole('heading', { level: 1, name: 'Meus packs' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Entendi' })).toBeNull();
  });

  it('CA-T01-02: "Zé" é recusado com a regra; "Ze_123" é aceito e gravado', async () => {
    const backend = firstRunBackend();
    const user = userEvent.setup();
    renderApp('/boas-vindas');
    await user.click(await screen.findByRole('button', { name: 'Entendi' }, FIRST_PAINT));
    const field = await screen.findByRole<HTMLInputElement>('textbox', {
      name: 'Nome do jogador nos testes',
    });
    expect(field.value).toBe('Jogador');
    expect(document.activeElement).toBe(
      screen.getByRole('heading', { level: 1, name: 'Nome do jogador nos testes' }),
    );

    await user.clear(field);
    await user.type(field, 'Zé');
    expect(
      screen.getByText('Use só letras sem acento, números e _ (sem espaço), de 3 a 16 caracteres.'),
    ).toBeDefined();
    expect(field.getAttribute('aria-invalid')).toBe('true');
    expect(screen.getByRole('button', { name: 'Próximo' }).hasAttribute('disabled')).toBe(true);
    await user.keyboard('{Enter}');
    expect(
      screen.getByRole('heading', { level: 1, name: 'Nome do jogador nos testes' }),
    ).toBeDefined();

    await user.clear(field);
    await user.type(field, 'Ze_123');
    expect(field.getAttribute('aria-invalid')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Próximo' }));
    await user.click(await screen.findByRole('button', { name: 'Próximo' }));
    await user.click(await screen.findByRole('button', { name: 'Concluir' }));
    await screen.findByRole('heading', { level: 1, name: 'Meus packs' });
    expect(backend.callsOf('settings_update')[0]?.args).toEqual({
      patch: { playerName: 'Ze_123' },
    });
  });

  it('CA-T01-03: "Pular, configuro depois" conclui sem gravar chaves', async () => {
    const backend = firstRunBackend();
    const user = userEvent.setup();
    renderApp('/');
    await goToKeys(user);
    await user.type(screen.getByLabelText('Chave da CurseForge'), 'nao-vai');
    await user.click(screen.getByRole('button', { name: 'Pular, configuro depois' }));
    await screen.findByRole('heading', { level: 1, name: 'Meus packs' });
    expect(backend.callsOf('secrets_set')).toEqual([]);
    expect(backend.callsOf('secrets_backend_set')).toEqual([]);
    expect(backend.state.exists).toBe(true);
  });

  it('a pasta é escolhida pelo diálogo nativo e aparece também em "Onde ficam as coisas"', async () => {
    firstRunBackend({
      settings_choose_packs_dir: () => ({
        firstRun: false,
        packsDir: 'E:\\Modpacks',
        defaultPacksDir: 'C:\\Users\\teste\\Documents\\Warden',
      }),
    });
    const user = userEvent.setup();
    renderApp('/boas-vindas');
    await user.click(await screen.findByRole('button', { name: 'Entendi' }, FIRST_PAINT));
    await user.click(await screen.findByRole('button', { name: 'Próximo' }));
    await user.click(await screen.findByRole('button', { name: 'Escolher…' }));
    const aside = screen.getByRole('complementary', { name: 'Onde ficam as coisas' });
    await waitFor(() => {
      expect(within(aside).getByText('E:\\Modpacks')).toBeDefined();
    });
    expect(screen.getByRole<HTMLInputElement>('textbox', { name: 'Pasta dos packs' }).value).toBe(
      'E:\\Modpacks',
    );
  });

  it('chaves digitadas e o .env (com confirmação) são gravados ao concluir, antes do settings.json', async () => {
    const backend = firstRunBackend();
    const user = userEvent.setup();
    renderApp('/');
    await goToKeys(user);
    await user.type(screen.getByLabelText('Chave da CurseForge'), 'cf-123');
    await user.type(screen.getByLabelText('Token do GitHub'), 'gh-456');
    await user.click(screen.getByRole('radio', { name: /Arquivo \.env/ }));
    const dialog = await screen.findByRole('alertdialog', {
      name: 'Guardar as chaves num arquivo .env?',
    });
    await user.click(within(dialog).getByRole('button', { name: 'Usar arquivo .env' }));
    await waitFor(() => {
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });
    expect(screen.getByRole<HTMLInputElement>('radio', { name: /Arquivo \.env/ }).checked).toBe(
      true,
    );
    // Nada gravado antes de concluir.
    expect(backend.callsOf('secrets_backend_set')).toEqual([]);

    await user.click(screen.getByRole('button', { name: 'Concluir' }));
    await screen.findByRole('heading', { level: 1, name: 'Meus packs' });
    const order = backend.calls
      .map((call) => call.command)
      .filter((command) =>
        ['secrets_backend_set', 'secrets_set', 'settings_update'].includes(command),
      );
    expect(order).toEqual(['secrets_backend_set', 'secrets_set', 'secrets_set', 'settings_update']);
    expect(backend.callsOf('secrets_backend_set')[0]?.args).toEqual({ backend: 'envfile' });
    expect(backend.callsOf('secrets_set').map((call) => call.args)).toEqual([
      { kind: 'curseforge', value: 'cf-123' },
      { kind: 'github', value: 'gh-456' },
    ]);
  });

  it('falha ao gravar uma chave mantém o assistente aberto, com o erro, sem encerrar a primeira execução', async () => {
    const backend = firstRunBackend({
      secrets_set: () => ipcError(makeAppError({ domain: 'secrets', code: 'VAULT_UNAVAILABLE' })),
    });
    const user = userEvent.setup();
    renderApp('/');
    await goToKeys(user);
    await user.type(screen.getByLabelText('Chave do Gemini'), 'g-1');
    await user.click(screen.getByRole('button', { name: 'Concluir' }));
    expect(await screen.findByText('A primeira execução não foi concluída')).toBeDefined();
    expect(screen.getByText(/^O cofre do Windows não está acessível agora/)).toBeDefined();
    expect(backend.callsOf('settings_update')).toEqual([]);
    expect(backend.state.exists).toBe(false);
    expect(screen.getByRole('button', { name: 'Concluir' }).hasAttribute('disabled')).toBe(false);
  });

  it('reaberto com chaves já salvas, avisa que em branco mantém a chave', async () => {
    mockSettingsBackend({
      secrets: { backend: 'keyring', curseforge: true, gemini: false, github: false },
    });
    const user = userEvent.setup();
    renderApp('/boas-vindas');
    await goToKeys(user);
    expect(screen.getByText('Já configurada. Deixe em branco para manter.')).toBeDefined();
  });

  it('se o estado não puder ser lido, o app abre no início em vez de travar', async () => {
    mockSettingsBackend(
      {},
      { settings_status: () => ipcError(makeAppError({ domain: 'core', code: 'IO' })) },
    );
    renderApp('/');
    expect(await screen.findByRole('heading', { level: 1, name: 'Meus packs' })).toBeDefined();
  });

  it('passa no axe (aviso e chaves)', async () => {
    firstRunBackend();
    const user = userEvent.setup();
    const { container } = renderApp('/');
    await screen.findByRole('button', { name: 'Entendi' }, FIRST_PAINT);
    expect(await axePage(container)).toHaveNoViolations();
    await goToKeys(user);
    expect(await axePage(container)).toHaveNoViolations();
  });
});
