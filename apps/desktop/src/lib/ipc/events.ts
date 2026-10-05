/**
 * Assinatura dos eventos globais (ARCHITECTURE §4.2) com limpeza segura.
 *
 * O `listen` do Tauri é assíncrono: o componente pode desmontar antes de a assinatura
 * existir. `subscribe` cuida disso e devolve uma função de cancelamento síncrona, que é o que o
 * `useEffect` espera.
 */
import type { QueryClient } from '@tanstack/react-query';
import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event';
import { useEffect, useRef } from 'react';

import { log } from '../log';
import { events, type PackChanged } from './bindings';
import { queryKeys } from './keys';

/** O que os eventos gerados (`events.operationUpdated`…) oferecem para ouvir. */
export interface Listenable<T> {
  listen: (callback: EventCallback<T>) => Promise<UnlistenFn>;
}

/** Ouve um evento até a função devolvida ser chamada. */
export function subscribe<T>(event: Listenable<T>, handler: (payload: T) => void): () => void {
  let unlisten: UnlistenFn | null = null;
  let cancelled = false;
  event
    .listen(({ payload }) => {
      if (!cancelled) {
        handler(payload);
      }
    })
    .then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    })
    .catch((error: unknown) => {
      log.error('falha ao assinar um evento do Warden', error);
    });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

/** Ouve um evento enquanto o componente estiver montado. O `handler` mais recente é usado. */
export function useTauriEvent<T>(event: Listenable<T>, handler: (payload: T) => void): void {
  const latest = useRef(handler);
  useEffect(() => {
    latest.current = handler;
  });
  useEffect(
    () =>
      subscribe(event, (payload) => {
        latest.current(payload);
      }),
    [event],
  );
}

/** Invalida as queries das áreas do pack que mudaram (`pack-changed`). */
export function invalidatePackChange(queryClient: QueryClient, change: PackChanged): Promise<void> {
  if (change.areas.length === 0) {
    return queryClient.invalidateQueries({ queryKey: queryKeys.pack(change.packId) });
  }
  return Promise.all(
    change.areas.map((area) =>
      queryClient.invalidateQueries({ queryKey: queryKeys.packArea(change.packId, area) }),
    ),
  ).then(() => undefined);
}

/** Liga a invalidação por `pack-changed` (uma vez, na raiz do app). */
export function usePackChangedInvalidation(queryClient: QueryClient): void {
  useTauriEvent(events.packChanged, (change) => {
    void invalidatePackChange(queryClient, change);
  });
}
