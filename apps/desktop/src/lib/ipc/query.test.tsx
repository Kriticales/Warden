import { QueryClientProvider, useQuery } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { createQueryClient } from '../../app/App';
import { makeAppError } from '../../test/factories';
import { commandError, commandQuery, useCommandMutation } from './query';
import { CommandError, type CommandResult } from './result';

function wrapperWith(client = createQueryClient()) {
  return {
    client,
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  };
}

const ok = <T,>(data: T): Promise<CommandResult<T>> => Promise.resolve({ status: 'ok', data });

describe('commandQuery', () => {
  it('entrega o dado do comando', async () => {
    const { wrapper } = wrapperWith();
    const { result } = renderHook(() => useQuery(commandQuery(['x'], () => ok(7))), { wrapper });
    await waitFor(() => {
      expect(result.current.data).toBe(7);
    });
  });

  it('erro do comando vira CommandError com o AppError', async () => {
    const appError = makeAppError({ domain: 'core', code: 'TIMEOUT' });
    const { wrapper } = wrapperWith();
    const { result } = renderHook(
      () =>
        useQuery(
          commandQuery(['y'], () =>
            Promise.resolve<CommandResult<number>>({ status: 'error', error: appError }),
          ),
        ),
      { wrapper },
    );
    await waitFor(() => {
      expect(result.current.isError).toBe(true);
    });
    expect(result.current.error).toBeInstanceOf(CommandError);
    expect(commandError(result.current.error)).toBe(appError);
    expect(commandError(new Error('outro'))).toBeNull();
  });
});

describe('useCommandMutation', () => {
  it('invalida as chaves ao terminar e chama o onSettled de quem usa', async () => {
    const { client, wrapper } = wrapperWith();
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    const onSettled = vi.fn();
    const command = vi.fn((value: number) => ok(value * 2));
    const { result } = renderHook(
      () => useCommandMutation(command, { invalidates: [['pack', 'p1', 'inventory']], onSettled }),
      { wrapper },
    );
    let data: number | undefined;
    await act(async () => {
      data = await result.current.mutateAsync(21);
    });
    expect(data).toBe(42);
    expect(command).toHaveBeenCalledWith(21);
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['pack', 'p1', 'inventory'] });
    expect(onSettled).toHaveBeenCalledTimes(1);
  });

  it('também invalida quando falha (a escrita pode ter mudado parte do pack)', async () => {
    const { client, wrapper } = wrapperWith();
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    const appError = makeAppError({ domain: 'core', code: 'IO' });
    const { result } = renderHook(
      () =>
        useCommandMutation(
          () => Promise.resolve<CommandResult<null>>({ status: 'error', error: appError }),
          { invalidates: [['app', 'operations']] },
        ),
      { wrapper },
    );
    await act(async () => {
      await result.current.mutateAsync(undefined).catch(() => undefined);
    });
    await waitFor(() => {
      expect(result.current.error?.appError).toBe(appError);
    });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['app', 'operations'] });
  });
});
