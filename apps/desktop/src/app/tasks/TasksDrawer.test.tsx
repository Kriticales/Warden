import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import type { OperationSnapshot } from '../../lib/ipc/bindings';
import { axeComponent } from '../../test/axe';
import { ipcError, mockBackend, type MockBackend } from '../../test/backend';
import { BASE_TIME_MS, makeAppError, makeOperation, makeProgress } from '../../test/factories';
import { renderApp } from '../../test/render';
import { useTasksUi } from './tasks-store';

async function openDrawer(
  user: ReturnType<typeof userEvent.setup>,
  indicatorName: RegExp | string,
) {
  const indicator = await screen.findByRole('button', { name: indicatorName });
  await user.click(indicator);
  return screen.findByRole('dialog', { name: 'Tarefas' });
}

/** Emite até a assinatura do evento existir (o `listen` do Tauri é assíncrono). */
async function emitUntil(backend: MockBackend, payload: OperationSnapshot, check: () => void) {
  await waitFor(async () => {
    await backend.emit('operation-updated', payload);
    check();
  });
}

describe('gaveta de Tarefas (T22)', () => {
  it('vazia: estado vazio com o que aparece ali, e passa no axe', async () => {
    const user = userEvent.setup();
    mockBackend();
    renderApp('/');
    const drawer = await openDrawer(user, 'Nenhuma tarefa em andamento');
    expect(within(drawer).getByRole('heading', { name: 'Nenhuma tarefa ainda' })).toBeDefined();
    expect(document.activeElement).toBe(within(drawer).getByRole('heading', { name: 'Tarefas' }));
    expect(await axeComponent(document.body)).toHaveNoViolations();
  });

  it('CA-T22-01 parcial: cancela pela interface uma operação simulada', async () => {
    const user = userEvent.setup();
    const running = makeOperation({
      progress: makeProgress(160 * 1024 * 1024, 420 * 1024 * 1024, 'bytes'),
    });
    const cancelling: OperationSnapshot = { ...running, state: 'cancelling', cancellable: false };
    const cancelled: OperationSnapshot = {
      ...cancelling,
      state: 'cancelled',
      finishedAtMs: BASE_TIME_MS + 1000,
    };
    // Backend simulado com estado: `operation_cancel` pede a parada; os eventos contam o resto.
    let current = [running];
    const backend = mockBackend({
      operations_list: () => current,
      operation_cancel: () => {
        current = [cancelling];
        return null;
      },
    });
    renderApp('/');
    const drawer = await openDrawer(user, '1 tarefa em andamento');
    const bar = within(drawer).getByRole('progressbar', { name: 'Tarefa do Warden' });
    expect(bar.getAttribute('aria-valuenow')).toBe('38');
    expect(within(drawer).getByText('160,0 MB de 420,0 MB')).toBeDefined();
    expect(await axeComponent(document.body)).toHaveNoViolations();

    await user.click(within(drawer).getByRole('button', { name: 'Cancelar: Tarefa do Warden' }));
    expect(backend.callsOf('operation_cancel')).toEqual([
      { command: 'operation_cancel', args: { operationId: running.id } },
    ]);
    await emitUntil(backend, cancelling, () => {
      expect(within(drawer).getByRole('button', { name: 'Cancelando…' })).toBeDefined();
    });
    current = [cancelled];
    await emitUntil(backend, cancelled, () => {
      expect(within(drawer).getByText(/Tarefa do Warden · Cancelada · /)).toBeDefined();
    });
    expect(within(drawer).queryByRole('progressbar')).toBeNull();
    // Com a gaveta aberta, o resto da tela fica escondido do leitor de tela (modal).
    expect(
      screen.getByRole('button', { name: 'Nenhuma tarefa em andamento', hidden: true }),
    ).toBeDefined();
  });

  it('cancelar que falha mostra o erro na própria tarefa', async () => {
    const user = userEvent.setup();
    const running = makeOperation();
    mockBackend({
      operations_list: () => [running],
      operation_cancel: () =>
        ipcError(makeAppError({ domain: 'app', code: 'OPERATION_NOT_CANCELLABLE' })),
    });
    renderApp('/');
    const drawer = await openDrawer(user, '1 tarefa em andamento');
    await user.click(within(drawer).getByRole('button', { name: 'Cancelar: Tarefa do Warden' }));
    expect(
      await within(drawer).findByText(
        'Essa tarefa está numa etapa que não pode ser interrompida. Espere ela terminar.',
      ),
    ).toBeDefined();
  });

  it('esperando a trava e sem Cancelar quando a operação não permite', async () => {
    const user = userEvent.setup();
    mockBackend({
      operations_list: () => [
        makeOperation({ state: 'waitingForLock' }),
        makeOperation({ cancellable: false, stage: { id: 's', labelKey: 'nao.existe' } }),
      ],
    });
    renderApp('/');
    const drawer = await openDrawer(user, '2 tarefas em andamento');
    expect(within(drawer).getByText('Aguardando outra operação neste pack')).toBeDefined();
    expect(within(drawer).getAllByRole('button', { name: /^Cancelar/ })).toHaveLength(1);
  });

  it('concluídas com resultado; a que falhou abre os detalhes do erro', async () => {
    const user = userEvent.setup();
    mockBackend({
      operations_list: () => [
        makeOperation({ state: 'succeeded', finishedAtMs: Date.now() }),
        makeOperation({
          state: 'failed',
          finishedAtMs: Date.now(),
          error: makeAppError({ domain: 'core', code: 'NETWORK_UNAVAILABLE' }, { detail: 'dns' }),
        }),
      ],
    });
    renderApp('/');
    const drawer = await openDrawer(user, '1 tarefa falhou');
    expect(within(drawer).getByText(/· Concluída ·/)).toBeDefined();
    const toggle = within(drawer).getByRole('button', { name: 'Ver detalhes' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    await user.click(toggle);
    expect(
      within(drawer).getByText('Sem conexão com a internet. Confira a conexão e tente de novo.'),
    ).toBeDefined();
    expect(within(drawer).getByRole('button', { name: 'Ocultar detalhes' })).toBeDefined();
    expect(within(drawer).getByText('Mostra as últimas 50 tarefas.')).toBeDefined();
    expect(await axeComponent(document.body)).toHaveNoViolations();
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    // Falha vista: o indicador volta ao normal e o foco volta para ele.
    const indicator = screen.getByRole('button', { name: 'Nenhuma tarefa em andamento' });
    expect(document.activeElement).toBe(indicator);
  });

  it('erro ao ler a lista: ErrorPanel com "Tentar de novo"', async () => {
    const user = userEvent.setup();
    let fail = true;
    mockBackend({
      operations_list: () =>
        fail ? ipcError(makeAppError({ domain: 'core', code: 'TIMEOUT' })) : [],
    });
    renderApp('/');
    const drawer = await openDrawer(user, 'Nenhuma tarefa em andamento');
    expect(
      await within(drawer).findByText(
        'A operação demorou demais e foi interrompida. Tente de novo.',
      ),
    ).toBeDefined();
    fail = false;
    await user.click(within(drawer).getByRole('button', { name: 'Tentar de novo' }));
    expect(await within(drawer).findByText('Nenhuma tarefa ainda')).toBeDefined();
  });

  it('carregando a lista', async () => {
    const user = userEvent.setup();
    mockBackend({ operations_list: () => new Promise(() => undefined) });
    renderApp('/');
    const drawer = await openDrawer(user, 'Nenhuma tarefa em andamento');
    expect(within(drawer).getByRole('status').textContent).toBe('Lendo as tarefas…');
  });

  it('operação que termina com a gaveta fechada mostra o toast; "Ver detalhes" abre a gaveta', async () => {
    const user = userEvent.setup();
    const running = makeOperation();
    const failed: OperationSnapshot = {
      ...running,
      state: 'failed',
      finishedAtMs: Date.now(),
      error: makeAppError({ domain: 'core', code: 'CANCELLED' }),
    };
    // O backend devolve a lista atual (a gaveta relê ao abrir).
    let current = [running];
    const backend = mockBackend({ operations_list: () => current });
    renderApp('/');
    await screen.findByRole('button', { name: '1 tarefa em andamento' });
    current = [failed];
    await waitFor(async () => {
      await backend.emit('operation-updated', failed);
      expect(screen.getByText('Falhou: Tarefa do Warden')).toBeDefined();
    });
    expect(screen.getByRole('button', { name: '1 tarefa falhou' })).toBeDefined();
    await user.click(screen.getByRole('button', { name: 'Ver detalhes' }));
    const drawer = await screen.findByRole('dialog', { name: 'Tarefas' });
    expect(useTasksUi.getState().expandedId).toBe(running.id);
    expect(within(drawer).getByRole('button', { name: 'Ocultar detalhes' })).toBeDefined();
  });

  it('nova operação chega pelo evento e acende o indicador', async () => {
    const backend = mockBackend();
    const started = makeOperation();
    renderApp('/');
    await screen.findByRole('button', { name: 'Nenhuma tarefa em andamento' });
    await waitFor(async () => {
      await backend.emit('operation-updated', started);
      expect(screen.getByRole('button', { name: '1 tarefa em andamento' })).toBeDefined();
    });
  });
});
