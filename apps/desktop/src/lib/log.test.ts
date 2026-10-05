import { describe, expect, it } from 'vitest';

import { mockBackend } from '../test/backend';
import { LOG_LEVELS } from './ipc/log-bridge';
import { describeError, log } from './log';

describe('log', () => {
  it('manda o nível e a mensagem para o tauri-plugin-log', async () => {
    const backend = mockBackend();
    log.info('abrindo');
    log.warn('cuidado');
    log.error('falhou', new Error('motivo'));
    await Promise.resolve();
    const calls = backend.callsOf('plugin:log|log').map((call) => call.args);
    expect(calls[0]).toEqual({ level: LOG_LEVELS.info, message: 'abrindo' });
    expect(calls[1]).toEqual({ level: LOG_LEVELS.warn, message: 'cuidado' });
    expect(calls[2]?.level).toBe(LOG_LEVELS.error);
    expect(String(calls[2]?.message)).toMatch(/^falhou: Error: motivo/);
  });

  it('nunca lança, mesmo sem o app por trás', () => {
    expect(() => {
      log.error('sem backend');
    }).not.toThrow();
  });

  it('descreve qualquer erro', () => {
    expect(describeError('texto')).toBe('texto');
    expect(describeError({ a: 1 })).toBe('{"a":1}');
    expect(describeError(undefined)).toBe('undefined');
    const error = new Error('x');
    Object.defineProperty(error, 'stack', { value: undefined });
    expect(describeError(error)).toBe('Error: x');
  });
});
