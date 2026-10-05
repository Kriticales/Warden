import { describe, expect, it } from 'vitest';

import { mockBackend } from '../test/backend';
import { openExternal, safeExternalUrl } from './external';

describe('links externos', () => {
  it('só https é aceito', () => {
    expect(safeExternalUrl('https://modrinth.com/mod/jei')?.href).toBe(
      'https://modrinth.com/mod/jei',
    );
    expect(safeExternalUrl('http://modrinth.com')).toBeNull();
    expect(safeExternalUrl('javascript:alert(1)')).toBeNull();
    expect(safeExternalUrl('file:///C:/Windows')).toBeNull();
    expect(safeExternalUrl('/relativo')).toBeNull();
    expect(safeExternalUrl('')).toBeNull();
    expect(safeExternalUrl(null)).toBeNull();
  });

  it('abre https pelo plugin opener e recusa o resto sem chamar o plugin', async () => {
    const backend = mockBackend();
    await openExternal('https://www.patreon.com/LexManos/');
    await openExternal('http://exemplo.com');
    const opened = backend.callsOf('plugin:opener|open_url');
    expect(opened).toHaveLength(1);
    expect(opened[0]?.args.url).toBe('https://www.patreon.com/LexManos/');
    expect(backend.callsOf('plugin:log|log')).toHaveLength(1);
  });

  it('falha do plugin vira registro, não exceção', async () => {
    const backend = mockBackend({
      'plugin:opener|open_url': () => Promise.reject(new Error('sem navegador')),
    });
    await expect(openExternal('https://modrinth.com')).resolves.toBeUndefined();
    expect(backend.callsOf('plugin:log|log')[0]?.args.message).toContain('sem navegador');
  });
});
