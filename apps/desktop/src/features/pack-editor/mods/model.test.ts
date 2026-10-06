import { describe, expect, it } from 'vitest';

import { AE2, INVALID, makeItem, OUTSIDE, RESOURCE_PACK, SHADER, SODIUM } from '../editor.fixtures';
import {
  countByKind,
  groupByKind,
  groupItems,
  hasFilters,
  itemsInPack,
  matches,
  NO_FILTERS,
  normalize,
  selectedItems,
  sortItems,
  tableRows,
} from './model';

const ALL = [SHADER, SODIUM, OUTSIDE, RESOURCE_PACK, AE2, INVALID];

describe('lista de Mods: filtros', () => {
  it('sem filtro mostra tudo; a busca ignora maiúsculas e acentos e olha o arquivo', () => {
    expect(ALL.filter((item) => matches(item, NO_FILTERS))).toHaveLength(ALL.length);
    const search = (text: string) =>
      ALL.filter((item) => matches(item, { ...NO_FILTERS, search: text })).map((i) => i.name);
    expect(search('SODIUM')).toEqual(['Sodium', 'sodium-extra.pw.toml']);
    expect(search('appliedenergistics2')).toEqual(['Applied Energistics 2']);
    expect(normalize('Ação Rápida')).toBe('acao rapida');
    expect(
      [makeItem({ name: 'Criação' })].filter((item) =>
        matches(item, { ...NO_FILTERS, search: 'criacao' }),
      ),
    ).toHaveLength(1);
  });

  it('filtra por tipo, fonte, lado e "com problemas"', () => {
    const names = (filters: Partial<typeof NO_FILTERS>) =>
      ALL.filter((item) => matches(item, { ...NO_FILTERS, ...filters })).map((i) => i.name);
    expect(names({ kind: 'shader' })).toEqual([SHADER.name]);
    expect(names({ source: 'curseforge' })).toEqual([AE2.name]);
    expect(names({ side: 'client' })).toEqual([SHADER.name, SODIUM.name, RESOURCE_PACK.name]);
    expect(names({ show: 'problems' })).toEqual([OUTSIDE.name, INVALID.name]);
    expect(names({ show: 'pinned' })).toEqual([SODIUM.name]);
    expect(names({ show: 'optional' })).toEqual([]);
    expect(hasFilters(NO_FILTERS)).toBe(false);
    expect(hasFilters({ ...NO_FILTERS, search: '  ' })).toBe(false);
    expect(hasFilters({ ...NO_FILTERS, side: 'server' })).toBe(true);
  });
});

describe('lista de Mods: ordem e agrupamento', () => {
  it('ordena por nome (números em ordem natural) nos dois sentidos', () => {
    const items = ['Mod 10', 'mod 2', 'Ápice', 'Zeta'].map((name) => makeItem({ name }));
    expect(sortItems(items, 'asc').map((i) => i.name)).toEqual([
      'Ápice',
      'mod 2',
      'Mod 10',
      'Zeta',
    ]);
    expect(sortItems(items, 'desc').map((i) => i.name)).toEqual([
      'Zeta',
      'Mod 10',
      'mod 2',
      'Ápice',
    ]);
  });

  it('agrupa por tipo na ordem Mods, Resource packs, Shaders, sem grupos vazios', () => {
    const groups = groupByKind(ALL);
    expect(groups.map((g) => g.key)).toEqual(['mod', 'resourcePack', 'shader']);
    expect(groups[0]?.items.map((i) => i.name)).toEqual([
      SODIUM.name,
      OUTSIDE.name,
      AE2.name,
      INVALID.name,
    ]);
  });

  it('o agrupamento é genérico (gancho da W-08: por grupo)', () => {
    const groups = groupItems(ALL, (item) => (item.source === 'modrinth' ? 'b' : 'a'), ['b']);
    expect(groups.map((g) => g.key)).toEqual(['b', 'a']);
    expect(groups.flatMap((g) => g.items)).toHaveLength(ALL.length);
  });

  it('linhas da tabela: grupo recolhido mostra só o cabeçalho', () => {
    const groups = groupByKind(ALL);
    const open = tableRows(groups, new Set());
    expect(open).toHaveLength(groups.length + ALL.length);
    const closed = tableRows(groups, new Set(['mod'] as const));
    expect(closed.filter((row) => row.type === 'item')).toHaveLength(2);
    expect(closed[0]).toEqual({ type: 'group', key: 'mod', count: 4, collapsed: true });
  });

  it('conta os itens no pack (fora do índice não conta) e por tipo', () => {
    expect(itemsInPack(ALL)).toBe(5);
    expect(countByKind(ALL)).toEqual({ mod: 3, resourcePack: 1, shader: 1, other: 0 });
  });

  it('a seleção esquece itens que sumiram da lista', () => {
    const chosen = selectedItems([SODIUM, AE2], new Set([SODIUM.path, 'mods/sumiu.pw.toml']));
    expect(chosen).toEqual([SODIUM]);
  });
});
