import { useQueryClient, type QueryClient } from '@tanstack/react-query';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';

import type { CacheCleared, GameState, StorageUsage } from '../../../../lib/ipc/bindings';
import { axeComponent } from '../../../../test/axe';
import { deferred, ipcError, mockBackend } from '../../../../test/backend';
import { makeAppError, makeOperation } from '../../../../test/factories';
import { renderWithProviders } from '../../../../test/render';
import { useTestStore } from '../../../test/store';
import { StorageSection } from './StorageSection';

const MB = 1024 * 1024;
const GB = 1024 * MB;

function usage(overrides: Partial<StorageUsage> = {}): StorageUsage {
  return {
    cache: Math.round(1.8 * GB),
    // O banco de metadados (que fica) é o resto do cache.
    clearable: Math.round(1.7 * GB),
    minecraft: Math.round(2.4 * GB),
    javas: 610 * MB,
    instances: Math.round(3.1 * GB),
    ...overrides,
  };
}

function game(overrides: Partial<GameState> = {}): GameState {
  return {
    packId: 'pack-1',
    packName: 'Vale Sereno',
    state: 'running',
    sessionId: null,
    operationId: null,
    profile: null,
    startedAtMs: 0,
    ...overrides,
  };
}

/** Os pares rótulo → valor da lista. */
function values(): Record<string, string> {
  const list = screen.getByLabelText('Espaço usado pelo Warden');
  const terms = within(list).getAllByRole('term');
  const definitions = within(list).getAllByRole('definition');
  return Object.fromEntries(
    terms.map((term, index) => [term.textContent, definitions[index]?.textContent ?? '']),
  );
}

describe('Configurações → Armazenamento (T21, A-06)', () => {
  beforeEach(() => {
    useTestStore.setState({ game: null });
  });

  it('mostra cada parte em MB/GB depois da medição; antes, "Medindo…"; passa no axe', async () => {
    const measuring = deferred<StorageUsage>();
    mockBackend({ storage_usage: () => measuring.promise });
    const { container } = renderWithProviders(<StorageSection />);
    expect(screen.getByRole('region', { name: 'Armazenamento' })).toBeDefined();
    expect(values()).toEqual({
      'Cache de downloads': 'Medindo…',
      'Minecraft e loaders': 'Medindo…',
      Javas: 'Medindo…',
      'Instâncias de teste': 'Medindo…',
    });
    measuring.resolve(usage());
    await screen.findByText('610,0 MB');
    expect(values()).toEqual({
      'Cache de downloads': '1,8 GB',
      'Minecraft e loaders': '2,4 GB',
      Javas: '610,0 MB',
      'Instâncias de teste': '3,1 GB',
    });
    expect(await axeComponent(container)).toHaveNoViolations();
  });

  it('erro na medição mostra o erro com "Tentar de novo"', async () => {
    const user = userEvent.setup();
    let fail = true;
    mockBackend({
      storage_usage: () => {
        if (fail) {
          fail = false;
          return ipcError(makeAppError({ domain: 'core', code: 'IO' }));
        }
        return usage();
      },
    });
    renderWithProviders(<StorageSection />);
    await user.click(await screen.findByRole('button', { name: 'Tentar de novo' }));
    expect(await screen.findByText('610,0 MB')).toBeDefined();
  });

  it('"Limpar cache" pede confirmação, avisa o que fica e mede de novo', async () => {
    const user = userEvent.setup();
    let current = usage();
    const backend = mockBackend({
      storage_usage: () => current,
      storage_clear_cache: (): CacheCleared => {
        current = usage({ cache: 64 * 1024, clearable: 0 });
        return { freed: Math.round(1.7 * GB), skipped: 0 };
      },
    });
    renderWithProviders(<StorageSection />);
    await screen.findByText('1,8 GB');
    await user.click(screen.getByRole('button', { name: 'Limpar cache' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Limpar o cache?' });
    expect(
      within(dialog).getByText(
        'Apaga os arquivos baixados guardados e os temporários (1,7 GB). Instâncias de teste, mundos, Javas, arquivos do Minecraft e os seus packs ficam. O próximo teste baixa de novo só o que faltar.',
      ),
    ).toBeDefined();
    await user.click(within(dialog).getByRole('button', { name: 'Limpar cache' }));
    expect(await screen.findByText('Cache limpo: 1,7 GB liberados.')).toBeDefined();
    await waitFor(() => {
      expect(values()['Cache de downloads']).toBe('64,0 KB');
    });
    // As outras partes não mudaram.
    expect(values()['Instâncias de teste']).toBe('3,1 GB');
    expect(backend.callsOf('storage_clear_cache')).toHaveLength(1);
    expect(backend.callsOf('storage_usage')).toHaveLength(2);
    // O foco volta para quem abriu o diálogo.
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Limpar cache' }));
    });
  });

  it('itens em uso ficam e o aviso diz quantos', async () => {
    const user = userEvent.setup();
    mockBackend({
      storage_usage: () => usage(),
      storage_clear_cache: (): CacheCleared => ({ freed: 512 * MB, skipped: 2 }),
    });
    renderWithProviders(<StorageSection />);
    await screen.findByText('1,8 GB');
    await user.click(screen.getByRole('button', { name: 'Limpar cache' }));
    const dialog = await screen.findByRole('alertdialog');
    await user.click(within(dialog).getByRole('button', { name: 'Limpar cache' }));
    expect(await screen.findByText('Cache limpo: 512,0 MB liberados.')).toBeDefined();
    expect(screen.getByText('2 itens em uso ficaram para a próxima limpeza.')).toBeDefined();
  });

  it('com o jogo aberto, "Limpar cache" fica indisponível e diz por quê', async () => {
    const backend = mockBackend({ storage_usage: () => usage() });
    useTestStore.setState({ game: game() });
    renderWithProviders(<StorageSection />);
    await screen.findByText('1,8 GB');
    const button = screen.getByRole('button', { name: 'Limpar cache' });
    expect(button).toHaveProperty('disabled', true);
    expect(button.getAttribute('aria-describedby')).toBeTruthy();
    expect(screen.getByText('Feche o jogo de Vale Sereno para limpar o cache.')).toBeDefined();
    expect(backend.callsOf('storage_clear_cache')).toHaveLength(0);
  });

  it('com tarefa em andamento, "Limpar cache" fica indisponível e diz por quê', async () => {
    const running = makeOperation({ state: 'running' });
    const backend = mockBackend({
      storage_usage: () => usage(),
      operations_list: () => [running],
    });
    renderWithProviders(<StorageSection />);
    expect(
      await screen.findByText('Espere 1 tarefa em andamento terminar para limpar o cache.'),
    ).toBeDefined();
    expect(screen.getByRole('button', { name: 'Limpar cache' })).toHaveProperty('disabled', true);
    expect(backend.callsOf('storage_usage')).toHaveLength(1);
  });

  it('o Rust recusa (jogo aberto por outro caminho): o diálogo mostra o motivo', async () => {
    const user = userEvent.setup();
    mockBackend({
      storage_usage: () => usage(),
      storage_clear_cache: () =>
        ipcError(
          makeAppError(
            { domain: 'app', code: 'STORAGE_GAME_RUNNING' },
            { params: { packName: 'Vale Sereno' } },
          ),
        ),
    });
    renderWithProviders(<StorageSection />);
    await screen.findByText('1,8 GB');
    await user.click(screen.getByRole('button', { name: 'Limpar cache' }));
    const dialog = await screen.findByRole('alertdialog');
    await user.click(within(dialog).getByRole('button', { name: 'Limpar cache' }));
    expect(
      await within(dialog).findByText(
        'O cache não foi limpo porque o jogo do pack Vale Sereno está aberto. Feche o jogo e tente de novo.',
      ),
    ).toBeDefined();
  });

  it('mede de novo quando uma tarefa termina, quando o jogo fecha e quando a tabela de Java muda', async () => {
    let calls = 0;
    const backend = mockBackend({
      storage_usage: () => {
        calls += 1;
        return usage({ instances: calls * GB });
      },
    });
    let queryClient: QueryClient | null = null;
    function GrabClient() {
      queryClient = useQueryClient();
      return null;
    }
    renderWithProviders(
      <>
        <GrabClient />
        <StorageSection />
      </>,
    );
    await screen.findByText('1,0 GB');

    // Uma tarefa (um teste, adicionar mods…) terminou.
    await act(() =>
      backend.emit('operation-updated', makeOperation({ state: 'succeeded', finishedAtMs: 1 })),
    );
    await screen.findByText('2,0 GB');
    // Uma tarefa em andamento não muda nada ainda.
    await act(() => backend.emit('operation-updated', makeOperation({ state: 'running' })));
    // O jogo fechou.
    await act(() => backend.emit('game-state', game({ state: 'exited' })));
    await screen.findByText('3,0 GB');
    // Um Java foi removido (a tabela de Java recebe a lista nova direto no cache).
    act(() => {
      queryClient?.setQueryData(['java', 'runtimes'], { runtimes: [] });
      queryClient?.setQueryData(['java', 'runtimes'], { runtimes: [], removed: true });
    });
    await screen.findByText('4,0 GB');
    expect(calls).toBe(4);
  });
});
