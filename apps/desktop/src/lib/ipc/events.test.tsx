import { QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { createQueryClient } from '../../app/App';
import { mockBackend } from '../../test/backend';
import type { PackChanged } from './bindings';
import { events } from './bindings';
import {
  invalidatePackChange,
  subscribe,
  usePackChangedInvalidation,
  useTauriEvent,
  type Listenable,
} from './events';
import { queryKeys } from './keys';

describe('subscribe', () => {
  it('cancelar antes de a assinatura existir também solta o ouvinte depois', async () => {
    const unlisten = vi.fn();
    let resolve: (fn: () => void) => void = () => undefined;
    const event: Listenable<number> = {
      listen: () =>
        new Promise((done) => {
          resolve = done;
        }),
    };
    const handler = vi.fn();
    const cancel = subscribe(event, handler);
    cancel();
    resolve(unlisten);
    await waitFor(() => {
      expect(unlisten).toHaveBeenCalledTimes(1);
    });
  });

  it('não chama o handler depois de cancelado', async () => {
    let deliver: (payload: number) => void = () => undefined;
    const unlisten = vi.fn();
    const event: Listenable<number> = {
      listen: (callback) => {
        deliver = (payload) => {
          callback({ event: 'x', id: 1, payload });
        };
        return Promise.resolve(unlisten);
      },
    };
    const handler = vi.fn();
    const cancel = subscribe(event, handler);
    await Promise.resolve();
    deliver(1);
    cancel();
    deliver(2);
    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(1);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('falha ao assinar fica registrada e não derruba nada', async () => {
    const backend = mockBackend();
    const event: Listenable<number> = { listen: () => Promise.reject(new Error('sem ipc')) };
    subscribe(event, vi.fn());
    await waitFor(() => {
      expect(backend.callsOf('plugin:log|log')).toHaveLength(1);
    });
  });
});

describe('useTauriEvent', () => {
  it('recebe o evento global emitido pelo backend', async () => {
    const backend = mockBackend();
    const handler = vi.fn();
    renderHook(() => {
      useTauriEvent(events.packChanged, handler);
    });
    const change: PackChanged = { packId: 'p1', areas: ['configs'] };
    await waitFor(async () => {
      await backend.emit('pack-changed', change);
      expect(handler).toHaveBeenCalledWith(change);
    });
  });
});

describe('pack-changed', () => {
  it('invalida só as áreas que mudaram', async () => {
    const client = createQueryClient();
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    await invalidatePackChange(client, { packId: 'p1', areas: ['inventory', 'history'] });
    expect(invalidate.mock.calls).toEqual([
      [{ queryKey: queryKeys.packArea('p1', 'inventory') }],
      [{ queryKey: queryKeys.packArea('p1', 'history') }],
    ]);
  });

  it('sem áreas, invalida o pack inteiro', async () => {
    const client = createQueryClient();
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    await invalidatePackChange(client, { packId: 'p2', areas: [] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ['pack', 'p2'] });
  });

  it('a query de outra área e de outro pack continua válida', async () => {
    const backend = mockBackend();
    const client = createQueryClient();
    client.setQueryData(['pack', 'p1', 'configs', 'arvore'], 1);
    client.setQueryData(['pack', 'p1', 'inventory'], 2);
    client.setQueryData(['pack', 'p2', 'configs'], 3);
    renderHook(
      () => {
        usePackChangedInvalidation(client);
      },
      {
        wrapper: ({ children }: { children: ReactNode }) => (
          <QueryClientProvider client={client}>{children}</QueryClientProvider>
        ),
      },
    );
    await waitFor(async () => {
      await backend.emit('pack-changed', { packId: 'p1', areas: ['configs'] });
      expect(client.getQueryState(['pack', 'p1', 'configs', 'arvore'])?.isInvalidated).toBe(true);
    });
    expect(client.getQueryState(['pack', 'p1', 'inventory'])?.isInvalidated).toBe(false);
    expect(client.getQueryState(['pack', 'p2', 'configs'])?.isInvalidated).toBe(false);
  });
});
