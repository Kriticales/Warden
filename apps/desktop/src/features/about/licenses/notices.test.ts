import { describe, expect, it } from 'vitest';

import {
  countItems,
  filterNotices,
  hasItem,
  loadNoticesFrom,
  NOTICES_SCHEMA,
  parseNotices,
} from './notices';
import { makeNotices } from './notices.fixture';

describe('avisos de terceiros (formato)', () => {
  it('aceita o formato do xtask', () => {
    const notices = makeNotices();
    expect(parseNotices(JSON.parse(JSON.stringify(notices)))).toEqual(notices);
    expect(countItems(notices)).toBe(6);
  });

  it.each([
    ['não é objeto', 'x'],
    ['outra versão do formato', { ...makeNotices(), schema: 2 }],
    ['textos que não são texto', { ...makeNotices(), texts: [1] }],
    ['grupos ausentes', { schema: NOTICES_SCHEMA, texts: [] }],
    ['grupo desconhecido', { ...makeNotices(), groups: [{ id: 'go', items: [] }] }],
    [
      'índice de texto fora da lista',
      {
        ...makeNotices(),
        groups: [{ id: 'rust', items: [{ name: 'a', version: '1', license: 'MIT', texts: [9] }] }],
      },
    ],
    [
      'item sem versão',
      {
        ...makeNotices(),
        groups: [{ id: 'rust', items: [{ name: 'a', license: 'MIT', texts: [] }] }],
      },
    ],
    [
      'url que não é texto',
      {
        ...makeNotices(),
        groups: [
          { id: 'npm', items: [{ name: 'a', version: '1', license: 'MIT', url: 3, texts: [] }] },
        ],
      },
    ],
  ])('recusa: %s', (_, raw) => {
    expect(parseNotices(raw)).toBeNull();
  });

  it('lê o arquivo gerado; sem arquivo, devolve null', async () => {
    const notices = makeNotices();
    await expect(
      loadNoticesFrom({ './generated/avisos.json': () => Promise.resolve(notices) }),
    ).resolves.toEqual(notices);
    await expect(loadNoticesFrom({})).resolves.toBeNull();
    await expect(
      loadNoticesFrom({ './generated/avisos.json': () => Promise.resolve({ schema: 99 }) }),
    ).resolves.toBeNull();
  });

  it('filtra por nome ou licença, sem diferenciar maiúsculas, e tira grupos vazios', () => {
    const { groups } = makeNotices();
    expect(filterNotices(groups, '  ')).toBe(groups);
    const byName = filterNotices(groups, 'REACT');
    expect(byName.map((group) => group.id)).toEqual(['npm']);
    expect(byName[0]?.items.map((item) => item.name)).toEqual(['react']);
    const byLicense = filterNotices(groups, 'apache');
    expect(byLicense.flatMap((group) => group.items.map((item) => item.name))).toEqual([
      'serde',
      'portablemc',
    ]);
    expect(filterNotices(groups, 'nada-disso')).toEqual([]);
  });

  it('sabe se um componente foi compilado', () => {
    const notices = makeNotices();
    expect(hasItem(notices, 'rust', 'portablemc')).toBe(true);
    expect(hasItem(notices, 'npm', 'portablemc')).toBe(false);
  });
});
