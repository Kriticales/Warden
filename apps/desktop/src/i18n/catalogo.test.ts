import { describe, expect, it } from 'vitest';

import { resources } from './index';

describe('catálogo de textos (carga)', () => {
  it('carrega todos os arquivos de pt-BR/ como namespaces', () => {
    const loaded = Object.keys(resources['pt-BR']);
    expect(loaded).toContain('comum');
    expect(loaded).toContain('boasVindas');
    expect(loaded.length).toBeGreaterThanOrEqual(11);
  });
});
