import { describe, expect, it } from 'vitest';

import { compareSha1 } from './sha1';

const SHA = '003c114c85ca88ef3362e018deb6aca0c682d6a1';

describe('conferência do SHA-1 entre as fontes', () => {
  it('o mesmo arquivo em alguma versão da outra fonte', () => {
    expect(compareSha1(SHA, [{ sha1: 'ff' }, { sha1: SHA.toUpperCase() }])).toBe('same');
  });

  it('nenhuma versão com o mesmo SHA-1, ou nenhuma versão', () => {
    expect(compareSha1(SHA, [{ sha1: 'ff' }, { sha1: null }])).toBe('different');
    expect(compareSha1(SHA, [])).toBe('different');
  });

  it('sem SHA-1 na versão escolhida não há o que comparar', () => {
    expect(compareSha1(null, [{ sha1: SHA }])).toBe('unknown');
    expect(compareSha1('  ', [{ sha1: SHA }])).toBe('unknown');
  });
});
