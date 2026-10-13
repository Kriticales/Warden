/**
 * "Recarregar scripts" na tela do jogo aberto (SPEC T13 passo 6; T26): só com KubeJS ou
 * CraftTweaker no pack; recarrega os scripts de servidor e, quando é preciso digitar o comando
 * no jogo, mostra o aviso com Copiar.
 */
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { PackId, ReloadOutcome, ScriptsInfo } from '../../../lib/ipc/bindings';
import { mockBackend } from '../../../test/backend';
import { renderWithProviders } from '../../../test/render';
import { ReloadScriptsButton } from './ReloadScriptsButton';

const PACK = '01J00000000000000000000000' as PackId;

function info(env: ScriptsInfo['env']): ScriptsInfo {
  return { env, kubejsSnippets: [], crafttweakerSnippets: [], instanceReady: true };
}

describe('Recarregar scripts na tela do teste', () => {
  it('pack sem KubeJS nem CraftTweaker: sem botão', async () => {
    const backend = mockBackend();
    renderWithProviders(<ReloadScriptsButton packId={PACK} />);
    await waitFor(() => {
      expect(backend.callsOf('scripts_info')).toHaveLength(1);
    });
    expect(screen.queryByRole('button', { name: 'Recarregar scripts' })).toBeNull();
  });

  it('KubeJS 6: recarrega os scripts de servidor e oferece copiar o comando', async () => {
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, 'writeText');
    const backend = mockBackend({
      scripts_info: () =>
        info({ kubejs: { tool: 'kubejs', major: 6, version: null }, crafttweaker: null }),
      scripts_reload: () =>
        ({
          route: { kind: 'copyCommand', command: '/kubejs reload server_scripts' },
          response: [],
        }) satisfies ReloadOutcome,
    });
    renderWithProviders(<ReloadScriptsButton packId={PACK} />);
    await user.click(await screen.findByRole('button', { name: 'Recarregar scripts' }));
    await waitFor(() => {
      expect(backend.callsOf('scripts_reload')[0]?.args).toMatchObject({
        path: 'kubejs/server_scripts/scripts.js',
      });
    });
    await screen.findByText('No KubeJS 6, recarregar precisa de um comando no jogo.');
    await user.click(screen.getByRole('button', { name: 'Copiar /kubejs reload server_scripts' }));
    expect(writeText).toHaveBeenCalledWith('/kubejs reload server_scripts');
  });

  it('KubeJS 7 pelo servidor web: avisa que recarregou', async () => {
    const user = userEvent.setup();
    mockBackend({
      scripts_info: () =>
        info({ kubejs: { tool: 'kubejs', major: 7, version: null }, crafttweaker: null }),
      scripts_reload: () =>
        ({ route: { kind: 'webServer', target: 'server' }, response: [] }) satisfies ReloadOutcome,
    });
    renderWithProviders(<ReloadScriptsButton packId={PACK} />);
    await user.click(await screen.findByRole('button', { name: 'Recarregar scripts' }));
    await screen.findByText('Scripts recarregados no jogo');
  });

  it('CraftTweaker do 1.12.2: explica que é preciso reiniciar', async () => {
    const user = userEvent.setup();
    const backend = mockBackend({
      scripts_info: () =>
        info({ kubejs: null, crafttweaker: { tool: 'crafttweaker', major: 4, version: null } }),
      scripts_reload: () =>
        ({ route: { kind: 'restartGame' }, response: [] }) satisfies ReloadOutcome,
    });
    renderWithProviders(<ReloadScriptsButton packId={PACK} />);
    await user.click(await screen.findByRole('button', { name: 'Recarregar scripts' }));
    await screen.findByText(
      'O CraftTweaker do Minecraft 1.12.2 não recarrega scripts com o jogo aberto.',
    );
    expect(backend.callsOf('scripts_reload')[0]?.args).toMatchObject({
      path: 'scripts/scripts.zs',
    });
  });
});
