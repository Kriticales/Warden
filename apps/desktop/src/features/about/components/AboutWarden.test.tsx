import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { axeComponent } from '../../../test/axe';
import { deferred, ipcError, mockBackend } from '../../../test/backend';
import { makeAppError, makeAppInfo } from '../../../test/factories';
import { renderWithProviders } from '../../../test/render';
import { AboutWarden, FORGE_SUPPORT_URL } from './AboutWarden';

const LEGAL =
  'NÃO É UM PRODUTO OFICIAL DO MINECRAFT. NÃO É APROVADO PELA MOJANG OU PELA MICROSOFT NEM ASSOCIADO A ELAS.';

describe('Sobre o Warden (T23)', () => {
  it('CA-T01-04: aviso legal, versão e commit; passa no axe', async () => {
    mockBackend({ app_info: () => makeAppInfo({ version: '0.2.0', commit: 'a1b2c3d4e5f6' }) });
    const { container } = renderWithProviders(<AboutWarden />);
    expect(screen.getByRole('region', { name: 'Sobre o Warden' })).toBeDefined();
    expect(screen.getByText(LEGAL)).toBeDefined();
    expect(
      screen.getByText(
        'O Warden abre o Minecraft em modo offline só para testar seus packs. Você precisa possuir o Minecraft: Java Edition.',
      ),
    ).toBeDefined();
    expect(await screen.findByText('0.2.0')).toBeDefined();
    expect(screen.getByText('a1b2c3d4e5f6')).toBeDefined();
    expect(screen.queryByText('Compilação')).toBeNull();
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('build de desenvolvimento e commit desconhecido', async () => {
    mockBackend({ app_info: () => makeAppInfo({ commit: null, debugBuild: true }) });
    renderWithProviders(<AboutWarden headingLevel={3} />);
    expect(await screen.findByText('Desenvolvimento (debug)')).toBeDefined();
    expect(screen.queryByText('Commit')).toBeNull();
    expect(screen.getByRole('heading', { level: 3, name: 'Sobre o Warden' })).toBeDefined();
  });

  it('carregando e erro (o aviso legal aparece mesmo assim)', async () => {
    const response = deferred<never>();
    mockBackend({ app_info: () => response.promise });
    renderWithProviders(<AboutWarden />);
    expect(screen.getByRole('status').textContent).toBe('Lendo a versão…');
    expect(screen.getByText(LEGAL)).toBeDefined();
    response.fail(makeAppError({ domain: 'core', code: 'TIMEOUT' }));
    expect(
      await screen.findByText('A operação demorou demais e foi interrompida. Tente de novo.'),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Tentar de novo' })).toBeDefined();
  });

  it('"Tentar de novo" relê a versão', async () => {
    const user = userEvent.setup();
    let calls = 0;
    mockBackend({
      app_info: () => {
        calls += 1;
        return calls === 1
          ? ipcError(makeAppError({ domain: 'core', code: 'TIMEOUT' }))
          : makeAppInfo();
      },
    });
    renderWithProviders(<AboutWarden />);
    await user.click(await screen.findByRole('button', { name: 'Tentar de novo' }));
    expect(await screen.findByText('0.1.0')).toBeDefined();
  });

  it('"Apoiar o Forge" abre o Patreon no navegador, com dica', async () => {
    const user = userEvent.setup();
    const backend = mockBackend();
    renderWithProviders(<AboutWarden />);
    const link = screen.getByRole('button', { name: 'Apoiar o Forge' });
    await user.hover(link);
    expect(
      await screen.findByRole('tooltip', {
        name: 'O instalador do Forge pede apoio ao projeto. Abre o Patreon do autor no navegador.',
      }),
    ).toBeDefined();
    await user.click(link);
    await waitFor(() => {
      expect(backend.callsOf('plugin:opener|open_url')[0]?.args.url).toBe(FORGE_SUPPORT_URL);
    });
  });
});
