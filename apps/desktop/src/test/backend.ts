/**
 * Backend simulado para os testes de componente: `mockIPC` com eventos ligados, respostas por
 * comando e o registro das chamadas. Comando sem resposta definida falha o teste (devolve um
 * `app.INTERNAL` dizendo qual faltou), para nada passar por acaso.
 */
import { emit } from '@tauri-apps/api/event';
import { mockIPC } from '@tauri-apps/api/mocks';

import type { AppError } from '../lib/ipc/bindings';
import { makeAppError, makeAppInfo } from './factories';

/** Resposta de um comando: valor, função dos argumentos ou promessa. Lançar = erro do IPC. */
export type Handler = (args: Record<string, unknown>) => unknown;

export interface IpcCall {
  command: string;
  args: Record<string, unknown>;
}

export interface MockBackend {
  calls: IpcCall[];
  /** Chamadas de um comando. */
  callsOf: (command: string) => IpcCall[];
  /** Emite um evento global como o Rust faria (`operation-updated`, `pack-changed`). */
  emit: (event: string, payload: unknown) => Promise<void>;
}

/** Erro do IPC: o Tauri rejeita com o `AppError` serializado. */
export function ipcError(error: AppError): never {
  // É o que o IPC do Tauri faz: rejeita com o objeto serializado, não com um `Error`.
  // eslint-disable-next-line @typescript-eslint/only-throw-error
  throw error;
}

/** Resposta que só chega quando o teste manda (para ver o estado de carregamento antes). */
export interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  /** Rejeita como o IPC: com o `AppError` serializado. */
  fail: (error: AppError) => void;
}

export function deferred<T>(): Deferred<T> {
  let resolve: (value: T) => void = () => undefined;
  let fail: (error: AppError) => void = () => undefined;
  const promise = new Promise<T>((done, reject) => {
    resolve = done;
    fail = (error) => {
      // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- igual ao IPC
      reject(error);
    };
  });
  return { promise, resolve, fail };
}

const DEFAULTS: Record<string, Handler> = {
  app_info: () => makeAppInfo(),
  operations_list: () => [],
  // A tela inicial é Meus packs (P1-07): sem packs, a lista vem vazia.
  packs_list: () => [],
  // Vigia de mudanças externas do pack aberto (A-05).
  pack_watch_start: () => null,
  pack_watch_stop: () => null,
  // Registros do frontend (`tauri-plugin-log`) e links externos (`opener`).
  'plugin:log|log': () => null,
  'plugin:opener|open_url': () => null,
};

/** Liga o backend simulado. `handlers` substitui ou acrescenta comandos aos padrões. */
export function mockBackend(handlers: Record<string, Handler> = {}): MockBackend {
  const calls: IpcCall[] = [];
  const all = { ...DEFAULTS, ...handlers };
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      calls.push({ command, args });
      const handler = all[command];
      if (!handler) {
        // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- igual ao IPC
        return Promise.reject(
          makeAppError(
            { domain: 'app', code: 'INTERNAL' },
            { detail: `comando sem resposta no teste: ${command}` },
          ),
        );
      }
      return handler(args);
    },
    { shouldMockEvents: true },
  );
  return {
    calls,
    callsOf: (command) => calls.filter((call) => call.command === command),
    emit: (event, payload) => emit(event, payload),
  };
}
